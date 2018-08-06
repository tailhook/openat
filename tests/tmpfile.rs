extern crate tempfile;
extern crate openat;

use std::io::{self, Read, Write};
use openat::Dir;

#[test]
#[cfg(target_os="linux")]
fn unnamed_tmp_file_link() -> Result<(), io::Error> {
    let tmp = tempfile::tempdir()?;
    let dir = Dir::open(tmp.path())?;
    let mut f = dir.new_unnamed_file(0o777)?;
    f.write(b"hello\n")?;
    dir.link_file_at(&f, "hello.txt")?;
    let mut f = dir.open_file("hello.txt")?;
    let mut buf = String::with_capacity(10);
    f.read_to_string(&mut buf)?;
    assert_eq!(buf, "hello\n");
    Ok(())
}
