#![warn(missing_docs)]

extern crate libc;

mod dir;
mod ffi;
mod list;

pub use list::Directory;

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

