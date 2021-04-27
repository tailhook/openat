extern crate libc;

fn main () {
    unsafe {
        let conf_tests = std::ffi::CString::new("conf_tests").unwrap();
        libc::open(conf_tests.as_ptr(), libc::O_PATH);
    }
}
