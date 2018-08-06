extern crate tempfile;
extern crate openat;

use std::io::{self, Read, Write};
use std::os::unix::fs::PermissionsExt;
use openat::Dir;
use std::process::Command;

#[test]
#[cfg(target_os="linux")]
fn unnamed_tmp_file_link() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let dir = Dir::open(tmp.path()).expect("open");
    let mut f = dir.new_unnamed_file(0o666).expect("new file");
    // This fixes bug in old glibc
    f.set_permissions(PermissionsExt::from_mode(0o644));
    println!("Filemeta {:?}", f.metadata().expect("meta"));
    f.write(b"hello\n").expect("write");
    dir.link_file_at(&f, "hello.txt").expect("linkat");
    Command::new("ls").arg("-la").arg(tmp.path()).status().expect("ls");
    let mut f = dir.open_file("hello.txt").expect("read");
    let mut buf = String::with_capacity(10);
    f.read_to_string(&mut buf).expect("read data");
    assert_eq!(buf, "hello\n");
}
