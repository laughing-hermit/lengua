use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use http_body_util::BodyExt;
use lengua_idea::Native;
use wasmtime_wasi_http::{Error as HttpError, WasiBody};

use crate::OUTPUT;
use crate::outgoing::{Reply, reply, whole};

const TEXT: &str = "text/plain; charset=utf-8";
const POLL: Duration = Duration::from_millis(5);

fn text(status: u16, message: &str) -> Reply {
    reply(status, TEXT, 0, whole(message.as_bytes().to_vec()))
}

fn read_all(pipe: Option<impl Read + Send + 'static>) -> Option<thread::JoinHandle<Vec<u8>>> {
    pipe.map(|pipe| {
        thread::spawn(move || {
            let mut bytes = Vec::new();
            let most = u64::try_from(OUTPUT).unwrap_or(u64::MAX);
            let _ = pipe.take(most).read_to_end(&mut bytes);
            bytes
        })
    })
}

pub(crate) fn run_part(part: &Native, input: &[u8], limit: Duration) -> Result<(bool, Vec<u8>), String> {
    let bytes = std::fs::read(&part.binary)
        .map_err(|e| format!("lengua could not read the native part `{}`: {e}", part.name))?;
    if lengua_idea::sha256_hex(&bytes) != part.binary_sha256 {
        return Err(format!(
            "the native part `{}` is not as its lock signed it; call check on its folder",
            part.name
        ));
    }
    let place = part.binary.parent().unwrap_or(&part.binary);
    let mut child = Command::new(&part.binary)
        .current_dir(place)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("lengua could not start the native part `{}`: {e}", part.name))?;
    let given = input.to_vec();
    let writer = child.stdin.take().map(|mut pipe| {
        thread::spawn(move || {
            let _ = pipe.write_all(&given);
        })
    });
    let out = read_all(child.stdout.take());
    let err = read_all(child.stderr.take());
    let started = Instant::now();
    let mut ended = None;
    while ended.is_none() {
        match child.try_wait() {
            Ok(Some(status)) => ended = Some(status.success()),
            Ok(None) if started.elapsed() < limit => thread::sleep(POLL),
            Ok(None) | Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                ended = Some(false);
            }
        }
    }
    if let Some(writer) = writer {
        let _ = writer.join();
    }
    let stdout = out.and_then(|r| r.join().ok()).unwrap_or_default();
    let stderr = err.and_then(|r| r.join().ok()).unwrap_or_default();
    let said = String::from_utf8_lossy(&stderr);
    if let Some(first) = said.lines().next().filter(|l| !l.trim().is_empty()) {
        eprintln!("lengua: the native part `{}` says: {first}", part.name);
    }
    Ok(match ended {
        Some(true) => (true, stdout),
        _ => (false, stderr),
    })
}

pub(crate) async fn call(request: hyper::Request<WasiBody>, part: Native, limit: Duration) -> Reply {
    if request.method() != hyper::Method::POST {
        return text(405, "a native part answers only `POST`: the body is its standard input");
    }
    let bytes = request.into_body().collect().await?.to_bytes().to_vec();
    let found = tokio::task::spawn_blocking(move || run_part(&part, &bytes, limit))
        .await
        .map_err(|e| HttpError::InternalError(Some(e.to_string())))?;
    match found {
        Ok((true, out)) => reply(200, TEXT, 0, whole(out)),
        Ok((false, err)) => reply(502, TEXT, 0, whole(err)),
        Err(message) => text(503, &message),
    }
}
