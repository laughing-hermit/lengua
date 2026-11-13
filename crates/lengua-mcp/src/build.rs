use std::collections::HashMap;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread;
use std::time::Instant;

use lengua_idea::Build;

use crate::doctor;
use crate::host::{Host, grant_for, outside};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Problem {
    pub file: String,
    pub line: usize,
    pub code: String,
    pub message: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum State {
    Building,
    Built {
        component: PathBuf,
        size: u64,
        sha256: String,
    },
    Failed {
        problems: Vec<Problem>,
    },
}

pub(crate) struct Status {
    pub state: State,
    pub seconds: u64,
}

struct Job {
    inputs: String,
    started: Instant,
    state: Arc<Mutex<(State, u64)>>,
    child: Arc<Mutex<Option<Child>>>,
}

#[derive(Clone)]
pub(crate) struct Builds {
    jobs: Arc<Mutex<HashMap<PathBuf, Job>>>,
    host: Host,
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

fn problem(file: &str, line: usize, code: &str, message: String) -> Problem {
    Problem {
        file: file.to_string(),
        line,
        code: code.to_string(),
        message,
    }
}

impl Job {
    fn status(&self) -> Status {
        let (state, seconds) = lock(&self.state).clone();
        let seconds = if state == State::Building {
            self.started.elapsed().as_secs()
        } else {
            seconds
        };
        Status { state, seconds }
    }

    fn current(&self, inputs: &str) -> bool {
        if self.inputs != inputs {
            return false;
        }
        match &lock(&self.state).0 {
            State::Built { component, .. } => component.is_file(),
            State::Building | State::Failed { .. } => true,
        }
    }

    fn stop(&self) {
        if let Some(child) = lock(&self.child).as_mut() {
            let _ = child.kill();
        }
    }

    fn start(folder: &Path, build: &Build, host: &Host) -> Self {
        let job = Self {
            inputs: build.inputs.clone(),
            started: Instant::now(),
            state: Arc::new(Mutex::new((State::Building, 0))),
            child: Arc::new(Mutex::new(None)),
        };
        let folder = folder.to_path_buf();
        let build = build.clone();
        let state = Arc::clone(&job.state);
        let child = Arc::clone(&job.child);
        let started = job.started;
        let host = host.clone();
        thread::spawn(move || {
            let result = run(&folder, &build, &child, &host);
            *lock(&state) = (result, started.elapsed().as_secs());
        });
        job
    }
}

impl Builds {
    pub(crate) fn new(host: Host) -> Self {
        Self {
            jobs: Arc::new(Mutex::new(HashMap::new())),
            host,
        }
    }

    pub(crate) fn status(&self, folder: &Path, build: &Build) -> Status {
        let mut jobs = lock(&self.jobs);
        if let Some(job) = jobs.get(folder) {
            if job.current(&build.inputs) {
                return job.status();
            }
            job.stop();
        }
        jobs.remove(folder);
        let doctor = doctor::examine(folder);
        if !doctor.ok() {
            return Status {
                state: State::Failed {
                    problems: vec![problem(
                        "Cargo.toml",
                        1,
                        "toolchain-missing",
                        doctor.problems(),
                    )],
                },
                seconds: 0,
            };
        }
        let job = Job::start(folder, build, &self.host);
        let status = job.status();
        jobs.insert(folder.to_path_buf(), job);
        status
    }
}

fn cargo(
    folder: &Path,
    args: &[&str],
    slot: &Mutex<Option<Child>>,
) -> Result<(bool, String), String> {
    let mut child = Command::new("cargo")
        .args(args)
        .current_dir(folder)
        .env("CARGO_TERM_COLOR", "never")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("lengua could not start cargo: {e}"))?;
    let out_pipe = child.stdout.take();
    let err_pipe = child.stderr.take();
    *lock(slot) = Some(child);
    let reader = thread::spawn(move || {
        let mut bytes = Vec::new();
        if let Some(mut pipe) = out_pipe {
            let _ = pipe.read_to_end(&mut bytes);
        }
        bytes
    });
    let mut bytes = Vec::new();
    if let Some(mut pipe) = err_pipe {
        let _ = pipe.read_to_end(&mut bytes);
    }
    let mut all = reader.join().unwrap_or_default();
    all.extend_from_slice(&bytes);
    let taken = lock(slot).take();
    let status = taken
        .ok_or_else(|| "the build was stopped".to_string())?
        .wait()
        .map_err(|e| format!("lengua could not wait for cargo: {e}"))?;
    Ok((status.success(), String::from_utf8_lossy(&all).to_string()))
}

fn fetch_stamp(folder: &Path) -> PathBuf {
    folder.join("target").join("lengua").join("fetched")
}

fn fetch(folder: &Path, build: &Build, slot: &Mutex<Option<Child>>) -> Result<(), Vec<Problem>> {
    let manifest = fs::read(folder.join("Cargo.toml")).unwrap_or_default();
    let hash = lengua_idea::sha256_hex(&manifest);
    let stamp = fetch_stamp(folder);
    if build.crates == 0 || fs::read_to_string(&stamp).is_ok_and(|s| s == hash) {
        return Ok(());
    }
    match cargo(folder, &["fetch", "--target", build.target], slot) {
        Ok((true, _)) => {
            let _ = stamp.parent().map(fs::create_dir_all);
            let _ = fs::write(&stamp, hash);
            Ok(())
        }
        Ok((false, text)) => Err(problems(folder, &text)),
        Err(message) => Err(vec![problem("Cargo.toml", 1, "build-error", message)]),
    }
}

pub(crate) fn format_problems(folder: &Path, text: &str) -> Vec<Problem> {
    text.lines()
        .filter_map(|line| {
            let place = line
                .trim()
                .strip_prefix("Diff in ")?
                .trim_end_matches(':');
            let (path, number) = place
                .split_once(" at line ")
                .or_else(|| place.rsplit_once(':'))?;
            let number: usize = number.trim().parse().ok()?;
            Some(problem(
                &relative(folder, path.trim()),
                number,
                "rust-format",
                format!("the code is not as `rustfmt` writes it, from line {number}; run `cargo fmt` in the program's folder"),
            ))
        })
        .collect()
}

fn step(folder: &Path, args: &[&str], slot: &Mutex<Option<Child>>) -> Result<(), Vec<Problem>> {
    match cargo(folder, args, slot) {
        Ok((true, _)) => Ok(()),
        Ok((false, text)) => Err(problems(folder, &text)),
        Err(message) => Err(vec![problem("Cargo.toml", 1, "build-error", message)]),
    }
}

fn run(folder: &Path, build: &Build, slot: &Mutex<Option<Child>>, host: &Host) -> State {
    if let Err(problems) = fetch(folder, build, slot) {
        return State::Failed { problems };
    }
    if let Ok((false, text)) = cargo(folder, &["fmt", "--check"], slot) {
        let found = format_problems(folder, &text);
        if !found.is_empty() {
            return State::Failed { problems: found };
        }
    }
    let target = build.target;
    let clippy = [
        "clippy",
        "--release",
        "--target",
        target,
        "--offline",
        "--message-format=short",
        "--",
        "-D",
        "warnings",
    ];
    let compile = [
        "build",
        "--release",
        "--target",
        target,
        "--offline",
        "--message-format=short",
    ];
    for args in [&clippy[..], &compile[..]] {
        if let Err(problems) = step(folder, args, slot) {
            return State::Failed { problems };
        }
    }
    built(folder, build, host)
}

pub(crate) const SIZE_LIMIT: usize = 64 << 20;

pub(crate) fn too_large(size: usize, limit: usize) -> Option<Problem> {
    (size > limit).then(|| {
        problem(
            "Cargo.toml",
            1,
            "component-size",
            format!("the component has {size} bytes, and lengua's limit is {limit} bytes ({} MiB); make the program smaller", limit >> 20),
        )
    })
}

fn ungranted(bytes: &[u8], sha256: &str, build: &Build, host: &Host) -> Vec<Problem> {
    let wasm = Path::new(&build.artifact)
        .extension()
        .is_some_and(|e| e == "wasm");
    if !wasm {
        return Vec::new();
    }
    if let Some(found) = too_large(bytes.len(), SIZE_LIMIT) {
        return vec![found];
    }
    let component = match host.component(bytes, sha256) {
        Ok(component) => component,
        Err(error) => {
            return vec![problem(
                "Cargo.toml",
                1,
                "component-refused",
                format!("wasmtime did not take the component: {error}; lengua's host refuses threads, shared memory and relaxed SIMD, whose results can differ between machines"),
            )];
        }
    };
    outside(&host.imports(&component), &build.world)
        .into_iter()
        .map(|import| {
            let advice = grant_for(&import).map_or_else(
                || "; no grant allows it, so change the code so that it does not use it".to_string(),
                |grant| format!("; to allow it, add {grant} to `# Constraints`, or change the code so that it does not use it"),
            );
            problem(
                "idea.md",
                1,
                "import-ungranted",
                format!(
                    "the component imports `{import}`, which is not in the world the grants of the idea make, so the host does not link it{advice}"
                ),
            )
        })
        .collect()
}

fn built(folder: &Path, build: &Build, host: &Host) -> State {
    let component = folder.join(&build.artifact);
    match fs::read(&component) {
        Ok(bytes) => {
            let sha256 = lengua_idea::sha256_hex(&bytes);
            let problems = ungranted(&bytes, &sha256, build, host);
            if !problems.is_empty() {
                return State::Failed { problems };
            }
            State::Built {
                size: u64::try_from(bytes.len()).unwrap_or(u64::MAX),
                sha256,
                component,
            }
        }
        Err(error) => State::Failed {
            problems: vec![problem(
                "Cargo.toml",
                1,
                "build-error",
                format!(
                    "the build ended, but lengua could not read `{}`: {error}",
                    build.artifact
                ),
            )],
        },
    }
}

fn relative(folder: &Path, path: &str) -> String {
    let path = Path::new(path);
    let inside = path.strip_prefix(folder).unwrap_or(path);
    inside.to_string_lossy().replace('\\', "/")
}

fn located(folder: &Path, line: &str) -> Option<Problem> {
    let (at, code) = [(": error", "rust-error"), (": warning", "rust-warning")]
        .into_iter()
        .find_map(|(level, code)| line.find(level).map(|at| (at, code)))?;
    let (place, rest) = line.split_at(at);
    let mut parts = place.rsplitn(3, ':');
    let column: usize = parts.next()?.parse().ok()?;
    let number: usize = parts.next()?.parse().ok()?;
    let path = parts.next()?;
    let message = rest.strip_prefix(": ")?;
    Some(problem(
        &relative(folder, path),
        number,
        code,
        format!("column {column}: {message}"),
    ))
}

pub(crate) fn problems(folder: &Path, text: &str) -> Vec<Problem> {
    let mut out = Vec::new();
    for line in text.lines().map(str::trim_end) {
        if let Some(found) = located(folder, line) {
            out.push(found);
        } else if line.starts_with("error") && !line.starts_with("error: could not compile") {
            out.push(problem("Cargo.toml", 1, "build-error", line.to_string()));
        }
    }
    if out.is_empty() {
        let tail: Vec<&str> = text.lines().rev().take(20).collect();
        let tail: Vec<&str> = tail.into_iter().rev().collect();
        out.push(problem(
            "Cargo.toml",
            1,
            "build-error",
            format!("the build failed: {}", tail.join("\n")),
        ));
    }
    out.sort_by(|a, b| (&a.file, a.line).cmp(&(&b.file, b.line)));
    out
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{SIZE_LIMIT, format_problems, problems, too_large};

    #[test]
    fn each_error_gives_its_file_and_line() {
        let text = "   Compiling hello v0.1.0 (/work/hello)\nsrc/main.rs:3:5: error[E0425]: cannot find value `x` in this scope\nsrc\\lib.rs:1:1: warning: unused import\n/work/hello/src/main.rs:9:2: error: unused variable: `y`\nerror: could not compile `hello` (bin \"hello\") due to 2 previous errors\n";
        let found = problems(Path::new("/work/hello"), text);
        let places: Vec<(&str, usize, &str)> = found
            .iter()
            .map(|p| (p.file.as_str(), p.line, p.code.as_str()))
            .collect();
        assert_eq!(
            places,
            [
                ("src/lib.rs", 1, "rust-warning"),
                ("src/main.rs", 3, "rust-error"),
                ("src/main.rs", 9, "rust-error"),
            ]
        );
        assert_eq!(
            found[1].message,
            "column 5: error[E0425]: cannot find value `x` in this scope"
        );
    }

    #[test]
    fn an_error_with_no_place_is_on_the_manifest() {
        let found = problems(
            Path::new("/w"),
            "error: failed to parse manifest at `/w/Cargo.toml`\n",
        );
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].file, "Cargo.toml");
        assert_eq!(found[0].code, "build-error");
        let silent = problems(Path::new("/w"), "something went wrong\n");
        assert!(silent[0].message.contains("something went wrong"));
    }

    #[test]
    fn a_format_difference_gives_its_file_and_line() {
        let text = "Diff in /work/hello/src/main.rs:3:\n     fn main() {\n-    let x=1;\n+    let x = 1;\nDiff in /work/hello/src/cart.rs at line 9:\nsome other line\n";
        let found = format_problems(Path::new("/work/hello"), text);
        let places: Vec<(&str, usize, &str)> = found
            .iter()
            .map(|p| (p.file.as_str(), p.line, p.code.as_str()))
            .collect();
        assert_eq!(
            places,
            [
                ("src/main.rs", 3, "rust-format"),
                ("src/cart.rs", 9, "rust-format")
            ]
        );
        assert!(format_problems(Path::new("/w"), "error: expected `;`\n").is_empty());
    }

    #[test]
    fn a_component_past_the_size_limit_is_refused() {
        assert!(too_large(SIZE_LIMIT, SIZE_LIMIT).is_none());
        let found = too_large(SIZE_LIMIT + 1, SIZE_LIMIT).expect("a problem");
        assert_eq!(found.code, "component-size");
        assert!(found.message.contains("64 MiB"), "{}", found.message);
    }
}
