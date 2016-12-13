use std::io;
use std::ffi::CString;
use std::os::unix::io::{AsRawFd, RawFd};
use std::os::unix::ffi::OsStrExt;
use std::path::Path;

use libc;
use ffi;
use list::{Directory, open_dir};

use {Dir, DirFd};


impl Dir {
    /// Creates a directory descriptor that resolves paths relative to current
    /// workding directory (AT_FDCWD)
    pub fn cwd() -> Dir {
        Dir(DirFd::Cwd)
    }

    /// Open a directory descriptor at specified path
    // TODO(tailhook) maybe accept only absolute paths?
    pub fn open<P: AsRef<Path>>(path: P) -> io::Result<Dir> {
        Dir::_open(path.as_ref())
    }

    fn _open(path: &Path) -> io::Result<Dir> {
        let path = CString::new(path.as_os_str().as_bytes())?;
        let fd = unsafe { libc::open(path.as_ptr(), ffi::O_PATH) };
        if fd < 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(Dir(DirFd::Fd(fd)))
        }
    }

    /// List subdirectory of this dir (or this directory itself, is empty path
    /// is specified)
    pub fn list_dir<P: AsRef<Path>>(&self, path: P) -> io::Result<Directory> {
        open_dir(self, path.as_ref())
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
                .iter().find(|x| x.name() == Path::new("lib.rs"))
                .is_some());
    }
}
