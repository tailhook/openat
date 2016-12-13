use std::io;
use std::ffi::CString;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;

use libc;

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
        let path = CString::new(path.as_ref().as_os_str().as_bytes())?;
        let fd = unsafe { libc::open(path.as_ptr(), libc::O_DIRECTORY) };
        if fd < 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(Dir(DirFd::Fd(fd)))
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
    use {Dir};

    #[test]
    fn test_open_ok() {
        assert!(Dir::open("src").is_ok());
    }

    #[test]
    #[should_panic(expected="Not a directory")]
    fn test_open_not_dir() {
        Dir::open("src/lib.rs").unwrap();
    }

    #[test]
    #[should_panic(expected="No such file or directory")]
    fn test_open_no_dir() {
        Dir::open("src/some-non-existent-file").unwrap();
    }
}
