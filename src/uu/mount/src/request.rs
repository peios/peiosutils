// This file is part of the peiosutils package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

//! Build a structured [`Action`] from parsed CLI matches (clap glue).
//!
//! The pure decision logic (verb resolution, operand disambiguation, option
//! partitioning) lives in [`crate::verb`] / [`crate::options`]; this module is
//! the thin layer that reads clap and assembles the request, applying the
//! cross-cutting validity rules (§8.2 policy/sddl) and combining propagation
//! from both `-o` tokens and `--make-*` flags.

use std::ffi::OsString;
use std::os::unix::ffi::OsStrExt;

use clap::ArgMatches;

use crate::cli::opt;
use crate::error::{MountError, Result};
use crate::options::{ParsedOptions, PolicyKind, PropChange, PropKind};
use crate::verb::{self, Operands, Verb, VerbFlags};

/// What the command should do.
#[derive(Debug)]
pub enum Action {
    /// No operands, no verb → list (optionally filtered by `-t`).
    List(ListRequest),
    /// A mount operation.
    Mount(Box<MountRequest>),
}

/// A list-mode request (§11).
#[derive(Debug, Default)]
pub struct ListRequest {
    /// `-t` fstype filter (may be a list / `no<type>` negation); empty = all.
    pub type_filter: Option<Vec<u8>>,
    /// `-l` — append filesystem labels.
    pub show_labels: bool,
}

/// A fully-resolved mount request.
#[derive(Debug)]
pub struct MountRequest {
    pub verb: Verb,
    pub source: Option<Vec<u8>>,
    pub target: Vec<u8>,
    /// `None` = `-t auto`/omitted (probe via libblkid, §7).
    pub fstype: Option<Vec<u8>>,
    pub options: ParsedOptions,
    /// Propagation changes to apply after the primary operation (§4.7), in
    /// order: `-o` tokens first, then `--make-*` flags.
    pub propagation: Vec<PropChange>,

    pub no_canonicalize_source: bool,
    pub no_canonicalize_target: bool,
    pub mkdir: Option<u32>,
    pub exclusive: bool,
    pub onlyonce: bool,
    pub internal_only: bool,
    pub namespace: Option<Vec<u8>>,
    pub synth_sddl: Option<Vec<u8>>,
    pub target_prefix: Option<Vec<u8>>,
    pub fake: bool,
    pub verbose: u8,
}

/// Read an `OsString` arg as opaque bytes (§1.8).
fn bytes(m: &ArgMatches, id: &str) -> Option<Vec<u8>> {
    m.get_one::<OsString>(id).map(|s| s.as_bytes().to_vec())
}

fn flag(m: &ArgMatches, id: &str) -> bool {
    m.get_flag(id)
}

/// Assemble the `Action` from clap matches.
pub fn build(m: &ArgMatches) -> Result<Action> {
    // -o: concatenate every occurrence with commas, at the byte level.
    let mut raw_opts: Vec<u8> = Vec::new();
    if let Some(values) = m.get_many::<OsString>(opt::OPTIONS) {
        for v in values {
            if !raw_opts.is_empty() {
                raw_opts.push(b',');
            }
            raw_opts.extend_from_slice(v.as_bytes());
        }
    }
    // -r/-w sugar folds into the option set.
    if flag(m, opt::READ_ONLY) {
        push_opt(&mut raw_opts, b"ro");
    }
    if flag(m, opt::READ_WRITE) {
        push_opt(&mut raw_opts, b"rw");
    }
    let options = ParsedOptions::parse(&raw_opts)?;

    let verb_flags = VerbFlags {
        bind: flag(m, opt::BIND),
        rbind: flag(m, opt::RBIND),
        r#move: flag(m, opt::MOVE),
        beneath: flag(m, opt::BENEATH),
    };
    let verb = verb::resolve_verb(verb_flags, &options)?;

    // Combine propagation: -o tokens first, then --make-* flags.
    let propagation = collect_propagation(m, &options);

    let positionals: Vec<Vec<u8>> = m
        .get_many::<OsString>(opt::OPERANDS)
        .map(|vs| vs.map(|s| s.as_bytes().to_vec()).collect())
        .unwrap_or_default();
    // -L/-U are source shortcuts forming a LABEL=/UUID= tag spec; they take
    // precedence over an explicit --source.
    let src_flag = tag_source(m).or_else(|| bytes(m, opt::SOURCE));
    let tgt_flag = bytes(m, opt::TARGET);

    // List mode: no operands, no explicit operand flags, no structural verb,
    // no propagation change (§4.1).
    if positionals.is_empty()
        && src_flag.is_none()
        && tgt_flag.is_none()
        && verb == Verb::New
        && propagation.is_empty()
    {
        return Ok(Action::List(ListRequest {
            type_filter: bytes(m, opt::TYPE).filter(|t| t != b"auto"),
            show_labels: flag(m, opt::SHOW_LABELS),
        }));
    }

    // A standalone propagation change (no structural verb) takes a lone target,
    // like remount (§4.5/§4.7).
    let propagation_only = !propagation.is_empty() && verb == Verb::New;
    let Operands { source, mut target } =
        verb::resolve_operands(verb, &positionals, src_flag, tgt_flag, propagation_only)?;

    // --target-prefix prepends to the (resolved) target (§4).
    let target_prefix = bytes(m, opt::TARGET_PREFIX);
    if let Some(prefix) = &target_prefix {
        target = join_path(prefix, &target);
    }

    let fstype = bytes(m, opt::TYPE).filter(|t| t != b"auto");
    let synth_sddl = bytes(m, opt::SYNTH_SDDL);

    // -m/--mkdir or X-mount.mkdir.
    let mkdir = mkdir_mode(m).or(options.mkdir);

    let req = MountRequest {
        verb,
        source,
        target,
        fstype,
        propagation,
        no_canonicalize_source: flag(m, opt::NO_CANONICALIZE) || options.nocanon_source,
        no_canonicalize_target: flag(m, opt::NO_CANONICALIZE) || options.nocanon_target,
        mkdir,
        exclusive: flag(m, opt::EXCLUSIVE),
        onlyonce: flag(m, opt::ONLYONCE),
        internal_only: flag(m, opt::INTERNAL_ONLY),
        namespace: bytes(m, opt::NAMESPACE),
        synth_sddl,
        target_prefix,
        fake: flag(m, opt::FAKE),
        verbose: m.get_count(opt::VERBOSE),
        options,
    };

    validate(&req)?;
    Ok(Action::Mount(Box::new(req)))
}

/// §8.2 / §8.4 validity rules that span fields.
fn validate(req: &MountRequest) -> Result<()> {
    // policy= only on a new mount of a real fs.
    if req.options.policy.is_some() && req.verb != Verb::New {
        return Err(MountError::Usage(
            "policy= is only valid on a new mount (not with bind/move/beneath/remount)".to_string(),
        ));
    }
    // --synth-sddl only with synth-* policy.
    if req.synth_sddl.is_some()
        && !matches!(
            req.options.policy,
            Some(PolicyKind::SynthEphemeral | PolicyKind::SynthPersist)
        )
    {
        return Err(MountError::Usage(
            "--synth-sddl requires -o policy=synth-ephemeral or synth-persist".to_string(),
        ));
    }
    // §8.4 client-side SDDL pre-validation — runs on every path (incl. --fake).
    crate::policy::validate_synth_sddl(req.synth_sddl.as_deref())?;
    Ok(())
}

fn collect_propagation(m: &ArgMatches, options: &ParsedOptions) -> Vec<PropChange> {
    let mut out = options.propagation.clone();
    for (id, kind, recursive) in [
        (opt::MAKE_SHARED, PropKind::Shared, false),
        (opt::MAKE_SLAVE, PropKind::Slave, false),
        (opt::MAKE_PRIVATE, PropKind::Private, false),
        (opt::MAKE_UNBINDABLE, PropKind::Unbindable, false),
        (opt::MAKE_RSHARED, PropKind::Shared, true),
        (opt::MAKE_RSLAVE, PropKind::Slave, true),
        (opt::MAKE_RPRIVATE, PropKind::Private, true),
        (opt::MAKE_RUNBINDABLE, PropKind::Unbindable, true),
    ] {
        if flag(m, id) {
            out.push(PropChange { kind, recursive });
        }
    }
    out
}

/// `-m`/`--mkdir[=mode]` → Some(mode). `default_missing_value` makes a bare
/// `-m` yield "0755".
fn mkdir_mode(m: &ArgMatches) -> Option<u32> {
    m.get_one::<String>(opt::MKDIR)
        .map(|s| u32::from_str_radix(s.trim_start_matches("0o"), 8).unwrap_or(0o755))
}

/// Build a `LABEL=`/`UUID=` source spec from `-L`/`-U` (libblkid resolves it).
fn tag_source(m: &ArgMatches) -> Option<Vec<u8>> {
    if let Some(label) = bytes(m, opt::LABEL) {
        let mut s = b"LABEL=".to_vec();
        s.extend_from_slice(&label);
        return Some(s);
    }
    if let Some(uuid) = bytes(m, opt::UUID) {
        let mut s = b"UUID=".to_vec();
        s.extend_from_slice(&uuid);
        return Some(s);
    }
    None
}

fn push_opt(buf: &mut Vec<u8>, opt: &[u8]) {
    if !buf.is_empty() {
        buf.push(b',');
    }
    buf.extend_from_slice(opt);
}

/// Join two path byte-strings with a single `/`.
fn join_path(prefix: &[u8], rest: &[u8]) -> Vec<u8> {
    let mut out = prefix.to_vec();
    if !out.ends_with(b"/") {
        out.push(b'/');
    }
    out.extend_from_slice(rest.strip_prefix(b"/").unwrap_or(rest));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::verb::Verb;

    fn parse(args: &[&str]) -> Action {
        let m = crate::cli::build()
            .try_get_matches_from(std::iter::once("mount").chain(args.iter().copied()))
            .expect("clap should accept these args");
        build(&m).expect("request build should succeed")
    }

    fn mount_req(args: &[&str]) -> Box<MountRequest> {
        match parse(args) {
            Action::Mount(r) => r,
            Action::List(_) => panic!("expected a mount action"),
        }
    }

    // Regression: the live-boot overlay invocation puts the source before -o
    // and the target after it. clap must accept the option-split operands
    // (this is the exact form that failed at boot).
    #[test]
    fn operands_interspersed_with_options() {
        let r = mount_req(&["-t", "overlay", "overlay", "-o", "lowerdir=/a,upperdir=/b", "/sysroot"]);
        assert_eq!(r.verb, Verb::New);
        assert_eq!(r.source.as_deref(), Some(b"overlay".as_slice()));
        assert_eq!(r.target, b"/sysroot");
        assert_eq!(r.fstype.as_deref(), Some(b"overlay".as_slice()));
    }

    #[test]
    fn squashfs_loop_form() {
        let r = mount_req(&["-o", "loop,ro", "-t", "squashfs", "/img.sqfs", "/lower"]);
        assert_eq!(r.source.as_deref(), Some(b"/img.sqfs".as_slice()));
        assert_eq!(r.target, b"/lower");
        assert!(r.options.loop_request.is_some());
    }

    #[test]
    fn source_between_options() {
        let r = mount_req(&["/dev/sda1", "-t", "ext4", "/mnt"]);
        assert_eq!(r.source.as_deref(), Some(b"/dev/sda1".as_slice()));
        assert_eq!(r.target, b"/mnt");
    }

    #[test]
    fn too_many_operands_is_usage_error() {
        let m = crate::cli::build()
            .try_get_matches_from(["mount", "a", "b", "c"])
            .unwrap();
        assert!(matches!(build(&m), Err(MountError::Usage(_))));
    }

    #[test]
    fn no_operands_is_list() {
        assert!(matches!(parse(&[]), Action::List(_)));
        assert!(matches!(parse(&["-t", "ext4"]), Action::List(_)));
    }

    #[test]
    fn lone_target_remount() {
        let r = mount_req(&["-o", "remount,ro", "/sysroot"]);
        assert_eq!(r.verb, Verb::Remount);
        assert_eq!(r.source, None);
        assert_eq!(r.target, b"/sysroot");
    }
}
