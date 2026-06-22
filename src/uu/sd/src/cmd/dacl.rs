// Shared read-modify-write helper for DACL/SACL edits.
//
// allow/deny/remove (DACL) and audit/unaudit (SACL) all do:
//   1. get_sd (DACL or SACL only)
//   2. parse → copy ACEs into a fresh AclBuilder
//   3. mutate (append or filter)
//   4. rebuild SD with just the changed component
//   5. set_sd with the matching SecInfo bit
//
// The new `peios` crate has no `AceBuilder`; ACLs are assembled with a
// sticky-error `AclBuilder` whose adders (`allow`/`deny`/`audit`/`label`/`add`)
// take `&mut self`. `AclEdit` wraps that builder plus an ACE counter (so callers
// can ask `is_empty()`), and copies a borrowed `AceView` back in verbatim via
// `AclBuilder::add(&Ace { … })`.

use crate::error::{Error, Result};
use crate::target::PathTarget;
use peios::file::{SecInfo, get_sd, set_sd};
use peios::security::{
    Ace, AceFlags, AceType, AceView, Acl, AclBuilder, AclView, SdBuilder, SdView, Sid, SidRef,
};

/// Which ACL we're editing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AclKind {
    Dacl,
    Sacl,
}

impl AclKind {
    pub fn security_info(self) -> SecInfo {
        match self {
            AclKind::Dacl => SecInfo::DACL,
            AclKind::Sacl => SecInfo::SACL,
        }
    }
}

/// A growing ACL plus a running ACE count. Wraps the new sticky-error
/// `AclBuilder` (whose adders take `&mut self`) and tracks how many ACEs were
/// appended so callers can detect a present-but-empty ACL.
pub struct AclEdit {
    builder: AclBuilder,
    count: usize,
}

impl AclEdit {
    pub fn new() -> AclEdit {
        AclEdit {
            builder: AclBuilder::new(),
            count: 0,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    /// Append an access-allowed ACE.
    pub fn allow(&mut self, sid: &SidRef, mask: u32, flags: u8) {
        self.builder.allow(sid, mask, AceFlags::from_bits_retain(flags));
        self.count += 1;
    }

    /// Append an access-denied ACE.
    pub fn deny(&mut self, sid: &SidRef, mask: u32, flags: u8) {
        self.builder.deny(sid, mask, AceFlags::from_bits_retain(flags));
        self.count += 1;
    }

    /// Append a system-audit ACE.
    pub fn audit(&mut self, sid: &SidRef, mask: u32, flags: u8) {
        self.builder.audit(sid, mask, AceFlags::from_bits_retain(flags));
        self.count += 1;
    }

    /// Append a mandatory-integrity label ACE.
    pub fn label(&mut self, integrity_rid: u32, policy: u32) {
        use peios::security::LabelPolicy;
        self.builder
            .label(integrity_rid, LabelPolicy::from_bits_retain(policy));
        self.count += 1;
    }

    /// Append a conditional (callback) ACE: a raw ACE-type discriminant with the
    /// `artx` condition bytecode in `app_data`.
    pub fn callback(&mut self, ace_type_raw: u8, sid: &SidRef, mask: u32, flags: u8, artx: &[u8]) {
        let ace = Ace {
            ace_type: AceType::Other(ace_type_raw),
            flags: AceFlags::from_bits_retain(flags),
            mask,
            sid,
            object_type: None,
            inherited_object_type: None,
            app_data: Some(artx),
        };
        self.builder.add(&ace);
        self.count += 1;
    }

    /// Copy a borrowed `AceView` back in verbatim (type, flags, mask, sid, and
    /// any callback/resource app-data preserved).
    pub fn copy_in(&mut self, ace: &AceView<'_>) {
        let Some(sid) = ace.sid() else {
            // An ACE with no trustee SID (object-only / malformed) cannot be
            // re-expressed through the SID-bearing `Ace`; drop it. The verbs
            // that copy ACEs (allow/audit/inherit) only deal with SID-bearing
            // DACL/SACL ACEs, so this is unreachable in practice.
            return;
        };
        let ace_lit = Ace {
            ace_type: ace.ace_type(),
            flags: ace.flags(),
            mask: ace.mask(),
            sid,
            object_type: ace.object_type(),
            inherited_object_type: ace.inherited_object_type(),
            app_data: ace.app_data(),
        };
        self.builder.add(&ace_lit);
        self.count += 1;
    }

    fn build(&self) -> Result<Acl> {
        self.builder
            .build()
            .map_err(|e| Error::Invalid(format!("building ACL: {e}")))
    }

    /// Public form of [`build`](Self::build), for callers that assemble the SD
    /// themselves (e.g. `sd inherit`).
    pub fn build_public(&self) -> Result<Acl> {
        self.build()
    }
}

impl Default for AclEdit {
    fn default() -> Self {
        Self::new()
    }
}

/// Read the named ACL into a fresh editable builder. Returns an empty builder
/// if the SD has no ACL of this kind.
pub fn read_acl_builder(target: &PathTarget, kind: AclKind) -> Result<AclEdit> {
    let sd = get_sd(target.dirfd(), target.as_path(), kind.security_info(), target.at_flags())
        .map_err(Error::from)?;
    let bytes = sd.as_bytes();
    if bytes.is_empty() {
        return Ok(AclEdit::new());
    }
    let view = SdView::parse(bytes).map_err(|e| Error::Invalid(format!("parsing SD bytes: {e}")))?;
    let acl_opt = match kind {
        AclKind::Dacl => view.dacl(),
        AclKind::Sacl => view.sacl(),
    };
    let mut b = AclEdit::new();
    if let Some(acl) = acl_opt {
        for ace in acl.iter() {
            b.copy_in(&ace);
        }
    }
    Ok(b)
}

/// Write `acl` back as the named component of `target`'s SD.
pub fn write_acl(target: &PathTarget, kind: AclKind, acl: AclEdit) -> Result<()> {
    let built = acl.build()?;
    let mut sd = SdBuilder::new();
    match kind {
        AclKind::Dacl => {
            sd.dacl(&built);
        }
        AclKind::Sacl => {
            sd.sacl(&built);
        }
    };
    let out = sd
        .build()
        .map_err(|e| Error::Invalid(format!("building SD: {e}")))?;
    set_sd(
        target.dirfd(),
        target.as_path(),
        kind.security_info(),
        &out,
        target.at_flags(),
    )
    .map_err(Error::from)?;
    Ok(())
}

/// Apply a filter to an existing ACL, returning a new editable builder with only
/// the ACEs the predicate keeps, plus the number dropped.
pub fn filter_acl<F>(target: &PathTarget, kind: AclKind, mut keep: F) -> Result<(AclEdit, usize)>
where
    F: FnMut(&AceView<'_>) -> bool,
{
    let sd = get_sd(target.dirfd(), target.as_path(), kind.security_info(), target.at_flags())
        .map_err(Error::from)?;
    let bytes = sd.as_bytes();
    if bytes.is_empty() {
        return Ok((AclEdit::new(), 0));
    }
    let view = SdView::parse(bytes).map_err(|e| Error::Invalid(format!("parsing SD bytes: {e}")))?;
    let acl_opt = match kind {
        AclKind::Dacl => view.dacl(),
        AclKind::Sacl => view.sacl(),
    };
    let mut b = AclEdit::new();
    let mut dropped = 0usize;
    if let Some(acl) = acl_opt {
        for ace in acl.iter() {
            if keep(&ace) {
                b.copy_in(&ace);
            } else {
                dropped += 1;
            }
        }
    }
    Ok((b, dropped))
}

/// The trustee SID of an ACE as an owned `Sid`, if it has one.
pub fn ace_sid(ace: &AceView<'_>) -> Option<Sid> {
    ace.sid().map(SidRef::to_sid)
}

/// The (mask, sid) of a plain access ACE, mirroring the old `as_mask_sid`.
pub fn ace_mask_sid(ace: &AceView<'_>) -> Option<(u32, Sid)> {
    ace.sid().map(|s| (ace.mask(), s.to_sid()))
}

/// True if this ACE is an "explicit" (non-inherited) one. Used to detect the
/// empty-DACL footgun where `sd remove` would leave a present-but-empty DACL.
pub fn is_explicit(ace: &AceView<'_>) -> bool {
    !ace.flags().contains(AceFlags::INHERITED)
}

/// Count ACEs in `acl` whose trustee is `principal`.
pub fn count_aces_for_principal(acl: Option<&AclView<'_>>, principal: &Sid) -> usize {
    let Some(acl) = acl else { return 0 };
    acl.iter()
        .filter(|ace| ace.sid().map(|s| s == &**principal).unwrap_or(false))
        .count()
}
