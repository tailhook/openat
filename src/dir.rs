use std::io;
use std::mem;
use std::ffi::{OsString, CStr};
use std::fs::{File, read_link};
use std::os::unix::io::{AsRawFd, RawFd, FromRawFd, IntoRawFd};
use std::os::unix::ffi::{OsStringExt};
use std::path::{PathBuf};

use libc;
use metadata::{self, Metadata};
use list::{DirIter, DOT, open_dir};

use {Dir, AsPath};

impl Dir {
    /// Creates a directory descriptor that resolves paths relative to current
    /// workding directory (AT_FDCWD)
    pub fn cwd() -> Dir {
        Dir(libc::AT_FDCWD)
    }

    /// Open a directory descriptor at specified path
    // TODO(tailhook) maybe accept only absolute paths?
    pub fn open<P: AsPath>(path: P) -> io::Result<Dir> {
        Dir::_open(to_cstr(path)?.as_ref())
    }

    fn _open(path: &CStr) -> io::Result<Dir> {
        let fd = unsafe {
            libc::open(path.as_ptr(), libc::O_PATH|libc::O_CLOEXEC)
        };
        if fd < 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(Dir(fd))
        }
    }

    /// List subdirectory of this dir
    ///
    /// You can list directory itself if `"."` is specified as path.
    pub fn list_dir<P: AsPath>(&self, path: P) -> io::Result<DirIter> {
        open_dir(self, to_cstr(path)?.as_ref())
    }

    /// Open subdirectory
    pub fn sub_dir<P: AsPath>(&self, path: P) -> io::Result<Dir> {
        self._sub_dir(to_cstr(path)?.as_ref())
    }

    fn _sub_dir(&self, path: &CStr) -> io::Result<Dir> {
        let fd = unsafe {
            libc::openat(self.0,
                        path.as_ptr(),
                        libc::O_PATH|libc::O_CLOEXEC|libc::O_NOFOLLOW)
        };
        if fd < 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(Dir(fd))
        }
    }

    /// Read link in this directory
    pub fn read_link<P: AsPath>(&self, path: P) -> io::Result<PathBuf> {
        self._read_link(to_cstr(path)?.as_ref())
    }

    fn _read_link(&self, path: &CStr) -> io::Result<PathBuf> {
        let mut buf = vec![0u8; 4096];
        let res = unsafe {
            libc::readlinkat(self.0,
                        path.as_ptr(),
                        buf.as_mut_ptr() as *mut libc::c_char, buf.len())
        };
        if res < 0 {
            Err(io::Error::last_os_error())
        } else {
            buf.truncate(res as usize);
            Ok(OsString::from_vec(buf).into())
        }
    }

    /// Open file for reading in this directory
    pub fn open_file<P: AsPath>(&self, path: P) -> io::Result<File> {
        self._open_file(to_cstr(path)?.as_ref(),
            libc::O_RDONLY, 0)
    }

    /// Open file for writing, create if necessary, truncate on open
    pub fn write_file<P: AsPath>(&self, path: P, mode: libc::mode_t)
        -> io::Result<File>
    {
        self._open_file(to_cstr(path)?.as_ref(),
            libc::O_CREAT|libc::O_WRONLY|libc::O_TRUNC,
            mode)
    }

    /// Open file for append, create if necessary
    pub fn append_file<P: AsPath>(&self, path: P, mode: libc::mode_t)
        -> io::Result<File>
    {
        self._open_file(to_cstr(path)?.as_ref(),
            libc::O_CREAT|libc::O_WRONLY|libc::O_APPEND,
            mode)
    }

    /// Create file for writing (and truncate) in this directory
    ///
    /// Deprecated alias for `write_file`
    #[deprecated(since="0.1.7", note="please use `write_file` instead")]
    pub fn create_file<P: AsPath>(&self, path: P, mode: libc::mode_t)
        -> io::Result<File>
    {
        self._open_file(to_cstr(path)?.as_ref(),
            libc::O_CREAT|libc::O_WRONLY|libc::O_TRUNC,
            mode)
    }

    /// Create file if not exists, fail if exists
    ///
    /// This function checks existence and creates file atomically with
    /// respect to other threads and processes.
    ///
    /// Technically it means passing `O_EXCL` flag to open.
    pub fn new_file<P: AsPath>(&self, path: P, mode: libc::mode_t)
        -> io::Result<File>
    {
        self._open_file(to_cstr(path)?.as_ref(),
            libc::O_CREAT|libc::O_EXCL|libc::O_WRONLY,
            mode)
    }

    /// Open file for reading and writing without truncation, create if needed
    pub fn update_file<P: AsPath>(&self, path: P, mode: libc::mode_t)
        -> io::Result<File>
    {
        self._open_file(to_cstr(path)?.as_ref(),
            libc::O_CREAT|libc::O_RDWR,
            mode)
    }

    fn _open_file(&self, path: &CStr, flags: libc::c_int, mode: libc::mode_t)
        -> io::Result<File>
    {
        unsafe {
            let res = libc::openat(self.0, path.as_ptr(),
                            flags|libc::O_CLOEXEC|libc::O_NOFOLLOW,
                            mode as libc::c_uint);
            if res < 0 {
                Err(io::Error::last_os_error())
            } else {
                Ok(File::from_raw_fd(res))
            }
        }
    }

    /// Make a symlink in this directory
    ///
    /// Note: the order of arguments differ from `symlinkat`
    pub fn symlink<P: AsPath, R: AsPath>(&self, path: P, value: R)
        -> io::Result<()>
    {
        self._symlink(to_cstr(path)?.as_ref(), to_cstr(value)?.as_ref())
    }
    fn _symlink(&self, path: &CStr, link: &CStr) -> io::Result<()> {
        unsafe {
            let res = libc::symlinkat(link.as_ptr(),
                self.0, path.as_ptr());
            if res < 0 {
                Err(io::Error::last_os_error())
            } else {
                Ok(())
            }
        }
    }

    /// Create a subdirectory in this directory
    pub fn create_dir<P: AsPath>(&self, path: P, mode: libc::mode_t)
        -> io::Result<()>
    {
        self._create_dir(to_cstr(path)?.as_ref(), mode)
    }
    fn _create_dir(&self, path: &CStr, mode: libc::mode_t) -> io::Result<()> {
        unsafe {
            let res = libc::mkdirat(self.0, path.as_ptr(), mode);
            if res < 0 {
                Err(io::Error::last_os_error())
            } else {
                Ok(())
            }
        }
    }

    /// Rename a file in this directory to another name (keeping same dir)
    pub fn local_rename<P: AsPath, R: AsPath>(&self, old: P, new: R)
        -> io::Result<()>
    {
        rename(self, to_cstr(old)?.as_ref(), self, to_cstr(new)?.as_ref())
    }

    /// Similar to `local_rename` but atomically swaps both paths
    ///
    /// Only supported on Linux.
    #[cfg(target_os="linux")]
    pub fn local_exchange<P: AsPath, R: AsPath>(&self, old: P, new: R)
        -> io::Result<()>
    {
        rename_flags(self, to_cstr(old)?.as_ref(),
            self, to_cstr(new)?.as_ref(),
            libc::RENAME_EXCHANGE)
    }

    /// Remove a subdirectory in this directory
    ///
    /// Note only empty directory may be removed
    pub fn remove_dir<P: AsPath>(&self, path: P)
        -> io::Result<()>
    {
        self._unlink(to_cstr(path)?.as_ref(), libc::AT_REMOVEDIR)
    }
    /// Remove a file in this directory
    pub fn remove_file<P: AsPath>(&self, path: P)
        -> io::Result<()>
    {
        self._unlink(to_cstr(path)?.as_ref(), 0)
    }
    fn _unlink(&self, path: &CStr, flags: libc::c_int) -> io::Result<()> {
        unsafe {
            let res = libc::unlinkat(self.0, path.as_ptr(), flags);
            if res < 0 {
                Err(io::Error::last_os_error())
            } else {
                Ok(())
            }
        }
    }

    /// Get the path of this directory (if possible)
    ///
    /// This uses symlinks in `/proc/self`, they sometimes may not be
    /// available so use with care.
    pub fn recover_path(&self) -> io::Result<PathBuf> {
        let fd = self.0;
        if fd != libc::AT_FDCWD {
            read_link(format!("/proc/self/fd/{}", fd))
        } else {
            read_link("/proc/self/cwd")
        }
    }

    /// Returns metadata of an entry in this directory
    pub fn metadata<P: AsPath>(&self, path: P) -> io::Result<Metadata> {
        self._stat(to_cstr(path)?.as_ref(), libc::AT_SYMLINK_NOFOLLOW)
    }
    fn _stat(&self, path: &CStr, flags: libc::c_int) -> io::Result<Metadata> {
        unsafe {
            let mut stat = mem::zeroed();
            let res = libc::fstatat(self.0, path.as_ptr(),
                &mut stat, flags);
            if res < 0 {
                Err(io::Error::last_os_error())
            } else {
                Ok(metadata::new(stat))
            }
        }
    }

}

/// Rename (move) a file between directories
///
/// Files must be on a single filesystem anyway. This funtion does **not**
/// fallback to copying if needed.
pub fn rename<P, R>(old_dir: &Dir, old: P, new_dir: &Dir, new: R)
    -> io::Result<()>
    where P: AsPath, R: AsPath,
{
    _rename(old_dir, to_cstr(old)?.as_ref(), new_dir, to_cstr(new)?.as_ref())
}

fn _rename(old_dir: &Dir, old: &CStr, new_dir: &Dir, new: &CStr)
    -> io::Result<()>
{
    unsafe {
        let res = libc::renameat(old_dir.0, old.as_ptr(),
            new_dir.0, new.as_ptr());
        if res < 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(())
        }
    }
}

/// Create a hardlink to a file
///
/// Files must be on a single filesystem even if they are in different
/// directories.
///
/// Note: by default ``linkat`` syscall doesn't resolve symbolic links, and
/// it's also behavior of this function. It's recommended to resolve symlinks
/// manually if needed.
pub fn hardlink<P, R>(old_dir: &Dir, old: P, new_dir: &Dir, new: R)
    -> io::Result<()>
    where P: AsPath, R: AsPath,
{
    _hardlink(old_dir, to_cstr(old)?.as_ref(), new_dir, to_cstr(new)?.as_ref())
}

fn _hardlink(old_dir: &Dir, old: &CStr, new_dir: &Dir, new: &CStr)
    -> io::Result<()>
{
    unsafe {
        let res = libc::linkat(old_dir.0, old.as_ptr(),
            new_dir.0, new.as_ptr(), 0);
        if res < 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(())
        }
    }
}

/// Rename (move) a file between directories with flags
///
/// Files must be on a single filesystem anyway. This funtion does **not**
/// fallback to copying if needed.
///
/// Only supported on Linux.
#[cfg(target_os="linux")]
pub fn rename_flags<P, R>(old_dir: &Dir, old: P, new_dir: &Dir, new: R,
    flags: libc::c_int)
    -> io::Result<()>
    where P: AsPath, R: AsPath,
{
    _rename_flags(old_dir, to_cstr(old)?.as_ref(),
        new_dir, to_cstr(new)?.as_ref(),
        flags)
}

#[cfg(target_os="linux")]
fn _rename_flags(old_dir: &Dir, old: &CStr, new_dir: &Dir, new: &CStr,
    flags: libc::c_int)
    -> io::Result<()>
{
    unsafe {
        let res = libc::syscall(
            libc::SYS_renameat2,
            old_dir.0, old.as_ptr(),
            new_dir.0, new.as_ptr(), flags);
        if res < 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(())
        }
    }
}

impl AsRawFd for Dir {
    #[inline]
    fn as_raw_fd(&self) -> RawFd {
        let fd = match self.0 {
            libc::AT_FDCWD => unsafe { libc::openat(self.0, DOT.as_ptr(), 0) },
            _ => self.0,
        };
        if fd < 0 {
            // TODO: libraries should not panic, consider a fallible method instead.
            panic!("as_raw_fd: invalid file descriptor")
        };
        fd
    }
}

impl FromRawFd for Dir {
    #[inline]
    unsafe fn from_raw_fd(fd: RawFd) -> Dir {
        Dir(fd)
    }
}

impl IntoRawFd for Dir {
    #[inline]
    fn into_raw_fd(self) -> RawFd {
        // TODO: libraries should not panic, consider a fallible method instead.
        let result = self.as_raw_fd();
        mem::forget(self);
        return result;
    }
}

impl Drop for Dir {
    fn drop(&mut self) {
        let fd = self.0;
        if fd != libc::AT_FDCWD {
            unsafe {
                libc::close(fd);
            }
        }
    }
}

fn to_cstr<P: AsPath>(path: P) -> io::Result<P::Buffer> {
    path.to_path()
    .ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidInput,
                       "nul byte in file name")
    })
}

#[cfg(test)]
mod test {
    use std::fs::File;
    use std::io::{Read};
    use std::path::Path;
    use std::os::unix::io::{AsRawFd, FromRawFd, IntoRawFd};
    use {Dir};

    #[test]
    fn test_open_ok() {
        assert!(Dir::open("src").is_ok());
    }

    #[test]
    fn test_open_file() {
        Dir::open("src/lib.rs").unwrap();
    }

    #[test]
    fn test_read_file() {
        let dir = Dir::open("src").unwrap();
        let mut buf = String::new();
        dir.open_file("lib.rs").unwrap()
            .read_to_string(&mut buf).unwrap();
        assert!(buf.find("extern crate libc;").is_some());
    }

    #[test]
    fn test_from_into() {
        let dir = Dir::open("src").unwrap();
        let dir = unsafe { Dir::from_raw_fd(dir.into_raw_fd()) };
        let mut buf = String::new();
        dir.open_file("lib.rs").unwrap()
            .read_to_string(&mut buf).unwrap();
        assert!(buf.find("extern crate libc;").is_some());
    }

    #[test]
    #[should_panic(expected="No such file or directory")]
    fn test_open_no_dir() {
        Dir::open("src/some-non-existent-file").unwrap();
    }

    #[test]
    fn test_list() {
        let dir = Dir::open("src").unwrap();
        let me = dir.list_dir(".").unwrap();
        assert!(me.collect::<Result<Vec<_>, _>>().unwrap()
                .iter().find(|x| {
                    x.file_name() == Path::new("lib.rs").as_os_str()
                })
                .is_some());
    }

    #[test]
    fn test_cwd_as_fd() {
        let dir = Dir::cwd();
        let cwd = unsafe { File::from_raw_fd(dir.as_raw_fd()) };
        let meta = cwd.metadata().expect("failed to get metadata");
        assert!(meta.is_dir());
    }

    #[test]
    fn test_cwd_into_fd() {
        let dir = Dir::cwd();
        let cwd = unsafe { File::from_raw_fd(dir.into_raw_fd()) };
        let meta = cwd.metadata().expect("failed to get metadata");
        assert!(meta.is_dir());
    }

    #[test]
    fn test_cwd_from_into() {
        let cwd = Dir::cwd();
        let dir = unsafe { Dir::from_raw_fd(cwd.into_raw_fd()) };
        let meta = dir.metadata(".").expect("failed to get Dir metadata");
        assert!(meta.is_dir());
    }
}
