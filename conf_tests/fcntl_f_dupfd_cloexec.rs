extern crate libc;

fn main() {
    unsafe {
        let conf_tests = std::ffi::CString::new("conf_tests").unwrap();
        let fd = libc::open(conf_tests.as_ptr(), libc::O_DIRECTORY | libc::O_RDONLY);

        let dup = libc::fcntl(fd, libc::F_DUPFD_CLOEXEC, 0);
    }
}
