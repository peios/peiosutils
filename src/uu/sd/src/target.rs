// Path target resolution.
//
// v1 surface: filesystem paths via SdTarget::Path. The grammar is
// target-agnostic so we can grow `--process N` / `--registry /foo` later
// without reshaping `Target`.

use libp_sd::SdTarget;

/// What `sd` is operating on. v1: paths only.
#[derive(Debug, Clone)]
pub struct PathTarget {
    pub path: String,
    pub no_follow_symlinks: bool,
}

impl PathTarget {
    /// Lower to a borrowed `SdTarget` suitable for `get_sd` / `set_sd`.
    pub fn as_sd_target(&self) -> SdTarget<'_> {
        SdTarget::Path {
            dirfd: libp_sd::raw::FDCWD,
            path: &self.path,
            no_follow_symlinks: self.no_follow_symlinks,
        }
    }
}
