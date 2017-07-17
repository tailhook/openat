use std::io;
use std::ptr;
use std::ffi::{CStr, OsStr};
use std::os::unix::io::AsRawFd;
use std::os::unix::ffi::OsStrExt;

use ffi;
use libc;

use {Dir, Entry, SimpleType};


// We have such weird constants because C types are ugly
const DOT: [u8; 2] = [b'.' as u8, 0];
const DOTDOT: [u8; 3] = [b'.' as u8, b'.' as u8, 0];


/// Iterator over directory entries
///
/// Created using `Dir::list_dir()`
#[derive(Debug)]
pub struct DirIter {
    dir: *mut libc::DIR,
}

impl Entry {
    /// Returns the file name of the this entry
    pub fn file_name(&self) -> &OsStr {
        OsStr::from_bytes(self.name.to_bytes())
    }
    /// Returns simplified type of entry
    pub fn simple_type(&self) -> Option<SimpleType> {
        self.file_type
    }
}

impl DirIter {

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

pub fn open_dir(dir: &Dir, path: &CStr) -> io::Result<DirIter> {
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
            Ok(DirIter { dir: dir })
        }
    }
}

impl Iterator for DirIter {
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
                            file_type: match (*e).d_type {
                                0 => None,
                                libc::DT_REG => Some(SimpleType::File),
                                libc::DT_DIR => Some(SimpleType::Dir),
                                libc::DT_LNK => Some(SimpleType::Symlink),
                                _ => Some(SimpleType::Other),
                            },
                        }));
                    }
                }
            }
        }
    }
}

impl Drop for DirIter {
    fn drop(&mut self) {
        unsafe {
            libc::closedir(self.dir);
        }
    }
}
