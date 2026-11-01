//! The canonical `fill`, which check holds word for word to assets/fill.s, read as a whole: at
//! its loop's head the run takes, at once, what every possible sequence of reads leaves, for
//! every way the input can be cut. A read can end after any byte, so fill can stop after any end
//! byte that has arrived, or else when its buffer is full or the input has ended.

use super::{Cpu, End, Image, State, Step, Stop};
use crate::isa::Isa;
use crate::prove::term::Term;

/// Where a CPU's fill keeps what it works with at its loop's head, as the slots of the prover's
/// registers: x86-64's in rdi, rsi, rdx, r10 and r9, AArch64's in x9, x1, x2, w11 and x12.
#[derive(Debug)]
pub(super) struct FillAbi {
    /// The file descriptor it reads.
    pub(super) fd: usize,
    /// Where the next read goes.
    pub(super) cursor: usize,
    /// How much room is left.
    pub(super) space: usize,
    /// The end byte, or -1 for none.
    pub(super) end: usize,
    /// How many reads it has left.
    pub(super) counter: usize,
}

impl FillAbi {
    /// `isa`'s.
    pub(super) const fn of(isa: Isa) -> &'static Self {
        match isa {
            Isa::X86_64 => &Self {
                fd: 7,
                cursor: 6,
                space: 2,
                end: 10,
                counter: 9,
            },
            Isa::Aarch64 => &Self {
                fd: 9,
                cursor: 1,
                space: 2,
                end: 11,
                counter: 12,
            },
        }
    }
}

/// Where fill's code is, found by its canonical shape.
#[derive(Debug, Clone, Copy)]
pub struct Fill {
    /// The loop's head, `mov eax, SYS_READ`.
    head: u64,
    /// The instruction after its `syscall`, which the kernel leaves in rcx.
    after_call: u64,
    /// `.Lfill_done`, where it works out how much it read.
    done: u64,
}

/// fill's instructions, by mnemonic, as assets/fill.s assembles them.
const SHAPE: [&str; 27] = [
    "test", "je", "test", "je", "mov", "mov", "mov", "mov", "syscall", "test", "js", "je", "cmp",
    "ja", "add", "sub", "je", "movzx", "cmp", "je", "dec", "jne", "mov", "jmp", "mov", "sub",
    "ret",
];

impl Fill {
    /// fill's places in `image`, when it has a function named fill of the canonical shape.
    pub fn find(image: &Image, machine: &crate::checker::decode::Machine) -> Option<Self> {
        let function = machine.functions.get("fill")?;
        let instructions = function.x86();
        if instructions.len() < SHAPE.len() {
            return None;
        }
        let same = instructions
            .iter()
            .zip(SHAPE)
            .all(|(instruction, mnemonic)| {
                format!("{:?}", instruction.mnemonic()).to_ascii_lowercase() == mnemonic
            });
        let _ = image;
        same.then(|| Self {
            head: instructions[7].ip(),
            after_call: instructions[9].ip(),
            done: instructions[24].ip(),
        })
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
        let registers = self.state.registers;
        let roles = FillAbi::of(self.image.isa);
        let constant = |pool: &crate::prove::term::Pool, term: Term| {
            pool.constant(term)
                .and_then(|value| u64::try_from(value).ok())
        };
        let (Some(fd), Some(cursor), Some(space), Some(end)) = (
            constant(self.pool, registers[roles.fd]),
            constant(self.pool, registers[roles.cursor]),
            constant(self.pool, registers[roles.space]),
            constant(self.pool, registers[roles.end]),
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
        let Some((stream, available, after)) = self.stream(fd)? else {
            return Ok(None);
        };
        let last = usize::try_from(space)
            .unwrap_or(usize::MAX)
            .min(available.len());
        let mut outcomes: Vec<(Term, usize, bool)> = Vec::new();
        // A read of standard input or a connection can end right after any end byte that has
        // arrived: fill stops there. A file's read gives everything asked for.
        if let Some(end) = end
            && self.state.every_cut
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
            Stream::Stdin => self.state.stdin_read,
            Stream::Connection(number) => self.consumed(number),
            Stream::File(_) => 0,
        };
        let original = self.state.clone();
        let mut choices = Vec::new();
        for (condition, taken, at_end) in outcomes {
            if self.pool.truth(condition) == Some(false) {
                continue;
            }
            let mut outcome = original.clone();
            std::mem::swap(self.state, &mut outcome);
            let applied = self.take(fill, &stream, &available, cursor, space, taken);
            if let Err(stop) = applied {
                self.state.end = Some(stop);
            } else if at_end && let After::Wait(why) = &after {
                self.state.end = Some(End::Stuck(why.clone()));
            }
            if taken < last {
                self.state.fill_cut.get_or_insert(before + taken);
            }
            std::mem::swap(self.state, &mut outcome);
            choices.push((condition, outcome));
        }
        Ok(Some(Step::Choose(choices)))
    }

    /// The stream `fd` reads from: what it holds, and what follows. `None` for a descriptor a
    /// read fails on, which the loop then runs as code.
    fn stream(&mut self, fd: u64) -> Result<Option<(Stream, Vec<Term>, After)>, Stop> {
        if fd == 0 {
            let left = self.state.stdin[self.state.stdin_read..].to_vec();
            return Ok(Some((Stream::Stdin, left, After::End)));
        }
        if let Ok(socket) = self.connection_of(fd) {
            let Some(number) = socket else {
                return Ok(None);
            };
            let (left, waits) = self.unread(number)?;
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
        let Some(open) = self.state.open.get(&fd).filter(|open| !open.writing) else {
            return Ok(None);
        };
        let files = std::rc::Rc::clone(&self.state.files);
        let contents = files.get(&open.name).map_or(&[][..], Vec::as_slice);
        let left = contents[open.position.min(contents.len())..].to_vec();
        Ok(Some((Stream::File(fd), left, After::End)))
    }

    /// fill having read `taken` bytes of `available` into the buffer at `cursor`, which has
    /// `space` bytes left: the bytes in place, the stream on, and the run at `.Lfill_done`.
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
            Stream::Stdin => self.state.stdin_read += taken,
            Stream::File(fd) => {
                if let Some(open) = self.state.open.get_mut(fd) {
                    open.position += taken;
                }
            }
            Stream::Connection(number) => self.consume(*number, taken),
        }
        let taken = taken as u64;
        let roles = FillAbi::of(self.image.isa);
        self.state.registers[roles.cursor] = self.pool.bv(64, u128::from(cursor + taken));
        self.state.registers[roles.space] = self.pool.bv(64, u128::from(space - taken));
        // What the reads leave: on x86-64 the kernel's rcx and r11, and a loop count that depends
        // on how the input was cut.
        if self.image.isa == Isa::X86_64 {
            self.state.registers[1] = self.pool.bv(64, u128::from(fill.after_call));
            self.state.registers[11] = self.pool.bv(64, 0x202);
        }
        let count = self.pool.len();
        self.state.registers[roles.counter] = self.pool.var(64, &format!("fill{count}.reads"));
        self.state.rip = fill.done;
        Ok(())
    }
}

impl State {
    /// The same state, proving for every way the input can be cut rather than running as one
    /// read gives it.
    #[must_use]
    pub fn for_every_cut(mut self) -> Self {
        self.every_cut = true;
        self
    }
}
