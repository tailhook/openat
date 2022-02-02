use std::io;
use std::ptr;
use std::ffi::{CStr, OsStr, CString};
use std::mem;
use std::os::unix::ffi::OsStrExt;
use std::sync::atomic::{AtomicPtr, Ordering};
use std::sync::Arc;

use libc;

use crate::{dir::libc_ok, metadata, Metadata, SimpleType};

// We have such weird constants because C types are ugly
const DOT: [libc::c_char; 2] = [b'.' as libc::c_char, 0];
const DOTDOT: [libc::c_char; 3] = [b'.' as libc::c_char, b'.' as libc::c_char, 0];

/// Iterator over directory entries
///
/// Created using `Dir::list_dir()`
#[derive(Debug)]
pub struct DirIter {
    // Needs Arc here to be shared with Entries, for metdata()
    dir: Arc<DirHandle>,
}

// It may not be thread-safe to call readdir concurrently from multiple threads on a single
// `DIR*`, but all `Send` requires is that we can call it from different threads
// non-concurrently - so this is fine.
//
// `man readdir` says:
//
// > It is expected that a future version of POSIX.1 will require that readdir() be
// > thread-safe when concurrently employed on different directory streams.
//
// so in the future we may also be able to implement `Sync`.
unsafe impl Send for DirIter {}

/// Position in a DirIter as obtained by 'DirIter::current_position()'
///
/// The position is only valid for the DirIter it was retrieved from.
pub struct DirPosition {
    pos: libc::c_long,
}

/// Entry returned by iterating over `DirIter` iterator
#[derive(Debug)]
pub struct Entry {
    dir: Arc<DirHandle>,
    pub name: CString,
    pub file_type: Option<SimpleType>,
    pub ino: libc::ino_t,
}

impl Entry {
    /// Returns the file name of this entry
    pub fn file_name(&self) -> &OsStr {
        OsStr::from_bytes(self.name.to_bytes())
    }

    /// Returns the simplified type of this entry
    pub fn simple_type(&self) -> Option<SimpleType> {
        self.file_type
    }

    /// Returns the inode number of this entry
    pub fn inode(&self) -> libc::ino_t {
        self.ino
    }

    /// Returns the metadata of this entry
    pub fn metadata(&self) -> io::Result<Metadata> {
        unsafe {
            let mut stat = mem::zeroed(); // TODO(cehteh): uninit
            libc_ok(libc::fstatat(
                libc::dirfd(self.dir.raw()?),
                self.name.as_ptr(),
                &mut stat,
                libc::AT_SYMLINK_NOFOLLOW,
            ))?;
            Ok(metadata::new(stat))
        }
    }

    /// Closes the iterators directory handle, stops the iteration
    pub fn stop(&self) {
        self.dir.close();
    }
}

#[cfg(any(target_os="linux", target_os="fuchsia"))]
unsafe fn errno_location() -> *mut libc::c_int {
    libc::__errno_location()
}

#[cfg(any(target_os="openbsd", target_os="netbsd", target_os="android"))]
unsafe fn errno_location() -> *mut libc::c_int {
    libc::__errno()
}

#[cfg(not(any(target_os="linux", target_os="openbsd", target_os="netbsd", target_os="android", target_os="fuchsia")))]
unsafe fn errno_location() -> *mut libc::c_int {
    libc::__error()
}

impl DirIter {

    unsafe fn next_entry(&mut self) -> io::Result<Option<&libc::dirent>>
    {
        // Reset errno to detect if error occurred
        *errno_location() = 0;

        let entry = libc::readdir(self.dir.raw()?);
        if entry == ptr::null_mut() {
            if *errno_location() == 0 {
                return Ok(None)
            } else {
                return Err(io::Error::last_os_error());
            }
        }
        return Ok(Some(&*entry));
    }

    /// Returns the current directory iterator position. The result should be handled as opaque value
    pub fn current_position(&self) -> io::Result<DirPosition> {
        let pos = unsafe { libc::telldir(self.dir.raw()?) };

        if pos == -1 {
            Err(io::Error::last_os_error())
        } else {
            Ok(DirPosition { pos })
        }
    }

    // note the C-API does not report errors for seekdir/rewinddir, thus we don't do as well.
    /// Sets the current directory iterator position to some location queried by 'current_position()'
    pub fn seek(&self, position: DirPosition) {
        if let Ok(dir) = self.dir.raw() {
            unsafe { libc::seekdir(dir, position.pos) };
        }
    }

    /// Resets the current directory iterator position to the beginning
    pub fn rewind(&self) {
        if let Ok(dir) = self.dir.raw() {
            unsafe { libc::rewinddir(dir) };
        }
    }

    /// Closes the DIR handle, frees the underlying file descriptor
    pub fn close(&mut self) {
        self.dir.close();
    }
}

pub fn open_dirfd(fd: libc::c_int) -> io::Result<DirIter> {
    let dir = unsafe { libc::fdopendir(fd) };
    if dir == std::ptr::null_mut() {
        Err(io::Error::last_os_error())
    } else {
        Ok(DirIter {
            dir: Arc::new(DirHandle::new(dir)),
        })
    }
}

impl Iterator for DirIter {
    type Item = io::Result<Entry>;
    fn next(&mut self) -> Option<Self::Item> {
        unsafe {
            loop {
                let dir = Arc::clone(&self.dir);
                match self.next_entry() {
                    Err(e) => return Some(Err(e)),
                    Ok(None) => return None,
                    Ok(Some(e)) if e.d_name[..2] == DOT => continue,
                    Ok(Some(e)) if e.d_name[..3] == DOTDOT => continue,
                    Ok(Some(e)) => {
                        return Some(Ok(Entry {
                            dir,
                            name: CStr::from_ptr((e.d_name).as_ptr()).to_owned(),
                            file_type: match e.d_type {
                                0 => None,
                                libc::DT_REG => Some(SimpleType::File),
                                libc::DT_DIR => Some(SimpleType::Dir),
                                libc::DT_LNK => Some(SimpleType::Symlink),
                                _ => Some(SimpleType::Other),
                            },
                            ino: e.d_ino,
                        }));
                    }
                }
            }
        }
    }
}

#[derive(Debug)]
struct DirHandle(AtomicPtr<libc::DIR>);

impl DirHandle {
    fn new(dir: *mut libc::DIR) -> Self {
        DirHandle(AtomicPtr::new(dir))
    }

    fn raw(&self) -> io::Result<*mut libc::DIR> {
        let dir = self.0.load(Ordering::Acquire);
        if !dir.is_null() {
            Ok(dir)
        } else {
            Err(io::Error::from_raw_os_error(libc::EBADF))
        }
    }

    fn close(&self) {
        let dir = self.0.swap(std::ptr::null_mut(), Ordering::AcqRel);
        if !dir.is_null() {
            unsafe {
                libc::closedir(dir);
            }
        }
    }
}

impl Drop for DirHandle {
    fn drop(&mut self) {
        self.close();
    }
}

#[cfg(test)]
mod test {
    use crate::Dir;

    #[test]
    fn test() {
        let d = Dir::open(".").unwrap();
        for e in d.list_self().unwrap() {
            if let Ok(e) = e {
                if let Ok(m) = e.metadata() {
                    eprintln!("{:?} : {:?}", e.file_name(), m);
                }
            }
        }
    }
}
