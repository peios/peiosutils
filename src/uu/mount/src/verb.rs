// This file is part of the peiosutils package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

//! Operation-mode (verb) selection and §4 argument-shape disambiguation.
//!
//! Kept pure (no clap, no IO) so the disambiguation rules — which are subtle
//! and faithful to util-linux — are exhaustively unit-testable.

use crate::error::{MountError, Result};
use crate::options::ParsedOptions;

/// A structural operation mode (§2.1). Propagation is *not* here — it is a
/// non-exclusive trailing step (§4.7), carried separately.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verb {
    /// New filesystem instance: `fsopen→fsconfig→fsmount→move_mount`.
    New,
    /// Bind (`--bind`/`-o bind`) or recursive bind (`--rbind`/`-o rbind`).
    Bind { recursive: bool },
    /// Relocate an existing mount (`--move`/`-o move`).
    Move,
    /// Move beneath the top mount at the target (`--beneath`).
    Beneath,
    /// Reconfigure an existing mount (`-o remount`).
    Remount,
}

/// The verb selectors coming from the command line + `-o` meta-verbs.
#[derive(Debug, Default, Clone, Copy)]
pub struct VerbFlags {
    pub bind: bool,
    pub rbind: bool,
    pub r#move: bool,
    pub beneath: bool,
}

/// Resolve the structural verb from CLI flags and `-o` meta-verbs, enforcing
/// the §4.6 mutual exclusion. Returns `Verb::New` when no structural verb is
/// named (the operands then decide new-mount vs list, §4.1).
pub fn resolve_verb(flags: VerbFlags, o: &ParsedOptions) -> Result<Verb> {
    let mut chosen: Vec<Verb> = Vec::new();
    if flags.bind || o.meta_bind {
        chosen.push(Verb::Bind { recursive: false });
    }
    if flags.rbind || o.meta_rbind {
        chosen.push(Verb::Bind { recursive: true });
    }
    if flags.r#move || o.meta_move {
        chosen.push(Verb::Move);
    }
    if flags.beneath {
        chosen.push(Verb::Beneath);
    }
    if o.meta_remount {
        chosen.push(Verb::Remount);
    }

    match chosen.as_slice() {
        [] => Ok(Verb::New),
        // bind + rbind collapse to the recursive bind (both were requested);
        // any other multiple is a conflict.
        [Verb::Bind { .. }, Verb::Bind { .. }] => Ok(Verb::Bind { recursive: true }),
        [one] => Ok(*one),
        _ => Err(MountError::Usage(
            "the bind/rbind, move, beneath and remount verbs are mutually exclusive".to_string(),
        )),
    }
}

/// Whether a verb takes a lone target (no source operand): remount and a
/// standalone propagation change (§4.5). `Move`/`Bind`/`Beneath`/`New` need a
/// source.
pub fn verb_takes_lone_target(verb: Verb) -> bool {
    matches!(verb, Verb::Remount)
}

/// Resolved operands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Operands {
    pub source: Option<Vec<u8>>,
    pub target: Vec<u8>,
}

/// Resolve source/target from positionals and explicit `--source`/`--target`
/// (§4.1–4.5). `propagation_only` is true when there is no structural verb but a
/// standalone `--make-*`/propagation `-o` token (which, like remount, takes a
/// lone target).
///
/// Returns a usage error for the un-resolvable single-operand case (§4.2: no
/// fstab on peios).
pub fn resolve_operands(
    verb: Verb,
    positionals: &[Vec<u8>],
    src_flag: Option<Vec<u8>>,
    tgt_flag: Option<Vec<u8>>,
    propagation_only: bool,
) -> Result<Operands> {
    let lone_target = verb_takes_lone_target(verb) || propagation_only;

    // Explicit flags take precedence and may combine with one positional.
    match (src_flag, tgt_flag) {
        (Some(s), Some(t)) => return Ok(Operands { source: Some(s), target: t }),
        (Some(s), None) => {
            let t = positionals.first().cloned().ok_or_else(missing_target)?;
            return Ok(Operands { source: Some(s), target: t });
        }
        (None, Some(t)) => {
            let s = positionals.first().cloned();
            return Ok(Operands { source: s, target: t });
        }
        (None, None) => {}
    }

    match positionals {
        [target] if lone_target => Ok(Operands { source: None, target: target.clone() }),
        [_single] => Err(MountError::Usage(
            "a single operand cannot be resolved without /etc/fstab; \
             supply both SOURCE and TARGET, or use --source/--target"
                .to_string(),
        )),
        [source, target] => Ok(Operands { source: Some(source.clone()), target: target.clone() }),
        [] => Err(missing_target()),
        _ => Err(MountError::Usage("too many operands".to_string())),
    }
}

fn missing_target() -> MountError {
    MountError::Usage("a target is required".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts(s: &str) -> ParsedOptions {
        ParsedOptions::parse(s.as_bytes()).unwrap()
    }
    fn pos(items: &[&str]) -> Vec<Vec<u8>> {
        items.iter().map(|s| s.as_bytes().to_vec()).collect()
    }

    #[test]
    fn no_verb_is_new() {
        assert_eq!(resolve_verb(VerbFlags::default(), &opts("")).unwrap(), Verb::New);
    }

    #[test]
    fn bind_flag_and_o_bind_agree() {
        let f = VerbFlags { bind: true, ..Default::default() };
        assert_eq!(resolve_verb(f, &opts("")).unwrap(), Verb::Bind { recursive: false });
        assert_eq!(
            resolve_verb(VerbFlags::default(), &opts("bind")).unwrap(),
            Verb::Bind { recursive: false }
        );
    }

    #[test]
    fn rbind_is_recursive() {
        let f = VerbFlags { rbind: true, ..Default::default() };
        assert_eq!(resolve_verb(f, &opts("")).unwrap(), Verb::Bind { recursive: true });
    }

    #[test]
    fn bind_plus_rbind_collapses_to_recursive() {
        let f = VerbFlags { bind: true, rbind: true, ..Default::default() };
        assert_eq!(resolve_verb(f, &opts("")).unwrap(), Verb::Bind { recursive: true });
    }

    #[test]
    fn structural_verbs_are_mutually_exclusive() {
        let f = VerbFlags { bind: true, r#move: true, ..Default::default() };
        assert!(matches!(resolve_verb(f, &opts("")), Err(MountError::Usage(_))));
        assert!(matches!(
            resolve_verb(VerbFlags::default(), &opts("remount,move")),
            Err(MountError::Usage(_))
        ));
    }

    #[test]
    fn two_positionals_are_source_target() {
        let r = resolve_operands(Verb::New, &pos(&["/dev/sda1", "/mnt"]), None, None, false).unwrap();
        assert_eq!(r.source.as_deref(), Some(b"/dev/sda1".as_slice()));
        assert_eq!(r.target, b"/mnt");
    }

    #[test]
    fn single_operand_without_fstab_is_error_for_new() {
        assert!(matches!(
            resolve_operands(Verb::New, &pos(&["/mnt"]), None, None, false),
            Err(MountError::Usage(_))
        ));
    }

    #[test]
    fn remount_takes_lone_target() {
        let r = resolve_operands(Verb::Remount, &pos(&["/mnt"]), None, None, false).unwrap();
        assert_eq!(r.source, None);
        assert_eq!(r.target, b"/mnt");
    }

    #[test]
    fn standalone_propagation_takes_lone_target() {
        let r = resolve_operands(Verb::New, &pos(&["/mnt"]), None, None, true).unwrap();
        assert_eq!(r.source, None);
        assert_eq!(r.target, b"/mnt");
    }

    #[test]
    fn explicit_flags_override() {
        let r = resolve_operands(
            Verb::New,
            &pos(&[]),
            Some(b"src".to_vec()),
            Some(b"tgt".to_vec()),
            false,
        )
        .unwrap();
        assert_eq!(r.source.as_deref(), Some(b"src".as_slice()));
        assert_eq!(r.target, b"tgt");
    }

    #[test]
    fn source_flag_plus_one_positional_target() {
        let r = resolve_operands(Verb::New, &pos(&["/mnt"]), Some(b"src".to_vec()), None, false).unwrap();
        assert_eq!(r.source.as_deref(), Some(b"src".as_slice()));
        assert_eq!(r.target, b"/mnt");
    }
}
