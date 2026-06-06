// This file is part of the uutils coreutils package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

#![cfg(any(target_os = "linux", target_os = "android"))]

//! runcon is not applicable on Peios: it runs a command in a specified
//! SELinux security context, and Peios's LSM is KACS, which replaces
//! SELinux outright. SELinux has no presence on Peios and none is
//! planned, so `uumain` errors out unconditionally.
//!
//! The original SELinux implementation is preserved verbatim in the
//! `selinux_impl` module, behind the off-by-default `feat_selinux`
//! cargo feature — dormant, not deleted. It stays gated because the
//! `selinux` crate links `libselinux`, which cannot build on a Peios
//! toolchain; the default build excludes it.

use uucore::error::UResult;

#[cfg(feature = "feat_selinux")]
#[allow(unused)]
mod errors;
#[cfg(feature = "feat_selinux")]
#[allow(unused)]
mod selinux_impl;

#[uucore::main]
pub fn uumain(_args: impl uucore::Args) -> UResult<()> {
    Err(uucore::error::not_applicable_on_peios(
        "it sets an SELinux security context; Peios's LSM is KACS, \
         which replaces SELinux",
    ))
}
