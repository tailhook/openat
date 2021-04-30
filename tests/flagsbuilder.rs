extern crate openat;
use openat::Dir;

#[test]
fn dir_flags_builder_basic() {
    let dir = Dir::flags()
        .without(libc::O_CLOEXEC)
        .with(libc::O_NOFOLLOW)
        .open("src");

    assert!(dir.is_ok());
}
