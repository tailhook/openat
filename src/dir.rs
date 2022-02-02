use std::ffi::{CStr, CString, OsString, OsStr };
use std::os::unix::ffi::OsStrExt;
use std::io::{self, Error};
use std::mem;
use std::fs::{File, read_link};
use std::os::unix::io::{AsRawFd, RawFd, FromRawFd, IntoRawFd};
use std::os::unix::ffi::{OsStringExt};
use std::path::{Path, PathBuf};

use libc;
use crate::list::{open_dirfd, DirIter, Entry};
use crate::metadata::{self, Metadata};

use crate::{AsPath, DirFlags, DirMethodFlags, SimpleType};

/// Value if the libc::O_DIRECTORY flag when supported by the system, otherwise 0
#[cfg(feature = "o_directory")]
pub const O_DIRECTORY: libc::c_int = libc::O_DIRECTORY;
/// Value if the libc::O_DIRECTORY flag when supported by the system, otherwise 0
#[cfg(not(feature = "o_directory"))]
pub const O_DIRECTORY: libc::c_int = 0;

/// Value if the libc::O_PATH flag when supported by the system, otherwise 0
#[cfg(feature = "o_path")]
pub const O_PATH: libc::c_int = libc::O_PATH;
/// Value if the libc::O_PATH flag when supported by the system, otherwise 0
#[cfg(not(feature = "o_path"))]
pub const O_PATH: libc::c_int = 0;

/// Value if the libc::O_SEARCH flag when supported by the system, otherwise 0
#[cfg(feature = "o_search")]
pub const O_SEARCH: libc::c_int = libc::O_SEARCH;
/// Value if the libc::O_SEARCH flag when supported by the system, otherwise 0
#[cfg(not(feature = "o_search"))]
pub const O_SEARCH: libc::c_int = 0;

/// A safe wrapper around directory file descriptor
///
/// Construct it either with ``Dir::cwd()`` or ``Dir::open(path)``
///
#[derive(Debug)]
pub struct Dir(pub(crate) RawFd);

impl Dir {
    /// Creates a directory descriptor that resolves paths relative to current
    /// working directory (AT_FDCWD)
    #[deprecated(since="0.1.15", note="\
        Use `Dir::open(\".\")` instead. \
        Dir::cwd() doesn't open actual file descriptor and uses magic value \
        instead which resolves to current dir on any syscall invocation. \
        This is usually counter-intuitive and yields a broken \
        file descriptor when using `Dir::as_raw_fd`. \
        Will be removed in version v0.2 of the library.")]
    pub fn cwd() -> Dir {
        Dir(libc::AT_FDCWD)
    }

    /// Create a flags builder for Dir objects.  Initial flags default to `O_CLOEXEC'. More
    /// flags can be set added by 'with()' and existing/default flags can be removed by
    /// 'without()'. The flags builder can the be used to 'open()' to create
    /// a Dir handle.
    #[inline]
    pub fn flags() -> DirFlags {
        DirFlags::new(libc::O_CLOEXEC)
    }

    /// Open a directory descriptor at specified path with O_PATH when available.
    /// A descriptor obtained with this flag is restricted to do only certain operations:
    /// - It may be used as anchor for opening sub-objects
    /// - One can query metadata of this directory
    ///
    /// This handle is not suitable for a 'Dir::list()' call and may yield a runtime error.
    /// Use 'Dir::flags().open()' to get a handle without O_PATH defined or use
    /// 'Dir::list_self()' which clone-upgrades the handle it used for the iteration.
    // TODO(tailhook) maybe accept only absolute paths?
    pub fn open<P: AsPath>(path: P) -> io::Result<Dir> {
        Dir::_open(to_cstr(path)?.as_ref(), O_PATH | libc::O_CLOEXEC)
    }

    pub(crate) fn _open(path: &CStr, flags: libc::c_int) -> io::Result<Dir> {
        let fd = unsafe { libc_ok(libc::open(path.as_ptr(), O_DIRECTORY | flags))? };
        Ok(Dir(fd))
    }

    /// Checks if the fd associated with the Dir object is really a directory.
    /// There are subtle differences in how directories can be opened and what properties the
    /// resulting file handles have. On some platforms it is possible that
    /// Dir::open("somefile") succeeds. This will usually raise errors later when one tries to
    /// do Directory operations on this. While checking if such an handle comes with cost of a
    /// potential expensive 'stat()' operation. This library makes the assumption that in the
    /// 'usual' case Dir objects are only created on directories and operations on Dir handles
    /// handle errors properly. Still in some cases one may check a freshly created handle
    /// explicitly. Thats what 'is_dir()' is for. Returns 'true' when the underlying handles
    /// represents a directory and false otherwise.
    pub fn is_dir(&self) -> bool {
        match fd_type(self.0).unwrap_or(FdType::Other) {
            FdType::NormalDir | FdType::OPathDir => true,
            FdType::Other => false,
        }
    }

    /// List subdirectory of this dir
    ///
    /// You can list directory itself with `list_self`.
    pub fn list_dir<P: AsPath>(&self, path: P) -> io::Result<DirIter> {
        self.with(O_SEARCH).sub_dir(path)?.list()
    }

    /// List this dir
    pub fn list_self(&self) -> io::Result<DirIter> {
        self.with(O_SEARCH).clone_upgrade()?.list()
    }

    /// Create a DirIter from a Dir
    /// Dir must not be a handle opened with O_PATH.
    pub fn list(self) -> io::Result<DirIter> {
        let fd = self.0;
        std::mem::forget(self);
        open_dirfd(fd)
    }

    /// Create a flags builder for member methods. Defaults to `O_CLOEXEC | O_NOFOLLOW` plus
    /// the given flags. Further flags can be added/removed by the 'with()'/'without()'
    /// members. And finally be used by 'sub_dir()' and the different 'open()' calls.
    #[inline]
    pub fn with<'a>(&'a self, flags: libc::c_int) -> DirMethodFlags<'a> {
        DirMethodFlags::new(self, libc::O_CLOEXEC | libc::O_NOFOLLOW | flags)
    }

    /// Create a flags builder for member methods. Defaults to `O_CLOEXEC | O_NOFOLLOW` with
    /// the given flags (which may O_CLOEXEC or O_NOFOLLOW) removed. Further flags can be
    /// added/removed by the 'with()'/'without()' members. And finally be used by 'sub_dir()'
    /// and the different 'open()' calls.
    #[inline]
    pub fn without<'a>(&'a self, flags: libc::c_int) -> DirMethodFlags<'a> {
        DirMethodFlags::new(self, (libc::O_CLOEXEC | libc::O_NOFOLLOW) & !flags)
    }

    /// Open subdirectory with O_PATH when available.
    /// A descriptor obtained with this flag is restricted to do only certain operations:
    /// - It may be used as anchor for opening sub-objects
    /// - One can query metadata of this directory
    ///
    /// This handle is not suitable for a 'Dir::list()' call and may yield a runtime error.
    /// Use 'Dir::with(0).sub_dir()' to get a handle without O_PATH defined or use
    /// 'Dir::list_dir()' which clone-upgrades the handle it used for the iteration.
    ///
    /// Note that this method does not resolve symlinks by default, so you may have to call
    /// [`read_link`] to resolve the real path first or create a handle
    /// 'without(libc::O_NOFOLLOW)'.
    ///
    /// [`read_link`]: #method.read_link
    pub fn sub_dir<P: AsPath>(&self, path: P) -> io::Result<Dir> {
        self._sub_dir(
            to_cstr(path)?.as_ref(),
            O_PATH | libc::O_CLOEXEC | libc::O_NOFOLLOW,
        )
    }

    pub(crate) fn _sub_dir(&self, path: &CStr, flags: libc::c_int) -> io::Result<Dir> {
        Ok(Dir(unsafe {
            libc_ok(libc::openat(self.0, path.as_ptr(), flags | O_DIRECTORY))?
        }))
    }

    /// Read link in this directory
    pub fn read_link<P: AsPath>(&self, path: P) -> io::Result<PathBuf> {
        self._read_link(to_cstr(path)?.as_ref())
    }

    fn _read_link(&self, path: &CStr) -> io::Result<PathBuf> {
        let mut buf = vec![0u8; 4096];
        let res = unsafe {
            libc::readlinkat(self.0,
                        path.as_ptr(),
                        buf.as_mut_ptr() as *mut libc::c_char, buf.len())
        };
        if res < 0 {
            Err(io::Error::last_os_error())
        } else {
            buf.truncate(res as usize);
            Ok(OsString::from_vec(buf).into())
        }
    }

    /// Open file for reading in this directory
    ///
    /// Note that this method does not resolve symlinks by default, so you may have to call
    /// [`read_link`] to resolve the real path first.
    ///
    /// [`read_link`]: #method.read_link
    pub fn open_file<P: AsPath>(&self, path: P) -> io::Result<File> {
        self._open_file(to_cstr(path)?.as_ref(),
            libc::O_RDONLY, 0)
    }

    /// Open file for writing, create if necessary, truncate on open
    ///
    /// If there exists a symlink at the destination path, this method will fail. In that case, you
    /// will need to remove the symlink before calling this method. If you are on Linux, you can
    /// alternatively create an unnamed file with [`new_unnamed_file`] and then rename it,
    /// clobbering the symlink at the destination.
    ///
    /// [`new_unnamed_file`]: #method.new_unnamed_file
    pub fn write_file<P: AsPath>(&self, path: P, mode: libc::mode_t)
        -> io::Result<File>
    {
        self._open_file(to_cstr(path)?.as_ref(),
            libc::O_CREAT|libc::O_WRONLY|libc::O_TRUNC,
            mode)
    }

    /// Open file for append, create if necessary
    ///
    /// If there exists a symlink at the destination path, this method will fail. In that case, you
    /// will need to call [`read_link`] to resolve the real path first.
    ///
    /// [`read_link`]: #method.read_link
    pub fn append_file<P: AsPath>(&self, path: P, mode: libc::mode_t)
        -> io::Result<File>
    {
        self._open_file(to_cstr(path)?.as_ref(),
            libc::O_CREAT|libc::O_WRONLY|libc::O_APPEND,
            mode)
    }

    /// Create file for writing (and truncate) in this directory
    ///
    /// Deprecated alias for `write_file`
    ///
    /// If there exists a symlink at the destination path, this method will fail. In that case, you
    /// will need to remove the symlink before calling this method. If you are on Linux, you can
    /// alternatively create an unnamed file with [`new_unnamed_file`] and then rename it,
    /// clobbering the symlink at the destination.
    ///
    /// [`new_unnamed_file`]: #method.new_unnamed_file
    #[deprecated(since="0.1.7", note="please use `write_file` instead")]
    pub fn create_file<P: AsPath>(&self, path: P, mode: libc::mode_t)
        -> io::Result<File>
    {
        self._open_file(to_cstr(path)?.as_ref(),
            libc::O_CREAT|libc::O_WRONLY|libc::O_TRUNC,
            mode)
    }

    /// Create a tmpfile in this directory which isn't linked to any filename
    ///
    /// This works by passing `O_TMPFILE` into the openat call. The flag is
    /// supported only on linux. So this function always returns error on
    /// such systems.
    ///
    /// **WARNING!** On glibc < 2.22 file permissions of the newly created file
    /// may be arbitrary. Consider chowning after creating a file.
    ///
    /// Note: It may be unclear why creating unnamed file requires a dir. There
    /// are two reasons:
    ///
    /// 1. It's created (and occupies space) on a real filesystem, so the
    ///    directory is a way to find out which filesystem to attach file to
    /// 2. This method is mostly needed to initialize the file then link it
    ///    using ``link_file_at`` to the real directory entry. When linking
    ///    it must be linked into the same filesystem. But because for most
    ///    programs finding out filesystem layout is an overkill the rule of
    ///    thumb is to create a file in the the target directory.
    ///
    /// Currently, we recommend to fallback on any error if this operation
    /// can't be accomplished rather than relying on specific error codes,
    /// because semantics of errors are very ugly.
    #[cfg(feature = "o_tmpfile")]
    pub fn new_unnamed_file(&self, mode: libc::mode_t)
        -> io::Result<File>
    {
        self._open_file(unsafe { CStr::from_bytes_with_nul_unchecked(b".\0") },
            libc::O_TMPFILE|libc::O_WRONLY,
            mode)
    }

    /// Create a tmpfile in this directory which isn't linked to any filename
    ///
    /// This works by passing `O_TMPFILE` into the openat call. The flag is
    /// supported only on linux. So this function always returns error on
    /// such systems.
    ///
    /// Note: It may be unclear why creating unnamed file requires a dir. There
    /// are two reasons:
    ///
    /// 1. It's created (and occupies space) on a real filesystem, so the
    ///    directory is a way to find out which filesystem to attach file to
    /// 2. This method is mostly needed to initialize the file then link it
    ///    using ``link_file_at`` to the real directory entry. When linking
    ///    it must be linked into the same filesystem. But because for most
    ///    programs finding out filesystem layout is an overkill the rule of
    ///    thumb is to create a file in the the target directory.
    ///
    /// Currently, we recommend to fallback on any error if this operation
    /// can't be accomplished rather than relying on specific error codes,
    /// because semantics of errors are very ugly.
    #[cfg(not(feature = "o_tmpfile"))]
    pub fn new_unnamed_file<P: AsPath>(&self, _mode: libc::mode_t)
        -> io::Result<File>
    {
        Err(io::Error::new(io::ErrorKind::Other,
            "creating unnamed tmpfiles is only supported on linux"))
    }

    /// Link open file to a specified path
    ///
    /// This is used with ``new_unnamed_file()`` to create and initialize the
    /// file before linking it into a filesystem. This requires `/proc` to be
    /// mounted and works **only on linux**.
    ///
    /// On systems other than linux this always returns error. It's expected
    /// that in most cases this methos is not called if ``new_unnamed_file``
    /// fails. But in obscure scenarios where `/proc` is not mounted this
    /// method may fail even on linux. So your code should be able to fallback
    /// to a named file if this method fails too.
    #[cfg(feature = "link_file_at")]
    pub fn link_file_at<F: AsRawFd, P: AsPath>(&self, file: &F, path: P)
        -> io::Result<()>
    {
        let fd_path = format!("/proc/self/fd/{}", file.as_raw_fd());
        _hardlink(&Dir(libc::AT_FDCWD), to_cstr(fd_path)?.as_ref(),
            &self, to_cstr(path)?.as_ref(),
            libc::AT_SYMLINK_FOLLOW)
    }

    /// Link open file to a specified path
    ///
    /// This is used with ``new_unnamed_file()`` to create and initialize the
    /// file before linking it into a filesystem. This requires `/proc` to be
    /// mounted and works **only on linux**.
    ///
    /// On systems other than linux this always returns error. It's expected
    /// that in most cases this methos is not called if ``new_unnamed_file``
    /// fails. But in obscure scenarios where `/proc` is not mounted this
    /// method may fail even on linux. So your code should be able to fallback
    /// to a named file if this method fails too.
    //NOTE(cehteh): would it make sense to remove this function (for non linux), this will
    // generate a compile time error rather than a runtime error, which most likely is
    // favorable since the semantic cant easily emulated.
    #[cfg(not(feature = "link_file_at"))]
    pub fn link_file_at<F: AsRawFd, P: AsPath>(&self, _file: F, _path: P)
        -> io::Result<()>
    {
        Err(io::Error::new(io::ErrorKind::Other,
            "linking unnamed fd to directories is only supported on linux"))
    }

    /// Create file if not exists, fail if exists
    ///
    /// This function checks existence and creates file atomically with
    /// respect to other threads and processes.
    ///
    /// Technically it means passing `O_EXCL` flag to open.
    pub fn new_file<P: AsPath>(&self, path: P, mode: libc::mode_t)
        -> io::Result<File>
    {
        self._open_file(to_cstr(path)?.as_ref(),
            libc::O_CREAT|libc::O_EXCL|libc::O_WRONLY,
            mode)
    }

    /// Open file for reading and writing without truncation, create if needed
    ///
    /// If there exists a symlink at the destination path, this method will fail. In that case, you
    /// will need to call [`read_link`] to resolve the real path first.
    ///
    /// [`read_link`]: #method.read_link
    pub fn update_file<P: AsPath>(&self, path: P, mode: libc::mode_t)
        -> io::Result<File>
    {
        self._open_file(to_cstr(path)?.as_ref(),
            libc::O_CREAT|libc::O_RDWR,
            mode)
    }

    pub(crate) fn _open_file(&self, path: &CStr, flags: libc::c_int, mode: libc::mode_t)
        -> io::Result<File>
    {
        unsafe {
            // Note: In below call to `openat`, *mode* must be cast to
            // `unsigned` because the optional `mode` argument to `openat` is
            // variadic in the signature. Since integers are not implicitly
            // promoted as they are in C this would break on Freebsd where
            // *mode_t* is an alias for `uint16_t`.
            let res = libc_ok(
                libc::openat(self.0, path.as_ptr(),
                             flags|libc::O_CLOEXEC|libc::O_NOFOLLOW,
                             mode as libc::c_uint)
            )?;
            Ok(File::from_raw_fd(res))
        }
    }

    /// Make a symlink in this directory
    ///
    /// Note: the order of arguments differ from `symlinkat`
    pub fn symlink<P: AsPath, R: AsPath>(&self, path: P, value: R)
        -> io::Result<()>
    {
        self._symlink(to_cstr(path)?.as_ref(), to_cstr(value)?.as_ref())
    }
    fn _symlink(&self, path: &CStr, link: &CStr) -> io::Result<()> {
        unsafe {
            let res = libc::symlinkat(link.as_ptr(),
                self.0, path.as_ptr());
            if res < 0 {
                Err(io::Error::last_os_error())
            } else {
                Ok(())
            }
        }
    }

    /// Create a subdirectory in this directory
    pub fn create_dir<P: AsPath>(&self, path: P, mode: libc::mode_t)
        -> io::Result<()>
    {
        self._create_dir(to_cstr(path)?.as_ref(), mode)
    }
    fn _create_dir(&self, path: &CStr, mode: libc::mode_t) -> io::Result<()> {
        unsafe {
            libc_ok(libc::mkdirat(self.0, path.as_ptr(), mode))?;
        }
        Ok(())
    }

    /// Rename a file in this directory to another name (keeping same dir)
    pub fn local_rename<P: AsPath, R: AsPath>(&self, old: P, new: R)
        -> io::Result<()>
    {
        rename(self, to_cstr(old)?.as_ref(), self, to_cstr(new)?.as_ref())
    }

    /// Similar to `local_rename` but atomically swaps both paths
    ///
    /// Only supported on Linux.
    #[cfg(feature = "rename_exchange")]
    pub fn local_exchange<P: AsPath, R: AsPath>(&self, old: P, new: R)
        -> io::Result<()>
    {
        // Workaround https://github.com/tailhook/openat/issues/35
        // AKA https://github.com/rust-lang/libc/pull/2116
        // Unfortunately since we made this libc::c_int in our
        // public API, we can't easily change it right now.
        let flags = libc::RENAME_EXCHANGE as libc::c_int;
        rename_flags(self, to_cstr(old)?.as_ref(),
            self, to_cstr(new)?.as_ref(),
            flags)
    }

    /// Remove a subdirectory in this directory
    ///
    /// Note only empty directory may be removed
    pub fn remove_dir<P: AsPath>(&self, path: P)
        -> io::Result<()>
    {
        self._unlink(to_cstr(path)?.as_ref(), libc::AT_REMOVEDIR)
    }
    /// Remove a file in this directory
    pub fn remove_file<P: AsPath>(&self, path: P)
        -> io::Result<()>
    {
        self._unlink(to_cstr(path)?.as_ref(), 0)
    }
    fn _unlink(&self, path: &CStr, flags: libc::c_int) -> io::Result<()> {
        unsafe {
            libc_ok(libc::unlinkat(self.0, path.as_ptr(), flags))?;
        }
        Ok(())
    }

    /// Removes a directory with all its contents
    pub fn remove_recursive<P: AsPath + Copy>(&self, path: P) -> io::Result<()> {
        self.list_dir(path)?.try_for_each(|entry| -> io::Result<()> {
            match entry {
                Ok(entry) if entry.simple_type() == Some(SimpleType::Dir) => {
                    self.sub_dir(path)?
                        .remove_recursive(entry.file_name())
                }
                Ok(entry) => {
                    self.remove_file(entry.file_name())
                }
                Err(err) =>  Err(err)
            }
        })?;
        self.remove_dir(path)
    }

    /// Removes a directory with all its contents in a atomic way.  This is done by renaming
    /// the 'path' to some unique name first.  When tmp_dir is given as sub direcory of 'self'
    /// on the same filesystem.  the 'path' will be moved into that. When tmp_dir is "" then
    /// the rename is done in place.  The unique name is determined by changing the path
    /// extension to an increasing number (.0 .1 .2 ... .65534) until the rename
    /// succeeds. When renaming will not succeed this will eventually return an error.
    pub fn remove_recursive_atomic<P, T>(&self, path: P, tmp_dir: T)
                                      -> io::Result<()>
    where
        P: AsPath + Copy + std::convert::AsRef<std::path::Path>,
        T: AsPath + std::convert::AsRef<std::path::Path>
    {
        let mut to_delete = PathBuf::from(tmp_dir.as_ref());
        if to_delete.as_os_str().is_empty() {
            // In place, full name
            to_delete.push(path);
        } else {
            // use the tmp_dir and the file_name part
            to_delete.push(Path::new(path.as_ref()).file_name().unwrap());
        }

        for i in 0u16..u16::MAX {
            to_delete.set_extension(i.to_string());
            if self.local_rename(path, &to_delete).is_ok() {
                return self.remove_recursive(&to_delete);
            }
        }
        Err(io::Error::new(io::ErrorKind::Other, "Could not rename for remove"))
    }


    /// Get the path of this directory (if possible)
    ///
    /// This uses symlinks in `/proc/self`, they sometimes may not be
    /// available so use with care.
    #[cfg(feature = "proc_self_fd")]
    pub fn recover_path(&self) -> io::Result<PathBuf> {
        let fd = self.0;
        if fd != libc::AT_FDCWD {
            read_link(format!("/proc/self/fd/{}", fd))
        } else {
            read_link("/proc/self/cwd")
        }
    }

    /// Returns metadata of an entry in this directory
    ///
    /// If the destination path is a symlink, this will return the metadata of the symlink itself.
    /// If you would like to follow the symlink and return the metadata of the target, you will
    /// have to call [`read_link`] to resolve the real path first.
    ///
    /// [`read_link`]: #method.read_link
    pub fn metadata<P: AsPath>(&self, path: P) -> io::Result<Metadata> {
        self._stat(to_cstr(path)?.as_ref(), libc::AT_SYMLINK_NOFOLLOW)
    }
    fn _stat(&self, path: &CStr, flags: libc::c_int) -> io::Result<Metadata> {
        unsafe {
            let mut stat = mem::zeroed(); // TODO(cehteh): uninit
            libc_ok(libc::fstatat(self.0, path.as_ptr(), &mut stat, flags))?;
            Ok(metadata::new(stat))
        }
    }

    /// Returns the metadata of the directory itself.
    pub fn self_metadata(&self) -> io::Result<Metadata> {
        unsafe {
            let mut stat = mem::zeroed(); // TODO(cehteh): uninit
            libc_ok(libc::fstat(self.0, &mut stat))?;
            Ok(metadata::new(stat))
        }
    }

    /// Constructs a new `Dir` from a given raw file descriptor,
    /// ensuring it is a directory file descriptor first.
    ///
    /// This function **consumes ownership** of the specified file
    /// descriptor. The returned `Dir` will take responsibility for
    /// closing it when it goes out of scope.
    pub unsafe fn from_raw_fd_checked(fd: RawFd) -> io::Result<Self> {
        match fd_type(fd)? {
            FdType::NormalDir | FdType::OPathDir => Ok(Dir(fd)),
            FdType::Other => Err(io::Error::from_raw_os_error(libc::ENOTDIR)),
        }
    }

    /// Creates a new independently owned handle to the underlying directory.
    /// The new handle has the same (Normal/O_PATH) semantics as the original handle.
    pub fn try_clone(&self) -> io::Result<Self> {
        Ok(Dir(clone_dirfd(self.0)?))
    }

    /// Creates a new 'Normal' independently owned handle to the underlying directory.
    pub fn clone_upgrade(&self) -> io::Result<Self> {
        Ok(Dir(clone_dirfd_upgrade(self.0, 0)?))
    }

    /// Creates a new 'O_PATH' restricted independently owned handle to the underlying directory.
    pub fn clone_downgrade(&self) -> io::Result<Self> {
        Ok(Dir(clone_dirfd_downgrade(self.0)?))
    }
}

const CURRENT_DIRECTORY: [libc::c_char; 2] = [b'.' as libc::c_char, 0];

//TODO(cehteh): eventually the clone calls should replicate O_SEARCH, maybe other file flags
fn clone_dirfd(fd: libc::c_int) -> io::Result<libc::c_int> {
    unsafe {
        match fd_type(fd)? {
            FdType::NormalDir => libc_ok(libc::openat(
                fd,
                &CURRENT_DIRECTORY as *const libc::c_char,
                O_DIRECTORY | libc::O_CLOEXEC,
            )),
            #[cfg(feature = "o_path")]
            FdType::OPathDir => libc_ok(libc::dup(fd)),
            _ => Err(io::Error::from_raw_os_error(libc::ENOTDIR)),
        }
    }
}

pub(crate) fn clone_dirfd_upgrade(fd: libc::c_int, flags: libc::c_int) -> io::Result<libc::c_int> {
    unsafe {
        match fd_type(fd)? {
            FdType::NormalDir | FdType::OPathDir => libc_ok(libc::openat(
                fd,
                &CURRENT_DIRECTORY as *const libc::c_char,
                flags | O_DIRECTORY | libc::O_CLOEXEC,
            )),
            _ => Err(io::Error::from_raw_os_error(libc::ENOTDIR)),
        }
    }
}

fn clone_dirfd_downgrade(fd: libc::c_int) -> io::Result<libc::c_int> {
    unsafe {
        match fd_type(fd)? {
            #[cfg(feature = "o_path")]
            FdType::NormalDir => libc_ok(libc::openat(
                fd,
                &CURRENT_DIRECTORY as *const libc::c_char,
                libc::O_PATH | O_DIRECTORY | libc::O_CLOEXEC,
            )),
            #[cfg(not(feature = "o_path"))]
            FdType::NormalDir => libc_ok(libc::openat(
                fd,
                &CURRENT_DIRECTORY as *const libc::c_char,
                O_DIRECTORY | libc::O_CLOEXEC,
            )),
            #[cfg(feature = "o_path")]
            FdType::OPathDir => libc_ok(libc::dup(fd)),
            _ => Err(io::Error::from_raw_os_error(libc::ENOTDIR)),
        }
    }
}

enum FdType {
    NormalDir,
    OPathDir,
    Other,
}

// OSes with fcntl() that returns O_DIRECTORY
// Linux has O_PATH
#[cfg(all(feature = "o_path", feature = "fcntl_o_directory"))]
fn fd_type(fd: libc::c_int) -> io::Result<FdType> {
    let flags = unsafe { libc_ok(libc::fcntl(fd, libc::F_GETFL))? };
    if flags & libc::O_DIRECTORY != 0 {
        if flags & libc::O_PATH != 0 {
            Ok(FdType::OPathDir)
        } else {
            Ok(FdType::NormalDir)
        }
    } else {
        Ok(FdType::Other)
    }
}

#[cfg(all(not(feature = "o_path"), feature = "fcntl_o_directory"))]
fn fd_type(fd: libc::c_int) -> io::Result<FdType> {
    let flags = unsafe { libc_ok(libc::fcntl(fd, libc::F_GETFL))? };
    if flags & libc::O_DIRECTORY != 0 {
        Ok(FdType::NormalDir)
    } else {
        Ok(FdType::Other)
    }
}

// OSes where fcntl won't return O_DIRECTORY use stat()
#[cfg(not(feature = "fcntl_o_directory"))]
fn fd_type(fd: libc::c_int) -> io::Result<FdType> {
    unsafe {
        let mut stat = mem::zeroed(); // TODO(cehteh): uninit
        libc_ok(libc::fstat(fd, &mut stat))?;
        match stat.st_mode & libc::S_IFMT {
            libc::S_IFDIR => Ok(FdType::NormalDir),
            _ => Ok(FdType::Other),
        }
    }
}

#[inline]
pub(crate) fn libc_ok(ret: libc::c_int) -> io::Result<libc::c_int> {
    if ret != -1 {
        Ok(ret)
    } else {
        Err(io::Error::last_os_error())
    }
}

/// Rename (move) a file between directories
///
/// Files must be on a single filesystem anyway. This funtion does **not**
/// fallback to copying if needed.
pub fn rename<P, R>(old_dir: &Dir, old: P, new_dir: &Dir, new: R)
    -> io::Result<()>
    where P: AsPath, R: AsPath,
{
    _rename(old_dir, to_cstr(old)?.as_ref(), new_dir, to_cstr(new)?.as_ref())
}

fn _rename(old_dir: &Dir, old: &CStr, new_dir: &Dir, new: &CStr) -> io::Result<()> {
    unsafe {
        libc_ok(libc::renameat(old_dir.0, old.as_ptr(), new_dir.0, new.as_ptr()))?;
    }
    Ok(())
}

/// Create a hardlink to a file
///
/// Files must be on a single filesystem even if they are in different
/// directories.
///
/// Note: by default ``linkat`` syscall doesn't resolve symbolic links, and
/// it's also behavior of this function. It's recommended to resolve symlinks
/// manually if needed.
pub fn hardlink<P, R>(old_dir: &Dir, old: P, new_dir: &Dir, new: R)
    -> io::Result<()>
    where P: AsPath, R: AsPath,
{
    _hardlink(old_dir, to_cstr(old)?.as_ref(),
              new_dir, to_cstr(new)?.as_ref(),
              0)
}

fn _hardlink(
    old_dir: &Dir,
    old: &CStr,
    new_dir: &Dir,
    new: &CStr,
    flags: libc::c_int,
) -> io::Result<()> {
    unsafe {
        libc_ok(libc::linkat(old_dir.0, old.as_ptr(), new_dir.0, new.as_ptr(), flags))?;
    }
    Ok(())
}

/// Rename (move) a file between directories with flags
///
/// Files must be on a single filesystem anyway. This funtion does **not**
/// fallback to copying if needed.
///
/// Only supported on Linux.
#[cfg(feature = "renameat_flags")]
pub fn rename_flags<P, R>(old_dir: &Dir, old: P, new_dir: &Dir, new: R,
    flags: libc::c_int)
    -> io::Result<()>
    where P: AsPath, R: AsPath,
{
    _rename_flags(old_dir, to_cstr(old)?.as_ref(),
        new_dir, to_cstr(new)?.as_ref(),
        flags)
}

#[cfg(feature = "renameat_flags")]
fn _rename_flags(old_dir: &Dir, old: &CStr, new_dir: &Dir, new: &CStr,
    flags: libc::c_int)
    -> io::Result<()>
{
    unsafe {
        let res = libc::syscall(
            libc::SYS_renameat2,
            old_dir.0, old.as_ptr(),
            new_dir.0, new.as_ptr(), flags);
        if res < 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(())
        }
    }
}

impl AsRawFd for Dir {
    #[inline]
    fn as_raw_fd(&self) -> RawFd {
        self.0
    }
}

impl FromRawFd for Dir {
    /// The user must guarantee that the passed in `RawFd` is in fact
    /// a directory file descriptor.
    #[inline]
    unsafe fn from_raw_fd(fd: RawFd) -> Dir {
        Dir(fd)
    }
}

impl IntoRawFd for Dir {
    #[inline]
    fn into_raw_fd(self) -> RawFd {
        let result = self.0;
        mem::forget(self);
        return result;
    }
}

impl Drop for Dir {
    fn drop(&mut self) {
        let fd = self.0;
        if fd != libc::AT_FDCWD {
            unsafe {
                libc::close(fd);
            }
        }
    }
}

pub(crate) fn to_cstr<P: AsPath>(path: P) -> io::Result<P::Buffer> {
    path.to_path()
    .ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidInput,
                       "nul byte in file name")
    })
}

#[cfg(test)]
mod test {
    use std::io::{Read};
    use std::path::Path;
    use std::os::unix::io::{FromRawFd, IntoRawFd};
    use crate::{Dir};

    #[test]
    fn test_open_ok() {
        assert!(Dir::open("src").is_ok());
    }

    #[test]
    fn test_read_file() {
        let dir = Dir::open("src").unwrap();
        let mut buf = String::new();
        dir.open_file("lib.rs").unwrap()
            .read_to_string(&mut buf).unwrap();
        assert!(buf.find("extern crate libc;").is_some());
    }

    #[test]
    fn test_from_into() {
        let dir = Dir::open("src").unwrap();
        let dir = unsafe { Dir::from_raw_fd(dir.into_raw_fd()) };
        let mut buf = String::new();
        dir.open_file("lib.rs").unwrap()
            .read_to_string(&mut buf).unwrap();
        assert!(buf.find("extern crate libc;").is_some());
    }

    #[test]
    #[should_panic(expected="No such file or directory")]
    fn test_open_no_dir() {
        Dir::open("src/some-non-existent-file").unwrap();
    }

    #[test]
    fn test_list() {
        let dir = Dir::flags().open("src").unwrap();
        let me = dir.list().unwrap();
        assert!(me
            .collect::<Result<Vec<_>, _>>()
            .unwrap()
            .iter()
            .find(|x| { x.file_name() == Path::new("lib.rs").as_os_str() })
            .is_some());
    }

    #[test]
    #[cfg(feature = "o_path")]
    #[should_panic(expected = "Bad file descriptor")]
    fn test_list_opath_fail() {
        let dir = Dir::open("src").unwrap();
        let me = dir.list().unwrap();
        assert!(me
            .collect::<Result<Vec<_>, _>>()
            .unwrap()
            .iter()
            .find(|x| { x.file_name() == Path::new("lib.rs").as_os_str() })
            .is_some());
    }

    #[test]
    fn test_list_self() {
        let dir = Dir::open("src").unwrap();
        let me = dir.list_self().unwrap();
        assert!(me
            .collect::<Result<Vec<_>, _>>()
            .unwrap()
            .iter()
            .find(|x| { x.file_name() == Path::new("lib.rs").as_os_str() })
            .is_some());
    }

    #[test]
    fn test_list_dot() {
        let dir = Dir::open("src").unwrap();
        let me = dir.list_dir(".").unwrap();
        assert!(me
            .collect::<Result<Vec<_>, _>>()
            .unwrap()
            .iter()
            .find(|x| { x.file_name() == Path::new("lib.rs").as_os_str() })
            .is_some());
    }

    #[test]
    fn test_list_dir() {
        let dir = Dir::open(".").unwrap();
        let me = dir.list_dir("src").unwrap();
        assert!(me
            .collect::<Result<Vec<_>, _>>()
            .unwrap()
            .iter()
            .find(|x| { x.file_name() == Path::new("lib.rs").as_os_str() })
            .is_some());
    }

    #[test]
    fn test_from_raw_fd_checked() {
        let fd = Dir::open(".").unwrap().into_raw_fd();
        let dir = unsafe { Dir::from_raw_fd_checked(fd) }.unwrap();
        let filefd = dir.open_file("src/lib.rs").unwrap().into_raw_fd();
        match unsafe { Dir::from_raw_fd_checked(filefd) } {
            Ok(_) => assert!(false, "from_raw_fd_checked succeeded on a non-directory fd!"),
            Err(e) => assert_eq!(e.raw_os_error().unwrap(), libc::ENOTDIR)
        }
    }

    #[test]
    fn test_try_clone() {
        let d = Dir::open(".").unwrap();
        let d2 = d.try_clone().unwrap();
        drop(d);
        let _file = d2.open_file("src/lib.rs").unwrap();
    }

    #[test]
    fn test_remove_recursive() {
        let d = Dir::open(".").unwrap();

        d.create_dir("test_remove", 0o777).unwrap();
        d.create_dir("test_remove/foo", 0o777).unwrap();
        d.create_dir("test_remove/foo/bar", 0o777).unwrap();
        d.create_dir("test_remove/bar", 0o777).unwrap();

        d.remove_recursive("test_remove").unwrap();
    }

    #[test]
    fn test_remove_recursive_atomic_notmp() {
        let d = Dir::open(".").unwrap();

        d.create_dir("test_removeatomic", 0o777).unwrap();
        d.create_dir("test_removeatomic/foo", 0o777).unwrap();
        d.create_dir("test_removeatomic/foo/bar", 0o777).unwrap();
        d.create_dir("test_removeatomic/bar", 0o777).unwrap();

        d.remove_recursive_atomic("test_removeatomic/foo", "").unwrap();
        d.remove_recursive_atomic("test_removeatomic", "").unwrap();
    }

    #[test]
    fn test_remove_recursive_atomic_tmp() {
        let d = Dir::open(".").unwrap();

        d.create_dir("test_removetmp", 0o777).unwrap();
        d.create_dir("test_removeatomictmp", 0o777).unwrap();
        d.create_dir("test_removeatomictmp/foo", 0o777).unwrap();
        d.create_dir("test_removeatomictmp/foo/bar", 0o777).unwrap();
        d.create_dir("test_removeatomictmp/bar", 0o777).unwrap();

        d.remove_recursive_atomic("test_removeatomictmp/foo", "test_removetmp").unwrap();
        d.remove_recursive_atomic("test_removeatomictmp", "test_removetmp").unwrap();
        d.remove_dir("test_removetmp").unwrap();
    }
}
