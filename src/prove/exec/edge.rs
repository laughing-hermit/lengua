//! A call to one of lengua's operations (`os_read`, `os_open` and the rest), modelled whole: what
//! the kernel does for the call the operation makes, as the run's model has it for the system
//! call itself, with the results and the registers the edge's convention gives. No code of the
//! edge runs, so a proof reads only the program's, and an operation answers with its own subset
//! of the closed set of errors, whatever else the kernel could say.

use super::calls::{Call, Convention};
use super::{AT_FDCWD, Cpu, EBADF, EINVAL, ENOTTY, Step, Stop, unsupported};
use crate::edge::{Op, kind};
use crate::prove::term::Term;

/// `EIO`: what an error outside an operation's own list becomes.
const EIO: i64 = 5;

/// `ESPIPE`: a seek on a pipe, which is what a test's standard descriptors are.
const ESPIPE: i64 = 29;

/// `openat`'s flags for each of `os_open`'s modes that make a file to write: the file made and
/// emptied.
const WRITE_FLAGS: u64 = 0o1101;

/// `unlinkat`'s flag for a folder.
const AT_REMOVEDIR: u64 = 0o1000;

impl Cpu<'_> {
    /// The call of `op` the running instruction makes: its arguments are in the registers of the
    /// edge's convention, which are the system call's own, and it goes on at the next
    /// instruction.
    pub(super) fn edge_call(&mut self, op: Op) -> Result<Step, Stop> {
        let convention = Convention::of(self.image.isa);
        let given = convention.arguments.map(|slot| self.state.registers[slot]);
        let fdcwd = self.pool.bv(64, u128::from(AT_FDCWD));
        match op {
            // The same arguments as the system call's.
            Op::Read => self.delegate(op, Call::Read, 0),
            Op::Write => self.delegate(op, Call::Write, 1),
            Op::Close => self.delegate(op, Call::Close, 3),
            Op::Shutdown => self.delegate(op, Call::Shutdown, 48),
            Op::Poll => self.delegate(op, Call::Poll, 7),
            Op::Exit => self.delegate(op, Call::ExitGroup, 231),
            Op::Clock => {
                if self.constant(given[0])? & 0xffff_ffff > 1 {
                    return self.finish(op, (-EINVAL).cast_unsigned());
                }
                self.delegate(op, Call::ClockGettime, 228)
            }
            Op::Sleep => {
                let (clock, absolute) = (self.constant(given[0])?, self.constant(given[1])?);
                if clock & 0xffff_ffff > 1 || absolute & 0xffff_ffff > 1 {
                    return self.finish(op, (-EINVAL).cast_unsigned());
                }
                // What is left of the time isn't asked for.
                self.delegate_with(op, Call::ClockNanosleep, 230, &[(3, 0)])
            }
            Op::Random => {
                if self.constant(given[1])? > crate::edge::RANDOM_MOST {
                    return self.finish(op, (-EINVAL).cast_unsigned());
                }
                self.delegate_with(op, Call::Getrandom, 318, &[(2, 0)])
            }
            Op::Accept => {
                if self.constant(given[0])? & 0xffff_ffff != 3 {
                    return self.finish(op, (-EBADF).cast_unsigned());
                }
                // Neither the peer's address nor flags are asked for.
                self.delegate_with(op, Call::Accept4, 288, &[(1, 0), (2, 0), (3, 0)])
            }
            Op::Open => {
                let mode = self.constant(given[1])? & 0xffff_ffff;
                let flags = match mode {
                    0 => 0,
                    1 => WRITE_FLAGS,
                    2 | 3 => {
                        return Err(unsupported(
                            "opening a file to add to or to make, which the model doesn't follow",
                        ));
                    }
                    4 => self.image.linux().o_directory,
                    _ => return self.finish(op, (-EINVAL).cast_unsigned()),
                };
                let flags = self.pool.bv(64, u128::from(flags));
                let step = self.open_with(fdcwd, 0, flags)?;
                self.normalize(op);
                Ok(step)
            }
            Op::Info => {
                let result = self.info(given)?;
                self.finish(op, result)
            }
            Op::List => {
                let descriptor = self.constant(given[0])? & 0xffff_ffff;
                if !self.state.open.contains_key(&descriptor) {
                    return self.finish(op, (-EBADF).cast_unsigned());
                }
                Err(unsupported(
                    "listing a folder, whose entries the system gives in an order of its own",
                ))
            }
            Op::Seek => {
                let descriptor = self.constant(given[0])? & 0xffff_ffff;
                if descriptor <= 2 {
                    return self.finish(op, (-ESPIPE).cast_unsigned());
                }
                if !self.state.open.contains_key(&descriptor) {
                    return self.finish(op, (-EBADF).cast_unsigned());
                }
                Err(unsupported("seeking in a file, which the model doesn't follow"))
            }
            Op::Mkdir => {
                let name = self.name_at(fdcwd, 0, "os_mkdir")?;
                let result = match self.state.refused(&name, true, false) {
                    Some(errno) => (-errno).cast_unsigned(),
                    None => self.make_folder(name),
                };
                self.finish(op, result)
            }
            Op::Remove => {
                let kind = self.constant(given[1])? & 0xffff_ffff;
                if kind > 1 {
                    return self.finish(op, (-EINVAL).cast_unsigned());
                }
                let name = self.name_at(fdcwd, 0, "os_remove")?;
                let result = match self.state.refused(&name, false, true) {
                    Some(errno) => (-errno).cast_unsigned(),
                    None => self.remove(name, if kind == 1 { AT_REMOVEDIR } else { 0 })?,
                };
                self.finish(op, result)
            }
            Op::Rename => {
                let from = self.name_at(fdcwd, 0, "os_rename")?;
                let to = self.name_at(fdcwd, 1, "os_rename")?;
                let refused = self
                    .state
                    .refused(&from, false, true)
                    .or_else(|| self.state.refused(&to, true, true));
                let result = match refused {
                    Some(errno) => (-errno).cast_unsigned(),
                    None => self.rename(from, to)?,
                };
                self.finish(op, result)
            }
            // As in a test, none of the standard descriptors is a terminal.
            Op::Terminal | Op::Window => self.finish(op, (-ENOTTY).cast_unsigned()),
            Op::ThreadStart | Op::ThreadJoin => Err(unsupported(
                "starting or joining a thread, which only lengua's library does",
            )),
        }
    }

    /// What the kernel's `call` does with the registers as they are.
    fn delegate(&mut self, op: Op, call: Call, number: u64) -> Result<Step, Stop> {
        self.delegate_with(op, call, number, &[])
    }

    /// What the kernel's `call` does, with the arguments `fixed` (by position, to a value) set
    /// for it, which the program didn't give: afterwards they are what the program had in them.
    fn delegate_with(
        &mut self,
        op: Op,
        call: Call,
        number: u64,
        fixed: &[(usize, u64)],
    ) -> Result<Step, Stop> {
        let slots = Convention::of(self.image.isa).arguments;
        let kept: Vec<(usize, Term, Option<super::Owner>)> = fixed
            .iter()
            .map(|(position, _)| {
                let slot = slots[*position];
                (slot, self.state.registers[slot], self.state.owners[slot])
            })
            .collect();
        for (position, value) in fixed {
            let slot = slots[*position];
            self.state.registers[slot] = self.pool.bv(64, u128::from(*value));
            self.state.owners[slot] = None;
        }
        let step = self.perform(Some(call), number);
        for (slot, term, owner) in kept {
            self.state.registers[slot] = term;
            self.state.owners[slot] = owner;
        }
        let step = step?;
        self.normalize(op);
        Ok(step)
    }

    /// The operation's `result`, and on to the next instruction.
    fn finish(&mut self, op: Op, result: u64) -> Result<Step, Stop> {
        self.state.returns(self.pool, result, self.next);
        self.normalize(op);
        Ok(Step::Next)
    }

    /// The result an operation gives for an error the system gave: the error when it is one the
    /// operation lists, and `EIO` when it isn't.
    fn normalize(&mut self, op: Op) {
        let Some(value) = self
            .pool
            .constant(self.state.registers[0])
            .and_then(|value| u64::try_from(value).ok())
        else {
            return;
        };
        let signed = value.cast_signed();
        if let Some(errno) = signed
            .checked_neg()
            .filter(|errno| (1..=4095).contains(errno))
            && !u32::try_from(errno).is_ok_and(|errno| op.errors().contains(&errno))
        {
            self.state.registers[0] = self.pool.bv(64, (-EIO).cast_unsigned().into());
        }
    }

    /// `os_info(descriptor, out)`: the size and kind of an open file, in the 16 bytes at `out`.
    fn info(&mut self, given: [Term; 6]) -> Result<u64, Stop> {
        let descriptor = self.constant(given[0])? & 0xffff_ffff;
        let out = self.constant(given[1])?;
        self.lent(1, out, 16, true, "os_info")?;
        let Some(open) = self.state.open.get(&descriptor).cloned() else {
            if descriptor <= 2 {
                return Err(unsupported("the details of standard input or output"));
            }
            return Ok((-EBADF).cast_unsigned());
        };
        let size = self.state.file(&open.name).map_or(0, <[Term]>::len);
        let size = self.pool.bv(64, size as u128);
        let file = self.pool.bv(64, u128::from(kind::FILE));
        self.save(out, size)?;
        self.save(out + 8, file)?;
        Ok(0)
    }
}
