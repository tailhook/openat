extern crate tempfile;
extern crate openat;

use std::io::{self, Read, Write};
use openat::Dir;
use std::process::Command;

#[test]
#[cfg(target_os="linux")]
fn unnamed_tmp_file_link() -> Result<(), io::Error> {
    let tmp = tempfile::tempdir().expect("tempdir");
    let dir = Dir::open(tmp.path()).expect("open");
    let mut f = dir.new_unnamed_file(0o777).expect("new file");
    f.write(b"hello\n").expect("write");
    dir.link_file_at(&f, "hello.txt").expect("linkat");
    Command::new("ls").arg("-la").arg(tmp.path()).status().expect("ls");
    let mut f = dir.open_file("hello.txt").expect("read");
    let mut buf = String::with_capacity(10);
    f.read_to_string(&mut buf).expect("read data");
    assert_eq!(buf, "hello\n");
    Ok(())
}
