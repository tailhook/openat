use std::fs::Permissions;
use std::os::unix::fs::PermissionsExt;

use libc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::SimpleType;


/// A file metadata
///
/// Because we can't freely create a `std::fs::Metadata` object we have to
/// implement our own structure.
pub struct Metadata {
    stat: libc::stat,
}

impl Metadata {
    /// Returns simplified type of the directory entry
    pub fn simple_type(&self) -> SimpleType {
        match self.file_type().unwrap_or(0) as libc::mode_t {
            libc::S_IFREG => SimpleType::File,
            libc::S_IFDIR => SimpleType::Dir,
            libc::S_IFLNK => SimpleType::Symlink,
            0 => SimpleType::Unknown,
            _ => SimpleType::Other,
        }
    }
    /// Returns underlying stat structure
    pub fn stat(&self) -> &libc::stat {
        &self.stat
    }
    /// Returns `true` if the entry is a regular file
    pub fn is_file(&self) -> bool {
        self.simple_type() == SimpleType::File
    }
    /// Returns `true` if the entry is a directory
    pub fn is_dir(&self) -> bool {
        self.simple_type() == SimpleType::Dir
    }
    /// Returns permissions of the entry
    pub fn permissions(&self) -> Permissions {
        Permissions::from_mode(self.stat.st_mode as u32)
    }
    /// Returns file size
    pub fn len(&self) -> u64 {
        self.stat.st_size as u64
    }
    /// Return low level file type, if available
    pub fn file_type(&self) -> Option<u32> {
        Some(self.stat.st_mode as u32 & libc::S_IFMT as u32)
    }
    /// Return device node, if available
    pub fn ino(&self) -> Option<libc::ino_t> {
        Some(self.stat.st_ino)
    }
    /// Return device node major of the file, if available
    pub fn dev_major(&self) -> Option<u32> {
        Some(major(self.stat.st_dev))
    }
    /// Return device node minor of the file, if available
    pub fn dev_minor(&self) -> Option<u32> {
        Some(minor(self.stat.st_dev))
    }
    /// Return device node major of an device descriptor, if available
    pub fn rdev_major(&self) -> Option<u32> {
        match self.file_type()? as libc::mode_t {
            libc::S_IFBLK | libc::S_IFCHR => Some(major(self.stat.st_rdev)),
            _ => None,
        }
    }
    /// Return device node minor of an device descriptor, if available
    pub fn rdev_minor(&self) -> Option<u32> {
        match self.file_type()? as libc::mode_t {
            libc::S_IFBLK | libc::S_IFCHR => Some(minor(self.stat.st_rdev)),
            _ => None,
        }
    }
    /// Return preferered I/O Blocksize, if available
    pub fn blksize(&self) -> Option<u32> {
        Some(self.stat.st_blksize as u32)
    }
    /// Return the number of 512 bytes blocks, if available
    pub fn blocks(&self) -> Option<u64> {
        Some(self.stat.st_blocks as u64)
    }
    /// Returns file size (same as len() but Option), if available
    pub fn size(&self) -> Option<u64> {
        Some(self.stat.st_size as u64)
    }
    /// Returns number of hard-links, if available
    pub fn nlink(&self) -> Option<u32> {
        Some(self.stat.st_nlink as u32)
    }
    /// Returns user id, if available
    pub fn uid(&self) -> Option<u32> {
        Some(self.stat.st_uid as u32)
    }
    /// Returns group id, if available
    pub fn gid(&self) -> Option<u32> {
        Some(self.stat.st_gid as u32)
    }
    /// Returns mode bits, if available
    pub fn file_mode(&self) -> Option<u32> {
        Some(self.stat.st_mode as u32 & 0o7777)
    }
    /// Returns last access time, if available
    pub fn atime(&self) -> Option<SystemTime> {
        Some(unix_systemtime(self.stat.st_atime, self.stat.st_atime_nsec))
    }
    /// Returns creation, if available
    pub fn btime(&self) -> Option<SystemTime> {
        None
    }
    /// Returns last status change time, if available
    pub fn ctime(&self) -> Option<SystemTime> {
        Some(unix_systemtime(self.stat.st_ctime, self.stat.st_ctime_nsec))
    }
    /// Returns last modification time, if available
    pub fn mtime(&self) -> Option<SystemTime> {
        Some(unix_systemtime(self.stat.st_mtime, self.stat.st_mtime_nsec))
    }
}

pub fn new(stat: libc::stat) -> Metadata {
    Metadata { stat: stat }
}

fn unix_systemtime(sec: libc::time_t, nsec: i64) -> SystemTime {
    UNIX_EPOCH + Duration::from_secs(sec as u64) + Duration::from_nanos(nsec as u64)
}

#[cfg(not(target_os = "macos"))]
pub fn major(dev: libc::dev_t) -> u32 {
    unsafe { libc::major(dev) }
}

#[cfg(not(target_os = "macos"))]
pub fn minor(dev: libc::dev_t) -> u32 {
    unsafe { libc::minor(dev) }
}

// major/minor are not in rust's darwin libc (why)
// see https://github.com/apple/darwin-xnu/blob/main/bsd/sys/types.h
#[cfg(target_os = "macos")]
pub fn major(dev: libc::dev_t) -> u32 {
    (dev as u32 >> 24) & 0xff
}

#[cfg(target_os = "macos")]
pub fn minor(dev: libc::dev_t) -> u32 {
    dev as u32 & 0xffffff
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn dir() {
        let d = crate::Dir::open(".").unwrap();
        let m = d.metadata("src").unwrap();
        assert_eq!(m.simple_type(), SimpleType::Dir);
        assert!(m.is_dir());
        assert!(!m.is_file());
    }

    #[test]
    fn file() {
        let d = crate::Dir::open("src").unwrap();
        let m = d.metadata("lib.rs").unwrap();
        assert_eq!(m.simple_type(), SimpleType::File);
        assert!(!m.is_dir());
        assert!(m.is_file());
    }
}
