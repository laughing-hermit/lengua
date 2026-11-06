//! lengua's calls into the operating system, kept apart from the rest so that the lengua crate can
//! forbid unsafe code outright: any call that needs it is wrapped here in a safe function, with a
//! test on the runner of the system it is for.
//!
//! The spikes in `tests/spikes.rs` ask the real macOS and Windows runners what the toolchain and
//! the systems do (round 1 of the roadmap in todo.md); their answers are recorded in ROADMAP.md.

/// Makes this process, and the program it becomes, ignore SIGXFSZ, the signal a write past the
/// file size limit would otherwise end it with. Ignored, the system fails the write with `EFBIG`
/// instead, after a short write up to the limit: a program sees a full file as an error it can
/// handle, as it does on every system lengua runs on, not as its own end.
///
/// An ignored signal stays ignored when the process becomes another program.
///
/// # Errors
///
/// The system's reason, when it won't change what the signal does.
#[cfg(unix)]
pub fn ignore_file_size_signal() -> Result<(), String> {
    use nix::sys::signal::{SigHandler, Signal, signal};
    // SAFETY: `SigIgn` installs no handler, so no code of ours runs in a signal's context, and
    // nothing else in this process depends on what SIGXFSZ did before.
    unsafe { signal(Signal::SIGXFSZ, SigHandler::SigIgn) }
        .map(|_| ())
        .map_err(|error| error.to_string())
}

#[cfg(all(test, unix))]
mod tests {
    use super::ignore_file_size_signal;

    /// A process that has asked to ignore SIGXFSZ survives raising it, which ends it by default.
    #[test]
    fn a_process_that_ignores_the_file_size_signal_survives_it() {
        ignore_file_size_signal().expect("the signal can be ignored");
        nix::sys::signal::raise(nix::sys::signal::Signal::SIGXFSZ).expect("the signal is raised");
    }
}
