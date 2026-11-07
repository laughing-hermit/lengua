//! `lengua-spawn`: the small process that `lengua-mcp measure` starts a program from on Linux
//! (src/measure.rs), so that the peak resident memory the system counts for the program is the
//! program's own. Linux begins a new process's count of its peak at what the process it came
//! from held: from lengua-mcp, megabytes, more than many a program's own peak; from this program,
//! which forks it, the few hundred KiB of this one's own that its copy holds.
//!
//! Usage: `lengua-spawn <report> <program> [<argument>...]`. It runs the program with the
//! arguments, the program's path its own name, and with this process's streams, folder and
//! environment; waits for it; and writes one line to the file `report`: the program's exit code,
//! or `signal` when a signal ended it, then its CPU time in user space and in the kernel and its
//! time on the clock, in nanoseconds, and its peak resident memory, in bytes. SIGTERM stops the
//! program. It exits with 0 once the report is written, or with 125 when it can't run the program
//! or write the report. Other systems start a program with memory of its own, and `measure`
//! counts it straight from lengua-mcp there.

use std::process::ExitCode;

/// The exit code when it can't run the program or write the report, as `lengua-mcp measure`'s.
const REFUSED: u8 = 125;

#[cfg(target_os = "linux")]
fn main() -> ExitCode {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStringExt as _;

    let mut args = std::env::args_os().skip(1);
    let Some(report) = args.next() else {
        eprintln!("usage: lengua-spawn <report> <program> [<argument>...]");
        return ExitCode::from(REFUSED);
    };
    let words: Result<Vec<CString>, _> = args.map(|word| CString::new(word.into_vec())).collect();
    let Ok(words) = words else {
        eprintln!("lengua-spawn: an argument holds a NUL byte");
        return ExitCode::from(REFUSED);
    };
    let spent = match lengua_os::run_forked(&words) {
        Ok(spent) => spent,
        Err(problem) => {
            eprintln!("lengua-spawn: {problem}");
            return ExitCode::from(REFUSED);
        }
    };
    let exit = spent.exit.map_or_else(|| "signal".to_owned(), |code| code.to_string());
    let counts = [spent.user_ns, spent.system_ns, spent.wall_ns, spent.peak_bytes];
    let counts: Vec<String> = counts.iter().map(u64::to_string).collect();
    let line = format!("{exit} {}\n", counts.join(" "));
    match std::fs::write(&report, line) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            let file = std::path::Path::new(&report).display();
            eprintln!("lengua-spawn: {file}: {error}");
            ExitCode::from(REFUSED)
        }
    }
}

/// Elsewhere a program starts with memory of its own, and `measure` counts it straight.
#[cfg(not(target_os = "linux"))]
fn main() -> ExitCode {
    eprintln!("lengua-spawn: lengua-mcp measure starts programs from it on Linux alone");
    ExitCode::from(REFUSED)
}
