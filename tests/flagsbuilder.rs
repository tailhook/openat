use openat_ct as openat;
use openat::{Dir};

#[test]
fn dir_flags_builder_basic() {
    let dir = Dir::flags()
        .without(libc::O_CLOEXEC)
        .with(libc::O_NOFOLLOW)
        .open("src");

    assert!(dir.is_ok());
}

#[test]
fn dir_flags_builder_reuse() {
    let dir_flags = Dir::flags().without(libc::O_CLOEXEC).with(libc::O_NOFOLLOW);

    let src_dir = dir_flags.open("src");
    let tests_dir = dir_flags.open("tests");

    assert!(src_dir.is_ok());
    assert!(tests_dir.is_ok());
}

#[test]
fn method_flags_builder_basic() {
    let dir = Dir::open("src").unwrap();
    let file = dir.without(libc::O_NOFOLLOW).open_file("dir.rs");
    assert!(file.is_ok());
}

#[test]
fn method_flags_builder_reuse() {
    let dir = Dir::open("src").unwrap();
    let dir_flags = dir.without(libc::O_NOFOLLOW);

    let file1 = dir_flags.open_file("dir.rs");
    let file2 = dir_flags.open_file("builder.rs");

    assert!(file1.is_ok());
    assert!(file2.is_ok());
}

#[test]
fn method_flags_exported() {
    let dir = Dir::flags()
        .with(openat::O_PATH)
        .open("src");

    assert!(dir.is_ok());
}
