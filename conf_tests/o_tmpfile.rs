extern crate libc;

fn main () {
    unsafe {
        let conf_tests = std::ffi::CString::new("conf_tests").unwrap();
        libc::open(conf_tests.as_ptr(),
                   libc::O_TMPFILE | libc::O_RDWR,
                   libc::S_IRUSR | libc::S_IWUSR);
    }
}
