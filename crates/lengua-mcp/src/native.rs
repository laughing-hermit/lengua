use std::io::{Read, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use lengua_host::{Ending, OUTPUT, Outcome};

const POLL: Duration = Duration::from_millis(5);

fn drain(pipe: Option<impl Read + Send + 'static>) -> Option<thread::JoinHandle<Vec<u8>>> {
    pipe.map(|pipe| {
        thread::spawn(move || {
            let mut bytes = Vec::new();
            let most = u64::try_from(OUTPUT).unwrap_or(u64::MAX);
            let _ = pipe.take(most).read_to_end(&mut bytes);
            bytes
        })
    })
}

pub(crate) fn run(
    binary: &Path,
    arguments: &[String],
    stdin: &[u8],
    root: &Path,
    limit: Duration,
) -> Outcome {
    let spawned = Command::new(binary)
        .args(arguments)
        .current_dir(root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn();
    let mut child = match spawned {
        Ok(child) => child,
        Err(error) => {
            return Outcome::refused(format!(
                "lengua could not start the native part `{}`: {error}",
                binary.display()
            ));
        }
    };
    let input = stdin.to_vec();
    let writer = child.stdin.take().map(|mut pipe| {
        thread::spawn(move || {
            let _ = pipe.write_all(&input);
        })
    });
    let out = drain(child.stdout.take());
    let err = drain(child.stderr.take());
    let started = Instant::now();
    let mut status = None;
    loop {
        match child.try_wait() {
            Ok(Some(found)) => {
                status = Some(found);
                break;
            }
            Err(_) => break,
            Ok(None) if started.elapsed() >= limit => {
                let _ = child.kill();
                let _ = child.wait();
                break;
            }
            Ok(None) => thread::sleep(POLL),
        }
    }
    if let Some(writer) = writer {
        let _ = writer.join();
    }
    let stdout = out.and_then(|r| r.join().ok()).unwrap_or_default();
    let stderr = err.and_then(|r| r.join().ok()).unwrap_or_default();
    let ending = match status.map(|s| s.code()) {
        Some(Some(code)) => Ending::Exited(code),
        Some(None) => Ending::Trapped("the native part was stopped by a signal".to_string()),
        None => Ending::OutOfTime(
            "the native part ran past the time of its idea, and lengua stopped its process"
                .to_string(),
        ),
    };
    Outcome {
        ending,
        stdout,
        stderr,
    }
}
