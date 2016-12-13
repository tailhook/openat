use libc::{c_int, DIR, dirent};


pub const O_PATH: c_int = 0o10000000;

extern {
    pub fn fdopendir(fd: c_int) -> *mut DIR;
    pub fn readdir(dir: *mut DIR) -> *const dirent;
}
