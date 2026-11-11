//! `program_run`, lengua's own: what a program calls to start a program its idea names on a
//! `- Runs:` line, give it input, wait for it, and get what it printed and its exit code
//! ([`crate::starts`]). The order of the checks is the defence: the command is read as text,
//! a rule of the idea must cover its name and every argument, and only then is anything looked
//! for. In a test nothing is started: a step of the test says what a command gives, and a
//! command no step gives isn't there.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use super::{Fail, Host, memory};
use crate::starts::{self, End, MAX_ARGUMENTS, MAX_COMMAND, Unfound};
use crate::wasm::wasi::errno;

/// The command at `at`, as text: the program's name, then each argument, each ended by a NUL.
fn command_of(bytes: &[u8]) -> Result<Vec<&str>, u16> {
    if bytes.len() > MAX_COMMAND {
        return Err(errno::INVAL);
    }
    let Some(text) = bytes.strip_suffix(&[0]) else {
        return Err(errno::INVAL);
    };
    let text = std::str::from_utf8(text).map_err(|_| errno::ILSEQ)?;
    let pieces: Vec<&str> = text.split('\0').collect();
    if pieces.len() > MAX_ARGUMENTS + 1 || pieces[0].is_empty() {
        return Err(errno::INVAL);
    }
    Ok(pieces)
}

/// A command as a report shows it.
fn shown(pieces: &[&str]) -> String {
    let words: Vec<String> = pieces
        .iter()
        .map(|piece| format!("`{}`", piece.escape_debug()))
        .collect();
    format!("the program {}", words.join(" "))
}

/// The folders no started program may lie in, since the program itself may write there: its
/// working folder, every place its arguments name under a `- Files:` line, and the system's
/// temporary folder. Each with its links followed.
fn barred(host: &Host) -> Vec<PathBuf> {
    let mut folders: Vec<PathBuf> = Vec::new();
    folders.extend(host.working.clone());
    folders.push(std::env::temp_dir());
    let grants = &host.policy.reach.get().files;
    for (place, text) in host.args.iter().enumerate().skip(1) {
        for folder in [false, true] {
            if crate::named::granted(grants, place, folder).is_some()
                && let Ok(found) = crate::named::resolve(&host.ground, text, folder)
            {
                folders.push(found.root);
            }
        }
    }
    folders
        .iter()
        .map(|folder| folder.canonicalize().unwrap_or_else(|_| folder.clone()))
        .collect()
}

/// Writes what a started program gave: its output at `output`, how many bytes that is, and its
/// exit code.
fn give(
    memory: &mut [u8],
    output: (u32, u32),
    (size, code): (u32, u32),
    printed: &[u8],
    exit: i32,
) -> Result<(), Fail> {
    let room = memory::range(memory, output.0, output.1)?;
    let count = printed.len().min(room.len());
    memory[room.start..room.start + count].copy_from_slice(&printed[..count]);
    memory::write_u32(memory, size, u32::try_from(count).unwrap_or(u32::MAX))?;
    memory::write_u32(memory, code, i32::cast_unsigned(exit))
}

/// `program_run(command, command_len, input, input_len, output, output_len, out_size,
/// out_code)`: starts the program the command names with its arguments, gives it the input as
/// its standard input, waits until it ends, and writes what it printed, how many bytes that is,
/// and its exit code.
///
/// # Errors
///
/// A buffer outside memory; `INVAL` for a command that isn't a name and arguments each ended
/// by a NUL, `ILSEQ` for one that isn't text; `NOTCAPABLE` for a command no `- Runs:` line of
/// the idea covers, and for a program lengua's rules refuse; `NOENT` for a program that isn't
/// there, which in a test is one no step gives; `TIMEDOUT` for one that ran past its time, and
/// `FBIG` for one that printed more than the buffer holds, both ended by lengua; `IO` for one
/// the system didn't start.
pub fn program_run(
    host: &mut Host,
    memory: &mut [u8],
    command: (u32, u32),
    input: (u32, u32),
    output: (u32, u32),
    out: (u32, u32),
) -> Result<(), Fail> {
    memory::check(memory, out.0, 4)?;
    memory::check(memory, out.1, 4)?;
    memory::check(memory, output.0, output.1)?;
    let given = memory::bytes(memory, input.0, input.1)?.to_vec();
    let text = memory::bytes(memory, command.0, command.1)?.to_vec();
    let pieces = command_of(&text).map_err(Fail::Errno)?;
    let (name, arguments) = (pieces[0], &pieces[1..]);
    let what = shown(&pieces);
    let grants = host.policy.reach.get();
    let Some(rule) = grants.runs.iter().find(|rule| rule.covers(name, arguments)) else {
        host.note(
            what,
            Some("no `- Runs:` line of its idea covers that command".to_owned()),
            true,
        );
        return Err(Fail::Errno(errno::NOTCAPABLE));
    };
    let room = usize::try_from(output.1).unwrap_or(usize::MAX);
    // A test starts nothing: a step of the test says what the command gives.
    if let Some(internet) = &host.internet {
        let stood = internet
            .programs
            .iter()
            .find(|stood| {
                stood.program == name
                    && stood.arguments.len() == arguments.len()
                    && stood
                        .arguments
                        .iter()
                        .zip(arguments)
                        .all(|(stood, given)| stood.as_str() == *given)
            })
            .cloned();
        let Some(stood) = stood else {
            host.note(
                what,
                Some("no step of the test says what that command gives".to_owned()),
                false,
            );
            return Err(Fail::Errno(errno::NOENT));
        };
        host.note(what, None, false);
        give(
            memory,
            output,
            out,
            stood.output.as_bytes(),
            i32::from(stood.code),
        )?;
        if stood.output.len() > room {
            return Err(Fail::Errno(errno::FBIG));
        }
        return Ok(());
    }
    let program = match (host.finder)(name, &barred(host)) {
        Ok(program) => program,
        Err(Unfound::Missing(why)) => {
            host.note(what, Some(why), false);
            return Err(Fail::Errno(errno::NOENT));
        }
        Err(Unfound::Refused(why)) => {
            host.note(what, Some(why), true);
            return Err(Fail::Errno(errno::NOTCAPABLE));
        }
    };
    host.limits.check()?;
    // Its line's time, and never past the run's own.
    let mut until = Instant::now() + Duration::from_secs(rule.seconds);
    if let Some(deadline) = host.limits.deadline {
        until = until.min(deadline);
    }
    let stop = Arc::clone(&host.limits.stop);
    let stopped = move || stop.load(Ordering::SeqCst);
    match starts::start(&program, arguments, &given, room, until, &stopped) {
        Err(error) => {
            host.note(what, Some(format!("it didn't start: {error}")), false);
            Err(Fail::Errno(errno::IO))
        }
        Ok(End::Exited {
            code,
            output: printed,
        }) => {
            host.note(what, None, false);
            give(memory, output, out, &printed, code)
        }
        Ok(End::TooMuch(printed)) => {
            host.note(
                what,
                Some(format!(
                    "it printed more than {room} bytes, and lengua ended it"
                )),
                false,
            );
            give(memory, output, out, &printed, 0)?;
            Err(Fail::Errno(errno::FBIG))
        }
        Ok(End::TimedOut) => {
            // The run's own deadline, or lengua's stop, ends the whole program.
            host.limits.check()?;
            host.note(
                what,
                Some(format!(
                    "it ran past its {} seconds, and lengua ended it",
                    rule.seconds
                )),
                false,
            );
            Err(Fail::Errno(errno::TIMEDOUT))
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;
    use crate::reach::Grants;
    use crate::starts::{Rule, Stood};
    use crate::wasm::host::policy::{Policy, Reach};
    use crate::wasm::host::tests::world;
    use crate::wasm::job::Internet;

    /// What this test's own process is started again with, to be a program a test starts.
    const CHILD: &str = "lengua-child";

    /// The arguments that start this test's own process again as the ignored test `name` of
    /// this module: a program every runner has, with nothing to install.
    fn child(name: &str) -> String {
        format!(
            "`--exact`, `wasm::host::start::tests::{name}`, `{CHILD}`, `--ignored` and \
             `--nocapture`"
        )
    }

    /// Whether this process was started as a test's child.
    fn is_child() -> bool {
        std::env::args().any(|argument| argument == CHILD)
    }

    /// This test's own program, whatever name is asked for: what the tests start.
    fn this_test(_: &str, barred: &[PathBuf]) -> Result<PathBuf, Unfound> {
        let program =
            std::env::current_exe().map_err(|error| Unfound::Missing(error.to_string()))?;
        let resolved = program.canonicalize().unwrap_or_else(|_| program.clone());
        if barred.iter().any(|folder| resolved.starts_with(folder)) {
            return Err(Unfound::Refused(
                "it lies where the program may write".to_owned(),
            ));
        }
        Ok(program)
    }

    /// A host whose idea's `- Runs:` lines say `lines`, which starts this test's own program
    /// for every name; in a test's world when `programs` are given.
    fn host_with(lines: &[&str], programs: Option<Vec<Stood>>) -> Host {
        let mut grants = Grants::default();
        for line in lines {
            let rule = Rule::read(line).unwrap_or_else(|why| panic!("{line}: {why}"));
            starts::add(&mut grants.runs, rule).expect("room for it");
        }
        let policy = Policy {
            reach: Reach::of(&grants),
            ..Policy::default()
        };
        let mut world = world(policy);
        world.internet = programs.map(|programs| Internet {
            programs,
            ..Internet::default()
        });
        let mut host = Host::new(world);
        host.finder = this_test;
        host
    }

    /// What `program_run` answers for `command`, a name and arguments, given `input`, with
    /// `room` bytes for the output: the error number, what was printed, and the exit code.
    fn ran(host: &mut Host, command: &[&str], input: &[u8], room: u32) -> (u16, Vec<u8>, u32) {
        let mut memory = vec![0_u8; 1 << 21];
        let mut text = Vec::new();
        for piece in command {
            text.extend_from_slice(piece.as_bytes());
            text.push(0);
        }
        memory[1024..1024 + text.len()].copy_from_slice(&text);
        memory[8192..8192 + input.len()].copy_from_slice(input);
        let length = |bytes: &[u8]| u32::try_from(bytes.len()).expect("it fits");
        let answer = program_run(
            host,
            &mut memory,
            (1024, length(&text)),
            (8192, length(input)),
            (1 << 20, room),
            (8, 12),
        );
        let number = match answer {
            Ok(()) => 0,
            Err(Fail::Errno(number)) => number,
            Err(Fail::Stop(stop)) => panic!("{stop}"),
        };
        let size = memory::read_u32(&memory, 8).expect("the size");
        let code = memory::read_u32(&memory, 12).expect("the code");
        let start = 1 << 20;
        let printed = memory[start..start + usize::try_from(size).expect("it fits")].to_vec();
        (number, printed, code)
    }

    /// A child: prints its standard input back between two marks, and whether it has any
    /// environment variable but the two that are Windows's own: `SystemRoot`, which lengua
    /// gives it, and `PROCESSOR_ARCHITECTURE`, the CPU's name, which Windows on ARM64 puts in
    /// every process it starts. Run by itself it does nothing.
    #[test]
    #[ignore = "a program the tests of this module start"]
    fn child_says_what_it_got() {
        if !is_child() {
            return;
        }
        let mut input = String::new();
        std::io::Read::read_to_string(&mut std::io::stdin(), &mut input).expect("its input");
        let variables: Vec<String> = std::env::vars_os()
            .map(|(name, _)| name.to_string_lossy().to_ascii_uppercase())
            .filter(|name| !name.starts_with('='))
            .filter(|name| name != "SYSTEMROOT" && name != "PROCESSOR_ARCHITECTURE")
            .collect();
        let folder = std::env::current_dir().expect("a current folder");
        let empty = std::fs::read_dir(&folder).is_ok_and(|mut entries| entries.next().is_none());
        println!("<<{input}>> variables={variables:?} empty_folder={empty}");
    }

    /// A child: waits a minute. Run by itself it does nothing.
    #[test]
    #[ignore = "a program the tests of this module start"]
    fn child_waits() {
        if !is_child() {
            return;
        }
        std::thread::sleep(Duration::from_secs(60));
    }

    /// A child: prints without end. Run by itself it does nothing.
    #[test]
    #[ignore = "a program the tests of this module start"]
    fn child_prints_without_end() {
        if !is_child() {
            return;
        }
        let line = "x".repeat(4096);
        loop {
            println!("{line}");
        }
    }

    /// The threat: a program started with what its idea doesn't bound, or one its idea doesn't
    /// name. Nothing is looked for, and nothing starts, for a command no rule covers.
    #[test]
    fn what_no_rule_covers_is_not_started() {
        let says = format!(
            "`tool` with the arguments {}",
            child("child_says_what_it_got")
        );
        let mut host = host_with(&[&says], None);
        let name = "wasm::host::start::tests::child_says_what_it_got";
        let covered = ["tool", "--exact", name, CHILD, "--ignored", "--nocapture"];
        let (number, printed, code) = ran(&mut host, &covered, b"", 65_536);
        assert_eq!(
            (number, code),
            (0, 0),
            "{}",
            String::from_utf8_lossy(&printed)
        );
        // One argument more, one fewer, one changed, another program.
        let mut more = covered.to_vec();
        more.push("--show-output");
        let fewer = &covered[..5];
        let mut changed = covered;
        changed[4] = "--include-ignored";
        let mut other = covered;
        other[0] = "rm";
        for command in [&more[..], fewer, &changed[..], &other[..]] {
            let (number, printed, _) = ran(&mut host, command, b"", 65_536);
            assert_eq!(number, errno::NOTCAPABLE, "{command:?}");
            assert!(printed.is_empty(), "{command:?}");
        }
        // What isn't a command at all.
        let mut memory = vec![0_u8; 65_536];
        memory[1024..1028].copy_from_slice(b"tool");
        let malformed = program_run(
            &mut host,
            &mut memory,
            (1024, 4),
            (0, 0),
            (2048, 16),
            (8, 12),
        );
        assert_eq!(malformed, Err(Fail::Errno(errno::INVAL)));
        memory[1024..1027].copy_from_slice(&[0xff, 0xfe, 0]);
        let not_text = program_run(
            &mut host,
            &mut memory,
            (1024, 3),
            (0, 0),
            (2048, 16),
            (8, 12),
        );
        assert_eq!(not_text, Err(Fail::Errno(errno::ILSEQ)));
        // A host with no grant, and the report of what lengua refused.
        let mut none = host_with(&[], None);
        assert_eq!(ran(&mut none, &covered, b"", 64).0, errno::NOTCAPABLE);
        let refused = host
            .notes
            .opened
            .iter()
            .filter(|noted| {
                noted
                    .problem
                    .as_deref()
                    .is_some_and(|why| why.contains("no `- Runs:` line"))
            })
            .count();
        assert_eq!(refused, 4);
    }

    /// The threat: a started program reads the user's environment, or sits beside what the
    /// program wrote. It gets no environment, its input, and an empty folder of its own.
    #[test]
    fn a_started_program_gets_no_environment_and_its_input() {
        let says = format!(
            "`tool` with the arguments {}",
            child("child_says_what_it_got")
        );
        let mut host = host_with(&[&says], None);
        let name = "wasm::host::start::tests::child_says_what_it_got";
        let command = ["tool", "--exact", name, CHILD, "--ignored", "--nocapture"];
        let (number, printed, code) = ran(&mut host, &command, b"what it was given", 65_536);
        let printed = String::from_utf8_lossy(&printed).into_owned();
        assert_eq!((number, code), (0, 0), "{printed}");
        assert!(
            printed.contains("<<what it was given>> variables=[] empty_folder=true"),
            "{printed}"
        );
    }

    /// The threat: a started program that never ends. It is ended at its line's time, and the
    /// program hears TIMEDOUT.
    #[test]
    fn a_started_program_ends_at_its_time() {
        let waits = format!(
            "`tool` with the arguments {}, within 1 second",
            child("child_waits")
        );
        let mut host = host_with(&[&waits], None);
        let name = "wasm::host::start::tests::child_waits";
        let command = ["tool", "--exact", name, CHILD, "--ignored", "--nocapture"];
        let started = Instant::now();
        let (number, ..) = ran(&mut host, &command, b"", 65_536);
        assert_eq!(number, errno::TIMEDOUT);
        let took = started.elapsed();
        assert!(
            took >= Duration::from_millis(900) && took < Duration::from_secs(30),
            "{took:?}"
        );
        // The run's own deadline comes first, and ends the whole program.
        let mut host = host_with(&[&waits], None);
        host.limits.deadline = Some(Instant::now() + Duration::from_millis(200));
        let mut memory = vec![0_u8; 65_536];
        let mut text = Vec::new();
        for piece in command {
            text.extend_from_slice(piece.as_bytes());
            text.push(0);
        }
        memory[1024..1024 + text.len()].copy_from_slice(&text);
        let length = u32::try_from(text.len()).expect("it fits");
        let ended = program_run(
            &mut host,
            &mut memory,
            (1024, length),
            (0, 0),
            (8192, 64),
            (8, 12),
        );
        assert_eq!(ended, Err(Fail::Stop(crate::wasm::host::Stop::TimedOut)));
    }

    /// The threat: a started program that prints without end. It prints at most the buffer:
    /// past it, it is ended, and the program hears FBIG with the buffer full.
    #[test]
    fn a_started_program_prints_at_most_the_buffer() {
        let prints = format!(
            "`tool` with the arguments {}",
            child("child_prints_without_end")
        );
        let mut host = host_with(&[&prints], None);
        let name = "wasm::host::start::tests::child_prints_without_end";
        let command = ["tool", "--exact", name, CHILD, "--ignored", "--nocapture"];
        let started = Instant::now();
        let (number, printed, _) = ran(&mut host, &command, b"", 10_000);
        assert_eq!(number, errno::FBIG);
        assert_eq!(printed.len(), 10_000);
        assert!(started.elapsed() < Duration::from_secs(9), "it was ended");
    }

    /// The threat: a test that starts a real program. In a test nothing starts: a step says
    /// what a command gives, and a command no step gives isn't there.
    #[test]
    fn a_test_starts_nothing() {
        let stood = Stood {
            program: "hostname".to_owned(),
            arguments: Vec::new(),
            output: "box\n".to_owned(),
            code: 0,
        };
        let failing = Stood {
            program: "tool".to_owned(),
            arguments: vec!["-x".to_owned(), "value".to_owned()],
            output: String::new(),
            code: 3,
        };
        let waits = format!("`tool` with the arguments {}", child("child_waits"));
        let mut host = host_with(
            &[
                "`hostname` with no arguments",
                "`tool` with the arguments `-x` and a value",
                &waits,
            ],
            Some(vec![stood, failing]),
        );
        // The finder would start this test's own program: in a test it is never asked.
        fn never(name: &str, _: &[PathBuf]) -> Result<PathBuf, Unfound> {
            panic!("a test looked for {name}");
        }
        host.finder = never;
        assert_eq!(
            ran(&mut host, &["hostname"], b"", 64),
            (0, b"box\n".to_vec(), 0)
        );
        assert_eq!(
            ran(&mut host, &["tool", "-x", "value"], b"", 64),
            (0, Vec::new(), 3)
        );
        // A command the idea grants and no step gives isn't there.
        assert_eq!(
            ran(&mut host, &["tool", "-x", "other"], b"", 64).0,
            errno::NOENT
        );
        let name = "wasm::host::start::tests::child_waits";
        let granted = ["tool", "--exact", name, CHILD, "--ignored", "--nocapture"];
        assert_eq!(ran(&mut host, &granted, b"", 64).0, errno::NOENT);
        // The idea's rules hold in a test too, and so does the buffer.
        assert_eq!(
            ran(&mut host, &["hostname", "-f"], b"", 64).0,
            errno::NOTCAPABLE
        );
        assert_eq!(
            ran(&mut host, &["hostname"], b"", 2),
            (errno::FBIG, b"bo".to_vec(), 0)
        );
    }

    /// The threat: a program starts a file it wrote itself. A program that lies in the working
    /// folder, or where an argument of the program names a file it may write, isn't started.
    #[test]
    fn a_program_in_the_working_folder_is_not_started() {
        let says = format!(
            "`tool` with the arguments {}",
            child("child_says_what_it_got")
        );
        let name = "wasm::host::start::tests::child_says_what_it_got";
        let command = ["tool", "--exact", name, CHILD, "--ignored", "--nocapture"];
        let here = std::env::current_exe().expect("this test");
        let folder = here.parent().map(Path::to_path_buf).expect("its folder");
        let mut host = host_with(&[&says], None);
        host.working = Some(folder);
        let (number, printed, _) = ran(&mut host, &command, b"", 65_536);
        assert_eq!(number, errno::NOTCAPABLE);
        assert!(printed.is_empty());
        assert!(host.notes.opened.iter().any(|noted| {
            noted
                .problem
                .as_deref()
                .is_some_and(|why| why.contains("may write"))
        }));
        // The system's temporary folder is barred too, whatever else is.
        assert!(barred(&host).len() >= 2);
    }
}
