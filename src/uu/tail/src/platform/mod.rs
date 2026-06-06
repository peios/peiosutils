// This file is part of the uutils coreutils package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

pub use self::unix::{Pid, ProcessChecker, supports_pid_checks};

mod unix;
