use libc::{c_int, DIR, dirent};

#[cfg(all(target_os="linux", not(target_arch ="arm")))]
use libc;

#[cfg(all(target_os="linux", not(target_arch ="arm")))]
pub const O_DIRECTORY: c_int = libc::O_DIRECTORY;

#[cfg(all(target_os="linux", target_arch ="arm"))]
pub const O_DIRECTORY: c_int = 0o40000;

pub const O_PATH: c_int = 0o10000000;

extern {
    pub fn fdopendir(fd: c_int) -> *mut DIR;
    pub fn readdir(dir: *mut DIR) -> *const dirent;
}
