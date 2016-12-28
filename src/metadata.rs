use std::fs::Permissions;
use std::os::unix::fs::PermissionsExt;

use libc;

use SimpleType;


/// A file metadata
///
/// Because we can't freely create a `std::fs::Metadata` object we have to
/// implement our own structure.
pub struct Metadata {
    stat: libc::stat,
}

impl Metadata {
    /// Returns simplified type of the directory entry
    pub fn simple_type(&self) -> Option<SimpleType> {
        let typ = self.stat.st_mode & libc::S_IFMT;
        match typ {
            libc::S_IFREG => Some(SimpleType::File),
            libc::S_IFDIR => Some(SimpleType::Dir),
            libc::S_IFLNK => Some(SimpleType::Symlink),
            _ => Some(SimpleType::Other)
        }
    }
    /// Returns underlying stat structure
    pub fn stat(&self) -> &libc::stat {
        &self.stat
    }
    /// Returns `true` if the entry is a regular file
    pub fn is_file(&self) -> bool {
        self.simple_type() == Some(SimpleType::File)
    }
    /// Returns `true` if the entry is a directory
    pub fn is_dir(&self) -> bool {
        self.simple_type() == Some(SimpleType::File)
    }
    /// Returns permissions of the entry
    pub fn permissions(&self) -> Permissions {
        Permissions::from_mode(self.stat.st_mode)
    }
    /// Returns file size
    pub fn len(&self) -> u64 {
        self.stat.st_size as u64
    }
}

pub fn new(stat: libc::stat) -> Metadata {
    Metadata { stat: stat }
}
