// This file is part of the uutils coreutils package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.
pub use self::unix::{instantiate_current_writer, paths_refer_to_same_file};

mod unix;
