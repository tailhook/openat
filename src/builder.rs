use std::ffi::CStr;
use std::fs::File;
use std::io;

use crate::dir::{clone_dirfd_upgrade, to_cstr};
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
}

/// 'Dir::with(&self)'/'Dir::with(&self)' creates a new DirMethodsFlags object with default
/// (O_CLOEXEC|O_NOFOLLOW) flags. One can then freely add/remove flags to the set.
/// Implements proxies for the Dir:: methods that open contained objects.
#[derive(Copy, Clone)]
pub struct DirMethodFlags<'a> {
    object: &'a Dir,
    flags:  libc::c_int,
}

impl<'a> DirMethodFlags<'a> {
    #[inline]
    pub(crate) fn new(object: &'a Dir, flags: libc::c_int) -> Self {
        Self { object, flags }
    }

    /// Sets the given flags
    #[inline]
    pub fn with(self, flags: libc::c_int) -> Self {
        Self {
            object: self.object,
            flags:  self.flags | flags,
        }
    }

    /// Clears the given flags
    #[inline]
    pub fn without(self, flags: libc::c_int) -> Self {
        Self {
            object: self.object,
            flags:  self.flags & !flags,
        }
    }

    /// Open subdirectory
    #[inline]
    pub fn sub_dir<P: AsPath>(&self, path: P) -> io::Result<Dir> {
        self.object._sub_dir(to_cstr(path)?.as_ref(), self.flags)
    }

    /// Open file for reading in this directory
    #[inline]
    pub fn open_file<P: AsPath>(&self, path: P) -> io::Result<File> {
        self.object
            ._open_file(to_cstr(path)?.as_ref(), self.flags | libc::O_RDONLY, 0)
    }

    /// Open file for writing, create if necessary, truncate on open
    #[inline]
    pub fn write_file<P: AsPath>(&self, path: P, mode: libc::mode_t) -> io::Result<File> {
        self.object._open_file(
            to_cstr(path)?.as_ref(),
            self.flags | libc::O_CREAT | libc::O_WRONLY | libc::O_TRUNC,
            mode,
        )
    }

    /// Open file for append, create if necessary
    #[inline]
    pub fn append_file<P: AsPath>(&self, path: P, mode: libc::mode_t) -> io::Result<File> {
        self.object._open_file(
            to_cstr(path)?.as_ref(),
            self.flags | libc::O_CREAT | libc::O_WRONLY | libc::O_APPEND,
            mode,
        )
    }

    /// Create a tmpfile in this directory which isn't linked to any filename
    #[cfg(feature = "o_tmpfile")]
    #[inline]
    pub fn new_unnamed_file(&self, mode: libc::mode_t) -> io::Result<File> {
        self.object._open_file(
            unsafe { CStr::from_bytes_with_nul_unchecked(b".\0") },
            self.flags | libc::O_TMPFILE | libc::O_WRONLY,
            mode,
        )
    }

    /// Create file if not exists, fail if exists
    #[inline]
    pub fn new_file<P: AsPath>(&self, path: P, mode: libc::mode_t) -> io::Result<File> {
        self.object._open_file(
            to_cstr(path)?.as_ref(),
            self.flags | libc::O_CREAT | libc::O_EXCL | libc::O_WRONLY,
            mode,
        )
    }

    /// Creates a new 'Normal' independently owned handle to the underlying directory.
    pub fn clone_upgrade(&self) -> io::Result<Dir> {
        Ok(Dir::new(clone_dirfd_upgrade(
            self.object.rawfd()?,
            self.flags,
        )?))
    }
}
