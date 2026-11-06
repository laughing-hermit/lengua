//! Files and folders in the working folder, through cap-std, so every path is resolved beneath the
//! folder with no unsafe code, and with lengua's rules on top, the same on every system:
//!
//! - a name is held to what every system takes ([`crate::names`]), and a new name that differs
//!   from one in its folder only in case or normalization is refused with `EXIST`;
//! - names are byte-exact: a name that a case-insensitive system would take as another's, such
//!   as `README.TXT` for `readme.txt`, isn't found, as on Linux;
//! - no path leads through a link or out of the folder: either is `ACCES`;
//! - a file or folder the program holds open isn't renamed or removed: `BUSY`;
//! - a folder lists sorted by name, turned round in a test's second run, without `.` and `..`;
//! - a write that takes a file past its limit, or the files past their total, is cut short and
//!   the next fails with `FBIG`.

use std::collections::BTreeMap;
use std::io::{Read as _, Seek as _, SeekFrom, Write as _};
use std::path::PathBuf;

use cap_std::fs::{Dir, File, FileType, OpenOptions};

use super::fds::{Descriptor, FOLDER, Table};
use super::limits::Limits;
use super::{Fail, Host, memory};
use crate::wasm::wasi::{SYMLINK_FOLLOW, errno, fdflags, filetype, oflags, rights, size, whence};

/// What a name in the folder is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Kind {
    /// A file.
    File,
    /// A folder.
    Folder,
    /// A link, which no path goes through.
    Link,
    /// Anything else, such as a named pipe.
    Other,
}

impl Kind {
    /// What a folder's entry of `file_type` is.
    fn of(file_type: FileType) -> Self {
        if file_type.is_symlink() {
            Self::Link
        } else if file_type.is_dir() {
            Self::Folder
        } else if file_type.is_file() {
            Self::File
        } else {
            Self::Other
        }
    }

    /// WASI's number for it.
    const fn filetype(self) -> u8 {
        match self {
            Self::File => filetype::REGULAR_FILE,
            Self::Folder => filetype::DIRECTORY,
            Self::Link => filetype::SYMBOLIC_LINK,
            Self::Other => filetype::UNKNOWN,
        }
    }
}

/// One entry of a folder, which sort by name, byte for byte.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Entry {
    /// Its name.
    pub name: String,
    /// What it is.
    pub kind: Kind,
}

/// A folder the program holds open, by its names from the working folder.
#[derive(Debug, Default)]
pub struct Folder {
    /// Its names from the working folder: none for the working folder itself.
    pub path: Vec<String>,
    /// Its entries as `fd_readdir` last began listing them.
    pub listing: Option<Vec<Entry>>,
}

/// A file the program holds open.
#[derive(Debug)]
pub struct Open {
    /// The file.
    pub file: File,
    /// Its names from the working folder.
    pub path: Vec<String>,
    /// Whether the program may read it.
    pub read: bool,
    /// Whether the program may write it.
    pub write: bool,
    /// Whether every write goes at its end.
    pub append: bool,
}

/// The program's working folder.
#[derive(Debug)]
pub struct Files {
    /// The folder, beneath which every path is resolved.
    root: Dir,
    /// The entries of each folder read since the program last changed one, by its path.
    listings: BTreeMap<String, Vec<Entry>>,
    /// Whether folders list turned round, as in a test's second run.
    reversed: bool,
}

/// The path cap-std takes for `names`: `.` for none.
fn path_of(names: &[String]) -> PathBuf {
    if names.is_empty() {
        PathBuf::from(".")
    } else {
        PathBuf::from(names.join("/"))
    }
}

/// WASI's error for what the system said, as far as it's one of the closed set.
#[must_use]
pub fn errno_of(error: &std::io::Error) -> u16 {
    use std::io::ErrorKind;
    match error.kind() {
        ErrorKind::NotFound => errno::NOENT,
        ErrorKind::PermissionDenied => errno::ACCES,
        ErrorKind::AlreadyExists => errno::EXIST,
        ErrorKind::NotADirectory => errno::NOTDIR,
        ErrorKind::IsADirectory => errno::ISDIR,
        ErrorKind::DirectoryNotEmpty => errno::NOTEMPTY,
        ErrorKind::InvalidInput | ErrorKind::InvalidFilename => errno::INVAL,
        ErrorKind::WouldBlock => errno::AGAIN,
        ErrorKind::BrokenPipe => errno::PIPE,
        ErrorKind::ResourceBusy => errno::BUSY,
        ErrorKind::FileTooLarge => errno::FBIG,
        _ => errno::IO,
    }
}

/// The failure of a call the system answered with `error`.
fn failed(error: &std::io::Error) -> Fail {
    Fail::Errno(errno_of(error))
}

/// WASI's error for a rule of [`crate::names`] a name breaks, given by Linux's `number`.
fn broken_errno(number: i64) -> u16 {
    match number {
        crate::names::ENAMETOOLONG => errno::NAMETOOLONG,
        crate::names::EILSEQ => errno::ILSEQ,
        crate::names::EEXIST => errno::EXIST,
        crate::names::EBUSY => errno::BUSY,
        _ => errno::INVAL,
    }
}

/// The names `text`, a path the program gives, leads to from the folder at `base`, with `.`
/// dropped and `..` taken back a name, as no name on the way may be a link.
///
/// # Errors
///
/// A rule of every system's names it breaks, `NOENT` for an empty path, and `ACCES` for one that
/// starts at the root or climbs out of the working folder.
pub fn names_in(base: &[String], text: &[u8]) -> Result<Vec<String>, u16> {
    if text.is_empty() {
        return Err(errno::NOENT);
    }
    if let Some(broken) = crate::names::broken(text) {
        return Err(broken_errno(broken.errno));
    }
    let text = std::str::from_utf8(text).map_err(|_| errno::ILSEQ)?;
    if text.starts_with('/') {
        return Err(errno::ACCES);
    }
    let mut names = base.to_vec();
    for name in text.split('/') {
        match name {
            "" | "." => {}
            ".." => {
                if names.pop().is_none() {
                    return Err(errno::ACCES);
                }
            }
            other => names.push(other.to_owned()),
        }
    }
    Ok(names)
}

/// The entries of the folder at `folder` beneath `root`, sorted by name, without `.` and `..`.
fn read_folder(root: &Dir, folder: &[String]) -> Result<Vec<Entry>, u16> {
    let entries = if folder.is_empty() {
        root.entries()
    } else {
        root.read_dir(path_of(folder))
    }
    .map_err(|error| errno_of(&error))?;
    let mut found = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|error| errno_of(&error))?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if name == "." || name == ".." {
            continue;
        }
        let kind = entry.file_type().map_or(Kind::Other, Kind::of);
        found.push(Entry { name, kind });
    }
    found.sort();
    Ok(found)
}

/// Whether the program holds a file or folder open at `names` or beneath it: the working folder
/// itself, descriptor 3, is never one.
fn busy(fds: &Table, names: &[String]) -> bool {
    fds.iter().any(|(fd, descriptor)| match descriptor {
        Descriptor::File(open) => open.path.starts_with(names),
        Descriptor::Folder(folder) => fd != FOLDER && folder.path.starts_with(names),
        _ => false,
    })
}

impl Files {
    /// The working folder `root`, listing turned round when `reversed`.
    #[must_use]
    pub fn new(root: Dir, reversed: bool) -> Self {
        Self {
            root,
            listings: BTreeMap::new(),
            reversed,
        }
    }

    /// The entries of the folder at `folder`, sorted by name: read once, and kept until the
    /// program changes something.
    fn listing(&mut self, folder: &[String]) -> Result<&Vec<Entry>, u16> {
        use std::collections::btree_map::Entry as Slot;
        match self.listings.entry(folder.join("/")) {
            Slot::Occupied(kept) => Ok(kept.into_mut()),
            Slot::Vacant(empty) => Ok(empty.insert(read_folder(&self.root, folder)?)),
        }
    }

    /// What `names` is, or `None` when its folder has no such entry, byte for byte. Every name
    /// before the last must be a folder.
    ///
    /// # Errors
    ///
    /// `NOENT` for a folder on the way that isn't there, `NOTDIR` for one that's a file, and
    /// `ACCES` for one that's a link.
    pub fn find(&mut self, names: &[String]) -> Result<Option<Kind>, u16> {
        for (depth, name) in names.iter().enumerate() {
            let listing = self.listing(&names[..depth])?;
            let found = listing
                .iter()
                .find(|entry| entry.name == *name)
                .map(|entry| entry.kind);
            if depth + 1 == names.len() {
                return Ok(found);
            }
            match found {
                Some(Kind::Folder) => {}
                Some(Kind::Link) => return Err(errno::ACCES),
                Some(Kind::File | Kind::Other) => return Err(errno::NOTDIR),
                None => return Err(errno::NOENT),
            }
        }
        // No names: the working folder itself.
        Ok(Some(Kind::Folder))
    }

    /// Whether a new name, the last of `names`, differs only in case or normalization from an
    /// entry of its folder.
    fn clashes(&mut self, names: &[String]) -> Result<bool, u16> {
        let Some((leaf, folder)) = names.split_last() else {
            return Ok(false);
        };
        let listing = self.listing(folder)?;
        Ok(listing
            .iter()
            .any(|entry| crate::names::clashes(leaf, &entry.name)))
    }

    /// The program changed the folder: every listing is read again.
    fn changed(&mut self) {
        self.listings.clear();
    }

    /// The folder at `folder`'s entries as `fd_readdir` gives them.
    fn ordered(&mut self, folder: &[String]) -> Result<Vec<Entry>, u16> {
        let reversed = self.reversed;
        let mut entries = self.listing(folder)?.clone();
        if reversed {
            entries.reverse();
        }
        Ok(entries)
    }
}

/// The working folder, which a path call needs, or `NOTCAPABLE`.
fn files(host: &mut Host) -> Result<&mut Files, Fail> {
    host.files.as_mut().ok_or(Fail::Errno(errno::NOTCAPABLE))
}

/// The names of the folder `fd` is, which a path is resolved from.
fn base(fds: &Table, fd: u32) -> Result<Vec<String>, Fail> {
    match fds.get(fd) {
        Some(Descriptor::Folder(folder)) => Ok(folder.path.clone()),
        Some(_) => Err(Fail::Errno(errno::NOTDIR)),
        None => Err(Fail::Errno(errno::BADF)),
    }
}

/// The names a path argument leads to from the folder `fd`.
fn resolve(host: &Host, memory: &[u8], fd: u32, path: u32, len: u32) -> Result<Vec<String>, Fail> {
    let text = memory::bytes(memory, path, len)?;
    let base = base(&host.fds, fd)?;
    names_in(&base, text).map_err(Fail::Errno)
}

/// Whether `fd` is the working folder, descriptor 3, still open.
fn preopened(host: &Host, fd: u32) -> bool {
    fd == FOLDER
        && host.policy.files
        && matches!(host.fds.get(fd), Some(Descriptor::Folder(folder)) if folder.path.is_empty())
}

/// `fd_prestat_get(fd, out)`: the working folder, descriptor 3, is a preopen named `.`; any other
/// descriptor, and every descriptor of a program that names no files, is `BADF`, which ends a
/// toolchain's search for preopens.
///
/// # Errors
///
/// A buffer outside memory.
pub fn fd_prestat_get(host: &Host, memory: &mut [u8], fd: u32, out: u32) -> Result<(), Fail> {
    memory::check(memory, out, size::PRESTAT)?;
    if !preopened(host, fd) {
        return Err(Fail::Errno(errno::BADF));
    }
    memory::zero(memory, out, size::PRESTAT)?;
    memory::write_u32(memory, out + 4, 1)
}

/// `fd_prestat_dir_name(fd, path, len)`: `.`.
///
/// # Errors
///
/// A buffer outside memory.
pub fn fd_prestat_dir_name(
    host: &Host,
    memory: &mut [u8],
    fd: u32,
    path: u32,
    len: u32,
) -> Result<(), Fail> {
    let range = memory::range(memory, path, len)?;
    if !preopened(host, fd) {
        return Err(Fail::Errno(errno::BADF));
    }
    if range.is_empty() {
        return Err(Fail::Errno(errno::NAMETOOLONG));
    }
    memory[range.start] = b'.';
    Ok(())
}

/// What `path_open` was asked for.
#[derive(Debug, Clone, Copy)]
pub struct Opening {
    /// `oflags`: make, a folder, new, empty.
    pub oflags: u16,
    /// The rights asked for, which say whether to read or write.
    pub rights: u64,
    /// `fdflags`: append.
    pub fdflags: u16,
}

/// `path_open(fd, dirflags, path, len, oflags, rights, inheriting, fdflags, out)`.
///
/// # Errors
///
/// A buffer outside memory.
pub fn path_open(
    host: &mut Host,
    memory: &mut [u8],
    fd: u32,
    path: (u32, u32),
    opening: Opening,
    out: u32,
) -> Result<(), Fail> {
    memory::check(memory, out, 4)?;
    let names = resolve(host, memory, fd, path.0, path.1)?;
    let create = opening.oflags & oflags::CREAT != 0;
    let folder = opening.oflags & oflags::DIRECTORY != 0;
    let new = opening.oflags & oflags::EXCL != 0;
    let empty = opening.oflags & oflags::TRUNC != 0;
    let append = opening.fdflags & fdflags::APPEND != 0;
    let write = opening.rights & rights::FD_WRITE != 0 || append || empty;
    let read = opening.rights & rights::FD_READ != 0 || !write;
    let files = files(host)?;
    let found = files.find(&names).map_err(Fail::Errno)?;
    let descriptor = match found {
        Some(Kind::Link | Kind::Other) => return Err(Fail::Errno(errno::ACCES)),
        Some(Kind::Folder) if create && new => return Err(Fail::Errno(errno::EXIST)),
        Some(Kind::Folder) if write => return Err(Fail::Errno(errno::ISDIR)),
        Some(Kind::Folder) => Descriptor::Folder(Folder {
            path: names,
            listing: None,
        }),
        Some(Kind::File) if folder => return Err(Fail::Errno(errno::NOTDIR)),
        Some(Kind::File) if create && new => return Err(Fail::Errno(errno::EXIST)),
        None if folder && create => return Err(Fail::Errno(errno::INVAL)),
        None if !create => return Err(Fail::Errno(errno::NOENT)),
        Some(Kind::File) | None => {
            // Every system makes these the same way only when they're asked for consistently.
            if (empty && append) || (create && !write) {
                return Err(Fail::Errno(errno::INVAL));
            }
            if found.is_none() && files.clashes(&names).map_err(Fail::Errno)? {
                return Err(Fail::Errno(errno::EXIST));
            }
            let mut options = OpenOptions::new();
            options
                .read(read)
                .write(write)
                .append(append)
                .truncate(empty)
                .create(create && !new)
                .create_new(create && new);
            let file = files
                .root
                .open_with(path_of(&names), &options)
                .map_err(|error| failed(&error))?;
            if found.is_none() {
                files.changed();
            }
            Descriptor::File(Box::new(Open {
                file,
                path: names,
                read,
                write,
                append,
            }))
        }
    };
    let opened = host.fds.add(descriptor).map_err(Fail::Errno)?;
    memory::write_u32(memory, out, opened)
}

/// Reads from `open` into `buffer`.
///
/// # Errors
///
/// `BADF` for a file not open to read, and what the system says.
pub fn read(open: &mut Open, buffer: &mut [u8]) -> Result<usize, Fail> {
    if !open.read {
        return Err(Fail::Errno(errno::BADF));
    }
    open.file.read(buffer).map_err(|error| failed(&error))
}

/// Writes `bytes` to `open`, as far as its limits let it.
///
/// # Errors
///
/// `BADF` for a file not open to write, `FBIG` at a limit, and what the system says.
pub fn write(open: &mut Open, limits: &mut Limits, bytes: &[u8]) -> Result<usize, Fail> {
    if !open.write {
        return Err(Fail::Errno(errno::BADF));
    }
    let position = if open.append {
        open.file.metadata().map(|metadata| metadata.len())
    } else {
        open.file.stream_position()
    }
    .map_err(|error| failed(&error))?;
    let room = limits.file_write(position, bytes.len())?;
    let wrote = open
        .file
        .write(&bytes[..room])
        .map_err(|error| failed(&error))?;
    limits.written = limits
        .written
        .saturating_add(u64::try_from(wrote).unwrap_or(u64::MAX));
    Ok(wrote)
}

/// `fd_seek(fd, offset, whence, out)` on a file.
///
/// # Errors
///
/// `INVAL` for a place before the start or a `whence` that isn't one, and what the system says.
pub fn seek(open: &mut Open, offset: i64, from: u8) -> Result<u64, Fail> {
    let place = match from {
        whence::SET => {
            SeekFrom::Start(u64::try_from(offset).map_err(|_| Fail::Errno(errno::INVAL))?)
        }
        whence::CUR => SeekFrom::Current(offset),
        whence::END => SeekFrom::End(offset),
        _ => return Err(Fail::Errno(errno::INVAL)),
    };
    open.file.seek(place).map_err(|error| failed(&error))
}

/// A file's size, for `fd_filestat_get`.
///
/// # Errors
///
/// What the system says.
pub fn length_of(open: &Open) -> Result<u64, Fail> {
    open.file
        .metadata()
        .map(|metadata| metadata.len())
        .map_err(|error| failed(&error))
}

/// Writes a `filestat` of `kind` and `bytes` at `out`: everything else zero.
///
/// # Errors
///
/// A buffer outside memory.
pub fn write_filestat(memory: &mut [u8], out: u32, kind: u8, bytes: u64) -> Result<(), Fail> {
    memory::zero(memory, out, size::FILESTAT)?;
    memory::write_u8(memory, out + 16, kind)?;
    memory::write_u64(memory, out + 32, bytes)
}

/// `path_filestat_get(fd, flags, path, len, out)`: a name's kind and size.
///
/// # Errors
///
/// A buffer outside memory.
pub fn path_filestat_get(
    host: &mut Host,
    memory: &mut [u8],
    fd: u32,
    flags: u32,
    path: (u32, u32),
    out: u32,
) -> Result<(), Fail> {
    memory::check(memory, out, size::FILESTAT)?;
    let names = resolve(host, memory, fd, path.0, path.1)?;
    let files = files(host)?;
    let (kind, bytes) = match files.find(&names).map_err(Fail::Errno)? {
        None => return Err(Fail::Errno(errno::NOENT)),
        Some(Kind::Link) if flags & SYMLINK_FOLLOW != 0 => return Err(Fail::Errno(errno::ACCES)),
        Some(Kind::File) => (
            filetype::REGULAR_FILE,
            files
                .root
                .metadata(path_of(&names))
                .map(|metadata| metadata.len())
                .map_err(|error| failed(&error))?,
        ),
        Some(kind) => (kind.filetype(), 0),
    };
    write_filestat(memory, out, kind, bytes)
}

/// `fd_readdir(fd, buf, len, cookie, used)`: the folder's entries from the `cookie`th on, each a
/// `dirent` and its name, as many as fit, the last cut short when it doesn't.
///
/// # Errors
///
/// A buffer outside memory.
pub fn fd_readdir(
    host: &mut Host,
    memory: &mut [u8],
    fd: u32,
    buffer: (u32, u32),
    cookie: u64,
    used: u32,
) -> Result<(), Fail> {
    memory::check(memory, used, 4)?;
    let range = memory::range(memory, buffer.0, buffer.1)?;
    let Some(files) = host.files.as_mut() else {
        return Err(Fail::Errno(errno::NOTCAPABLE));
    };
    let folder = match host.fds.get_mut(fd) {
        Some(Descriptor::Folder(folder)) => folder,
        Some(_) => return Err(Fail::Errno(errno::NOTDIR)),
        None => return Err(Fail::Errno(errno::BADF)),
    };
    if cookie == 0 || folder.listing.is_none() {
        folder.listing = Some(files.ordered(&folder.path).map_err(Fail::Errno)?);
    }
    host.notes.listed = true;
    let entries = folder.listing.as_deref().unwrap_or_default();
    let start = usize::try_from(cookie).unwrap_or(usize::MAX);
    let mut bytes = Vec::new();
    for (index, entry) in entries.iter().enumerate().skip(start) {
        if bytes.len() >= range.len() {
            break;
        }
        let next = u64::try_from(index + 1).unwrap_or(u64::MAX);
        bytes.extend_from_slice(&next.to_le_bytes());
        // The entry's inode number: its place, never 0, the same on every system.
        bytes.extend_from_slice(&next.to_le_bytes());
        bytes.extend_from_slice(
            &u32::try_from(entry.name.len())
                .unwrap_or(u32::MAX)
                .to_le_bytes(),
        );
        bytes.push(entry.kind.filetype());
        bytes.extend_from_slice(&[0; 3]);
        bytes.extend_from_slice(entry.name.as_bytes());
    }
    let count = bytes.len().min(range.len());
    memory[range.start..range.start + count].copy_from_slice(&bytes[..count]);
    memory::write_u32(memory, used, u32::try_from(count).unwrap_or(u32::MAX))
}

/// `path_create_directory(fd, path, len)`.
///
/// # Errors
///
/// A buffer outside memory.
pub fn path_create_directory(
    host: &mut Host,
    memory: &[u8],
    fd: u32,
    path: (u32, u32),
) -> Result<(), Fail> {
    let names = resolve(host, memory, fd, path.0, path.1)?;
    let files = files(host)?;
    if files.find(&names).map_err(Fail::Errno)?.is_some()
        || files.clashes(&names).map_err(Fail::Errno)?
    {
        return Err(Fail::Errno(errno::EXIST));
    }
    files
        .root
        .create_dir(path_of(&names))
        .map_err(|error| failed(&error))?;
    files.changed();
    Ok(())
}

/// `path_remove_directory(fd, path, len)`: an empty folder the program doesn't hold open.
///
/// # Errors
///
/// A buffer outside memory.
pub fn path_remove_directory(
    host: &mut Host,
    memory: &[u8],
    fd: u32,
    path: (u32, u32),
) -> Result<(), Fail> {
    let names = resolve(host, memory, fd, path.0, path.1)?;
    if names.is_empty() {
        return Err(Fail::Errno(errno::INVAL));
    }
    let held = busy(&host.fds, &names);
    let files = files(host)?;
    match files.find(&names).map_err(Fail::Errno)? {
        None => return Err(Fail::Errno(errno::NOENT)),
        Some(Kind::Folder) => {}
        Some(_) => return Err(Fail::Errno(errno::NOTDIR)),
    }
    if held {
        return Err(Fail::Errno(errno::BUSY));
    }
    if !files.listing(&names).map_err(Fail::Errno)?.is_empty() {
        return Err(Fail::Errno(errno::NOTEMPTY));
    }
    files
        .root
        .remove_dir(path_of(&names))
        .map_err(|error| failed(&error))?;
    files.changed();
    Ok(())
}

/// `path_unlink_file(fd, path, len)`: a file the program doesn't hold open, or a link itself.
///
/// # Errors
///
/// A buffer outside memory.
pub fn path_unlink_file(
    host: &mut Host,
    memory: &[u8],
    fd: u32,
    path: (u32, u32),
) -> Result<(), Fail> {
    let names = resolve(host, memory, fd, path.0, path.1)?;
    if names.is_empty() {
        return Err(Fail::Errno(errno::INVAL));
    }
    let held = busy(&host.fds, &names);
    let files = files(host)?;
    match files.find(&names).map_err(Fail::Errno)? {
        None => return Err(Fail::Errno(errno::NOENT)),
        Some(Kind::Folder) => return Err(Fail::Errno(errno::ISDIR)),
        Some(_) => {}
    }
    if held {
        return Err(Fail::Errno(errno::BUSY));
    }
    files
        .root
        .remove_file(path_of(&names))
        .map_err(|error| failed(&error))?;
    files.changed();
    Ok(())
}

/// `path_rename(fd, from, from_len, to_fd, to, to_len)`: a file over a file replaces it, and a
/// folder over an empty folder replaces it, as POSIX has it, on every system.
///
/// # Errors
///
/// A buffer outside memory.
pub fn path_rename(
    host: &mut Host,
    memory: &[u8],
    from: (u32, u32, u32),
    to: (u32, u32, u32),
) -> Result<(), Fail> {
    let source = resolve(host, memory, from.0, from.1, from.2)?;
    let target = resolve(host, memory, to.0, to.1, to.2)?;
    if source.is_empty() || target.is_empty() {
        return Err(Fail::Errno(errno::INVAL));
    }
    if target.len() > source.len() && target.starts_with(&source) {
        return Err(Fail::Errno(errno::INVAL));
    }
    let held = busy(&host.fds, &source) || busy(&host.fds, &target);
    let files = files(host)?;
    let Some(moving) = files.find(&source).map_err(Fail::Errno)? else {
        return Err(Fail::Errno(errno::NOENT));
    };
    let replaced = files.find(&target).map_err(Fail::Errno)?;
    if held {
        return Err(Fail::Errno(errno::BUSY));
    }
    if source == target {
        return Ok(());
    }
    match (moving, replaced) {
        (_, None) => {
            if files.clashes(&target).map_err(Fail::Errno)? {
                return Err(Fail::Errno(errno::EXIST));
            }
        }
        (Kind::Folder, Some(Kind::Folder)) => {
            if !files.listing(&target).map_err(Fail::Errno)?.is_empty() {
                return Err(Fail::Errno(errno::NOTEMPTY));
            }
            // Not every system renames a folder over another: the empty one goes first.
            files
                .root
                .remove_dir(path_of(&target))
                .map_err(|error| failed(&error))?;
            files.changed();
        }
        (Kind::Folder, Some(_)) => return Err(Fail::Errno(errno::NOTDIR)),
        (_, Some(Kind::Folder)) => return Err(Fail::Errno(errno::ISDIR)),
        (_, Some(_)) => {}
    }
    files
        .root
        .rename(path_of(&source), &files.root, path_of(&target))
        .map_err(|error| failed(&error))?;
    files.changed();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(base: &[&str], text: &str) -> Result<Vec<String>, u16> {
        let base: Vec<String> = base.iter().map(|name| (*name).to_owned()).collect();
        names_in(&base, text.as_bytes())
    }

    #[test]
    fn a_path_stays_beneath_the_folder() {
        assert_eq!(names(&[], "a/./b/../c"), Ok(vec!["a".to_owned(), "c".to_owned()]));
        assert_eq!(names(&["sub"], "../x"), Ok(vec!["x".to_owned()]));
        assert_eq!(names(&[], "../x"), Err(errno::ACCES));
        assert_eq!(names(&[], "/etc/passwd"), Err(errno::ACCES));
        assert_eq!(names(&[], "."), Ok(Vec::new()));
        assert_eq!(names(&[], ""), Err(errno::NOENT));
    }

    #[test]
    fn every_systems_rules_hold_for_names() {
        assert_eq!(names(&[], "a:b"), Err(errno::INVAL));
        assert_eq!(names(&[], "CON"), Err(errno::INVAL));
        assert_eq!(names(&[], "REPORT~1.TXT"), Err(errno::INVAL));
        assert_eq!(names(&[], &"a".repeat(256)), Err(errno::NAMETOOLONG));
        assert_eq!(names_in(&[], b"a\xffb"), Err(errno::ILSEQ));
    }

    fn folder_with(entries: &[(&str, &str)]) -> (tempfile::TempDir, Files) {
        let temporary = tempfile::tempdir().expect("a folder");
        for (name, contents) in entries {
            let path = temporary.path().join(name);
            if let Some(stripped) = name.strip_suffix('/') {
                std::fs::create_dir_all(temporary.path().join(stripped)).expect("a folder");
            } else {
                std::fs::write(path, contents).expect("a file");
            }
        }
        let root = Dir::open_ambient_dir(temporary.path(), cap_std::ambient_authority())
            .expect("the folder");
        (temporary, Files::new(root, false))
    }

    #[test]
    fn names_are_found_byte_for_byte() {
        let (_folder, mut files) = folder_with(&[("readme.txt", "hi"), ("sub/", "")]);
        let name = |text: &str| -> Vec<String> { text.split('/').map(str::to_owned).collect() };
        assert_eq!(files.find(&name("readme.txt")), Ok(Some(Kind::File)));
        // Not found as another case, even where the system's names ignore case.
        assert_eq!(files.find(&name("README.TXT")), Ok(None));
        assert_eq!(files.find(&name("sub")), Ok(Some(Kind::Folder)));
        assert_eq!(files.find(&name("readme.txt/x")), Err(errno::NOTDIR));
        assert_eq!(files.find(&name("nowhere/x")), Err(errno::NOENT));
        // A new name that differs only in case clashes; the same name doesn't.
        assert_eq!(files.clashes(&name("README.txt")), Ok(true));
        assert_eq!(files.clashes(&name("readme.txt")), Ok(false));
    }

    #[test]
    fn a_folder_lists_sorted_and_turned_round_in_a_second_run() {
        let (_folder, mut files) = folder_with(&[("b", ""), ("a", ""), ("c/", "")]);
        let listed = |files: &mut Files| -> Vec<String> {
            files
                .ordered(&[])
                .expect("a listing")
                .into_iter()
                .map(|entry| entry.name)
                .collect()
        };
        assert_eq!(listed(&mut files), ["a", "b", "c"]);
        files.reversed = true;
        assert_eq!(listed(&mut files), ["c", "b", "a"]);
    }
}
