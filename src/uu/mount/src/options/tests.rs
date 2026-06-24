// Unit tests for the §6 option partitioner.

use super::*;

/// Parse helper for the common UTF-8 case.
fn p(s: &str) -> ParsedOptions {
    ParsedOptions::parse(s.as_bytes()).expect("parse ok")
}

fn err(s: &str) -> MountError {
    ParsedOptions::parse(s.as_bytes()).expect_err("should be a usage error")
}

#[test]
fn category_a_simple_bits() {
    let o = p("ro,nosuid,nodev,noexec,nodiratime,nosymfollow");
    assert_eq!(
        o.attr_set,
        libc::MOUNT_ATTR_RDONLY
            | libc::MOUNT_ATTR_NOSUID
            | libc::MOUNT_ATTR_NODEV
            | libc::MOUNT_ATTR_NOEXEC
            | libc::MOUNT_ATTR_NODIRATIME
            | libc::MOUNT_ATTR_NOSYMFOLLOW
    );
    assert!(!o.recursive_attr);
    assert!(o.sb_rdonly.is_none());
}

#[test]
fn rw_clears_rdonly() {
    let o = p("rw");
    assert_eq!(o.attr_set & libc::MOUNT_ATTR_RDONLY, 0);
    assert_eq!(o.attr_clr & libc::MOUNT_ATTR_RDONLY, libc::MOUNT_ATTR_RDONLY);
}

#[test]
fn positive_overrides_negative_in_order() {
    let o = p("noexec,exec");
    assert_eq!(o.attr_set & libc::MOUNT_ATTR_NOEXEC, 0);
    assert_eq!(o.attr_clr & libc::MOUNT_ATTR_NOEXEC, libc::MOUNT_ATTR_NOEXEC);
}

#[test]
fn atime_last_wins_and_masks() {
    let o = p("noatime,strictatime");
    assert_eq!(o.attr_set & libc::MOUNT_ATTR__ATIME, libc::MOUNT_ATTR_STRICTATIME);
    assert_eq!(o.attr_clr & libc::MOUNT_ATTR__ATIME, libc::MOUNT_ATTR__ATIME);
}

#[test]
fn relatime_clears_mask_to_default() {
    let o = p("relatime");
    assert_eq!(o.attr_set & libc::MOUNT_ATTR__ATIME, 0); // RELATIME == 0
    assert_eq!(o.attr_clr & libc::MOUNT_ATTR__ATIME, libc::MOUNT_ATTR__ATIME);
}

#[test]
fn noatime_then_atime_resets_to_relatime() {
    let o = p("noatime,atime");
    assert_eq!(o.attr_set & libc::MOUNT_ATTR__ATIME, 0);
}

#[test]
fn ro_layer_qualifiers() {
    assert_eq!(p("ro=fs").sb_rdonly, Some(true));
    assert_eq!(p("rw=fs").sb_rdonly, Some(false));

    let rec = p("ro=recursive");
    assert!(rec.recursive_attr);
    assert_eq!(rec.attr_set & libc::MOUNT_ATTR_RDONLY, libc::MOUNT_ATTR_RDONLY);

    let vfs = p("ro=vfs");
    assert!(!vfs.recursive_attr);
    assert!(vfs.sb_rdonly.is_none());
    assert_eq!(vfs.attr_set & libc::MOUNT_ATTR_RDONLY, libc::MOUNT_ATTR_RDONLY);
}

#[test]
fn ro_bad_scope_is_usage_error() {
    assert!(matches!(err("ro=banana"), MountError::Usage(_)));
}

#[test]
fn category_b_superblock_flags() {
    let o = p("sync,dirsync,lazytime,iversion,silent");
    assert_eq!(
        o.sb_flags,
        vec![
            (SbFlag::Synchronous, true),
            (SbFlag::Dirsync, true),
            (SbFlag::Lazytime, true),
            (SbFlag::IVersion, true),
            (SbFlag::Silent, true),
        ]
    );
    assert_eq!(p("async").sb_flags, vec![(SbFlag::Synchronous, false)]);
    assert_eq!(p("loud").sb_flags, vec![(SbFlag::Silent, false)]);
}

#[test]
fn mand_is_ignored_with_a_note() {
    let o = p("mand");
    assert!(o.sb_flags.is_empty());
    assert_eq!(o.notes.len(), 1);
    assert!(o.notes[0].contains("obsolete"));
}

#[test]
fn category_c_fs_params_passthrough() {
    let o = p("subvol=@home,compress=zstd,discard");
    assert_eq!(
        o.fs_params,
        vec![
            (b"subvol".to_vec(), Some(b"@home".to_vec())),
            (b"compress".to_vec(), Some(b"zstd".to_vec())),
            (b"discard".to_vec(), None),
        ]
    );
}

#[test]
fn key_value_splits_on_first_equals() {
    let o = p("context=a=b=c");
    assert_eq!(o.fs_params, vec![(b"context".to_vec(), Some(b"a=b=c".to_vec()))]);
}

#[test]
fn quoting_protects_embedded_commas() {
    let o = ParsedOptions::parse(br#"opt="a,b,c",discard"#).unwrap();
    assert_eq!(
        o.fs_params,
        vec![
            (b"opt".to_vec(), Some(b"a,b,c".to_vec())),
            (b"discard".to_vec(), None),
        ]
    );
}

#[test]
fn non_utf8_value_passes_through() {
    // 0xff is not valid UTF-8; it must survive losslessly in the value.
    let spec = b"key=\xff\xfevalue";
    let o = ParsedOptions::parse(spec).unwrap();
    assert_eq!(o.fs_params, vec![(b"key".to_vec(), Some(b"\xff\xfevalue".to_vec()))]);
}

#[test]
fn category_d_meta_verbs() {
    assert!(p("bind").meta_bind);
    assert!(p("rbind").meta_rbind);
    assert!(p("move").meta_move);
    assert!(p("remount").meta_remount);
}

#[test]
fn category_d_loop_and_offsets() {
    assert_eq!(p("loop").loop_request, Some(LoopRequest::Auto));
    assert_eq!(
        p("loop=/dev/loop3").loop_request,
        Some(LoopRequest::Device(b"/dev/loop3".to_vec()))
    );
    assert_eq!(p("offset=1M").offset, Some(1 << 20));
    assert_eq!(p("sizelimit=2GiB").sizelimit, Some(2 << 30));
    assert_eq!(p("offset=512").offset, Some(512));
}

#[test]
fn offset_bad_value_is_usage_error() {
    assert!(matches!(err("offset=lots"), MountError::Usage(_)));
}

#[test]
fn category_d_propagation_order_and_recursive() {
    let o = p("private,rshared,unbindable");
    assert_eq!(
        o.propagation,
        vec![
            PropChange { kind: PropKind::Private, recursive: false },
            PropChange { kind: PropKind::Shared, recursive: true },
            PropChange { kind: PropKind::Unbindable, recursive: false },
        ]
    );
}

#[test]
fn category_e_policy() {
    assert_eq!(p("policy=deny-missing").policy, Some(PolicyKind::DenyMissing));
    assert_eq!(p("policy=synth-ephemeral").policy, Some(PolicyKind::SynthEphemeral));
    assert_eq!(p("policy=synth-persist").policy, Some(PolicyKind::SynthPersist));
    assert!(matches!(err("policy=unmanaged"), MountError::Usage(_)));
    assert!(matches!(err("policy=bogus"), MountError::Usage(_)));
}

#[test]
fn category_f_xmount() {
    assert_eq!(p("X-mount.mkdir").mkdir, Some(0o755));
    assert_eq!(p("X-mount.mkdir=0700").mkdir, Some(0o700));
    assert_eq!(p("X-mount.subdir=@sub").subdir, Some(b"@sub".to_vec()));
    assert!(p("X-mount.noloop").noloop);
    // A comma-bearing value must be quoted (else the comma splits the token).
    assert_eq!(
        ParsedOptions::parse(br#"X-mount.auto-fstypes="ext4,xfs""#).unwrap().auto_fstypes,
        Some(b"ext4,xfs".to_vec())
    );

    let both = p("X-mount.nocanonicalize");
    assert!(both.nocanon_source && both.nocanon_target);
    let src = p("X-mount.nocanonicalize=source");
    assert!(src.nocanon_source && !src.nocanon_target);
    let tgt = p("X-mount.nocanonicalize=target");
    assert!(!tgt.nocanon_source && tgt.nocanon_target);
}

#[test]
fn cut_xmount_options_are_rejected() {
    assert!(matches!(err("X-mount.idmap=foo"), MountError::Usage(_)));
    assert!(matches!(err("X-mount.owner=alice"), MountError::Usage(_)));
    assert!(matches!(err("X-mount.mode=0644"), MountError::Usage(_)));
    assert!(matches!(err("X-mount.bogus"), MountError::Usage(_)));
}

#[test]
fn defaults_expands_correctly() {
    // rw,suid,dev,exec,async — no atime change.
    let o = p("defaults");
    assert_eq!(o.attr_set, 0);
    assert_eq!(
        o.attr_clr,
        libc::MOUNT_ATTR_RDONLY | libc::MOUNT_ATTR_NOSUID | libc::MOUNT_ATTR_NODEV | libc::MOUNT_ATTR_NOEXEC
    );
    assert_eq!(o.sb_flags, vec![(SbFlag::Synchronous, false)]);
    assert_eq!(o.attr_set & libc::MOUNT_ATTR__ATIME, 0);
}

#[test]
fn defaults_then_override() {
    let o = p("defaults,ro,noatime");
    assert_eq!(o.attr_set & libc::MOUNT_ATTR_RDONLY, libc::MOUNT_ATTR_RDONLY);
    assert_eq!(o.attr_set & libc::MOUNT_ATTR__ATIME, libc::MOUNT_ATTR_NOATIME);
}

#[test]
fn fsmount_attr_flags_is_attr_set() {
    let o = p("ro,noexec");
    assert_eq!(
        u64::from(o.fsmount_attr_flags()),
        libc::MOUNT_ATTR_RDONLY | libc::MOUNT_ATTR_NOEXEC
    );
}

#[test]
fn touches_superblock_detection() {
    assert!(!p("ro,nosuid").touches_superblock());
    assert!(p("ro=fs").touches_superblock());
    assert!(p("sync").touches_superblock());
    assert!(p("compress=zstd").touches_superblock());
}

#[test]
fn empty_and_whitespace_tokens_skipped() {
    let o = p("ro, ,nodev,");
    assert_eq!(o.attr_set, libc::MOUNT_ATTR_RDONLY | libc::MOUNT_ATTR_NODEV);
    assert!(o.fs_params.is_empty());
}
