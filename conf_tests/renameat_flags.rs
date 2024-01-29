extern crate libc;

fn main() {
    let does_renameat2_exist = libc::SYS_renameat2;
}
