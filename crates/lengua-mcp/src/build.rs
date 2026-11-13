use std::collections::HashMap;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread;
use std::time::Instant;

use lengua_idea::{Build, Label, Trust};

use crate::doctor;
use crate::host::{Host, Seal, grant_for, outside};
use crate::{harness, mutation, testing};

pub(crate) const THRESHOLD: usize = 80;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Problem {
    pub file: String,
    pub line: usize,
    pub code: String,
    pub message: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Remark {
    pub file: String,
    pub line: Option<usize>,
    pub message: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Component {
    pub path: PathBuf,
    pub size: u64,
    pub sha256: String,
    pub seal: Option<Seal>,
    pub tested: String,
    pub trusted: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Stage {
    Building,
    BuildFailed,
    Testing,
    TestsFailed,
    Mutating,
    Done,
}

impl Stage {
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Building => "building",
            Self::BuildFailed => "build_failed",
            Self::Testing => "testing",
            Self::TestsFailed => "tests_failed",
            Self::Mutating => "mutating",
            Self::Done => "done",
        }
    }

    fn running(self) -> bool {
        matches!(self, Self::Building | Self::Testing | Self::Mutating)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Tests {
    pub total: usize,
    pub run: usize,
    pub failed: usize,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Mutants {
    pub total: usize,
    pub done: usize,
    pub caught: usize,
    pub survived: usize,
    pub unbuilt: usize,
    pub same: usize,
    pub cached: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Status {
    pub stage: Stage,
    pub seconds: u64,
    pub component: Option<Component>,
    pub problems: Vec<Problem>,
    pub remarks: Vec<Remark>,
    pub tests: Tests,
    pub mutants: Mutants,
    pub toolchain: String,
}

impl Status {
    fn new(stage: Stage) -> Self {
        Self {
            stage,
            seconds: 0,
            component: None,
            problems: Vec::new(),
            remarks: Vec::new(),
            tests: Tests::default(),
            mutants: Mutants::default(),
            toolchain: String::new(),
        }
    }

    pub(crate) fn score(&self) -> (usize, usize) {
        let caught = self.mutants.caught;
        (caught, caught + self.mutants.survived)
    }

    pub(crate) fn trust(&self) -> Trust {
        if self.stage != Stage::Done {
            return Trust::default();
        }
        let (caught, counted) = self.score();
        let label = if counted == 0 || caught * 100 >= THRESHOLD * counted {
            Label::Checked
        } else {
            Label::Tested
        };
        Trust {
            label,
            score: Some((caught, counted)),
        }
    }
}

pub(crate) type Shared = Arc<Mutex<Status>>;
pub(crate) type Slot = Arc<Mutex<Option<Child>>>;

struct Job {
    folder: PathBuf,
    inputs: String,
    started: Instant,
    state: Shared,
    child: Slot,
    stopped: Arc<AtomicBool>,
}

#[derive(Clone)]
pub(crate) struct Builds {
    jobs: Arc<Mutex<HashMap<PathBuf, Job>>>,
    host: Host,
}

pub(crate) fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

pub(crate) fn problem(file: &str, line: usize, code: &str, message: String) -> Problem {
    Problem {
        file: file.to_string(),
        line,
        code: code.to_string(),
        message,
    }
}

pub(crate) struct Worker {
    pub folder: PathBuf,
    pub build: Build,
    pub host: Host,
    pub state: Shared,
    pub child: Slot,
    pub stopped: Arc<AtomicBool>,
    pub started: Instant,
}

impl Worker {
    pub(crate) fn update(&self, change: impl FnOnce(&mut Status)) {
        change(&mut lock(&self.state));
    }

    pub(crate) fn stopped(&self) -> bool {
        self.stopped.load(Ordering::Relaxed)
    }

    pub(crate) fn finish(&self, stage: Stage, problems: Vec<Problem>) {
        let seconds = self.started.elapsed().as_secs();
        self.update(|s| {
            s.stage = stage;
            s.problems = problems;
            s.seconds = seconds;
        });
    }

    fn work(&self) {
        let component = match compile(&self.folder, &self.build, &self.child, &self.host) {
            Ok(component) => component,
            Err(problems) => return self.finish(Stage::BuildFailed, problems),
        };
        let screens: Vec<Remark> = self
            .build
            .tests
            .iter()
            .filter(|t| t.screen)
            .map(|t| Remark {
                file: t.file.clone(),
                line: Some(t.line),
                message: format!(
                    "the screen example `{}` runs at L8, with GPUI's test context; it was not run",
                    t.name
                ),
            })
            .collect();
        let cases = testing::cases(&self.build);
        let seal = component.seal.clone();
        self.update(|s| {
            s.component = Some(component);
            s.stage = Stage::Testing;
            s.remarks = screens;
            s.tests.total = cases.len();
        });
        let Some(seal) = seal else {
            return self.finish(Stage::Done, Vec::new());
        };
        let mut problems = Vec::new();
        for (index, case) in cases.iter().enumerate() {
            if self.stopped() {
                return;
            }
            let failed = testing::run_case(&self.folder, &self.build, &seal.lock, (index, case));
            if let Some(detail) = &failed {
                problems.push(testing::problem(case, detail));
            }
            self.update(|s| {
                s.tests.run += 1;
                s.tests.failed += usize::from(failed.is_some());
            });
        }
        if !problems.is_empty() {
            return self.finish(Stage::TestsFailed, problems);
        }
        self.update(|s| s.stage = Stage::Mutating);
        if mutation::run(self, &cases) {
            self.finish(Stage::Done, Vec::new());
        }
    }
}

impl Job {
    fn status(&self) -> Status {
        let mut status = lock(&self.state).clone();
        if status.stage.running() {
            status.seconds = self.started.elapsed().as_secs();
        }
        status
    }

    fn current(&self, inputs: &str) -> bool {
        if self.inputs != inputs {
            return false;
        }
        let state = lock(&self.state);
        state.component.as_ref().is_none_or(|c| {
            c.path.is_file()
                && c.seal
                    .as_ref()
                    .is_none_or(|s| s.lock.is_file() && self.folder.join(&s.sealed).is_file())
        })
    }

    fn stop(&self) {
        self.stopped.store(true, Ordering::Relaxed);
        if let Some(child) = lock(&self.child).as_mut() {
            let _ = child.kill();
        }
    }

    fn start(folder: &Path, build: &Build, host: &Host) -> Self {
        let job = Self {
            folder: folder.to_path_buf(),
            inputs: build.inputs.clone(),
            started: Instant::now(),
            state: Arc::new(Mutex::new(Status::new(Stage::Building))),
            child: Arc::new(Mutex::new(None)),
            stopped: Arc::new(AtomicBool::new(false)),
        };
        let worker = Worker {
            folder: folder.to_path_buf(),
            build: build.clone(),
            host: host.clone(),
            state: Arc::clone(&job.state),
            child: Arc::clone(&job.child),
            stopped: Arc::clone(&job.stopped),
            started: job.started,
        };
        thread::spawn(move || worker.work());
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

    pub(crate) fn snapshot(&self, folder: &Path, build: &Build) -> Option<Status> {
        let jobs = lock(&self.jobs);
        jobs.get(folder)
            .filter(|job| job.current(&build.inputs))
            .map(Job::status)
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
            let mut status = Status::new(Stage::BuildFailed);
            status.problems = vec![problem(
                "Cargo.toml",
                1,
                "toolchain-missing",
                doctor.problems(),
            )];
            return status;
        }
        let job = Job::start(folder, build, &self.host);
        lock(&job.state).toolchain = doctor.rustc();
        let status = job.status();
        jobs.insert(folder.to_path_buf(), job);
        status
    }
}

pub(crate) fn cargo(
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
                &relative(folder, path.trim().trim_start_matches(r"\\?\")),
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

fn compile(
    folder: &Path,
    build: &Build,
    slot: &Mutex<Option<Child>>,
    host: &Host,
) -> Result<Component, Vec<Problem>> {
    fetch(folder, build, slot)?;
    if let Ok((false, text)) = cargo(folder, &["fmt", "--check"], slot) {
        let found = format_problems(folder, &text);
        if !found.is_empty() {
            return Err(found);
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
    let building = [
        "build",
        "--release",
        "--target",
        target,
        "--offline",
        "--message-format=short",
    ];
    for args in [&clippy[..], &building[..]] {
        step(folder, args, slot)?;
    }
    if build.library {
        return library(folder, build, slot, host);
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

fn ungranted(
    bytes: &[u8],
    sha256: &str,
    world: &[String],
    host: &Host,
) -> Result<wasmtime::component::Component, Vec<Problem>> {
    if let Some(found) = too_large(bytes.len(), SIZE_LIMIT) {
        return Err(vec![found]);
    }
    host.component(bytes, sha256).map_err(|error| {
        vec![problem(
            "Cargo.toml",
            1,
            "component-refused",
            format!(
                "wasmtime did not take the component: {error}; lengua's host refuses threads, shared memory and relaxed SIMD, whose results can differ between machines"
            ),
        )]
    })
    .and_then(|component| {
        let found: Vec<Problem> = outside(&host.imports(&component), world)
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
            .collect();
        if found.is_empty() { Ok(component) } else { Err(found) }
    })
}

struct Part {
    bytes: Vec<u8>,
    sha256: String,
}

fn trusted_parts(folder: &Path, build: &Build, host: &Host) -> Result<(Vec<Part>, Vec<String>), Vec<Problem>> {
    let mut parts = Vec::new();
    let mut exports = Vec::new();
    let mut found = Vec::new();
    for part in &build.components {
        let Ok(bytes) = fs::read(folder.join(&part.file)) else {
            found.push(problem("idea.md", part.line, "component-missing", format!("lengua could not read the component `{}`", part.file)));
            continue;
        };
        let sha256 = lengua_idea::sha256_hex(&bytes);
        match host.component(&bytes, &sha256) {
            Ok(component) => {
                for import in outside(&host.imports(&component), &build.world) {
                    found.push(problem(
                        "idea.md",
                        part.line,
                        "component-import-ungranted",
                        format!("the trusted component `{}` imports `{import}`, which is not in the world the grants of the idea make; it runs under the program's grants only", part.file),
                    ));
                }
                exports.extend(host.exports(&component));
            }
            Err(error) => found.push(problem(
                "idea.md",
                part.line,
                "component-refused",
                format!("wasmtime did not take the component `{}`: {error}", part.file),
            )),
        }
        parts.push(Part { bytes, sha256 });
    }
    if found.is_empty() { Ok((parts, exports)) } else { Err(found) }
}

fn seal_failed(message: &str) -> Vec<Problem> {
    vec![problem(
        "Cargo.toml",
        1,
        "seal-failed",
        format!("lengua could not seal the precompiled code: {message}"),
    )]
}

fn built(folder: &Path, build: &Build, host: &Host) -> Result<Component, Vec<Problem>> {
    let component = folder.join(&build.artifact);
    let bytes = fs::read(&component).map_err(|error| {
        vec![problem(
            "Cargo.toml",
            1,
            "build-error",
            format!(
                "the build ended, but lengua could not read `{}`: {error}",
                build.artifact
            ),
        )]
    })?;
    let sha256 = lengua_idea::sha256_hex(&bytes);
    let (parts, exports) = trusted_parts(folder, build, host)?;
    let mut world = build.world.clone();
    world.extend(exports);
    ungranted(&bytes, &sha256, &world, host)?;
    crate::host::clear(folder);
    let mut trusted = Vec::new();
    for part in &parts {
        trusted.push(
            host.seal_code(folder, &part.bytes, &part.sha256)
                .map_err(|m| seal_failed(&m))?,
        );
    }
    let seal = host
        .seal(folder, &bytes, &sha256, &trusted)
        .map_err(|m| seal_failed(&m))?;
    Ok(Component {
        size: u64::try_from(bytes.len()).unwrap_or(u64::MAX),
        tested: sha256.clone(),
        sha256,
        seal: Some(seal),
        trusted,
        path: component,
    })
}

fn library(
    folder: &Path,
    build: &Build,
    slot: &Mutex<Option<Child>>,
    host: &Host,
) -> Result<Component, Vec<Problem>> {
    let path = folder.join(&build.artifact);
    let rlib = fs::read(&path).map_err(|error| {
        vec![problem(
            "Cargo.toml",
            1,
            "build-error",
            format!("the build ended, but lengua could not read `{}`: {error}", build.artifact),
        )]
    })?;
    let cases = testing::cases(build);
    let harness_build = build.harness();
    let spec = harness::Library {
        package: &build.package,
        dependency: &build.package,
        path: "../../..",
        harness: &harness_build.package,
    };
    harness::write(folder, harness::FOLDER, &spec, &cases)
        .map_err(|m| vec![problem("Cargo.toml", 1, "build-error", m)])?;
    let dir = folder.join(harness::FOLDER);
    let target_dir = folder.join("target").to_string_lossy().to_string();
    let args = [
        "build",
        "--release",
        "--target",
        build.target,
        "--offline",
        "--message-format=short",
        "--target-dir",
        target_dir.as_str(),
    ];
    match cargo(&dir, &args, slot) {
        Ok((true, _)) => {}
        Ok((false, text)) => return Err(harness::mapped(problems(&dir, &text), &cases)),
        Err(message) => return Err(vec![problem("Cargo.toml", 1, "build-error", message)]),
    }
    let bytes = fs::read(folder.join(&harness_build.artifact)).map_err(|error| {
        vec![problem("Cargo.toml", 1, "build-error", format!("lengua could not read the harness: {error}"))]
    })?;
    let sha256 = lengua_idea::sha256_hex(&bytes);
    ungranted(&bytes, &sha256, &harness_build.world, host)?;
    crate::host::clear(folder);
    let seal = host
        .seal(folder, &bytes, &sha256, &[])
        .map_err(|m| seal_failed(&m))?;
    Ok(Component {
        size: u64::try_from(rlib.len()).unwrap_or(u64::MAX),
        sha256: lengua_idea::sha256_hex(&rlib),
        tested: sha256,
        seal: Some(seal),
        trusted: Vec::new(),
        path,
    })
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
        let text = "Diff in /work/hello/src/main.rs:3:\n     fn main() {\n-    let x=1;\n+    let x = 1;\nDiff in /work/hello/src/cart.rs at line 9:\nsome other line\nDiff in \\\\?\\/work/hello/src/ui/view.rs:2:\n";
        let found = format_problems(Path::new("/work/hello"), text);
        let places: Vec<(&str, usize, &str)> = found
            .iter()
            .map(|p| (p.file.as_str(), p.line, p.code.as_str()))
            .collect();
        assert_eq!(
            places,
            [
                ("src/main.rs", 3, "rust-format"),
                ("src/cart.rs", 9, "rust-format"),
                ("src/ui/view.rs", 2, "rust-format")
            ]
        );
        assert_eq!(
            format_problems(Path::new("/w"), "error: expected `;`\n"),
            Vec::<super::Problem>::new()
        );
    }

    #[test]
    fn a_component_past_the_size_limit_is_refused() {
        assert!(too_large(SIZE_LIMIT, SIZE_LIMIT).is_none());
        let found = too_large(SIZE_LIMIT + 1, SIZE_LIMIT).expect("a problem");
        assert_eq!(found.code, "component-size");
        assert!(found.message.contains("64 MiB"), "{}", found.message);
    }
}
