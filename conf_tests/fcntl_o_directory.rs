extern crate libc;

fn main() {
    unsafe {
        let conf_tests = std::ffi::CString::new("conf_tests").unwrap();
        let fd = libc::open(conf_tests.as_ptr(), libc::O_DIRECTORY | libc::O_RDONLY);

        let flags = libc::fcntl(fd, libc::F_GETFL);

        if flags != -1 && flags & libc::O_DIRECTORY != 0 {
            std::process::exit(0);
        } else {
            std::process::exit(1);
        }
    }
}
