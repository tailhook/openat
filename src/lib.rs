//! # Handling Files Relative to File Descriptor
//!
//! Main concept here is a `Dir` which holds `O_PATH` file descriptor, you
//! can create it with:
//!
//! * `Dir::open("/some/path")` -- open this directory as a file descriptor
//! * `Dir::cwd()` -- current working directory
//!
//! *Note after opening file descriptors refer to same directory regardless of
//! where it's moved or mounted (with `pivot_root` or `mount --move`). It may
//! also be unmounted or be out of chroot and you will still be able to
//! access files relative to it.*
//!
//! *Note that `Dir::cwd()` always refers to the current working directory of
//! an application (and doesn't keep file descriptor too) and is sensitive
//! to `chdir`. But otherwise behaves the same (i.e. directory may be
//! moved and process is still in it). Anyway it's not more useful than
//! just using relative paths and traditional filesystem utilities.*
//!
//! Most other operations are done on `Dir` object and are executed relative
//! to it:
//!
//! * `Dir::list_dir()`
//! * `Dir::sub_dir()`
//! * `Dir::read_link()`
//! * `Dir::open_file()`
//! * `Dir::create_file()`
//! * `Dir::create_dir()`
//! * `Dir::symlink()`
//! * `Dir::local_rename()`
//!
//! Functions that expect path relative to the directory accept both the
//! traditional path-like objects, such as Path, PathBuf and &str, and
//! `Entry` type returned from `list_dir()`. The latter is faster as underlying
//! system call wants `CString` and we keep that in entry.
//!
//! Note that if path supplied to any method of dir is absolute the Dir file
//! descriptor is ignored.
//!
//! Also while all methods of dir accept any path if you want to prevent
//! certain symlink attacks and race condition you should only use
//! a single-component path. I.e. open one part of a chain at a time.
//!
#![warn(missing_docs)]

extern crate libc;

mod dir;
mod ffi;
mod list;
mod name;
mod filetype;

pub use list::DirIter;
pub use name::AsPath;
pub use dir::rename;
pub use filetype::SimpleType;

use std::ffi::CString;
use std::os::unix::io::RawFd;

/// A safe wrapper around directory file descriptor
///
/// Construct it either with ``Dir::cwd()`` or ``Dir::open(path)``
///
#[derive(Debug)]
pub struct Dir(DirFd);

#[derive(Debug)]
enum DirFd {
    Fd(RawFd),
    Cwd,
}

/// Entry returned by iterating over `DirIter` iterator
#[derive(Debug)]
pub struct Entry {
    name: CString,
    file_type: Option<SimpleType>,
}

#[cfg(test)]
mod test {
    use std::mem;
    use super::{Dir, DirFd};

    fn assert_sync<T: Sync>(x: T) -> T { x }
    fn assert_send<T: Send>(x: T) -> T { x }

    #[test]
    fn test() {
        let d = Dir(DirFd::Fd(3));
        let d = assert_sync(d);
        let d = assert_send(d);
        // don't execute close for our fake DirFd
        mem::forget(d);
    }
}

