use std::io;
use std::mem;
use std::ffi::{OsString, CStr};
use std::fs::{File, read_link};
use std::os::unix::io::{AsRawFd, RawFd, FromRawFd};
use std::os::unix::ffi::{OsStringExt};
use std::path::{PathBuf};

use libc;
use ffi;
use metadata::{self, Metadata};
use list::{DirIter, open_dir};

use {Dir, DirFd, AsPath};


impl Dir {
    /// Creates a directory descriptor that resolves paths relative to current
    /// workding directory (AT_FDCWD)
    pub fn cwd() -> Dir {
        Dir(DirFd::Cwd)
    }

    /// Open a directory descriptor at specified path
    // TODO(tailhook) maybe accept only absolute paths?
    pub fn open<P: AsPath>(path: P) -> io::Result<Dir> {
        Dir::_open(to_cstr(path)?.as_ref())
    }

    fn _open(path: &CStr) -> io::Result<Dir> {
        let fd = unsafe {
            libc::open(path.as_ptr(), ffi::O_PATH|libc::O_CLOEXEC)
        };
        if fd < 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(Dir(DirFd::Fd(fd)))
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
            libc::openat(self.as_raw_fd(),
                        path.as_ptr(),
                        ffi::O_PATH|libc::O_CLOEXEC|libc::O_NOFOLLOW)
        };
        if fd < 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(Dir(DirFd::Fd(fd)))
        }
    }

    /// Read link in this directory
    pub fn read_link<P: AsPath>(&self, path: P) -> io::Result<PathBuf> {
        self._read_link(to_cstr(path)?.as_ref())
    }

    fn _read_link(&self, path: &CStr) -> io::Result<PathBuf> {
        let mut buf = vec![0u8; 4096];
        let res = unsafe {
            libc::readlinkat(self.as_raw_fd(),
                        path.as_ptr(),
                        buf.as_mut_ptr() as *mut i8, buf.len())
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

    /// Create file for writing (and truncate) in this directory
    pub fn create_file<P: AsPath>(&self, path: P, mode: libc::mode_t)
        -> io::Result<File>
    {
        self._open_file(to_cstr(path)?.as_ref(),
            libc::O_CREAT|libc::O_WRONLY|libc::O_TRUNC,
            mode)
    }

    fn _open_file(&self, path: &CStr, flags: libc::c_int, mode: libc::mode_t)
        -> io::Result<File>
    {
        unsafe {
            let res = libc::openat(self.as_raw_fd(), path.as_ptr(),
                            flags|libc::O_CLOEXEC|libc::O_NOFOLLOW,
                            mode);
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
    pub fn symlink<P: AsPath, R: AsPath>(&self, path: P, value: P)
        -> io::Result<()>
    {
        self._symlink(to_cstr(path)?.as_ref(), to_cstr(value)?.as_ref())
    }
    fn _symlink(&self, path: &CStr, link: &CStr) -> io::Result<()> {
        unsafe {
            let res = libc::symlinkat(link.as_ptr(),
                self.as_raw_fd(), path.as_ptr());
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
            let res = libc::mkdirat(self.as_raw_fd(), path.as_ptr(), mode);
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

    /// Remove a subdirectory in this directory
    ///
    /// Note only empty directory may be removed
    pub fn remove_dir<P: AsPath>(&self, path: P)
        -> io::Result<()>
    {
        self._unlink(to_cstr(path)?.as_ref(), 0)
    }
    /// Remove a file in this directory
    pub fn remove_file<P: AsPath>(&self, path: P)
        -> io::Result<()>
    {
        self._unlink(to_cstr(path)?.as_ref(), ffi::AT_REMOVEDIR)
    }
    fn _unlink(&self, path: &CStr, flags: libc::c_int) -> io::Result<()> {
        unsafe {
            let res = libc::unlinkat(self.as_raw_fd(), path.as_ptr(), flags);
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
        match self.0 {
            DirFd::Fd(fd) => read_link(format!("/proc/self/fd/{}", fd)),
            DirFd::Cwd => read_link(format!("/proc/self/cwd")),
        }
    }

    /// Returns metadata of an entry in this directory
    pub fn metadata<P: AsPath>(&self, path: P) -> io::Result<Metadata> {
        self._stat(to_cstr(path)?.as_ref(), ffi::AT_SYMLINK_NOFOLLOW)
    }
    fn _stat(&self, path: &CStr, flags: libc::c_int) -> io::Result<Metadata> {
        unsafe {
            let mut stat = mem::zeroed();
            let res = libc::fstatat(self.as_raw_fd(), path.as_ptr(),
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
        let res = libc::renameat(old_dir.as_raw_fd(), old.as_ptr(),
            new_dir.as_raw_fd(), new.as_ptr());
        if res < 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(())
        }
    }
}

impl AsRawFd for Dir {
    fn as_raw_fd(&self) -> RawFd {
        match self.0 {
            DirFd::Fd(x) => x,
            DirFd::Cwd => libc::AT_FDCWD,
        }
    }
}

impl Drop for DirFd {
    fn drop(&mut self) {
        match *self {
            DirFd::Fd(x) => {
                unsafe {
                    libc::close(x);
                }
            }
            DirFd::Cwd => {}
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
    use std::io::{Read};
    use std::path::Path;
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
}
