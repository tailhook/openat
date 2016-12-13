#![warn(missing_docs)]

extern crate libc;

mod dir;

use std::sync::Arc;
use std::os::unix::io::RawFd;

/// A safe wrapper around directory file descriptor
///
/// Construct it either with ``Dir::cwd()`` or ``Dir::open(path)``
///
/// This structure contains an Arc to the actual file descriptor, so:
///
/// * so don't create to many of them at the same time (fd limit)
/// * structure can be freely sent and used by multiple threads
/// * cloning it is relatively cheap
///
#[derive(Clone, Debug)]
pub struct Dir(DirParam);

#[derive(Debug)]
struct DirFd(RawFd);

#[derive(Clone, Debug)]
enum DirParam {
    Fd(Arc<DirFd>),
    Cwd,
}


#[cfg(test)]
mod test {
    use std::mem;
    use std::sync::Arc;
    use super::{Dir, DirFd, DirParam};

    fn assert_sync<T: Sync>(x: T) -> T { x }
    fn assert_send<T: Send>(x: T) -> T { x }

    #[test]
    fn test() {
        let d = Dir(DirParam::Fd(Arc::new(DirFd(3))));
        let d = assert_sync(d);
        let d = assert_send(d);
        // don't execute close for our fake DirFd
        mem::forget(d);
    }
}

