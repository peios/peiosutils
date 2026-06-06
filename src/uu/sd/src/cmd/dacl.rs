// Shared read-modify-write helper for DACL/SACL edits.
//
// allow/deny/remove (DACL) and audit/unaudit (SACL) all do:
//   1. get_sd (DACL or SACL only)
//   2. parse → AclBuilder
//   3. mutate (append or filter)
//   4. rebuild SD with just the changed component
//   5. set_sd with the matching SecurityInfo bit
//
// This module hides the parse/rebuild dance.

use crate::error::{Error, Result};
use crate::target::PathTarget;
use libp_sd::{
    AceBuilder, AclBuilder, SdBuilder, SecurityDescriptor, SecurityInfo, get_sd, set_sd,
};
use libp_sd::consts::{ACE_FLAG_INHERITED, AceRef, Acl};
use libp_sd::Sid;

/// Which ACL we're editing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AclKind {
    Dacl,
    Sacl,
}

impl AclKind {
    pub fn security_info(self) -> SecurityInfo {
        match self {
            AclKind::Dacl => SecurityInfo::dacl(),
            AclKind::Sacl => SecurityInfo::sacl(),
        }
    }
}

/// Read the named ACL into a builder. Returns an empty builder if the SD
/// has no ACL of this kind.
pub fn read_acl_builder(target: &PathTarget, kind: AclKind) -> Result<AclBuilder> {
    let bytes = get_sd(&target.as_sd_target(), kind.security_info()).map_err(Error::from)?;
    if bytes.is_empty() {
        return Ok(AclBuilder::new());
    }
    let sd = SecurityDescriptor::parse(&bytes)
        .map_err(|e| Error::Invalid(format!("parsing SD bytes: {e}")))?;
    let acl_opt = match kind {
        AclKind::Dacl => sd.dacl(),
        AclKind::Sacl => sd.sacl(),
    };
    let mut b = AclBuilder::new();
    if let Some(acl_r) = acl_opt {
        let acl = acl_r.map_err(|e| Error::Invalid(format!("parsing ACL: {e}")))?;
        for ace_r in acl.aces_iter() {
            let ace = ace_r.map_err(|e| Error::Invalid(format!("parsing ACE: {e}")))?;
            b = b.ace(AceBuilder::from_ace_ref(&ace));
        }
    }
    Ok(b)
}

/// Write `acl` back as the named component of `target`'s SD.
pub fn write_acl(target: &PathTarget, kind: AclKind, acl: AclBuilder) -> Result<()> {
    let mut sd = SdBuilder::new();
    sd = match kind {
        AclKind::Dacl => sd.dacl(acl),
        AclKind::Sacl => sd.sacl(acl),
    };
    let bytes = sd
        .build()
        .map_err(|e| Error::Invalid(format!("building SD: {e}")))?;
    set_sd(&target.as_sd_target(), kind.security_info(), &bytes).map_err(Error::from)?;
    Ok(())
}

/// True if any ACE in `acl_bytes` matches `principal` after optional
/// kind-filter. Used by `--replace` to know whether it needs to filter.
pub fn count_aces_for_principal(acl_bytes: Option<&Acl<'_>>, principal: &Sid) -> usize {
    let Some(acl) = acl_bytes else { return 0 };
    let mut n = 0usize;
    for ace_r in acl.aces_iter() {
        if let Ok(ace) = ace_r {
            if let Some((_, sid)) = ace.as_mask_sid() {
                if &sid.to_owned() == principal {
                    n += 1;
                }
            }
        }
    }
    n
}

/// Apply a filter function to an existing ACL, returning a new builder
/// with only the ACEs the predicate keeps.
pub fn filter_acl<F>(target: &PathTarget, kind: AclKind, keep: F) -> Result<(AclBuilder, usize)>
where
    F: FnMut(&AceRef<'_>) -> bool,
{
    let bytes = get_sd(&target.as_sd_target(), kind.security_info()).map_err(Error::from)?;
    if bytes.is_empty() {
        return Ok((AclBuilder::new(), 0));
    }
    let sd = SecurityDescriptor::parse(&bytes)
        .map_err(|e| Error::Invalid(format!("parsing SD bytes: {e}")))?;
    let acl_opt = match kind {
        AclKind::Dacl => sd.dacl(),
        AclKind::Sacl => sd.sacl(),
    };
    let mut keep = keep;
    let mut b = AclBuilder::new();
    let mut dropped = 0usize;
    if let Some(acl_r) = acl_opt {
        let acl = acl_r.map_err(|e| Error::Invalid(format!("parsing ACL: {e}")))?;
        for ace_r in acl.aces_iter() {
            let ace = ace_r.map_err(|e| Error::Invalid(format!("parsing ACE: {e}")))?;
            if keep(&ace) {
                b = b.ace(AceBuilder::from_ace_ref(&ace));
            } else {
                dropped += 1;
            }
        }
    }
    Ok((b, dropped))
}

/// True if this ACE is an "explicit" (non-inherited) one. Used to detect
/// the empty-DACL footgun where `sd remove` would leave a present-but-empty
/// DACL.
pub fn is_explicit(ace: &AceRef<'_>) -> bool {
    ace.flags & ACE_FLAG_INHERITED == 0
}
