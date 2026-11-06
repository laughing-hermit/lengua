//! The world a run is in, as the prover models it for every front end: standard input, output
//! and error, the working folder's files and folders, the test's network, the clock and random
//! bytes, and the proof's account of the run: the conditions its inputs meet, what it must still
//! be shown never to do, and how it ended. A front end runs its machine's code ([`Core`]) and
//! calls the world for what a call of the system does: the world's models act on the run through
//! [`Host`], which reads and writes the program's memory as the front end holds it to the
//! memory rules. A system call's result is as Linux gives it, a count or a descriptor, or minus
//! an error number.
//!
//! [`Core`]: super::exec::Core

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::rc::Rc;

use super::exec::induction::Slot;
use super::exec::{End, Stop, unsupported};
use super::term::{Pool, Term};

pub mod net;

/// The most bytes of standard input left for which a proof takes every count a read can
/// return: every way 8 bytes can be cut is 128 runs.
const RAW_CUT_BYTES: u64 = 8;

// The error numbers the world's models give, as Linux numbers them.
pub(super) const ENOENT: i64 = 2;
pub(super) const EBADF: i64 = 9;
const EEXIST: i64 = 17;
const ENOTDIR: i64 = 20;
const EISDIR: i64 = 21;
const ENOTEMPTY: i64 = 39;
/// `EPIPE`: a write to a pipe nobody reads, such as a closed standard output.
const EPIPE: i64 = 32;
/// `unlinkat`'s flag for removing a folder rather than a file.
const AT_REMOVEDIR: u64 = 0x200;

/// The world one run is in, and the proof's account of the run.
#[derive(Debug, Clone, Default)]
pub struct World {
    /// Standard input, byte by byte.
    pub(super) stdin: Rc<Vec<Term>>,
    /// How much of standard input the run has read.
    pub(super) stdin_read: usize,
    /// Where standard input ends, when its length is the input's choice: `stdin` then holds as
    /// many bytes as it can be long, those past the end standing for nothing read.
    pub(super) stdin_length: Option<Term>,
    /// Where in `stdin` the text of that length starts: the bytes before it, such as a whole
    /// buffer-full an induction's base puts first, are there whatever the length.
    pub(super) stdin_from: usize,
    /// The pieces standard input arrives in, by length, when it doesn't arrive whole: each read
    /// gets at most what its piece has left.
    pub(super) stdin_pieces: Rc<Vec<usize>>,
    /// Whether standard output is closed, a pipe nobody reads: every write to it fails with
    /// `EPIPE`, as the test's own run has it.
    pub(super) stdout_closed: bool,
    /// Bytes written to standard output, shared with the runs a fork made until one writes.
    pub stdout: Rc<Vec<Term>>,
    /// Bytes written to standard error, shared the same way.
    pub stderr: Rc<Vec<Term>>,
    /// The files in the working folder before the run, by name.
    pub(super) files: Rc<BTreeMap<String, Vec<Term>>>,
    /// The files the run created or replaced, by name, with everything written to them.
    pub written: BTreeMap<String, Vec<Term>>,
    /// The empty folders in the working folder before the run.
    pub(super) folders_before: Rc<BTreeSet<String>>,
    /// The folders the run made.
    pub(super) made: BTreeSet<String>,
    /// The files and folders the run deleted, or renamed away.
    pub(super) removed: BTreeSet<String>,
    /// The files open, by descriptor.
    pub(super) open: BTreeMap<u64, Open>,
    /// The test's network.
    pub(super) network: Rc<net::Network>,
    /// The sockets open, by descriptor, with their connection's number once they have one.
    pub(super) sockets: BTreeMap<u64, Option<usize>>,
    /// Where each of the test's clients stands, once the program waits on its connections with
    /// poll.
    pub(super) peers: Option<Vec<net::Peer>>,
    /// The states the runs from one start have come back from poll in, after the clients took
    /// steps, shared by them all: one that comes to a state another already has goes no further.
    pub(super) polled: Rc<RefCell<HashSet<Vec<u64>>>>,
    /// The states this run came back from poll in.
    pub(super) history: Option<Rc<net::History>>,
    /// Every connection the run made or took, in order.
    pub connections: Vec<net::Connection>,
    /// The bytes a test supplies for `getrandom`, in order, when it supplies them: the kernel's
    /// are any value.
    pub(super) random: Option<Rc<Vec<Term>>>,
    /// How many of them the run has taken.
    pub(super) random_used: usize,
    /// The readings a test supplies for `clock_gettime`, seconds and nanoseconds, 64 bits each;
    /// the last one repeats. Empty, the kernel's are any time.
    pub(super) clock: Rc<Vec<(Term, Term)>>,
    /// How many of them the run has taken.
    pub(super) readings_used: usize,
    /// Whether the run read standard input or a connection with `read` itself rather than
    /// through fill, so how its input was cut matters.
    pub raw_reads: bool,
    /// Where fill stopped short of everything that had arrived, after an end byte: the cut a
    /// real run needs to stop there too.
    pub fill_cut: Option<usize>,
    /// Whether fill takes every way its input can be cut, as a proof does, or what one read
    /// gives, as a run of a test's sample does. Where it does, a read of standard input with
    /// at most [`RAW_CUT_BYTES`] left takes every count too.
    pub(super) every_cut: bool,
    /// Where this run's reads of standard input stopped short of what had arrived: bit `i`
    /// after byte `i + 1`.
    pub raw_cuts: u16,
    /// Where the run stood the first time it reached the read loop's head, in a proof by
    /// induction.
    pub(super) head: Option<Box<Arrival>>,
    /// Where it stood the second time, one buffer-full on.
    pub(super) second: Option<Box<Arrival>>,
    /// The conditions the inputs meet on this run.
    pub path: Vec<Term>,
    /// Accesses at addresses the input chooses that may leave their object: each the condition
    /// under which one does, and what it does then. The proof asks whether any can hold.
    pub obligations: Vec<(Term, String)>,
    /// The steps the run has taken: its machine's instructions.
    pub(super) steps: u64,
    /// How the run ended, once it has.
    pub end: Option<End>,
}

/// A file the program has open.
#[derive(Debug, Clone)]
pub(super) struct Open {
    /// The file's name in the working folder.
    pub(super) name: String,
    /// The next byte a read gets.
    pub(super) position: usize,
    /// Whether it is open for writing, rather than for reading.
    pub(super) writing: bool,
}

/// Where a run stood when it reached the read loop's head, in a proof by induction: what the
/// core's slots held, and how far it had read standard input and written its outputs.
#[derive(Debug, Clone)]
pub struct Arrival {
    /// What each of the core's slots held, by number.
    pub(super) slots: Vec<Term>,
    /// How much it had written to standard output.
    pub stdout: usize,
    /// How much to standard error.
    pub stderr: usize,
    /// How much of standard input it had read.
    pub stdin_read: usize,
}

impl Arrival {
    /// What `slot` held.
    #[must_use]
    pub fn held(&self, slot: Slot) -> Term {
        self.slots[slot.0]
    }
}

/// What the world's models act on, from the front end whose run calls them: the world itself,
/// the pool the run's terms are in, and the program's memory, held to the memory rules as the
/// front end holds the program's own accesses.
pub(super) trait Host {
    /// The run's world.
    fn world(&mut self) -> &mut World;

    /// The pool the run's terms are in.
    fn pool(&mut self) -> &mut Pool;

    /// Writes `value` little-endian at `address`.
    ///
    /// # Errors
    ///
    /// How the run ends where the program may not write there.
    fn save(&mut self, address: u64, value: Term) -> Result<(), Stop>;

    /// The `bytes` bytes little-endian at `address`.
    ///
    /// # Errors
    ///
    /// How the run ends where the program may not read there.
    fn load(&mut self, address: u64, bytes: u32) -> Result<Term, Stop>;
}

/// What a read comes to, as the world has it.
pub(super) enum Reading {
    /// Its result: the count it took, or minus an error number.
    Done(u64),
    /// Any count from 1 to this one, each a run of its own: a short read of standard input,
    /// where a proof takes every way the input can be cut ([`World::take_stdin`]).
    EveryCount(u64),
}

/// The ways a call on a name the input chooses can go ([`World::chosen`]).
pub(super) enum Chosen {
    /// Refused whole, with this error, whatever the name is.
    Refused(i64),
    /// Each way with the condition it goes so: the name is one of the test's files or folders,
    /// none of them (`None`), or a name some system refuses, with the error.
    Ways(Vec<(Term, Option<Result<String, i64>>)>),
}

impl World {
    /// The world as a run starts in it, with `stdin` on standard input: no files, no folders,
    /// no network, and the kernel's clock and random bytes.
    #[must_use]
    pub fn new(stdin: Vec<Term>) -> Self {
        Self {
            stdin: Rc::new(stdin),
            ..Self::default()
        }
    }

    /// What the file `name` holds after the run, if it exists: what the run wrote, or else what
    /// it held before.
    #[must_use]
    pub fn file(&self, name: &str) -> Option<&[Term]> {
        if let Some(written) = self.written.get(name) {
            return Some(written);
        }
        if self.removed.contains(name) {
            return None;
        }
        self.files.get(name).map(Vec::as_slice)
    }

    /// Whether `name` is a folder now: given or made, or holding a file or folder, and not
    /// removed since.
    #[must_use]
    pub fn is_folder(&self, name: &str) -> bool {
        if self.made.contains(name) {
            return true;
        }
        if self.removed.contains(name) {
            return false;
        }
        let inside = format!("{name}/");
        self.folders_before.contains(name)
            || self
                .folders_before
                .iter()
                .chain(&self.made)
                .any(|folder| folder.starts_with(&inside))
            || self
                .files
                .keys()
                .chain(self.written.keys())
                .any(|file| file.starts_with(&inside) && self.file(file).is_some())
    }

    /// The folders the run made that weren't there before it.
    #[must_use]
    pub fn folders_made(&self) -> Vec<&str> {
        self.made
            .iter()
            .filter(|folder| !self.folders_before.contains(*folder))
            .map(String::as_str)
            .collect()
    }

    /// Whether some system would refuse `name` for a file call, as lengua's broker does in a
    /// run (`crate::names`): the error number, when it would. A new name (`creates`) may not
    /// differ only in case or normalization from another in its folder, and a name a call
    /// renames or removes (`removes`) may not be open.
    #[must_use]
    pub fn refused(&self, name: &str, creates: bool, removes: bool) -> Option<i64> {
        if let Some(broken) = crate::names::broken(name.as_bytes()) {
            return Some(broken.errno);
        }
        if creates {
            let (folder, last) = name
                .rsplit_once('/')
                .map_or(("", name), |(folder, last)| (folder, last));
            let inside = if folder.is_empty() {
                String::new()
            } else {
                format!("{folder}/")
            };
            let clash = self
                .files
                .keys()
                .chain(self.written.keys())
                .chain(self.folders_before.iter())
                .chain(&self.made)
                .filter_map(|other| other.strip_prefix(&inside))
                .filter(|rest| !rest.contains('/'))
                .any(|rest| {
                    let full = format!("{inside}{rest}");
                    (self.file(&full).is_some() || self.is_folder(&full))
                        && crate::names::clashes(last, rest)
                });
            if clash {
                return Some(crate::names::EEXIST);
            }
        }
        if removes && self.open.values().any(|open| open.name == name) {
            return Some(crate::names::EBUSY);
        }
        None
    }

    /// Whether the folder holding `name` is there: the working folder, or a folder in it.
    fn parent_is_there(&self, name: &str) -> bool {
        name.rsplit_once('/')
            .is_none_or(|(parent, _)| self.is_folder(parent))
    }

    /// The files the working folder held before the run.
    #[must_use]
    pub fn files_before(&self) -> &BTreeMap<String, Vec<Term>> {
        &self.files
    }

    /// Standard input as the run reads it.
    #[must_use]
    pub fn stdin_bytes(&self) -> &[Term] {
        &self.stdin
    }

    /// How much of standard input the run has read.
    #[must_use]
    pub const fn stdin_taken(&self) -> usize {
        self.stdin_read
    }

    /// Where the run stood the first time it reached the read loop's head, once it has.
    #[must_use]
    pub fn at_head(&self) -> Option<&Arrival> {
        self.head.as_deref()
    }

    /// Where it stood the second time, one buffer-full on, once it has come back.
    #[must_use]
    pub fn at_second(&self) -> Option<&Arrival> {
        self.second.as_deref()
    }

    /// Whether this run and `other` have done the same in the world, so two runs of the same
    /// machine can be merged: neither has ended, and they have written the same output, read as
    /// much and taken as many of the test's bytes and readings, cut their input the same way,
    /// and written and opened as many files.
    pub(super) fn did_the_same(&self, other: &Self) -> bool {
        let same_stream = |x: &Rc<Vec<Term>>, y: &Rc<Vec<Term>>| Rc::ptr_eq(x, y) || x[..] == y[..];
        self.end.is_none()
            && other.end.is_none()
            && same_stream(&self.stdout, &other.stdout)
            && same_stream(&self.stderr, &other.stderr)
            && self.stdin_read == other.stdin_read
            && self.random_used == other.random_used
            && self.readings_used == other.readings_used
            && self.fill_cut == other.fill_cut
            && self.raw_cuts == other.raw_cuts
            && self.written.len() == other.written.len()
            && self.open.len() == other.open.len()
    }

    /// A read of at most `count` bytes from `fd` into `buffer`: standard input, a connection, or
    /// a file open for reading.
    ///
    /// # Errors
    ///
    /// How the run ends where the program may not write the bytes where it asked, or where the
    /// model doesn't follow the read.
    pub(super) fn read(
        host: &mut impl Host,
        fd: u64,
        buffer: u64,
        count: u64,
    ) -> Result<Reading, Stop> {
        if fd == 0 {
            let world = host.world();
            world.raw_reads = true;
            if world.stdin_length.is_some() {
                return Err(unsupported(
                    "standard input of a length the input chooses, read with read itself rather \
                     than fill",
                ));
            }
            // A pipe hands over at most 4096 bytes whole; past that, how the input arrives
            // matters, which only fill makes not matter. A test's sample is run as one read
            // gives it.
            if world.every_cut && world.stdin.len() > super::PIPE_WRITE {
                return Err(End::Unsupported(format!(
                    "standard input past {} bytes is more than one pipe write, which the proof \
                     reads whole; read it with fill to be proven for any length",
                    super::PIPE_WRITE
                )));
            }
            let available = (world.stdin.len() - world.stdin_read) as u64;
            // Short input read with read itself may arrive in any pieces: each count a read can
            // return is its own run.
            if world.every_cut
                && world.stdin_pieces.is_empty()
                && available <= RAW_CUT_BYTES
                && count.min(available) > 1
            {
                return Ok(Reading::EveryCount(count.min(available)));
            }
            let mut taken = count.min(available);
            if !world.stdin_pieces.is_empty() {
                let mut boundary = 0;
                for piece in world.stdin_pieces.iter() {
                    boundary += piece;
                    if boundary > world.stdin_read {
                        break;
                    }
                }
                taken = taken.min(boundary.saturating_sub(world.stdin_read) as u64);
            }
            let (stdin, read) = (Rc::clone(&world.stdin), world.stdin_read);
            for offset in 0..taken {
                host.save(buffer + offset, stdin[read + offset as usize])?;
            }
            host.world().stdin_read += taken as usize;
            return Ok(Reading::Done(taken));
        }
        if let Ok(socket) = host.world().connection_of(fd) {
            host.world().raw_reads = true;
            let result = match socket {
                Some(number) => Self::receive(host, number, buffer, count)?,
                None => Self::not_connected(),
            };
            return Ok(Reading::Done(result));
        }
        let world = host.world();
        if let Some(open) = world.open.get(&fd).cloned()
            && !open.writing
        {
            // A regular file gives everything asked for, up to its end.
            let files = Rc::clone(&world.files);
            let contents = files.get(&open.name).map_or(&[][..], Vec::as_slice);
            let available = (contents.len() - open.position.min(contents.len())) as u64;
            let taken = count.min(available);
            for offset in 0..taken {
                let byte = contents[open.position + offset as usize];
                host.save(buffer + offset, byte)?;
            }
            if let Some(open) = host.world().open.get_mut(&fd) {
                open.position += taken as usize;
            }
            return Ok(Reading::Done(taken));
        }
        Ok(Reading::Done((-EBADF).cast_unsigned()))
    }

    /// One of the ways a short read of standard input goes ([`Reading::EveryCount`]): `taken`
    /// bytes of it put at `buffer`, noting where the read stopped short of what had arrived.
    ///
    /// # Errors
    ///
    /// How the run ends where the program may not write the bytes there.
    pub(super) fn take_stdin(host: &mut impl Host, buffer: u64, taken: u64) -> Result<(), Stop> {
        let world = host.world();
        let (stdin, read) = (Rc::clone(&world.stdin), world.stdin_read);
        for offset in 0..taken {
            host.save(buffer + offset, stdin[read + offset as usize])?;
        }
        let world = host.world();
        world.stdin_read += taken as usize;
        if world.stdin_read < world.stdin.len() && world.stdin_read <= 16 {
            world.raw_cuts |= 1 << (world.stdin_read - 1);
        }
        Ok(())
    }

    /// A write of the `count` bytes at `buffer` to standard output or error, or to a file open
    /// for writing: the count, or minus the error number where `fd` is none of them. A closed
    /// standard output takes nothing: the write fails, as on a pipe nobody reads.
    ///
    /// # Errors
    ///
    /// How the run ends where the program may not read the bytes there.
    pub(super) fn write(
        host: &mut impl Host,
        fd: u64,
        buffer: u64,
        count: u64,
    ) -> Result<u64, Stop> {
        let world = host.world();
        let target = match fd {
            1 if world.stdout_closed => Err(EPIPE),
            1 | 2 => Ok(None),
            _ => match world.open.get(&fd) {
                Some(open) if open.writing => Ok(Some(open.name.clone())),
                _ => Err(EBADF),
            },
        };
        let file = match target {
            Ok(file) => file,
            Err(errno) => return Ok((-errno).cast_unsigned()),
        };
        for offset in 0..count {
            let byte = host.load(buffer + offset, 1)?;
            let world = host.world();
            match (&file, fd) {
                (Some(name), _) => world.written.entry(name.clone()).or_default().push(byte),
                (None, 1) => Rc::make_mut(&mut world.stdout).push(byte),
                (None, _) => Rc::make_mut(&mut world.stderr).push(byte),
            }
        }
        Ok(count)
    }

    /// `close(fd)`: a file or a socket closed, or minus the error number where `fd` is neither.
    pub(super) fn close(&mut self, fd: u64) -> u64 {
        if self.open.remove(&fd).is_some() || self.close_socket(fd) {
            0
        } else {
            (-EBADF).cast_unsigned()
        }
    }

    /// The file `name` opened, for writing (made, or emptied where `emptying`) or for reading,
    /// as the working folder stands: its descriptor, or minus the error number.
    ///
    /// # Errors
    ///
    /// Where the model doesn't follow the file: one written into without emptying it first, or
    /// read after the program wrote it.
    pub(super) fn open(
        &mut self,
        name: String,
        writing: bool,
        emptying: bool,
    ) -> Result<u64, Stop> {
        if self.is_folder(&name) {
            return Ok((-EISDIR).cast_unsigned());
        }
        if writing {
            if !self.parent_is_there(&name) {
                return Ok((-ENOENT).cast_unsigned());
            }
            let exists = self.file(&name).is_some();
            if exists && !emptying {
                return Err(unsupported("writing into a file without emptying it first"));
            }
            self.written.insert(name.clone(), Vec::new());
        } else if self.written.contains_key(&name) {
            return Err(unsupported("reading a file the program wrote"));
        } else if self.file(&name).is_none() {
            return Ok((-ENOENT).cast_unsigned());
        }
        let fd = self.free_descriptor();
        self.open.insert(
            fd,
            Open {
                name,
                position: 0,
                writing,
            },
        );
        Ok(fd)
    }

    /// The size of the file open at `fd`, when one is: what it holds now.
    pub(super) fn size_of_open(&self, fd: u64) -> Option<usize> {
        let open = self.open.get(&fd)?;
        Some(self.file(&open.name).map_or(0, <[Term]>::len))
    }

    /// The ways a call on a name the input chooses, `bytes`, can go: one for each file or folder
    /// of the test's the name can be, with its bytes those on the run's path, and one where it's
    /// none of them, refused where some system refuses the name; or refused whole, past every
    /// system's length. Where it's none of them, the name must be one the working folder could
    /// hold, never `/`, `.` or `..` in it, or the kernel's answer isn't the model's.
    ///
    /// # Errors
    ///
    /// Where the name may leave the working folder, or may hold characters past ASCII.
    pub(super) fn chosen(&self, pool: &mut Pool, bytes: &[Term]) -> Result<Chosen, Stop> {
        let mut names: Vec<String> = self
            .files
            .keys()
            .chain(self.written.keys())
            .chain(self.folders_before.iter())
            .chain(&self.made)
            .filter(|name| name.len() == bytes.len())
            .cloned()
            .collect();
        names.sort_unstable();
        names.dedup();
        let mut outcomes = Vec::new();
        let mut none = pool.boolean(true);
        for name in names {
            let mut same = pool.boolean(true);
            for (byte, wanted) in bytes.iter().zip(name.bytes()) {
                let wanted = pool.bv(8, u128::from(wanted));
                let equal = pool.eq(*byte, wanted);
                same = pool.bool_and(same, equal);
            }
            let other = pool.bool_not(same);
            none = pool.bool_and(none, other);
            outcomes.push((same, Some(name)));
        }
        let may = |pool: &Pool, byte: &Term, value: u8| {
            let (low, high) = pool.range(*byte);
            (low..=high).contains(&u128::from(value))
        };
        let seen: &Pool = pool;
        let strange = bytes.iter().any(|byte| may(seen, byte, b'/'))
            || (bytes.len() <= 2 && bytes.iter().all(|byte| may(seen, byte, b'.')));
        if strange && pool.truth(none) != Some(false) {
            return Err(unsupported(
                "a file name the input chooses that may leave the working folder",
            ));
        }
        // Where it's none of the test's, the name is held to every system's as a run holds it:
        // past every system's length, refused whole; Windows's ASCII rules, a split. A byte
        // past ASCII would need UTF-8's and Unicode's rules, which only a run checks.
        if pool.truth(none) != Some(false) {
            if bytes.len() > crate::names::MAX_NAME {
                return Ok(Chosen::Refused(crate::names::ENAMETOOLONG));
            }
            if bytes.iter().any(|byte| pool.range(*byte).1 >= 0x80) {
                return Err(unsupported(
                    "a file name the input chooses that may hold characters past ASCII, which \
                     lengua's naming rules check when it runs",
                ));
            }
        }
        let broken = breaks_windows_rules(pool, bytes);
        let fine = pool.bool_not(broken);
        let missing = pool.bool_and(none, fine);
        let refused = pool.bool_and(none, broken);
        outcomes.push((missing, None));
        let ways = outcomes
            .into_iter()
            .map(|(condition, name)| (condition, name.map(Ok)))
            .chain([(refused, Some(Err(crate::names::EINVAL)))])
            .collect();
        Ok(Chosen::Ways(ways))
    }

    /// `mkdirat(folder, name, mode)`: a new folder, where its parent is.
    pub(super) fn make_folder(&mut self, name: String) -> u64 {
        if self.is_folder(&name) || self.file(&name).is_some() {
            return (-EEXIST).cast_unsigned();
        }
        if !self.parent_is_there(&name) {
            return (-ENOENT).cast_unsigned();
        }
        self.removed.remove(&name);
        self.made.insert(name);
        0
    }

    /// `unlinkat(folder, name, flags)`: a file deleted, or with `AT_REMOVEDIR`, an empty folder.
    ///
    /// # Errors
    ///
    /// Flags the model doesn't follow.
    pub(super) fn remove(&mut self, name: String, flags: u64) -> Result<u64, Stop> {
        let inside = format!("{name}/");
        let errno =
            match flags {
                0 if self.file(&name).is_some() => {
                    self.written.remove(&name);
                    self.removed.insert(name);
                    return Ok(0);
                }
                0 if self.is_folder(&name) => EISDIR,
                AT_REMOVEDIR if self.is_folder(&name) => {
                    let holds =
                        self.files
                            .keys()
                            .chain(self.written.keys())
                            .any(|file| file.starts_with(&inside) && self.file(file).is_some())
                            || self.folders_before.iter().chain(&self.made).any(|folder| {
                                folder.starts_with(&inside) && self.is_folder(folder)
                            });
                    if holds {
                        ENOTEMPTY
                    } else {
                        self.made.remove(&name);
                        self.removed.insert(name);
                        return Ok(0);
                    }
                }
                AT_REMOVEDIR if self.file(&name).is_some() => ENOTDIR,
                0 | AT_REMOVEDIR => ENOENT,
                other => return Err(unsupported(format!("unlinkat with flags {other:#x}"))),
            };
        Ok((-errno).cast_unsigned())
    }

    /// `renameat(folder, from, folder, to)`: a file moved to a name, replacing what's there.
    ///
    /// # Errors
    ///
    /// A folder renamed, which the model doesn't follow.
    pub(super) fn rename(&mut self, from: String, to: String) -> Result<u64, Stop> {
        if self.is_folder(&from) {
            return Err(unsupported("renaming a folder"));
        }
        let Some(contents) = self.file(&from).map(<[Term]>::to_vec) else {
            return Ok((-ENOENT).cast_unsigned());
        };
        if self.is_folder(&to) {
            return Ok((-EISDIR).cast_unsigned());
        }
        if !self.parent_is_there(&to) {
            return Ok((-ENOENT).cast_unsigned());
        }
        if from != to {
            self.written.remove(&from);
            self.removed.insert(from);
            self.removed.remove(&to);
            self.written.insert(to, contents);
        }
        Ok(0)
    }

    /// A reading of the clock put at `buffer`, seconds then nanoseconds, 64 bits each: the
    /// test's, as the broker gives them, the last one repeating; or, where the test supplies
    /// none, any time the kernel could give, a fresh value for each.
    ///
    /// # Errors
    ///
    /// How the run ends where the program may not write the reading there.
    pub(super) fn read_clock(host: &mut impl Host, buffer: u64) -> Result<(), Stop> {
        let readings = Rc::clone(&host.world().clock);
        if !readings.is_empty() {
            let index = host.world().readings_used.min(readings.len() - 1);
            let (seconds, nanoseconds) = readings[index];
            host.save(buffer, seconds)?;
            host.save(buffer + 8, nanoseconds)?;
            host.world().readings_used += 1;
            return Ok(());
        }
        let pool = host.pool();
        let fresh = pool.len();
        let seconds = pool.var_in(64, &format!("clock{fresh}.seconds"), 0, (1 << 62) - 1);
        let nanoseconds = pool.var_in(64, &format!("clock{fresh}.nanoseconds"), 0, 999_999_999);
        host.save(buffer, seconds)?;
        host.save(buffer + 8, nanoseconds)
    }

    /// `count` random bytes put at `buffer`: the test's, as the broker gives them, zeros past
    /// their end; or, where the test supplies none, any bytes the kernel could give, a fresh
    /// value for each. The count, as `getrandom` gives it.
    ///
    /// # Errors
    ///
    /// How the run ends where the program may not write the bytes there, or asks for more at
    /// once than the model follows.
    pub(super) fn random(host: &mut impl Host, buffer: u64, count: u64) -> Result<u64, Stop> {
        if let Some(bytes) = host.world().random.clone() {
            if count > 4096 {
                return Err(unsupported("getrandom for more than 4096 bytes at once"));
            }
            let used = host.world().random_used;
            for offset in 0..count {
                let index = used + usize::try_from(offset).unwrap_or(usize::MAX);
                let byte = match bytes.get(index) {
                    Some(byte) => *byte,
                    None => host.pool().bv(8, 0),
                };
                host.save(buffer + offset, byte)?;
            }
            host.world().random_used += usize::try_from(count).unwrap_or(usize::MAX);
            return Ok(count);
        }
        if count > 256 {
            return Err(unsupported("getrandom for more than 256 bytes at once"));
        }
        let fresh = host.pool().len();
        for offset in 0..count {
            let byte = host.pool().var(8, &format!("random{fresh}.{offset}"));
            host.save(buffer + offset, byte)?;
        }
        Ok(count)
    }
}

/// Whether a name of these ASCII bytes breaks one of Windows's rules (`crate::names`): a
/// control character or one of `< > : " \ | ? *`, a dot or space at the end, a device name
/// Windows reserves, with any extension, in any case, or a short name's shape.
fn breaks_windows_rules(pool: &mut Pool, bytes: &[Term]) -> Term {
    let mut broken = Vec::new();
    let space = pool.bv(8, 0x20);
    for byte in bytes {
        broken.push(pool.ult(*byte, space));
        for refused in *b"<>:\"\\|?*" {
            let value = pool.bv(8, u128::from(refused));
            broken.push(pool.eq(*byte, value));
        }
    }
    if let Some(last) = bytes.last() {
        for ending in *b". " {
            let value = pool.bv(8, u128::from(ending));
            broken.push(pool.eq(*last, value));
        }
    }
    // Whether the bytes from `at` spell `word` in any case.
    let spells = |pool: &mut Pool, at: usize, word: &[u8]| -> Term {
        let mut letters = Vec::new();
        for (offset, wanted) in word.iter().enumerate() {
            let byte = bytes[at + offset];
            let upper = pool.bv(8, u128::from(*wanted));
            let lower = pool.bv(8, u128::from(wanted.to_ascii_lowercase()));
            let as_upper = pool.eq(byte, upper);
            let as_lower = pool.eq(byte, lower);
            letters.push(pool.bool_or(as_upper, as_lower));
        }
        pool.all(&letters)
    };
    // A stem of `stem` letters, alone or before a dot.
    let ends_stem = |pool: &mut Pool, stem: usize| -> Option<Term> {
        match bytes.len() {
            length if length == stem => Some(pool.boolean(true)),
            length if length > stem => {
                let dot = pool.bv(8, u128::from(b'.'));
                Some(pool.eq(bytes[stem], dot))
            }
            _ => None,
        }
    };
    if let Some(ended) = ends_stem(pool, 3) {
        for device in [b"CON", b"PRN", b"AUX", b"NUL"] {
            let named = spells(pool, 0, device);
            broken.push(pool.bool_and(named, ended));
        }
    }
    if let Some(ended) = ends_stem(pool, 4) {
        let one = pool.bv(8, u128::from(b'1'));
        let nine = pool.bv(8, u128::from(b'9'));
        let low = pool.ule(one, bytes[3]);
        let high = pool.ule(bytes[3], nine);
        let digit = pool.bool_and(low, high);
        for device in [b"COM", b"LPT"] {
            let named = spells(pool, 0, device);
            let numbered = pool.bool_and(named, digit);
            broken.push(pool.bool_and(numbered, ended));
        }
    }
    broken.push(short_name_shape(pool, bytes));
    pool.any(&broken)
}

/// Whether a name of these ASCII bytes has the shape of a Windows short name
/// (`crate::names::short_name_shape`): a stem of at most eight ending in `~` and digits,
/// then, if any, one dot and one to three more. The name's length is known, so each place
/// its dot and its `~` can be is a case.
fn short_name_shape(pool: &mut Pool, bytes: &[Term]) -> Term {
    let length = bytes.len();
    if length > crate::names::SHORT_STEM + 1 + crate::names::SHORT_EXTENSION {
        return pool.boolean(false);
    }
    let dot = pool.bv(8, u128::from(b'.'));
    let tilde = pool.bv(8, u128::from(b'~'));
    let zero = pool.bv(8, u128::from(b'0'));
    let nine = pool.bv(8, u128::from(b'9'));
    let mut cases = Vec::new();
    // Where the stem ends: at the name's end, with no dot, or at its one dot.
    for stem in 1..=length.min(crate::names::SHORT_STEM) {
        let extension = length - stem;
        if extension == 1 || extension > crate::names::SHORT_EXTENSION + 1 {
            continue;
        }
        let mut dots = Vec::new();
        for (at, byte) in bytes.iter().enumerate() {
            let is_dot = pool.eq(*byte, dot);
            dots.push(if at == stem {
                is_dot
            } else {
                pool.bool_not(is_dot)
            });
        }
        let mut tildes = Vec::new();
        for at in 0..stem.saturating_sub(1) {
            let mut parts = vec![pool.eq(bytes[at], tilde)];
            for byte in &bytes[at + 1..stem] {
                let low = pool.ule(zero, *byte);
                let high = pool.ule(*byte, nine);
                parts.push(pool.bool_and(low, high));
            }
            tildes.push(pool.all(&parts));
        }
        let separated = pool.all(&dots);
        let ended = pool.any(&tildes);
        cases.push(pool.bool_and(separated, ended));
    }
    pool.any(&cases)
}

#[cfg(test)]
mod tests {
    use super::short_name_shape;
    use crate::prove::term::{Pool, Term};

    /// The proof's short-name shape, on names whose bytes are known, gives the run's answer
    /// (`crate::names::short_name_shape`) for every name of up to thirteen bytes it's tried on.
    #[test]
    fn the_short_name_shape_the_proof_tests_is_the_runs() {
        let mut names = vec![
            "REPORT~1.TXT",
            "progra~1",
            "a~12",
            "~1",
            "AB12~1.tx",
            "backup~.txt",
            "notes~old.txt",
            "a~1.text",
            "a~1.b.c",
            "a1~",
            "report1.txt",
            "ABCDEFG~1.T",
            "ABCDEF~1.TXT",
            "A~1.",
            "A~1.B",
            ".txt",
            "~",
            "~~1",
            "a~1~2",
            "x.y~1",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect::<Vec<_>>();
        // Every arrangement of a few telling bytes, up to five long.
        let alphabet = *b"a~1.";
        for length in 1..=5 {
            for mut code in 0..alphabet.len().pow(length) {
                let mut name = String::new();
                for _ in 0..length {
                    name.push(char::from(alphabet[code % alphabet.len()]));
                    code /= alphabet.len();
                }
                names.push(name);
            }
        }
        for name in names {
            let mut pool = Pool::default();
            let bytes: Vec<Term> = name
                .bytes()
                .map(|byte| pool.bv(8, u128::from(byte)))
                .collect();
            let shape = short_name_shape(&mut pool, &bytes);
            assert_eq!(
                pool.truth(shape),
                Some(crate::names::short_name_shape(&name)),
                "{name}"
            );
        }
    }
}
