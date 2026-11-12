//! `term_is`, `term_size` and `term_raw`, lengua's own: what a program whose idea uses `the
//! terminal` calls to learn whether it runs in one, how large it is, and to get its keys as
//! they're pressed. Only a run from a shell has a terminal: a test's standard input is the
//! test's bytes and the `run` tool's is the tool's, so there `term_is` writes 0 and the other
//! two answer `NOTTY`.
//!
//! A terminal the program made raw is put back as it was when its host is dropped, which every
//! end of a run comes to: its `_start` returning, `proc_exit`, a trap, the deadline, lengua's
//! stop. For what ends the whole process instead, a signal or the console closing,
//! `lengua_os::terminal` puts it back from a handler.

use super::fds::{STDERR, STDIN, STDOUT};
use super::{Fail, Host, memory};
use crate::wasm::wasi::{errno, term};

/// The terminal a program runs in.
pub trait Terminal: Send {
    /// Its columns and rows; `None` when there is no terminal.
    fn size(&self) -> Option<(u16, u16)>;

    /// Makes standard input raw, keys as they're pressed with no echo, or puts it back as it
    /// was.
    ///
    /// # Errors
    ///
    /// `NOTTY` when standard input is no terminal, and `IO` when the system won't put it back.
    fn raw(&mut self, on: bool) -> Result<(), u16>;
}

/// No terminal: what a test and the `run` tool have.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Absent;

impl Terminal for Absent {
    fn size(&self) -> Option<(u16, u16)> {
        None
    }

    fn raw(&mut self, _: bool) -> Result<(), u16> {
        Err(errno::NOTTY)
    }
}

/// This process's own terminal, for a run from a shell. Dropping it puts the terminal back as
/// it was.
#[derive(Debug, Default)]
pub struct Own;

impl Terminal for Own {
    fn size(&self) -> Option<(u16, u16)> {
        lengua_os::terminal::size()
    }

    fn raw(&mut self, on: bool) -> Result<(), u16> {
        if on {
            lengua_os::terminal::raw().map_err(|_| errno::NOTTY)
        } else {
            lengua_os::terminal::restore().map_err(|_| errno::IO)
        }
    }
}

impl Drop for Own {
    fn drop(&mut self) {
        let _ = lengua_os::terminal::restore();
    }
}

/// `term_is(fd, out)`: writes 1 when standard input, output or error is a terminal, and 0 when
/// it isn't.
///
/// # Errors
///
/// A buffer outside memory, and `BADF` for any other descriptor.
pub fn term_is(host: &Host, memory: &mut [u8], fd: u32, out: u32) -> Result<(), Fail> {
    memory::check(memory, out, 4)?;
    let is = match fd {
        STDIN => host.stdin.terminal(),
        STDOUT => host.stdout.terminal(),
        STDERR => host.stderr.terminal(),
        _ => return Err(Fail::Errno(errno::BADF)),
    };
    memory::write_u32(memory, out, u32::from(is))
}

/// `term_size(out_columns, out_rows)`: writes the terminal's columns and rows.
///
/// # Errors
///
/// A buffer outside memory, and `NOTTY` when there is no terminal.
pub fn term_size(host: &Host, memory: &mut [u8], columns: u32, rows: u32) -> Result<(), Fail> {
    memory::check(memory, columns, 4)?;
    memory::check(memory, rows, 4)?;
    let Some((wide, high)) = host.terminal.size() else {
        return Err(Fail::Errno(errno::NOTTY));
    };
    memory::write_u32(memory, columns, u32::from(wide))?;
    memory::write_u32(memory, rows, u32::from(high))
}

/// `term_raw(mode)`: with 1, standard input gives keys as they're pressed, with no echo and no
/// line editing, and a wait for a key can end at its time; with 0, the terminal is as it was.
///
/// # Errors
///
/// `INVAL` for another mode, and `NOTTY` when standard input is no terminal.
pub fn term_raw(host: &mut Host, mode: u32) -> Result<(), Fail> {
    let on = match mode {
        term::RAW => true,
        term::AS_BEFORE => false,
        _ => return Err(Fail::Errno(errno::INVAL)),
    };
    host.terminal.raw(on).map_err(Fail::Errno)?;
    if on {
        // Keys are read by a thread of lengua's from now on, so that `poll_oneoff` can say
        // whether one is there.
        host.stdin.watch();
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;

    use super::*;
    use crate::wasm::engine::{self, Setup};
    use crate::wasm::host::linker;
    use crate::wasm::host::policy::Policy;
    use crate::wasm::host::tests::world;
    use crate::wasm::job::Ending;
    use crate::wasm::runner;

    /// A terminal of the test's own: it counts how often it was made raw, and how often it was
    /// put back, which dropping it does too when it is raw.
    struct Counted {
        raw: bool,
        set: Arc<AtomicUsize>,
        back: Arc<AtomicUsize>,
    }

    impl Terminal for Counted {
        fn size(&self) -> Option<(u16, u16)> {
            Some((132, 43))
        }

        fn raw(&mut self, on: bool) -> Result<(), u16> {
            if on {
                self.set.fetch_add(1, Ordering::SeqCst);
            } else if self.raw {
                self.back.fetch_add(1, Ordering::SeqCst);
            }
            self.raw = on;
            Ok(())
        }
    }

    impl Drop for Counted {
        fn drop(&mut self) {
            if self.raw {
                self.back.fetch_add(1, Ordering::SeqCst);
            }
        }
    }

    /// A policy that names the terminal.
    fn terminal() -> Policy {
        Policy {
            terminal: true,
            ..Policy::default()
        }
    }

    const IMPORTS: &str = r#"
  (import "lengua_term_1" "term_raw" (func $term_raw (param i32) (result i32)))
  (import "lengua_term_1" "term_size" (func $term_size (param i32 i32) (result i32)))
  (import "lengua_term_1" "term_is" (func $term_is (param i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "proc_exit" (func $exit (param i32)))
  (memory (export "memory") 1)"#;

    /// Runs a module that makes the terminal raw and then does `then`, in a world with a
    /// terminal of the test's own when `with`, for `time` at most: how it ended, how often the
    /// terminal was made raw, and how often it was put back.
    fn run(policy: Policy, with: bool, then: &str, time: Duration) -> (Ending, usize, usize) {
        let text = format!(
            "(module {IMPORTS} (func (export \"_start\") (local $err i32)
               (local.set $err (call $term_raw (i32.const 1)))
               {then}))"
        );
        let set = Arc::new(AtomicUsize::new(0));
        let back = Arc::new(AtomicUsize::new(0));
        let mut world = world(policy);
        if with {
            world.beyond.terminal = Some(Box::new(Counted {
                raw: false,
                set: Arc::clone(&set),
                back: Arc::clone(&back),
            }));
        }
        let engine = engine::engine(Setup::Test).expect("the engine");
        let linker = linker(&engine).expect("the host");
        let built = crate::wasm::text::build(&text).unwrap_or_else(|problem| panic!("{problem}"));
        let module = wasmtime::Module::from_binary(&engine, &built.wasm).expect("it compiles");
        let executed = runner::execute(
            &engine,
            &linker,
            &module,
            Setup::Test,
            Host::new(world),
            Some(time),
        );
        let ending = executed.ending.clone();
        // The run is over once its host is gone.
        drop(executed);
        (
            ending,
            set.load(Ordering::SeqCst),
            back.load(Ordering::SeqCst),
        )
    }

    /// The threat: a program leaves the terminal raw. However the run ends, the terminal is put
    /// back once: when the program returns, when it exits, when it traps, when it runs past its
    /// time, and when it puts the terminal back itself.
    #[test]
    fn the_mode_is_put_back_on_every_end() {
        let long = Duration::from_secs(10);
        let (ending, set, back) = run(terminal(), true, "", long);
        assert_eq!((ending, set, back), (Ending::Exited(0), 1, 1));
        let exits = "(call $exit (i32.add (local.get $err) (i32.const 7)))";
        let (ending, set, back) = run(terminal(), true, exits, long);
        assert_eq!((ending, set, back), (Ending::Exited(7), 1, 1));
        let (ending, set, back) = run(terminal(), true, "unreachable", long);
        assert!(matches!(ending, Ending::Trapped(..)), "{ending:?}");
        assert_eq!((set, back), (1, 1));
        let loops = "(loop $forever (br $forever))";
        let (ending, set, back) = run(terminal(), true, loops, Duration::from_millis(300));
        assert_eq!((ending, set, back), (Ending::TimedOut, 1, 1));
        // A program that puts it back itself: once, and not again at the end.
        let itself = "(call $exit (call $term_raw (i32.const 0)))";
        let (ending, set, back) = run(terminal(), true, itself, long);
        assert_eq!((ending, set, back), (Ending::Exited(0), 1, 1));
        // A mode that isn't one is INVAL, 28, and changes nothing.
        let wrong = "(call $exit (call $term_raw (i32.const 2)))";
        let (ending, set, back) = run(terminal(), true, wrong, long);
        assert_eq!((ending, set, back), (Ending::Exited(28), 1, 1));
    }

    /// The threat: a program sets the terminal's mode without the grant. Without `the
    /// terminal` in its idea, each function answers NOTCAPABLE, 76, and nothing is set.
    #[test]
    fn without_the_grant_nothing_is_set() {
        let long = Duration::from_secs(10);
        let exits = "(call $exit (local.get $err))";
        let (ending, set, back) = run(Policy::default(), true, exits, long);
        assert_eq!((ending, set, back), (Ending::Exited(76), 0, 0));
        let size = "(call $exit (call $term_size (i32.const 8) (i32.const 12)))";
        let (ending, ..) = run(Policy::default(), true, size, long);
        assert_eq!(ending, Ending::Exited(76));
        let is = "(call $exit (call $term_is (i32.const 0) (i32.const 8)))";
        let (ending, ..) = run(Policy::default(), true, is, long);
        assert_eq!(ending, Ending::Exited(76));
    }

    /// A test and the `run` tool have no terminal: `term_raw` and `term_size` answer NOTTY, 59,
    /// and `term_is` writes 0 for each standard stream. With a terminal, its size is written.
    #[test]
    fn a_test_has_no_terminal() {
        let long = Duration::from_secs(10);
        let exits = "(call $exit (local.get $err))";
        let (ending, set, back) = run(terminal(), false, exits, long);
        assert_eq!((ending, set, back), (Ending::Exited(59), 0, 0));
        let size = "(call $exit (call $term_size (i32.const 8) (i32.const 12)))";
        let (ending, ..) = run(terminal(), false, size, long);
        assert_eq!(ending, Ending::Exited(59));
        // Each stream is no terminal, 0, and a descriptor that is no stream is BADF, 8.
        let is = "(i32.store (i32.const 8) (i32.const 9))
                  (local.set $err (call $term_is (i32.const 0) (i32.const 8)))
                  (local.set $err (i32.add (local.get $err)
                    (call $term_is (i32.const 1) (i32.const 12))))
                  (local.set $err (i32.add (local.get $err)
                    (call $term_is (i32.const 2) (i32.const 16))))
                  (call $exit (i32.add
                    (i32.add (local.get $err) (i32.load (i32.const 8)))
                    (i32.add (i32.load (i32.const 12))
                      (i32.mul (i32.const 10) (call $term_is (i32.const 3) (i32.const 20))))))";
        let (ending, ..) = run(terminal(), false, is, long);
        assert_eq!(ending, Ending::Exited(80));
        // The columns and the rows of the test's own terminal: 132 and 43.
        let size = "(local.set $err (call $term_size (i32.const 8) (i32.const 12)))
                    (call $exit (i32.add (local.get $err)
                      (i32.add (i32.load (i32.const 8)) (i32.load (i32.const 12)))))";
        let (ending, ..) = run(terminal(), true, size, long);
        assert_eq!(ending, Ending::Exited(175));
        // A buffer outside the memory stops the program, as a load outside it would.
        let outside = "(call $exit (call $term_size (i32.const 65534) (i32.const 12)))";
        let (ending, ..) = run(terminal(), true, outside, long);
        assert!(!matches!(ending, Ending::Exited(_)), "{ending:?}");
    }
}
