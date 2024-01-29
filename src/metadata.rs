use std::fmt;
use std::fs::Permissions;
use std::os::unix::fs::PermissionsExt;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::SimpleType;

/// A file metadata
///
/// Because we can't freely create a `std::fs::Metadata` object we have to
/// implement our own structure.
pub struct Metadata {
    stat: libc::stat,
}

impl fmt::Debug for Metadata {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Metadata")
            .field("st_dev", &self.stat.st_dev)
            .field("st_ino", &self.stat.st_ino)
            .field("st_nlink", &self.stat.st_nlink)
            .field("st_mode", &self.stat.st_mode)
            .field("st_uid", &self.stat.st_uid)
            .field("st_gid", &self.stat.st_gid)
            .field("st_size", &self.stat.st_size)
            .field("st_blocks", &self.stat.st_blocks)
            .finish()
    }
}

/// Implements and exports the used types here. Depending on feature flags and operating
/// system the underlying types may change. By redefining them here this will stay consistent
/// to an user of the library.
#[allow(non_camel_case_types)]
#[allow(missing_docs)]
pub mod metadata_types {
    pub type mode_t = libc::mode_t;
    pub type ino_t = libc::ino_t;
    pub type dev_t = libc::dev_t;
    pub type c_uint = libc::c_uint;
    pub type blksize_t = libc::blksize_t;
    pub type blkcnt_t = libc::blkcnt_t;
    pub type off_t = libc::off_t;
    pub type nlink_t = libc::nlink_t;
    pub type uid_t = libc::uid_t;
    pub type gid_t = libc::gid_t;
}

use metadata_types::*;

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
    #[deprecated(
        since = "0.2.0",
        note = "future versions will use other underlying methods to gather metadata (statx on linux)."
    )]
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
    #[allow(clippy::len_without_is_empty)]
    #[deprecated(since = "0.2.0", note = "use Metadata::size(&self)")]
    pub fn len(&self) -> u64 {
        self.stat.st_size as u64
    }

    /// Return low level file mode, if available
    pub fn mode(&self) -> Option<mode_t> {
        Some(self.stat.st_mode)
    }

    /// Return low level file type, if available
    pub fn file_type(&self) -> Option<mode_t> {
        Some(self.stat.st_mode & libc::S_IFMT)
    }

    /// Return device node, if available
    pub fn ino(&self) -> Option<ino_t> {
        Some(self.stat.st_ino)
    }

    /// Return device node of the file, if available
    pub fn dev(&self) -> Option<dev_t> {
        Some(self.stat.st_dev)
    }

    /// Return device node major of the file, if available
    pub fn dev_major(&self) -> Option<c_uint> {
        Some(major(self.stat.st_dev))
    }

    /// Return device node minor of the file, if available
    pub fn dev_minor(&self) -> Option<c_uint> {
        Some(minor(self.stat.st_dev))
    }

    /// Return device node of an device descriptor, if available
    pub fn rdev(&self) -> Option<dev_t> {
        match self.mode()? {
            libc::S_IFBLK | libc::S_IFCHR => Some(self.stat.st_rdev),
            _ => None,
        }
    }

    /// Return device node major of an device descriptor, if available
    pub fn rdev_major(&self) -> Option<c_uint> {
        Some(major(self.rdev()?))
    }

    /// Return device node minor of an device descriptor, if available
    pub fn rdev_minor(&self) -> Option<c_uint> {
        Some(minor(self.rdev()?))
    }

    /// Return preferered I/O Blocksize, if available
    pub fn blksize(&self) -> Option<blksize_t> {
        Some(self.stat.st_blksize)
    }

    /// Return the number of 512 bytes blocks, if available
    pub fn blocks(&self) -> Option<blkcnt_t> {
        Some(self.stat.st_blocks)
    }

    /// Returns file size (same as len() but Option), if available
    pub fn size(&self) -> Option<off_t> {
        Some(self.stat.st_size)
    }

    /// Returns number of hard-links, if available
    pub fn nlink(&self) -> Option<nlink_t> {
        Some(self.stat.st_nlink)
    }

    /// Returns user id, if available
    pub fn uid(&self) -> Option<uid_t> {
        Some(self.stat.st_uid)
    }

    /// Returns group id, if available
    pub fn gid(&self) -> Option<gid_t> {
        Some(self.stat.st_gid)
    }

    /// Returns mode bits, if available
    pub fn file_mode(&self) -> Option<mode_t> {
        Some(self.stat.st_mode & 0o7777)
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
    Metadata { stat }
}

fn unix_systemtime(sec: libc::time_t, nsec: i64) -> SystemTime {
    UNIX_EPOCH + Duration::from_secs(sec as u64) + Duration::from_nanos(nsec as u64)
}

#[cfg(not(target_os = "macos"))]
pub fn major(dev: libc::dev_t) -> libc::c_uint {
    unsafe { libc::major(dev) }
}

#[cfg(not(target_os = "macos"))]
pub fn minor(dev: libc::dev_t) -> libc::c_uint {
    unsafe { libc::minor(dev) }
}

// major/minor are not in rust's darwin libc (why)
// see https://github.com/apple/darwin-xnu/blob/main/bsd/sys/types.h
#[cfg(target_os = "macos")]
pub fn major(dev: libc::dev_t) -> libc::c_uint {
    (dev as u32 >> 24) & 0xff
}

#[cfg(target_os = "macos")]
pub fn minor(dev: libc::dev_t) -> libc::c_uint {
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
