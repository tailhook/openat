use std::io;

use crate::dir::{to_cstr, O_PATH_FLAG};
use crate::{AsPath, Dir};

/// 'Dir::new()' creates a new DirFlags object with default (O_CLOEXEC) flags. One can then
/// freely add/remove flags to the set. The final open calls will add O_DIRECTORY and O_PATH
/// as applicable/supported but not verify or remove any defined flags. This allows passing
/// flags the 'openat' implementation is not even aware about. Thus the open call may fail
/// with some error when one constructed an invalid flag set.
#[derive(Copy, Clone)]
pub struct DirFlags {
    flags: libc::c_int,
}

impl DirFlags {
    #[inline]
    pub(crate) fn new(flags: libc::c_int) -> DirFlags {
        DirFlags { flags }
    }

    /// Sets the given flags
    #[inline]
    pub fn with(self, flags: libc::c_int) -> DirFlags {
        DirFlags {
            flags: self.flags | flags,
        }
    }

    /// Clears the given flags
    #[inline]
    pub fn without(self, flags: libc::c_int) -> DirFlags {
        DirFlags {
            flags: self.flags & !flags,
        }
    }

    /// Queries current flags
    #[inline]
    pub fn get_flags(&self) -> libc::c_int {
        self.flags
    }

    /// Open a directory descriptor at specified path
    #[inline]
    pub fn open<P: AsPath>(&self, path: P) -> io::Result<Dir> {
        Dir::_open(to_cstr(path)?.as_ref(), self.flags)
    }

    /// Open a lite directory descriptor at specified path
    #[inline]
    pub fn open_lite<P: AsPath>(&self, path: P) -> io::Result<Dir> {
        Dir::_open(to_cstr(path)?.as_ref(), O_PATH_FLAG | self.flags)
    }
}

