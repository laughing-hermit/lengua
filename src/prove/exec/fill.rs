//! lengua's own `fill`, which a program calls to fill a buffer from an input, read as a whole: at
//! its loop's head the run takes, at once, what every possible sequence of reads leaves, for every
//! way the input can be cut. A read can end after any byte, so fill can stop after any end byte
//! that has arrived, or else when its buffer is full or the input has ended.

use super::induction::Slot;
use super::{Cpu, End, Step, Stop};
use crate::isa::Isa;
use crate::prove::term::Term;

/// Where a CPU's fill keeps what it works with at its loop's head, as the core's slots, which
/// are the prover's registers: x86-64's in rdi, rsi, rdx, r10 and r9, AArch64's in x9, x1, x2,
/// w11 and x12.
#[derive(Debug)]
pub(super) struct FillAbi {
    /// The file descriptor it reads.
    pub(super) fd: Slot,
    /// Where the next read goes.
    pub(super) cursor: Slot,
    /// How much room is left.
    pub(super) space: Slot,
    /// The end byte, or -1 for none.
    pub(super) end: Slot,
    /// How many reads it has left.
    pub(super) counter: Slot,
}

impl FillAbi {
    /// `isa`'s.
    pub(super) const fn of(isa: Isa) -> &'static Self {
        match isa {
            Isa::X86_64 => &Self {
                fd: Slot(7),
                cursor: Slot(6),
                space: Slot(2),
                end: Slot(10),
                counter: Slot(9),
            },
            Isa::Aarch64 => &Self {
                fd: Slot(9),
                cursor: Slot(1),
                space: Slot(2),
                end: Slot(11),
                counter: Slot(12),
            },
        }
    }
}

/// Where fill's code is, found by the shape of lengua's own `fill`.
#[derive(Debug, Clone, Copy)]
pub struct Fill {
    /// The loop's head, where its read starts.
    head: u64,
    /// The instruction after its read, which the model leaves in rcx as a system call does.
    after_call: u64,
    /// `.Llengua_fill_done`, where it works out how much it read.
    done: u64,
}

/// A fill's instructions, by mnemonic on x86-64 and by the decoder's kind on AArch64, through
/// its `ret`; and which of them are the loop's head, the one after its read and
/// `.Llengua_fill_done`.
struct Shape {
    names: &'static [&'static str],
    head: usize,
    after_call: usize,
    done: usize,
}

impl Shape {
    /// The places in a fill of this shape, given where each of its instructions is.
    fn at(&self, address: impl Fn(usize) -> u64) -> Fill {
        Fill {
            head: address(self.head),
            after_call: address(self.after_call),
            done: address(self.done),
        }
    }
}

/// x86-64's fill: it reads with a `call os_read`, and has a frame of its own to keep the stack
/// aligned for it.
const X86_64: Shape = Shape {
    names: &[
        "sub", "test", "je", "test", "je", "mov", "mov", "mov", "call", "test", "js", "je", "cmp",
        "ja", "add", "sub", "je", "movzx", "cmp", "je", "dec", "jne", "mov", "jmp", "mov", "sub",
        "add", "ret",
    ],
    head: 8,
    after_call: 9,
    done: 24,
};

/// AArch64's fill, as the decoder reads it: it reads with a `bl os_read` and saves the link
/// register.
const AARCH64: Shape = Shape {
    names: &[
        "Pair",
        "CompareBranch",
        "CompareBranch",
        "LogicalShifted",
        "LogicalShifted",
        "LogicalShifted",
        "MoveWide",
        "LogicalShifted",
        "Branch",
        "TestBranch",
        "CompareBranch",
        "AddShifted",
        "BranchCond",
        "AddShifted",
        "AddShifted",
        "BranchCond",
        "LoadStore",
        "AddShifted",
        "BranchCond",
        "AddImm",
        "BranchCond",
        "MoveWide",
        "Branch",
        "AddShifted",
        "Pair",
        "BranchReg",
    ],
    head: 7,
    after_call: 9,
    done: 23,
};

impl Fill {
    /// fill's places, when the program has a function named fill of the library's shape.
    pub fn find(machine: &crate::checker::decode::Machine) -> Option<Self> {
        let function = machine.functions.get("fill")?;
        let words = function.words();
        if !words.is_empty() {
            let words = words.get(..AARCH64.names.len())?;
            return words
                .iter()
                .zip(AARCH64.names)
                .all(|(word, kind)| {
                    word.inst.as_ref().is_ok_and(|inst| {
                        let name = format!("{inst:?}");
                        name.split(|character: char| !character.is_alphanumeric())
                            .next()
                            == Some(*kind)
                    })
                })
                .then(|| AARCH64.at(|index| words[index].address));
        }
        let instructions = function.x86().get(..X86_64.names.len())?;
        instructions
            .iter()
            .zip(X86_64.names)
            .all(|(instruction, mnemonic)| {
                format!("{:?}", instruction.mnemonic()).to_ascii_lowercase() == *mnemonic
            })
            .then(|| X86_64.at(|index| instructions[index].ip()))
    }

    /// Whether `address` is the loop's head.
    pub fn at_head(self, address: u64) -> bool {
        address == self.head
    }

    /// The loop's head.
    pub const fn head(self) -> u64 {
        self.head
    }
}

/// Where the bytes fill reads come from.
#[derive(Clone, Copy)]
enum Stream {
    /// Standard input.
    Stdin,
    /// A file open for reading, by descriptor.
    File(u64),
    /// A connection, by number.
    Connection(usize),
}

/// What comes after everything the stream holds.
enum After {
    /// The input ends: a read returns 0.
    End,
    /// The peer waits with its side open: a read waits forever, for this reason.
    Wait(String),
}

impl Cpu<'_> {
    /// At fill's loop head: every way its reads can go, each its own run, or `None` to run the
    /// loop as code, as for a read that fails.
    pub(super) fn fill(&mut self, fill: Fill) -> Result<Option<Step>, Stop> {
        let registers = self.core.registers;
        let roles = FillAbi::of(self.image.isa);
        let constant = |pool: &crate::prove::term::Pool, term: Term| {
            pool.constant(term)
                .and_then(|value| u64::try_from(value).ok())
        };
        let (Some(fd), Some(cursor), Some(space), Some(end)) = (
            constant(self.pool, registers[roles.fd.0]),
            constant(self.pool, registers[roles.cursor.0]),
            constant(self.pool, registers[roles.space.0]),
            constant(self.pool, registers[roles.end.0]),
        ) else {
            return Ok(None);
        };
        let (fd, end) = (fd & 0xffff_ffff, end & 0xffff_ffff);
        if space == 0 || space > 1 << 32 {
            return Ok(None);
        }
        // Its reads may fill all the space it's given, whatever arrives.
        // A read's second argument, as fill's own cursor is.
        self.lent(1, cursor, space, true, "read")?;
        let end = u8::try_from(end).ok();
        let Some((stream, mut available, after)) = self.stream(fd)? else {
            return Ok(None);
        };
        // Standard input of a length the input chooses, none of it read yet and all of it room
        // for the buffer: one read takes all of it, however it's cut, its count that length.
        // Its bytes past that length go in too, standing for whatever the buffer held, which is
        // anything. What comes before it, as the buffer-full an induction's base puts first, is
        // read as any input of its own length is.
        if let (Stream::Stdin, Some(length)) = (stream, self.world.stdin_length)
            && self.world.stdin_read < self.world.stdin.len()
        {
            let from = self.world.stdin_from;
            if self.world.stdin_read < from {
                available.truncate(from - self.world.stdin_read);
            } else {
                let room = usize::try_from(space).unwrap_or(usize::MAX);
                if end.is_some() || self.world.stdin_read > from || room < available.len() {
                    return Err(super::unsupported(
                        "standard input of a length the input chooses read other than all at \
                         once",
                    ));
                }
                self.take_all(fill, &available, cursor, space, length)?;
                return Ok(Some(Step::Next));
            }
        }
        let last = usize::try_from(space)
            .unwrap_or(usize::MAX)
            .min(available.len());
        let mut outcomes: Vec<(Term, usize, bool)> = Vec::new();
        // A read of standard input or a connection can end right after any end byte that has
        // arrived: fill stops there. A file's read gives everything asked for.
        if let Some(end) = end
            && self.world.every_cut
            && !matches!(stream, Stream::File(_))
        {
            let wanted = self.pool.bv(8, u128::from(end));
            for taken in 1..last {
                let stops = self.pool.eq(available[taken - 1], wanted);
                if self.pool.truth(stops) != Some(false) {
                    outcomes.push((stops, taken, false));
                }
            }
        }
        // Otherwise it reads on until the buffer is full or nothing is left.
        let yes = self.pool.boolean(true);
        if last == usize::try_from(space).unwrap_or(usize::MAX) {
            outcomes.push((yes, last, false));
        } else {
            match end {
                Some(end) if last > 0 => {
                    let wanted = self.pool.bv(8, u128::from(end));
                    let stops = self.pool.eq(available[last - 1], wanted);
                    let reads_on = self.pool.bool_not(stops);
                    outcomes.push((stops, last, false));
                    outcomes.push((reads_on, last, true));
                }
                _ => outcomes.push((yes, last, true)),
            }
        }
        let before = match stream {
            Stream::Stdin => self.world.stdin_read,
            Stream::Connection(number) => self.world.consumed(number),
            Stream::File(_) => 0,
        };
        let original = self.fork();
        let mut choices = Vec::new();
        for (condition, taken, at_end) in outcomes {
            if self.pool.truth(condition) == Some(false) {
                continue;
            }
            let mut outcome = original.clone();
            self.swap(&mut outcome);
            let applied = self.take(fill, &stream, &available, cursor, space, taken);
            if let Err(stop) = applied {
                self.world.end = Some(stop);
            } else if at_end && let After::Wait(why) = &after {
                self.world.end = Some(End::Stuck(why.clone()));
            }
            if taken < last {
                self.world.fill_cut.get_or_insert(before + taken);
            }
            self.swap(&mut outcome);
            choices.push((condition, outcome));
        }
        Ok(Some(Step::Choose(choices)))
    }

    /// The stream `fd` reads from: what it holds, and what follows. `None` for a descriptor a
    /// read fails on, which the loop then runs as code.
    fn stream(&mut self, fd: u64) -> Result<Option<(Stream, Vec<Term>, After)>, Stop> {
        if fd == 0 {
            let left = self.world.stdin[self.world.stdin_read..].to_vec();
            return Ok(Some((Stream::Stdin, left, After::End)));
        }
        if let Ok(socket) = self.world.connection_of(fd) {
            let Some(number) = socket else {
                return Ok(None);
            };
            let (left, waits) = self.world.unread(number)?;
            let after = if waits {
                After::Wait(
                    "fill reads to the end of the test's client's request, but the client, as \
                     curl and browsers do, keeps the connection open while it waits for the \
                     answer: give fill the byte that ends the request, such as a newline"
                        .into(),
                )
            } else {
                After::End
            };
            return Ok(Some((Stream::Connection(number), left, after)));
        }
        let Some(open) = self.world.open.get(&fd).filter(|open| !open.writing) else {
            return Ok(None);
        };
        let files = std::rc::Rc::clone(&self.world.files);
        let contents = files.get(&open.name).map_or(&[][..], Vec::as_slice);
        let left = contents[open.position.min(contents.len())..].to_vec();
        Ok(Some((Stream::File(fd), left, After::End)))
    }

    /// fill having read `taken` bytes of `available` into the buffer at `cursor`, which has
    /// `space` bytes left: the bytes in place, the stream on, and the run at
    /// `.Llengua_fill_done`.
    fn take(
        &mut self,
        fill: Fill,
        stream: &Stream,
        available: &[Term],
        cursor: u64,
        space: u64,
        taken: usize,
    ) -> Result<(), Stop> {
        for (offset, byte) in available[..taken].iter().enumerate() {
            self.save(cursor + offset as u64, *byte)?;
        }
        match stream {
            Stream::Stdin => self.world.stdin_read += taken,
            Stream::File(fd) => {
                if let Some(open) = self.world.open.get_mut(fd) {
                    open.position += taken;
                }
            }
            Stream::Connection(number) => self.world.consume(*number, taken),
        }
        let taken = taken as u64;
        let roles = FillAbi::of(self.image.isa);
        self.core.registers[roles.cursor.0] = self.pool.bv(64, u128::from(cursor + taken));
        self.core.registers[roles.space.0] = self.pool.bv(64, u128::from(space - taken));
        // What the reads leave: on x86-64 the kernel's rcx and r11, on AArch64 the SVE state the
        // kernel clears, and a loop count that depends on how the input was cut.
        if self.image.isa == Isa::X86_64 {
            self.core.registers[1] = self.pool.bv(64, u128::from(fill.after_call));
            self.core.registers[11] = self.pool.bv(64, 0x202);
        }
        self.core.clear_sve(self.pool);
        let count = self.pool.len();
        self.core.registers[roles.counter.0] = self.pool.var(64, &format!("fill{count}.reads"));
        self.core.rip = fill.done;
        Ok(())
    }
}

impl Cpu<'_> {
    /// fill having read all of standard input, `length` of its bytes, a length the input
    /// chooses, into the buffer at `cursor` with `space` bytes: every byte `available` holds in
    /// place, standard input read to its end, and the run at `.Llengua_fill_done`.
    fn take_all(
        &mut self,
        fill: Fill,
        available: &[Term],
        cursor: u64,
        space: u64,
        length: Term,
    ) -> Result<(), Stop> {
        for (offset, byte) in available.iter().enumerate() {
            self.save(cursor + offset as u64, *byte)?;
        }
        self.world.stdin_read = self.world.stdin.len();
        let roles = FillAbi::of(self.image.isa);
        let start = self.pool.bv(64, u128::from(cursor));
        self.core.registers[roles.cursor.0] = self.pool.add(start, length);
        let room = self.pool.bv(64, u128::from(space));
        self.core.registers[roles.space.0] = self.pool.sub(room, length);
        if self.image.isa == Isa::X86_64 {
            self.core.registers[1] = self.pool.bv(64, u128::from(fill.after_call));
            self.core.registers[11] = self.pool.bv(64, 0x202);
        }
        self.core.clear_sve(self.pool);
        let count = self.pool.len();
        self.core.registers[roles.counter.0] = self.pool.var(64, &format!("fill{count}.reads"));
        self.core.rip = fill.done;
        Ok(())
    }
}
