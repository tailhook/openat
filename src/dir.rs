use std::io;
use std::ffi::{OsString, CStr};
use std::os::unix::io::{AsRawFd, RawFd};
use std::os::unix::ffi::{OsStringExt};
use std::path::{PathBuf};

use libc;
use ffi;
use list::{Directory, open_dir};

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
        let fd = unsafe { libc::open(path.as_ptr(), ffi::O_PATH) };
        if fd < 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(Dir(DirFd::Fd(fd)))
        }
    }

    /// List subdirectory of this dir
    ///
    /// You can list directory itself if `"."` is specified as path.
    pub fn list_dir<P: AsPath>(&self, path: P) -> io::Result<Directory> {
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
                        ffi::O_PATH)
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
