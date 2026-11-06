//! The broker's service on Linux: every path is opened beneath the program's folder with
//! `openat2`, which refuses a link or a way out whatever the path says, and a descriptor goes
//! back to the program over the channel as `SCM_RIGHTS`.

use std::collections::BTreeMap;
use std::io::{IoSlice, Read as _, Write as _};
use std::mem::MaybeUninit;
use std::os::fd::{AsFd as _, OwnedFd};
use std::os::unix::net::UnixStream;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use rustix::fs::{
    AtFlags, Dir, FileType, Mode as FileMode, OFlags, ResolveFlags, fstat, mkdirat, openat2,
    renameat, unlinkat,
};
use rustix::net::{SendAncillaryBuffer, SendAncillaryMessage, SendFlags, sendmsg};

use super::{
    Hello, Kind, Permit, REQUEST_LEN, Refusal, Request, encode_reply, parse_header, parse_request,
};
use crate::edge::Op;
use crate::edge::error::{EACCES, EBUSY, EEXIST, EINVAL, EIO, EMFILE};
use crate::names;
use crate::supply::Supply;

/// `ELOOP`: a name that is a link, where none may be.
const ELOOP: i32 = 40;

/// `EXDEV`: a name that leads out of the folder.
const EXDEV: i32 = 18;

/// The permissions of a file the program makes, rw-r--r--.
const FILE_MODE: u32 = 0o644;

/// The permissions of a folder the program makes, rwxr-xr-x.
const FOLDER_MODE: u32 = 0o755;

/// How every path is resolved: beneath the folder, with no link and no magic link on the way.
fn resolve() -> ResolveFlags {
    ResolveFlags::BENEATH | ResolveFlags::NO_SYMLINKS | ResolveFlags::NO_MAGICLINKS
}

/// What the broker answers a request with.
#[derive(Debug)]
pub struct Answer {
    /// The result: a count, a token or 0, or the negative of an error.
    pub result: i64,
    /// What follows the reply's fixed part.
    pub payload: Vec<u8>,
    /// A file's descriptor, to go with the reply.
    pub handover: Option<OwnedFd>,
}

impl Answer {
    /// An error: the closed set's, negated.
    fn error(errno: u32) -> Self {
        Self {
            result: -i64::from(errno),
            payload: Vec::new(),
            handover: None,
        }
    }

    /// A result with nothing else.
    fn result(result: i64) -> Self {
        Self {
            result,
            payload: Vec::new(),
            handover: None,
        }
    }
}

/// What the program has told the broker while it ran, which anyone holding the handle reads as it
/// happens: a test waits for the note of a server's wait before it connects.
#[derive(Debug, Default)]
pub struct Notes {
    waiting: AtomicBool,
    listed: AtomicBool,
    random_asked: AtomicUsize,
}

impl Notes {
    /// Whether the program is waiting for a connection or a key now: its last note of a wait
    /// has no note of its return after it.
    #[must_use]
    pub fn waiting(&self) -> bool {
        self.waiting.load(Ordering::SeqCst)
    }

    /// Whether the program listed a folder.
    #[must_use]
    pub fn listed(&self) -> bool {
        self.listed.load(Ordering::SeqCst)
    }

    /// How many random bytes the program asked for.
    #[must_use]
    pub fn random_asked(&self) -> usize {
        self.random_asked.load(Ordering::SeqCst)
    }
}

/// How the service ended.
#[derive(Debug)]
pub enum Outcome {
    /// The program closed the channel, or ended.
    Closed,
    /// The program sent what isn't a message of the protocol.
    Refused(Refusal),
    /// The program asked for what its spec doesn't allow.
    Forbidden(Kind),
    /// The channel failed.
    Failed(std::io::Error),
}

/// One program's broker.
#[derive(Debug)]
pub struct Broker {
    /// The program's folder, beneath which every path is resolved.
    root: OwnedFd,
    /// What the program is told first.
    hello: Hello,
    /// What its spec lets it ask for.
    permit: Permit,
    /// What a test supplies.
    supply: Supply,
    /// What the program has told it.
    notes: Arc<Notes>,
    /// Which clock reading the next call gets.
    clock_next: usize,
    /// The path of each file the program holds open, by the token its reply carried.
    opens: BTreeMap<u32, Vec<u8>>,
    /// The last token given.
    token: u32,
    /// The entries of each folder it has read, by the folder's path, until the program changes
    /// one.
    folders: BTreeMap<Vec<u8>, Vec<String>>,
}

/// The errno of `error`, as the closed set has it for `op`: a link or a way out of the folder is
/// `EACCES`, and an error `op` doesn't list is `EIO`.
fn closed(op: Op, errno: i32) -> u32 {
    let Ok(errno) = u32::try_from(errno) else {
        return EIO;
    };
    let errno = if errno == ELOOP.unsigned_abs() || errno == EXDEV.unsigned_abs() {
        EACCES
    } else {
        errno
    };
    if op.errors().contains(&errno) {
        errno
    } else {
        EIO
    }
}

/// The names of a path, with empty names and `.` dropped and `..` taken back a name, as the kernel
/// does where no name is a link, and none may be: or `EACCES` for a path that starts at the root
/// or climbs out of the folder.
fn names_of(path: &[u8]) -> Result<Vec<&[u8]>, u32> {
    if path.first() == Some(&b'/') {
        return Err(EACCES);
    }
    let mut names: Vec<&[u8]> = Vec::new();
    for name in path.split(|byte| *byte == b'/') {
        match name {
            b"" | b"." => {}
            b".." => {
                if names.pop().is_none() {
                    return Err(EACCES);
                }
            }
            other => names.push(other),
        }
    }
    Ok(names)
}

impl Broker {
    /// A broker for a program whose folder is `root`, which does for it what `permit` says.
    #[must_use]
    pub fn new(root: OwnedFd, hello: Hello, supply: Supply, permit: Permit) -> Self {
        Self {
            root,
            hello,
            permit,
            supply,
            notes: Arc::new(Notes::default()),
            clock_next: 0,
            opens: BTreeMap::new(),
            token: 0,
            folders: BTreeMap::new(),
        }
    }

    /// The first message the program is sent.
    #[must_use]
    pub const fn hello(&self) -> Hello {
        self.hello
    }

    /// What the program's spec lets it ask for.
    #[must_use]
    pub const fn permit(&self) -> Permit {
        self.permit
    }

    /// What the program has told the broker.
    #[must_use]
    pub fn notes(&self) -> Arc<Notes> {
        Arc::clone(&self.notes)
    }

    /// Answers a request. The notes get no answer, and the caller sends none.
    pub fn answer(&mut self, request: &Request) -> Answer {
        match request.kind {
            Kind::Open => self.open(&request.first, request.argument),
            Kind::Mkdir => self.mkdir(&request.first),
            Kind::Remove => self.remove(&request.first, request.argument),
            Kind::Rename => self.rename(&request.first, &request.second),
            Kind::Closed => {
                if let Ok(token) = u32::try_from(request.value) {
                    self.opens.remove(&token);
                }
                Answer::result(0)
            }
            Kind::Waiting => {
                self.notes.waiting.store(true, Ordering::SeqCst);
                Answer::result(0)
            }
            Kind::Resumed => {
                self.notes.waiting.store(false, Ordering::SeqCst);
                Answer::result(0)
            }
            Kind::Listed => {
                self.notes.listed.store(true, Ordering::SeqCst);
                Answer::result(0)
            }
            Kind::Clock => self.clock(),
            Kind::Random => self.random(usize::try_from(request.value).unwrap_or(usize::MAX)),
        }
    }

    /// The next clock reading: the supply's, the last one repeating.
    fn clock(&mut self) -> Answer {
        let Some(last) = self.supply.clock.len().checked_sub(1) else {
            return Answer::error(EINVAL);
        };
        let time = self.supply.clock[self.clock_next.min(last)];
        self.clock_next = self.clock_next.saturating_add(1);
        let mut payload = Vec::with_capacity(16);
        payload.extend_from_slice(&time.seconds.to_le_bytes());
        payload.extend_from_slice(&u64::from(time.nanoseconds).to_le_bytes());
        Answer {
            result: 0,
            payload,
            handover: None,
        }
    }

    /// The next `count` random bytes of the supply, zeros past its end.
    fn random(&mut self, count: usize) -> Answer {
        let Some(bytes) = &self.supply.random else {
            return Answer::error(EINVAL);
        };
        let start = self.notes.random_asked().min(bytes.len());
        let mut payload: Vec<u8> = bytes[start..].iter().take(count).copied().collect();
        payload.resize(count, 0);
        self.notes.random_asked.fetch_add(count, Ordering::SeqCst);
        Answer {
            result: i64::try_from(count).unwrap_or(i64::MAX),
            payload,
            handover: None,
        }
    }

    /// Whether the program holds a file open at `path`, or beneath it.
    fn busy(&self, path: &[u8]) -> bool {
        self.opens.values().any(|open| {
            open == path
                || (open.len() > path.len() && open.starts_with(path) && open[path.len()] == b'/')
        })
    }

    /// The entries of the folder at `path`, read once and kept until the program changes one.
    fn entries(&mut self, path: &[u8]) -> Result<&Vec<String>, i32> {
        if !self.folders.contains_key(path) {
            let folder = self.parent(path)?;
            let mut found = Vec::new();
            for entry in Dir::read_from(&folder).map_err(|errno| errno.raw_os_error())? {
                let entry = entry.map_err(|errno| errno.raw_os_error())?;
                let name = entry.file_name().to_string_lossy().into_owned();
                if name != "." && name != ".." {
                    found.push(name);
                }
            }
            self.folders.insert(path.to_vec(), found);
        }
        self.folders.get(path).ok_or(EIO.cast_signed())
    }

    /// Whether a new name, the last of `names`, differs only in case or normalization from an
    /// entry of its folder. A folder that can't be read has no clash: the call says why.
    fn clashes(&mut self, names: &[&[u8]]) -> bool {
        let Some((leaf, parent)) = names.split_last() else {
            return false;
        };
        let leaf = String::from_utf8_lossy(leaf).into_owned();
        match self.entries(&parent.join(&b'/')) {
            Ok(entries) => entries.iter().any(|other| names::clashes(&leaf, other)),
            Err(_) => false,
        }
    }

    /// The folder at `path` beneath the program's, opened without following a link.
    fn parent(&self, path: &[u8]) -> Result<OwnedFd, i32> {
        let path: &[u8] = if path.is_empty() { b"." } else { path };
        openat2(
            &self.root,
            path,
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
            FileMode::empty(),
            resolve(),
        )
        .map_err(|errno| errno.raw_os_error())
    }

    /// A token for a file just opened, which the program's note of its close gives back.
    fn record(&mut self, path: Vec<u8>) -> u32 {
        self.token = self.token.wrapping_add(1).max(1);
        self.opens.insert(self.token, path);
        self.token
    }

    /// `os_open(name, mode)`.
    fn open(&mut self, name: &[u8], mode: u8) -> Answer {
        if let Some(broken) = names::broken(name) {
            return Answer::error(closed(Op::Open, i32::try_from(broken.errno).unwrap_or(0)));
        }
        let names = match names_of(name) {
            Ok(names) => names,
            Err(errno) => return Answer::error(errno),
        };
        let (flags, creates) = match mode {
            0 => (OFlags::RDONLY, false),
            1 => (OFlags::WRONLY | OFlags::CREATE | OFlags::TRUNC, true),
            2 => (OFlags::WRONLY | OFlags::CREATE | OFlags::APPEND, true),
            3 => (OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL, true),
            _ => (OFlags::RDONLY | OFlags::DIRECTORY, false),
        };
        // A program holds at most [`crate::runner::OPEN_FILES`] descriptors, and its edge tells
        // the broker of each close: one that asks for more than that has hidden its closes.
        if u64::try_from(self.opens.len()).unwrap_or(u64::MAX) >= crate::runner::OPEN_FILES {
            return Answer::error(EMFILE);
        }
        if creates && self.clashes(&names) {
            return Answer::error(EEXIST);
        }
        let path = names.join(&b'/');
        let target: &[u8] = if path.is_empty() { b"." } else { &path };
        let permissions = if creates {
            FileMode::from_raw_mode(FILE_MODE)
        } else {
            FileMode::empty()
        };
        // Without O_NONBLOCK, opening a named pipe waits in the kernel for its other end, and the
        // broker, a thread of the server, would wait with it whatever becomes of the program.
        let opened = openat2(
            &self.root,
            target,
            flags | OFlags::CLOEXEC | OFlags::NOCTTY | OFlags::NONBLOCK,
            permissions,
            resolve(),
        );
        match opened {
            Ok(descriptor) => {
                // Only a file or a folder goes to the program: a named pipe, a device or a socket
                // in its folder is no business of its.
                match fstat(&descriptor) {
                    Ok(details)
                        if matches!(
                            FileType::from_raw_mode(details.st_mode),
                            FileType::RegularFile | FileType::Directory
                        ) => {}
                    Ok(_) => return Answer::error(EACCES),
                    Err(errno) => {
                        return Answer::error(closed(Op::Open, errno.raw_os_error()));
                    }
                }
                if creates {
                    self.folders.clear();
                }
                let token = self.record(path);
                Answer {
                    result: i64::from(token),
                    payload: Vec::new(),
                    handover: Some(descriptor),
                }
            }
            Err(errno) => Answer::error(closed(Op::Open, errno.raw_os_error())),
        }
    }

    /// `os_mkdir(name)`.
    fn mkdir(&mut self, name: &[u8]) -> Answer {
        let names = match self.checked(Op::Mkdir, name) {
            Ok(names) => names,
            Err(errno) => return Answer::error(errno),
        };
        // `.` is the folder itself, which is there.
        let Some((leaf, parent)) = names.split_last() else {
            return Answer::error(EEXIST);
        };
        if self.clashes(&names) {
            return Answer::error(EEXIST);
        }
        let folder = match self.parent(&parent.join(&b'/')) {
            Ok(folder) => folder,
            Err(errno) => return Answer::error(closed(Op::Mkdir, errno)),
        };
        match mkdirat(&folder, *leaf, FileMode::from_raw_mode(FOLDER_MODE)) {
            Ok(()) => {
                self.folders.clear();
                Answer::result(0)
            }
            Err(errno) => Answer::error(closed(Op::Mkdir, errno.raw_os_error())),
        }
    }

    /// `os_remove(name, kind)`: a file for 0, a folder for 1.
    fn remove(&mut self, name: &[u8], kind: u8) -> Answer {
        let names = match self.checked(Op::Remove, name) {
            Ok(names) => names,
            Err(errno) => return Answer::error(errno),
        };
        let Some((leaf, parent)) = names.split_last() else {
            return Answer::error(EINVAL);
        };
        if self.busy(&names.join(&b'/')) {
            return Answer::error(EBUSY);
        }
        let folder = match self.parent(&parent.join(&b'/')) {
            Ok(folder) => folder,
            Err(errno) => return Answer::error(closed(Op::Remove, errno)),
        };
        let flags = if kind == 1 {
            AtFlags::REMOVEDIR
        } else {
            AtFlags::empty()
        };
        match unlinkat(&folder, *leaf, flags) {
            Ok(()) => {
                self.folders.clear();
                Answer::result(0)
            }
            Err(errno) => Answer::error(closed(Op::Remove, errno.raw_os_error())),
        }
    }

    /// `os_rename(from, to)`.
    fn rename(&mut self, from: &[u8], to: &[u8]) -> Answer {
        let from_names = match self.checked(Op::Rename, from) {
            Ok(names) => names,
            Err(errno) => return Answer::error(errno),
        };
        let to_names = match self.checked(Op::Rename, to) {
            Ok(names) => names,
            Err(errno) => return Answer::error(errno),
        };
        let (Some((from_leaf, from_parent)), Some((to_leaf, to_parent))) =
            (from_names.split_last(), to_names.split_last())
        else {
            return Answer::error(EINVAL);
        };
        if self.busy(&from_names.join(&b'/')) {
            return Answer::error(EBUSY);
        }
        if self.clashes(&to_names) {
            return Answer::error(EEXIST);
        }
        let source = match self.parent(&from_parent.join(&b'/')) {
            Ok(folder) => folder,
            Err(errno) => return Answer::error(closed(Op::Rename, errno)),
        };
        let target = match self.parent(&to_parent.join(&b'/')) {
            Ok(folder) => folder,
            Err(errno) => return Answer::error(closed(Op::Rename, errno)),
        };
        match renameat(&source, *from_leaf, &target, *to_leaf) {
            Ok(()) => {
                self.folders.clear();
                Answer::result(0)
            }
            Err(errno) => Answer::error(closed(Op::Rename, errno.raw_os_error())),
        }
    }

    /// A name's rules, then its names: the errno `op` gives when it breaks one.
    fn checked<'a>(&self, op: Op, name: &'a [u8]) -> Result<Vec<&'a [u8]>, u32> {
        if let Some(broken) = names::broken(name) {
            return Err(closed(op, i32::try_from(broken.errno).unwrap_or(0)));
        }
        names_of(name)
    }
}

/// Sends a reply, with the file's descriptor when it has one.
fn send(stream: &mut UnixStream, answer: &Answer) -> std::io::Result<()> {
    let bytes = encode_reply(answer.result, &answer.payload);
    let Some(descriptor) = &answer.handover else {
        return stream.write_all(&bytes);
    };
    let mut space = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(1))];
    let mut control = SendAncillaryBuffer::new(&mut space);
    let descriptors = [descriptor.as_fd()];
    let _fits = control.push(SendAncillaryMessage::ScmRights(&descriptors));
    let sent = sendmsg(
        &*stream,
        &[IoSlice::new(&bytes)],
        &mut control,
        SendFlags::NOSIGNAL,
    )
    .map_err(std::io::Error::from)?;
    stream.write_all(&bytes[sent..])
}

/// Whether an error is the program's end of the channel going away.
fn gone(error: &std::io::Error) -> bool {
    matches!(
        error.kind(),
        std::io::ErrorKind::UnexpectedEof
            | std::io::ErrorKind::BrokenPipe
            | std::io::ErrorKind::ConnectionReset
    )
}

/// Sends the first message. The program reads it when it starts, so a caller that starts the
/// program sends it first.
///
/// # Errors
///
/// When the channel fails.
pub fn greet(broker: &Broker, stream: &mut UnixStream) -> std::io::Result<()> {
    stream.write_all(&broker.hello().encode())
}

/// Serves one program: says hello, then answers each request until the program ends, or sends
/// what isn't a request.
pub fn serve(broker: &mut Broker, stream: &mut UnixStream) -> Outcome {
    if let Err(error) = greet(broker, stream) {
        return if gone(&error) {
            Outcome::Closed
        } else {
            Outcome::Failed(error)
        };
    }
    answer_all(broker, stream)
}

/// Answers each request until the program ends, or sends what isn't a request, once the first
/// message is sent.
pub fn answer_all(broker: &mut Broker, stream: &mut UnixStream) -> Outcome {
    loop {
        let mut fixed = [0u8; REQUEST_LEN];
        if let Err(error) = stream.read_exact(&mut fixed) {
            return if gone(&error) {
                Outcome::Closed
            } else {
                Outcome::Failed(error)
            };
        }
        let header = match parse_header(&fixed) {
            Ok(header) => header,
            Err(refusal) => return Outcome::Refused(refusal),
        };
        let mut message = fixed.to_vec();
        message.resize(REQUEST_LEN + header.body(), 0);
        if let Err(error) = stream.read_exact(&mut message[REQUEST_LEN..]) {
            return if gone(&error) {
                Outcome::Closed
            } else {
                Outcome::Failed(error)
            };
        }
        let request = match parse_request(&message) {
            Ok(request) => request,
            Err(refusal) => return Outcome::Refused(refusal),
        };
        // What the spec doesn't allow ends the service, as what isn't a request does: an honest
        // edge asks for nothing its program's spec didn't name.
        if !broker.permit().allows(request.kind) {
            return Outcome::Forbidden(request.kind);
        }
        let answer = broker.answer(&request);
        if request.kind.has_reply()
            && let Err(error) = send(stream, &answer)
        {
            return if gone(&error) {
                Outcome::Closed
            } else {
                Outcome::Failed(error)
            };
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use rustix::net::{RecvAncillaryBuffer, RecvAncillaryMessage, RecvFlags, recvmsg};

    use super::super::{Mode, flag, parse_reply};
    use super::*;
    use crate::supply::Time;

    /// A broker for a program whose folder is `folder`, in a run.
    fn broker_in(folder: &Path) -> Broker {
        let root = rustix::fs::open(
            folder,
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
            FileMode::empty(),
        )
        .expect("the folder opens");
        Broker::new(
            root,
            Hello {
                mode: Mode::Run,
                flags: 0,
                file_limit: 0,
            },
            Supply::default(),
            Permit::ALL,
        )
    }

    fn request(kind: Kind, argument: u8, first: &str, second: &str) -> Request {
        Request {
            kind,
            argument,
            value: 0,
            first: first.as_bytes().to_vec(),
            second: second.as_bytes().to_vec(),
        }
    }

    /// The errno of an answer, or 0 for a success.
    fn errno(answer: &Answer) -> i64 {
        (-answer.result).max(0)
    }

    #[test]
    fn a_name_is_opened_made_and_refused_as_every_system_would() {
        let folder = tempfile::tempdir().expect("a folder");
        std::fs::write(folder.path().join("notes.txt"), "hello").expect("a file");
        let mut broker = broker_in(folder.path());
        // An existing file opens to read, and a missing one is not there.
        let read = broker.answer(&request(Kind::Open, 0, "notes.txt", ""));
        assert!(read.result >= 1 && read.handover.is_some(), "{read:?}");
        assert_eq!(
            errno(&broker.answer(&request(Kind::Open, 0, "gone.txt", ""))),
            2
        );
        // A file is made, emptied again, appended to, and made only when new.
        for (mode, wanted) in [(1, 0), (1, 0), (2, 0), (3, 17)] {
            let answer = broker.answer(&request(Kind::Open, mode, "made.txt", ""));
            assert_eq!(errno(&answer), wanted, "mode {mode}");
        }
        // A name that differs from an entry only in case is refused where it would be new.
        assert_eq!(
            errno(&broker.answer(&request(Kind::Open, 1, "MADE.TXT", ""))),
            17
        );
        assert_eq!(
            errno(&broker.answer(&request(Kind::Mkdir, 0, "Notes.TXT", ""))),
            17
        );
        // Names every system refuses.
        assert_eq!(
            errno(&broker.answer(&request(Kind::Open, 1, "a:b", ""))),
            22
        );
        assert_eq!(
            errno(&broker.answer(&request(Kind::Open, 1, "con", ""))),
            22
        );
        let long = "x".repeat(300);
        assert_eq!(
            errno(&broker.answer(&request(Kind::Open, 1, &long, ""))),
            36
        );
        // The folder itself opens as a folder, and not as a file to write.
        let here = broker.answer(&request(Kind::Open, 4, ".", ""));
        assert!(here.handover.is_some(), "{here:?}");
        assert_eq!(errno(&broker.answer(&request(Kind::Open, 1, ".", ""))), 21);
    }

    #[test]
    fn nothing_leads_out_of_the_folder() {
        let folder = tempfile::tempdir().expect("a folder");
        let outside = tempfile::tempdir().expect("another folder");
        std::fs::write(outside.path().join("secret"), "x").expect("a file");
        std::os::unix::fs::symlink(outside.path(), folder.path().join("link")).expect("a link");
        std::os::unix::fs::symlink(outside.path().join("secret"), folder.path().join("file"))
            .expect("a link");
        let mut broker = broker_in(folder.path());
        let absolute = outside.path().join("secret");
        for path in [
            "../secret",
            "a/../../secret",
            "link/secret",
            "link",
            "file",
            absolute.to_str().expect("text"),
        ] {
            for (kind, argument) in [(Kind::Open, 0), (Kind::Open, 1), (Kind::Open, 4)] {
                let answer = broker.answer(&request(kind, argument, path, ""));
                assert_eq!(errno(&answer), 13, "{path} with mode {argument}");
            }
        }
        // Nor does a way that only looks like one: the folder is reached by a name that climbs
        // and comes back.
        std::fs::create_dir(folder.path().join("sub")).expect("a folder");
        let back = broker.answer(&request(Kind::Open, 4, "sub/../sub", ""));
        assert!(back.handover.is_some(), "{back:?}");
        // The calls that change names do not follow a link either.
        assert_eq!(
            errno(&broker.answer(&request(Kind::Mkdir, 0, "link/new", ""))),
            13
        );
        assert_eq!(
            errno(&broker.answer(&request(Kind::Remove, 0, "link/secret", ""))),
            13
        );
        assert_eq!(
            errno(&broker.answer(&request(Kind::Rename, 0, "link/secret", "x"))),
            13
        );
        assert!(outside.path().join("secret").exists());
        // A link in the folder is itself removed, not what it points to.
        assert_eq!(
            errno(&broker.answer(&request(Kind::Remove, 0, "file", ""))),
            0
        );
        assert!(outside.path().join("secret").exists());
    }

    #[test]
    fn a_file_held_open_is_not_renamed_or_removed() {
        let folder = tempfile::tempdir().expect("a folder");
        std::fs::create_dir(folder.path().join("sub")).expect("a folder");
        std::fs::write(folder.path().join("sub/a.txt"), "a").expect("a file");
        std::fs::write(folder.path().join("b.txt"), "b").expect("a file");
        let mut broker = broker_in(folder.path());
        let held = broker.answer(&request(Kind::Open, 0, "sub/a.txt", ""));
        let token = u64::try_from(held.result).expect("a token");
        assert_eq!(
            errno(&broker.answer(&request(Kind::Remove, 0, "sub/a.txt", ""))),
            16
        );
        assert_eq!(
            errno(&broker.answer(&request(Kind::Remove, 1, "sub", ""))),
            16
        );
        assert_eq!(
            errno(&broker.answer(&request(Kind::Rename, 0, "sub", "moved"))),
            16
        );
        assert_eq!(
            errno(&broker.answer(&request(Kind::Rename, 0, "sub/a.txt", "c"))),
            16
        );
        // What isn't held goes.
        assert_eq!(
            errno(&broker.answer(&request(Kind::Rename, 0, "b.txt", "c.txt"))),
            0
        );
        // Once the program says it closed the file, it goes.
        let closed = Request {
            kind: Kind::Closed,
            argument: 0,
            value: token,
            first: Vec::new(),
            second: Vec::new(),
        };
        broker.answer(&closed);
        assert_eq!(
            errno(&broker.answer(&request(Kind::Remove, 0, "sub/a.txt", ""))),
            0
        );
        assert_eq!(
            errno(&broker.answer(&request(Kind::Remove, 1, "sub", ""))),
            0
        );
        // A folder with something in it stays, with the error the system gives.
        assert_eq!(
            errno(&broker.answer(&request(Kind::Mkdir, 0, "full", ""))),
            0
        );
        std::fs::write(folder.path().join("full/x"), "x").expect("a file");
        assert_eq!(
            errno(&broker.answer(&request(Kind::Remove, 1, "full", ""))),
            39
        );
    }

    #[test]
    fn a_test_gets_its_supply() {
        let folder = tempfile::tempdir().expect("a folder");
        let root = rustix::fs::open(
            folder.path(),
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
            FileMode::empty(),
        )
        .expect("the folder opens");
        let supply = Supply {
            random: Some(vec![1, 2, 3]),
            clock: vec![
                Time {
                    seconds: 100,
                    nanoseconds: 5,
                },
                Time {
                    seconds: 200,
                    nanoseconds: 6,
                },
            ],
        };
        let hello = Hello {
            mode: Mode::Test,
            flags: flag::RANDOM | flag::CLOCK,
            file_limit: 0,
        };
        let mut broker = Broker::new(root, hello, supply, Permit::ALL);
        let random = |count: u64| Request {
            kind: Kind::Random,
            argument: 0,
            value: count,
            first: Vec::new(),
            second: Vec::new(),
        };
        // The bytes in order, then zeros, however they are asked for.
        assert_eq!(broker.answer(&random(2)).payload, vec![1, 2]);
        assert_eq!(broker.answer(&random(3)).payload, vec![3, 0, 0]);
        assert_eq!(broker.notes().random_asked(), 5);
        // The readings in order, the last one repeating.
        let reading = |broker: &mut Broker| {
            let answer = broker.answer(&request(Kind::Clock, 0, "", ""));
            let mut seconds = [0u8; 8];
            seconds.copy_from_slice(&answer.payload[..8]);
            i64::from_le_bytes(seconds)
        };
        assert_eq!(
            [
                reading(&mut broker),
                reading(&mut broker),
                reading(&mut broker)
            ],
            [100, 200, 200]
        );
        // The notes.
        broker.answer(&request(Kind::Waiting, 0, "", ""));
        broker.answer(&request(Kind::Listed, 0, "", ""));
        assert!(broker.notes().waiting() && broker.notes().listed());
        // A wait ends with the note of the return from it.
        broker.answer(&request(Kind::Resumed, 0, "", ""));
        assert!(!broker.notes().waiting());
    }

    /// A client's side of the channel: a request goes out, and the reply and its descriptor come
    /// back.
    fn ask(stream: &mut UnixStream, request: &Request) -> (i64, Vec<u8>, Option<OwnedFd>) {
        stream
            .write_all(&request.encode())
            .expect("the request goes");
        let mut space = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(1))];
        let mut control = RecvAncillaryBuffer::new(&mut space);
        let mut fixed = [0u8; super::super::REPLY_LEN];
        let received = recvmsg(
            &*stream,
            &mut [std::io::IoSliceMut::new(&mut fixed)],
            &mut control,
            RecvFlags::empty(),
        )
        .expect("the reply comes");
        assert_eq!(received.bytes, fixed.len());
        let mut descriptor = None;
        for message in control.drain() {
            if let RecvAncillaryMessage::ScmRights(mut descriptors) = message {
                descriptor = descriptors.next();
            }
        }
        let (result, length) = parse_reply(&fixed).expect("a reply");
        let mut payload = vec![0u8; length];
        stream.read_exact(&mut payload).expect("the payload comes");
        (result, payload, descriptor)
    }

    #[test]
    fn a_file_comes_over_the_channel_and_a_hostile_message_ends_the_service() {
        let folder = tempfile::tempdir().expect("a folder");
        std::fs::write(folder.path().join("notes.txt"), "hello").expect("a file");
        let mut broker = broker_in(folder.path());
        let (mut client, mut server) = UnixStream::pair().expect("a channel");
        let service = std::thread::spawn(move || {
            let outcome = serve(&mut broker, &mut server);
            (outcome, broker.notes())
        });
        let mut first = [0u8; super::super::HELLO_LEN];
        client.read_exact(&mut first).expect("the first message");
        assert_eq!(
            Hello::decode(&first),
            Ok(Hello {
                mode: Mode::Run,
                flags: 0,
                file_limit: 0
            })
        );
        // The descriptor of a file arrives with the reply, and reads what is in it.
        let (result, _, descriptor) = ask(&mut client, &request(Kind::Open, 0, "notes.txt", ""));
        assert!(result >= 1, "{result}");
        let mut file = std::fs::File::from(descriptor.expect("a descriptor"));
        let mut text = String::new();
        file.read_to_string(&mut text).expect("the file reads");
        assert_eq!(text, "hello");
        // An error comes with no descriptor.
        let (result, _, descriptor) = ask(&mut client, &request(Kind::Open, 0, "gone", ""));
        assert_eq!(result, -2);
        assert!(descriptor.is_none());
        // A descriptor sent to the broker is ignored.
        let spare = std::fs::File::open(folder.path().join("notes.txt")).expect("a file");
        let mut space = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(1))];
        let mut control = SendAncillaryBuffer::new(&mut space);
        let fds = [spare.as_fd()];
        let _fits = control.push(SendAncillaryMessage::ScmRights(&fds));
        let ping = request(Kind::Waiting, 0, "", "").encode();
        sendmsg(
            &client,
            &[IoSlice::new(&ping)],
            &mut control,
            SendFlags::empty(),
        )
        .expect("the note goes");
        let (result, _, _) = ask(&mut client, &request(Kind::Open, 4, ".", ""));
        assert!(result >= 1);
        // Bytes that aren't a request end it.
        client
            .write_all(&[0xff; REQUEST_LEN])
            .expect("the bytes go");
        let (outcome, notes) = service.join().expect("the service ends");
        assert!(
            matches!(outcome, Outcome::Refused(Refusal::Kind(255))),
            "{outcome:?}"
        );
        assert!(notes.waiting());
    }

    #[test]
    fn the_service_ends_when_the_program_does() {
        let folder = tempfile::tempdir().expect("a folder");
        let mut broker = broker_in(folder.path());
        let (client, mut server) = UnixStream::pair().expect("a channel");
        let service = std::thread::spawn(move || serve(&mut broker, &mut server));
        drop(client);
        assert!(matches!(
            service.join().expect("the service ends"),
            Outcome::Closed
        ));
    }

    /// Sends `request` over `stream`, whole, as the edge does.
    fn send_request(stream: &mut UnixStream, request: &Request) {
        stream
            .write_all(&request.encode())
            .expect("the request is sent");
    }

    #[test]
    fn what_the_spec_does_not_allow_ends_the_service() {
        // A spec of the clock alone: no names, no random bytes.
        let calls = [crate::spec::Syscall::ClockGettime];
        let allowed = Permit::of(&calls);
        assert!(allowed.clock && !allowed.names && !allowed.random, "{allowed:?}");
        for kind in [Kind::Open, Kind::Mkdir, Kind::Remove, Kind::Rename, Kind::Random] {
            let folder = tempfile::tempdir().expect("a folder");
            let root = rustix::fs::open(
                folder.path(),
                OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
                FileMode::empty(),
            )
            .expect("the folder opens");
            let hello = Hello {
                mode: Mode::Run,
                flags: 0,
                file_limit: 0,
            };
            let mut broker = Broker::new(root, hello, Supply::default(), allowed);
            let (mut client, mut server) = UnixStream::pair().expect("a channel");
            let service = std::thread::spawn(move || answer_all(&mut broker, &mut server));
            let asked = match kind {
                Kind::Random => Request {
                    kind,
                    argument: 0,
                    value: 4,
                    first: Vec::new(),
                    second: Vec::new(),
                },
                Kind::Open => request(kind, 1, "made", ""),
                Kind::Rename => request(kind, 0, "made", "renamed"),
                _ => request(kind, 0, "made", ""),
            };
            send_request(&mut client, &asked);
            let outcome = service.join().expect("the service ends");
            assert!(
                matches!(outcome, Outcome::Forbidden(found) if found == kind),
                "{kind:?}: {outcome:?}"
            );
            assert!(
                std::fs::read_dir(folder.path()).expect("the folder").next().is_none(),
                "{kind:?} left something in the folder"
            );
        }
        // What the spec names is answered, and the notes always are.
        let folder = tempfile::tempdir().expect("a folder");
        let everything = broker_in(folder.path());
        assert!(everything.permit().allows(Kind::Open));
        let nothing = Permit::default();
        for kind in [Kind::Closed, Kind::Waiting, Kind::Listed, Kind::Resumed] {
            assert!(nothing.allows(kind), "{kind:?}");
        }
        assert!(!nothing.allows(Kind::Clock));
    }

    #[test]
    fn a_named_pipe_is_refused_and_does_not_hold_the_broker() {
        let folder = tempfile::tempdir().expect("a folder");
        rustix::fs::mknodat(
            rustix::fs::CWD,
            folder.path().join("pipe"),
            FileType::Fifo,
            FileMode::from_raw_mode(0o600),
            0,
        )
        .expect("a named pipe is made");
        let mut broker = broker_in(folder.path());
        let started = std::time::Instant::now();
        for mode in [0, 1, 2] {
            let answer = broker.answer(&request(Kind::Open, mode, "pipe", ""));
            assert!(answer.handover.is_none(), "mode {mode}");
            assert!(errno(&answer) > 0, "mode {mode}: a refusal");
        }
        assert!(
            started.elapsed() < std::time::Duration::from_secs(5),
            "the open waited for the pipe's other end"
        );
    }

    #[test]
    fn a_program_that_hides_its_closes_cannot_fill_the_brokers_table() {
        let folder = tempfile::tempdir().expect("a folder");
        std::fs::write(folder.path().join("file"), "x").expect("a file");
        let mut broker = broker_in(folder.path());
        let held = usize::try_from(crate::runner::OPEN_FILES).expect("a count");
        for number in 0..held {
            let answer = broker.answer(&request(Kind::Open, 0, "file", ""));
            assert_eq!(errno(&answer), 0, "open {number}");
        }
        let answer = broker.answer(&request(Kind::Open, 0, "file", ""));
        assert_eq!(errno(&answer), i64::from(EMFILE), "one more than a program can hold");
    }

    #[test]
    fn a_program_that_never_reads_cannot_hold_the_broker_for_ever() {
        let folder = tempfile::tempdir().expect("a folder");
        let mut broker = broker_in(folder.path());
        let (mut client, mut server) = UnixStream::pair().expect("a channel");
        server
            .set_write_timeout(Some(std::time::Duration::from_millis(300)))
            .expect("a timeout");
        let service = std::thread::spawn(move || answer_all(&mut broker, &mut server));
        // Clock requests are answered, and the answers pile up unread until the channel is full.
        let asked = request(Kind::Clock, 0, "", "");
        for _ in 0..20_000 {
            if client.write_all(&asked.encode()).is_err() {
                break;
            }
        }
        let outcome = service.join().expect("the service ends");
        assert!(
            matches!(outcome, Outcome::Failed(_) | Outcome::Closed),
            "{outcome:?}"
        );
    }
}
