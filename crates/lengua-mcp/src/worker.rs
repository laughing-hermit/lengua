use std::io::{Read, Write};
use std::path::Path;
use std::process::{Command, ExitCode, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use lengua_host::{Ending, Host, Input, OUTPUT, Outcome, Streams};
use lengua_idea::lock::{self, Kind};

const POLL: Duration = Duration::from_millis(5);
const FRAME: u64 = 1 << 20;

pub(crate) struct Request<'a> {
    pub kind: Kind,
    pub lock: &'a Path,
    pub folder: &'a Path,
    pub input: &'a Input,
    pub limit: Duration,
}

pub(crate) fn encode(outcome: &Outcome) -> Vec<u8> {
    let (name, code, message) = match &outcome.ending {
        Ending::Exited(code) => ("exited", *code, ""),
        Ending::OutOfTime(message) => ("out_of_time", 0, message.as_str()),
        Ending::OutOfMemory => ("out_of_memory", 0, ""),
        Ending::Trapped(message) => ("trapped", 0, message.as_str()),
        Ending::Refused(message) => ("refused", 0, message.as_str()),
    };
    let mut out = format!(
        "{name} {code} {} {} {}\n",
        message.len(),
        outcome.stdout.len(),
        outcome.stderr.len()
    )
    .into_bytes();
    out.extend_from_slice(message.as_bytes());
    out.extend_from_slice(&outcome.stdout);
    out.extend_from_slice(&outcome.stderr);
    out
}

pub(crate) fn decode(bytes: &[u8]) -> Option<Outcome> {
    let end = bytes.iter().position(|b| *b == b'\n')?;
    let head = std::str::from_utf8(bytes.get(..end)?).ok()?;
    let fields: Vec<&str> = head.split(' ').collect();
    let [name, code, sizes @ ..] = fields.as_slice() else {
        return None;
    };
    let sizes: Vec<usize> = sizes
        .iter()
        .map(|s| s.parse().ok())
        .collect::<Option<_>>()?;
    let [message, stdout, stderr] = sizes.as_slice() else {
        return None;
    };
    let rest = bytes.get(end + 1..)?;
    let (text, rest) = rest.split_at_checked(*message)?;
    let (out, rest) = rest.split_at_checked(*stdout)?;
    let (err, rest) = rest.split_at_checked(*stderr)?;
    if !rest.is_empty() {
        return None;
    }
    let text = String::from_utf8_lossy(text).to_string();
    let ending = match *name {
        "exited" => Ending::Exited(code.parse().ok()?),
        "out_of_time" => Ending::OutOfTime(text),
        "out_of_memory" => Ending::OutOfMemory,
        "trapped" => Ending::Trapped(text),
        "refused" => Ending::Refused(text),
        _ => return None,
    };
    Some(Outcome {
        ending,
        stdout: out.to_vec(),
        stderr: err.to_vec(),
    })
}

fn stopped() -> Outcome {
    Outcome {
        ending: Ending::OutOfTime(
            "the program ran past the time of its idea, and lengua stopped its process".to_string(),
        ),
        stdout: Vec::new(),
        stderr: Vec::new(),
    }
}

pub(crate) fn run(request: &Request) -> Outcome {
    let exe = match std::env::current_exe() {
        Ok(exe) => exe,
        Err(error) => {
            return Outcome::refused(format!("lengua could not find its own program: {error}"));
        }
    };
    let input = request.input;
    let root = input.root.as_deref().unwrap_or(request.folder);
    let mode = if input.fixed { "fixed" } else { "live" };
    let spawned = Command::new(exe)
        .arg("worker")
        .arg(request.kind.name())
        .arg(request.lock)
        .arg(request.folder)
        .arg(root)
        .arg(mode)
        .args(&input.arguments)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn();
    let mut child = match spawned {
        Ok(child) => child,
        Err(error) => {
            return Outcome::refused(format!(
                "lengua could not start the program's process: {error}"
            ));
        }
    };
    let stdin = input.stdin.clone();
    let writer = child.stdin.take().map(|mut pipe| {
        thread::spawn(move || {
            let _ = pipe.write_all(&stdin);
        })
    });
    let reader = child.stdout.take().map(|pipe| {
        thread::spawn(move || {
            let mut bytes = Vec::new();
            let most = u64::try_from(OUTPUT * 2).unwrap_or(u64::MAX) + FRAME;
            let _ = pipe.take(most).read_to_end(&mut bytes);
            bytes
        })
    });
    let started = Instant::now();
    let mut killed = false;
    loop {
        match child.try_wait() {
            Ok(Some(_)) | Err(_) => break,
            Ok(None) if started.elapsed() >= request.limit => {
                let _ = child.kill();
                let _ = child.wait();
                killed = true;
                break;
            }
            Ok(None) => thread::sleep(POLL),
        }
    }
    if let Some(writer) = writer {
        let _ = writer.join();
    }
    let bytes = reader.and_then(|r| r.join().ok()).unwrap_or_default();
    if killed {
        return stopped();
    }
    decode(&bytes).unwrap_or_else(|| Outcome {
        ending: Ending::Trapped("the program's process ended with no result".to_string()),
        stdout: Vec::new(),
        stderr: Vec::new(),
    })
}

fn outcome(kind: &str, lock_path: &Path, folder: &Path, input: &Input) -> Outcome {
    let Some(kind) = Kind::parse(kind) else {
        return Outcome::refused(format!("`{kind}` is not a kind of lock"));
    };
    let verified =
        match lock::user_key().and_then(|key| lock::verify(folder, lock_path, kind, &key)) {
            Ok(verified) => verified,
            Err(message) => return Outcome::refused(message),
        };
    let report = lengua_idea::program(folder);
    if !report.findings.is_empty() {
        return Outcome::refused(
            "the ideas have problems; call check, and fix them first".to_string(),
        );
    }
    let Some(build) = report.build else {
        return Outcome::refused("lengua could not read the program".to_string());
    };
    let Some(sealed) = verified.sealed() else {
        return Outcome::refused("the lock seals no code to run".to_string());
    };
    let host = match Host::new(&crate::host::config()) {
        Ok(host) => host,
        Err(message) => return Outcome::refused(message),
    };
    match lengua_seal::load(host.engine(), sealed) {
        Ok(component) => host.run(folder, &component, &build, input, Streams::Memory),
        Err(message) => Outcome::refused(message),
    }
}

pub(crate) fn serve(args: &[String]) -> ExitCode {
    let [kind, lock_path, folder, root, mode, arguments @ ..] = args else {
        eprintln!(
            "usage: lengua-mcp worker <kind> <lock> <folder> <root> <fixed | live> [arguments...]"
        );
        return ExitCode::from(2);
    };
    let mut stdin = Vec::new();
    let _ = std::io::stdin()
        .take(u64::try_from(OUTPUT).unwrap_or(u64::MAX))
        .read_to_end(&mut stdin);
    let input = Input {
        arguments: arguments.to_vec(),
        stdin,
        fixed: mode == "fixed",
        root: Some(Path::new(root).to_path_buf()),
    };
    let result = outcome(kind, Path::new(lock_path), Path::new(folder), &input);
    let mut out = std::io::stdout().lock();
    if out
        .write_all(&encode(&result))
        .and_then(|()| out.flush())
        .is_err()
    {
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}

#[cfg(test)]
mod tests {
    use super::{decode, encode};
    use lengua_host::{Ending, Outcome};

    #[test]
    fn an_outcome_goes_through_the_frame_and_back() {
        for ending in [
            Ending::Exited(3),
            Ending::OutOfTime("late\nvery".to_string()),
            Ending::OutOfMemory,
            Ending::Trapped("a trap".to_string()),
            Ending::Refused(String::new()),
        ] {
            let outcome = Outcome {
                ending,
                stdout: b"out\n\0".to_vec(),
                stderr: b"err".to_vec(),
            };
            assert_eq!(decode(&encode(&outcome)), Some(outcome));
        }
        assert_eq!(decode(b"exited 0 0 0 0\nextra"), None);
        assert_eq!(decode(b"other 0 0 0 0\n"), None);
        assert_eq!(decode(b"exited 0 5 0 0\nab"), None);
    }
}
