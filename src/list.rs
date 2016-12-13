use std::io;
use std::ptr;
use std::path::Path;
use std::ffi::{CString, CStr, OsStr};
use std::os::unix::io::AsRawFd;
use std::os::unix::ffi::OsStrExt;

use ffi;
use libc;

use {Dir};


// We have such weird constants because C types are ugly
const DOT: [i8; 2] = [b'.' as i8, 0];
const DOTDOT: [i8; 3] = [b'.' as i8, b'.' as i8, 0];


/// Iterator over directory entries
///
/// Created using `Dir::list_dir()`
#[derive(Debug)]
pub struct Directory {
    dir: *mut libc::DIR,
}

#[derive(Debug)]
pub struct Entry {
    name: CString,
}

impl Entry {
    pub fn name(&self) -> &Path {
        OsStr::from_bytes(self.name.to_bytes()).as_ref()
    }
}

impl Directory {

    unsafe fn next_entry(&mut self) -> io::Result<Option<*const libc::dirent>>
    {
        // Reset errno to detect if error occurred
        *libc::__errno_location() = 0;

        let entry = ffi::readdir(self.dir);
        if entry == ptr::null() {
            if *libc::__errno_location() == 0 {
                return Ok(None)
            } else {
                return Err(io::Error::last_os_error());
            }
        }
        return Ok(Some(entry));
    }
}

pub fn open_dir(dir: &Dir, path: &Path) -> io::Result<Directory> {
    let path = CString::new(path.as_os_str().as_bytes())?;
    let dir_fd = unsafe {
        libc::openat(dir.as_raw_fd(), path.as_ptr(), libc::O_DIRECTORY)
    };
    if dir_fd < 0 {
        Err(io::Error::last_os_error())
    } else {
        let dir = unsafe { ffi::fdopendir(dir_fd) };
        if dir == ptr::null_mut() {
            Err(io::Error::last_os_error())
        } else {
            Ok(Directory { dir: dir })
        }
    }
}

impl Iterator for Directory {
    type Item = io::Result<Entry>;
    fn next(&mut self) -> Option<Self::Item> {
        unsafe {
            loop {
                match self.next_entry() {
                    Err(e) => return Some(Err(e)),
                    Ok(None) => return None,
                    Ok(Some(e)) if (*e).d_name[..2] == DOT => continue,
                    Ok(Some(e)) if (*e).d_name[..3] == DOTDOT => continue,
                    Ok(Some(e)) => {
                        return Some(Ok(Entry {
                            name: CStr::from_ptr((&(*e).d_name).as_ptr())
                                .to_owned(),
                        }));
                    }
                }
            }
        }
    }
}

impl Drop for Directory {
    fn drop(&mut self) {
        unsafe {
            libc::closedir(self.dir);
        }
    }
}
