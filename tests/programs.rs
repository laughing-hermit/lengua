//! The programs lengua ships, and the compile skill's templates and fast programs, go through the
//! server on every target. Each case is written once, with a cell for each target (`PerCpu`):
//! that CPU's twin of the program (a folder of `examples`, of `tests/fixtures` or of
//! `tests/fixtures/linux-aarch64`, a template, a fast program), what the case changes in it, and
//! whatever the product does with it that depends on the hardware. A program follows every rule
//! and passes its tests; the ones a case asks about are also proven, run, and held to what their
//! ideas say: files read and written, folders made and swept, a sleep answered at once, the
//! environment given, dice rolled with the tests' random bytes, a program built on another,
//! words counted for every length, text capitalized with vectors, and the benchmark's programs.
//!
//! A target added to lengua breaks every table until its cells are filled. A case is tried on
//! every target this computer runs, natively or through QEMU's user mode (where the kernel can't
//! hold a program to its idea's system calls and nothing is measured); the other targets' cells
//! are read for well-formedness on any computer, and tried on their own runners.

mod common;
mod per_target;

use std::path::Path;
use std::time::{Duration, Instant};

use common::{Program, Protocol, Session};
use lengua::platform::{CpuFeatures, Emulator, Machine, Profile, Target};
use lengua::targets::PerCpu;
use per_target::Way;
use serde_json::{Value, json};

/// The compile skill's assets, from the repository's root.
const ASSETS: &str = "plugin/skills/compile/assets";

/// A change to a file: every `from` in it becomes `to`.
type Edit = (&'static str, &'static str);

/// A program's idea and code: the files of the repository they are, and what is changed in each
/// before the program is checked.
#[derive(Debug, Clone)]
struct Files {
    /// The idea, from the repository's root.
    idea_file: String,
    /// The code, from the repository's root.
    code_file: String,
    /// What is changed in the idea.
    idea_edits: Vec<Edit>,
    /// What is changed in the code.
    code_edits: Vec<Edit>,
}

impl Files {
    /// The idea.md and main.s of a program folder.
    fn folder(folder: &str) -> Self {
        Self::pair(&format!("{folder}/idea.md"), &format!("{folder}/main.s"))
    }

    /// An idea and a code file, each by its path from the repository's root.
    fn pair(idea: &str, code: &str) -> Self {
        Self {
            idea_file: idea.to_owned(),
            code_file: code.to_owned(),
            idea_edits: Vec::new(),
            code_edits: Vec::new(),
        }
    }

    /// These files with the idea changed by `edits` instead.
    fn idea_changed(&self, edits: &[Edit]) -> Self {
        Self {
            idea_edits: edits.to_vec(),
            ..self.clone()
        }
    }

    /// These files with the code changed by `edits` instead.
    fn code_changed(&self, edits: &[Edit]) -> Self {
        Self {
            code_edits: edits.to_vec(),
            ..self.clone()
        }
    }

    /// The idea's text, with its changes.
    fn idea(&self) -> String {
        edited(
            &self.idea_file,
            repository(&self.idea_file),
            &self.idea_edits,
        )
    }

    /// The code's text, with its changes.
    fn code(&self) -> String {
        edited(
            &self.code_file,
            repository(&self.code_file),
            &self.code_edits,
        )
    }
}

/// A file of the repository, as text.
fn repository(path: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(path);
    std::fs::read_to_string(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

/// `text` with every edit made. Each must find its text, so that a typo in a cell fails at once.
fn edited(file: &str, mut text: String, edits: &[Edit]) -> String {
    for &(from, to) in edits {
        assert!(text.contains(from), "{file} has no {from:?}");
        text = text.replace(from, to);
    }
    text
}

/// The twins of an example: x86-64's in `examples`, ARM64's among the fixtures. Both are compiled
/// from the very same idea.
fn example(name: &str) -> PerCpu<Files> {
    PerCpu {
        x86_64: Files::folder(&format!("examples/{name}")),
        aarch64: Files::folder(&format!("tests/fixtures/linux-aarch64/{name}")),
    }
}

/// The twins of a fixture.
fn fixture(name: &str) -> PerCpu<Files> {
    PerCpu {
        x86_64: Files::folder(&format!("tests/fixtures/{name}")),
        aarch64: Files::folder(&format!("tests/fixtures/linux-aarch64/{name}")),
    }
}

/// A template of the compile skill: one idea for both CPUs, and the code of each.
fn template(idea: &str, x86_64: &str, aarch64: &str) -> PerCpu<Files> {
    PerCpu {
        x86_64: Files::pair(&format!("{ASSETS}/{idea}"), &format!("{ASSETS}/{x86_64}")),
        aarch64: Files::pair(&format!("{ASSETS}/{idea}"), &format!("{ASSETS}/{aarch64}")),
    }
}

/// The skill's fast program `workload` for x86-64, with its own idea.
fn fast_x86_64(workload: &str) -> Files {
    Files::pair(
        &format!("{ASSETS}/fast/{workload}/idea.md"),
        &format!("{ASSETS}/fast/{workload}/main.s"),
    )
}

/// The skill's fast program `workload` for ARM64, which meets the idea of the x86-64 fast
/// program `speeds`: the program it speeds up, where the ideas of both CPUs' fast programs live.
fn fast_aarch64(speeds: &str, workload: &str) -> Files {
    Files::pair(
        &format!("{ASSETS}/fast/{speeds}/idea.md"),
        &format!("{ASSETS}/linux-aarch64/fast/{workload}/main.s"),
    )
}

fn text(value: &Value) -> &str {
    value.as_str().unwrap_or_default()
}

/// Whether any of `lines` contains `needle`.
fn any_line(lines: &Value, needle: &str) -> bool {
    lines
        .as_array()
        .is_some_and(|lines| lines.iter().any(|line| text(line).contains(needle)))
}

/// Whether the program follows every rule and passed its tests.
fn passed_all(report: &Value) -> bool {
    any_line(&report["tests"], "follows every rule and passed all")
}

/// Whether every proof line says "proven", as a line that says "tested, not proven" does too.
fn every_proof_says_proven(report: &Value) -> bool {
    report["proofs"]
        .as_array()
        .is_some_and(|proofs| proofs.iter().all(|proof| text(proof).contains("proven")))
}

fn solvers_installed() -> bool {
    let installed = std::process::Command::new("z3")
        .arg("--version")
        .output()
        .is_ok();
    if !installed {
        lengua::skiplog::note("z3 isn't installed");
    }
    installed
}

/// How a case's session proves.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Proving {
    /// Not at all: no time for proofs, so the case's time goes to its tests.
    No,
    /// With the proofs' full time (a runner gives 60 seconds), beside the other proving sessions.
    Full,
    /// With this many seconds for each version's proofs, beside the other proving sessions.
    For(u64),
    /// With this many seconds, once no other session proves and with the others kept from proving
    /// until it ends: a proof by induction must end within its time.
    AloneFor(u64),
}

/// The seconds a proof gets through QEMU's user mode when a case asks for the proofs' full time.
const EMULATED_SECONDS: u64 = 120;

/// A session for one target, and how this computer runs that target's programs.
struct Rig {
    session: Session,
    /// Natively, or through QEMU's user mode.
    way: Way,
    /// The CPU features the target's programs get here.
    features: CpuFeatures,
}

impl Rig {
    /// Ends the session.
    async fn close(self) {
        let session = self.session;
        session.service.cancel().await.expect("close the session");
    }
}

/// The CPU features a session for `target` runs programs with: this computer's, or under QEMU
/// the emulated CPU's.
fn features(target: Profile, way: Way) -> CpuFeatures {
    Target {
        profile: target,
        emulator: (way == Way::Emulated).then(|| Emulator {
            name: "qemu".into(),
        }),
    }
    .features(&Machine::detect())
}

/// A session for `target` as `proving` asks, where this computer runs the target's programs
/// (natively, or through QEMU's user mode) and, for proofs, the solvers are installed; otherwise
/// none, with a note in the run's summary. At most one is open at a time in a test, since a
/// session that proves alone holds the others back.
async fn rig_for(target: Profile, proving: Proving) -> Option<Rig> {
    let way = per_target::way(target)?;
    if proving != Proving::No && !solvers_installed() {
        return None;
    }
    let protocol = Protocol::Discover;
    let name = target.name();
    let session = match (way, proving) {
        (Way::Native, Proving::No) => Session::start(protocol).await,
        (Way::Native, Proving::Full) => Session::proving(protocol).await,
        (Way::Native, Proving::For(seconds)) => Session::proving_for(protocol, seconds).await,
        (Way::Native, Proving::AloneFor(seconds)) => {
            Session::proving_alone_for(protocol, seconds).await
        }
        (Way::Emulated, Proving::No) => Session::emulating(protocol, name).await,
        (Way::Emulated, Proving::Full) => {
            Session::emulating_for(protocol, name, EMULATED_SECONDS).await
        }
        (Way::Emulated, Proving::For(seconds) | Proving::AloneFor(seconds)) => {
            Session::emulating_for(protocol, name, seconds).await
        }
    };
    Some(Rig {
        session,
        way,
        features: features(target, way),
    })
}

/// A program folder holding `files`, its idea recorded.
async fn written(session: &Session, files: &Files) -> Program {
    let program = Program::new();
    program.write("idea.md", &files.idea());
    program.write("main.s", &files.code());
    session.record(&program).await;
    program
}

/// Checks until the job ends, which must be done. Once the grace for the proofs is over, a reply
/// can say done while the job is still writing the last proof to the lock, and says so ("still
/// running"): a case that reads the lock afterwards waits for the job to end (as `flow::done`
/// does, in tests/flow/mod.rs).
async fn done(what: &str, session: &Session, program: &Program) -> Value {
    let mut report = session.settle(program).await;
    for _ in 0..600 {
        if !text(&report["next_step"]).contains("still running") {
            break;
        }
        report = session.settle(program).await;
    }
    assert_eq!(report["state"], "done", "{what}: {report}");
    report
}

/// Checks until the job ends, which must be with failed tests; returns the problems.
async fn failed(what: &str, session: &Session, program: &Program) -> String {
    let report = session.settle(program).await;
    assert_eq!(report["state"], "tests_failed", "{what}: {report}");
    report["problems"]
        .as_array()
        .map(|problems| {
            problems
                .iter()
                .map(|problem| text(problem).to_owned())
                .collect::<Vec<_>>()
                .join("\n")
        })
        .unwrap_or_default()
}

fn with_args(program: &Program, args: &[&str]) -> Value {
    let mut arguments = program.arguments();
    arguments["args"] = json!(args);
    arguments
}

/// Runs the program with `stdin` as its standard input.
async fn run_with_stdin(session: &Session, program: &Program, stdin: &str) -> Value {
    let mut arguments = program.arguments();
    arguments["stdin"] = Value::from(stdin);
    session.call_ok("run", arguments).await
}

/// A run of a checked program: its arguments, and what it prints and exits with.
#[derive(Debug, Clone, Copy)]
struct Run {
    /// The arguments.
    args: &'static [&'static str],
    /// What it prints to standard output.
    stdout: &'static str,
    /// What it prints to standard error, unless this is empty, which asks for nothing.
    stderr: &'static str,
    /// The exit code.
    exit: i32,
}

/// A run that prints `stdout` and exits with 0.
fn prints(args: &'static [&'static str], stdout: &'static str) -> Run {
    Run {
        args,
        stdout,
        stderr: "",
        exit: 0,
    }
}

/// A run that prints nothing to standard output, `stderr` to standard error, and exits with
/// `exit`.
fn fails(args: &'static [&'static str], stderr: &'static str, exit: i32) -> Run {
    Run {
        args,
        stdout: "",
        stderr,
        exit,
    }
}

/// Runs the program with each of `runs` and holds it to what the run says.
async fn run_all(what: &str, session: &Session, program: &Program, runs: &[Run]) {
    for run in runs {
        let report = session.call_ok("run", with_args(program, run.args)).await;
        let context = format!("{what}, run with {:?}: {report}", run.args);
        assert_eq!(report["stdout"], run.stdout, "{context}");
        if !run.stderr.is_empty() {
            assert_eq!(report["stderr"], run.stderr, "{context}");
        }
        assert_eq!(report["exit_code"], run.exit, "{context}");
    }
}

/// A text on standard input, and what the program prints for it.
type Text = (&'static str, &'static str);

/// Runs the program with each of `texts` on its standard input and holds it to what it prints.
async fn filters(what: &str, session: &Session, program: &Program, texts: &[Text]) {
    for &(stdin, stdout) in texts {
        let run = run_with_stdin(session, program, stdin).await;
        assert_eq!(run["stdout"], stdout, "{what}, stdin {stdin:?}: {run}");
        assert_eq!(run["exit_code"], 0, "{what}, stdin {stdin:?}: {run}");
    }
}

/// What a report must say, checked all together: a proof that falls short in several ways shows
/// every one, not only the first, so that a run that costs minutes teaches the most it can.
struct Expect<'a> {
    /// The case, for the message.
    what: String,
    /// The report.
    report: &'a Value,
    /// What the report lacks.
    missing: Vec<String>,
}

impl<'a> Expect<'a> {
    fn new(what: &str, report: &'a Value) -> Self {
        Self {
            what: what.to_owned(),
            report,
            missing: Vec::new(),
        }
    }

    /// Notes `lacks` unless `holds`.
    fn that(&mut self, holds: bool, lacks: &str) {
        if !holds {
            self.missing.push(lacks.to_owned());
        }
    }

    /// Fails with everything the report lacks, if it lacks anything.
    fn finish(self) {
        assert!(
            self.missing.is_empty(),
            "{}: the report lacks {:#?}\nthe report: {}",
            self.what,
            self.missing,
            self.report
        );
    }
}

/// What a proof line says of a block.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Verdict {
    /// Proven for every input in range.
    Proven,
    /// Tested, not proven: the proof ran out of time or could not settle a path. No cell says
    /// this today, since ARM64's prover settles what x86-64's does; a target whose prover
    /// settles less pins its stop here, and a test then fails the day it settles more.
    #[allow(dead_code)]
    NotProven,
}

impl Verdict {
    /// Whether `proof`, a line of a report's proofs, says this.
    fn says(self, proof: &Value) -> bool {
        match self {
            Self::Proven => text(proof).contains("proven for every input in range"),
            Self::NotProven => text(proof).contains("tested, not proven"),
        }
    }
}

/// What a report's proofs say of a program's blocks, one after the other, and its memory line.
struct Proofs {
    /// The verdict on each block, in order.
    blocks: &'static [Verdict],
    /// What the memory line contains; nothing in particular when this is empty.
    memory: &'static str,
}

impl Proofs {
    /// Notes in `expect` what `report` lacks of this.
    fn held_by(&self, report: &Value, expect: &mut Expect<'_>) {
        for (index, verdict) in self.blocks.iter().enumerate() {
            expect.that(
                verdict.says(&report["proofs"][index]),
                &format!("block {index}: {verdict:?}"),
            );
        }
        expect.that(
            text(&report["memory"]).contains(self.memory),
            &format!("memory: {}", self.memory),
        );
    }
}

/// An optional extension of a CPU that a program may need.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Needs {
    /// Nothing beyond the target's baseline.
    Nothing,
    /// AVX-512's byte and word instructions, which x86-64 CPUs have only some of.
    Avx512bw,
    /// Scalable vectors, which ARM64 CPUs have only some of.
    Sve,
}

impl Needs {
    /// Why a CPU with `features` can't run a program that needs this, if it can't.
    fn lacked_by(self, features: &CpuFeatures) -> Option<&'static str> {
        match self {
            Self::Nothing => None,
            Self::Avx512bw => (!features.avx512bw).then_some("this CPU has no AVX-512BW"),
            Self::Sve => (features.sve == 0).then_some("this CPU has no SVE"),
        }
    }
}

/// A program of a tier case, where the CPUs have programs of their own for the same idea: what a
/// message calls it, its files and what it needs of the CPU.
struct Variant {
    name: &'static str,
    files: Files,
    needs: Needs,
}

fn variant(name: &'static str, files: Files, needs: Needs) -> Variant {
    Variant { name, files, needs }
}

/// Takes each variant through the server where the CPU has what it needs: it follows every rule
/// and passes its tests. A variant the CPU lacks the extension of is noted, never silent.
async fn variants_pass(target: Profile, rig: &Rig, variants: &[Variant]) {
    for variant in variants {
        let what = format!("{} on {}", variant.name, target.name());
        if let Some(lacks) = variant.needs.lacked_by(&rig.features) {
            lengua::skiplog::note(lacks);
            continue;
        }
        let program = written(&rig.session, &variant.files).await;
        let report = done(&what, &rig.session, &program).await;
        assert!(passed_all(&report), "{what}: {report}");
    }
}

/// Each target's twin of a program follows every rule and passes its tests, with no time for
/// proofs.
async fn pass(name: &str, programs: PerCpu<Files>) {
    for target in Profile::ALL {
        let Some(rig) = rig_for(target, Proving::No).await else {
            continue;
        };
        let what = format!("{name} on {}", target.name());
        let program = written(&rig.session, programs.get(target)).await;
        let report = done(&what, &rig.session, &program).await;
        assert!(passed_all(&report), "{what}: {report}");
        rig.close().await;
    }
}

// The decimals: exact cents, and doubles.

/// mean computes in exact cents and hypotenuse in doubles, with the CPU's square root: each
/// passes its tests and prints what the idea says for decimal arguments.
#[tokio::test]
async fn the_decimal_examples_pass_their_tests() {
    let cases = [
        ("mean", example("mean"), prints(&["2.5", "3.76"], "3.13\n")),
        (
            "hypotenuse",
            example("hypotenuse"),
            prints(&["3", "4"], "5.000\n"),
        ),
    ];
    for target in Profile::ALL {
        let Some(rig) = rig_for(target, Proving::No).await else {
            continue;
        };
        for (name, programs, run) in &cases {
            let what = format!("{name} on {}", target.name());
            let program = written(&rig.session, programs.get(target)).await;
            done(&what, &rig.session, &program).await;
            run_all(&what, &rig.session, &program, &[*run]).await;
        }
        rig.close().await;
    }
}

/// The hypotenuse computes in doubles and passes every test, its decimals rounded as the formula
/// rounds them, and its doubles are proven the formula's for whole sides and for decimal ones.
#[tokio::test]
async fn the_hypotenuse_computes_in_doubles() {
    let programs = example("hypotenuse");
    let runs = [
        prints(&["3", "4"], "5.000\n"),
        prints(&["1.5", "2.25"], "2.704\n"),
        prints(&["0.001", "0.001"], "0.001\n"),
        prints(&["1000000", "1000000"], "1414213.562\n"),
        fails(&["3", "four"], "error: side is not a number\n", 2),
    ];
    for target in Profile::ALL {
        let Some(rig) = rig_for(target, Proving::For(120)).await else {
            continue;
        };
        let session = &rig.session;
        let what = format!("hypotenuse on {}", target.name());
        let program = written(session, programs.get(target)).await;
        let report = done(&what, session, &program).await;
        let mut expect = Expect::new(&what, &report);
        expect.that(passed_all(&report), "a program that follows every rule");
        let proofs: Vec<&Value> = report["proofs"].as_array().into_iter().flatten().collect();
        expect.that(proofs.len() == 5, "a proof for each of the 5 blocks");
        for (number, proof) in proofs.into_iter().enumerate() {
            expect.that(
                text(proof).starts_with(&format!(
                    "generated B{}: proven for every input in range",
                    number + 1
                )),
                &format!("B{} proven for every input in range", number + 1),
            );
        }
        expect.that(
            text(&report["memory"]).contains("the proofs of B1, B2, B3, B4 and B5 cover"),
            "memory held on every run the proofs of B1 to B5 cover",
        );
        expect.finish();
        run_all(&what, session, &program, &runs).await;
        rig.close().await;
    }
}

/// What each target's proofs of mean say, block by block: the decimal means (B1), the whole ones
/// (B2), the decimal means again with the other sign (B3), and the three errors. Every block is
/// proven for every input in range on both CPUs: x86-64 in 414 seconds on a 4-vCPU runner (run
/// 41914657858), ARM64 in about 290 (run 41947013349; its B1 and B3 ask the solvers, every
/// question confirmed by z3 and cvc5, and the rest settles by its terms). ARM64's first three
/// were not proven until the prover's model of AArch64 read the overflow flag, `tbnz` on the top
/// bit and `msub` as x86-64's reads them: a path of each needed a solver question that z3 could
/// not answer in the time ("a path couldn't be settled: z3 ran out of time" after 600 seconds,
/// run 41914663693).
const MEAN: PerCpu<Proofs> = PerCpu {
    x86_64: Proofs {
        blocks: &[Verdict::Proven; 6],
        memory: "",
    },
    aarch64: Proofs {
        blocks: &[Verdict::Proven; 6],
        memory: "",
    },
};

/// How long each target's proofs of mean get: x86-64 needs seven minutes on a 4-vCPU runner, so
/// it gets fifteen for the pool's slower draws; ARM64 is given ten and needs about five.
const MEAN_PROVING: PerCpu<Proving> = PerCpu {
    x86_64: Proving::For(900),
    aarch64: Proving::For(600),
};

/// About three minutes with 16 cores, seven on a 4-vCPU runner: mean's two blocks of decimal
/// means take over a minute each.
#[tokio::test]
#[ignore = "slow: the proof takes minutes"]
async fn the_decimal_example_is_proven() {
    // mean takes decimals and prints exact cents: scaled(a, 2) and places(x, 2) in its tests.
    let programs = example("mean");
    for target in Profile::ALL {
        let Some(rig) = rig_for(target, *MEAN_PROVING.get(target)).await else {
            continue;
        };
        let what = format!("mean on {}", target.name());
        let program = written(&rig.session, programs.get(target)).await;
        let report = done(&what, &rig.session, &program).await;
        let mut expect = Expect::new(&what, &report);
        MEAN.get(target).held_by(&report, &mut expect);
        expect.finish();
        rig.close().await;
    }
}

// The files, folders, sleep, environment and dice.

/// upcase reads one file and writes another; its tests give files and check the files left, and
/// run on a folder of the user's it works on the files there, and only there.
#[tokio::test]
async fn the_file_example_reads_and_writes_files() {
    let programs = example("upcase");
    for target in Profile::ALL {
        let Some(rig) = rig_for(target, Proving::No).await else {
            continue;
        };
        let session = &rig.session;
        let what = format!("upcase on {}", target.name());
        let program = written(session, programs.get(target)).await;
        let report = done(&what, session, &program).await;
        assert!(
            text(&report["example"]).contains("left out.txt holding \"HELLO, WORLD 42!\\n\""),
            "{what}: {report}"
        );

        // Run on a folder of the user's: the program works on the files there, and only there.
        let user = tempfile::tempdir().expect("a user folder");
        std::fs::write(user.path().join("notes.txt"), "Hello, files!\n").expect("a user file");
        let mut arguments = with_args(&program, &["notes.txt", "loud.txt"]);
        arguments["folder"] = json!(user.path());
        let run = session.call_ok("run", arguments.clone()).await;
        assert_eq!(run["exit_code"], 0, "{what}: {run}");
        assert_eq!(
            std::fs::read_to_string(user.path().join("loud.txt")).expect("written"),
            "HELLO, FILES!\n",
            "{what}"
        );
        arguments["args"] = json!(["/etc/hostname", "loud.txt"]);
        let outside = session.call_ok("run", arguments.clone()).await;
        assert_eq!(
            outside["exit_code"], 2,
            "{what}: files outside the folder can't be opened: {outside}"
        );
        arguments["folder"] = json!("relative/folder");
        let error = session.call_error("run", arguments).await;
        assert!(
            error.contains("folder must be an absolute path"),
            "{what}: {error}"
        );
        rig.close().await;
    }
}

/// upcase's usage and missing-file behaviors draw their names with choice(…); the proof opens,
/// reads and writes files as the test gives them.
#[tokio::test]
async fn the_file_example_proves_its_file_handling() {
    let programs = example("upcase");
    for target in Profile::ALL {
        // Two minutes, so a machine busy with other tests' proofs still proves some lengths: the
        // model's runs of the samples, mebibyte files among them, come out of the same time.
        let Some(rig) = rig_for(target, Proving::For(120)).await else {
            continue;
        };
        let session = &rig.session;
        let what = format!("upcase on {}", target.name());
        let program = written(session, programs.get(target)).await;
        let report = done(&what, session, &program).await;
        // Usage (B2), a missing input (B3) and a file over the limit (B4, a mebibyte of any of its
        // characters) are proven; capitalizing a file (B1) is proven for every file up to the
        // length the proof's time reaches, of the mebibyte the idea allows.
        let proofs = &report["proofs"];
        let first = text(&proofs[0]);
        assert!(
            first.starts_with("generated B1: proven for every input with text up to ")
                && first.contains(" of 1048576 bytes (")
                && first.contains("; check with prove_further to prove on from there"),
            "{what}: {report}"
        );
        let reached = |line: &str| -> usize {
            line["generated B1: proven for every input with text up to ".len()..]
                .split(' ')
                .next()
                .and_then(|bytes| bytes.parse().ok())
                .expect("a length")
        };
        let before = reached(first);
        for index in 1..4 {
            assert!(
                text(&proofs[index]).contains("proven for every input in range"),
                "{what}: {report}"
            );
        }
        assert!(
            text(&proofs[3]).contains("(1 input combination, 1 path through"),
            "{what}: {report}"
        );
        let lock = program.read("lengua.lock").expect("the lock");
        assert!(
            lock.contains(&format!(
                "bounded = [{{ id = \"B1\", bytes = {before}, of = 1048576, more = true }}]"
            )),
            "{what}: {lock}"
        );
        // Proving further goes on from there, and the lock keeps the rest.
        let mut arguments = program.arguments();
        arguments["prove_further"] = json!(true);
        let report = session.settle_with(&program, arguments).await;
        assert_eq!(report["state"], "done", "{what}: {report}");
        let after = reached(text(&report["proofs"][0]));
        assert!(after > before, "{what}: {before} then {after}: {report}");
        for index in 1..4 {
            assert!(
                text(&report["proofs"][index]).contains("proven for every input in range"),
                "{what}: {report}"
            );
        }
        rig.close().await;
    }
}

/// A program makes a folder and moves a file inside its own folder, and the proof follows both.
#[tokio::test]
async fn a_program_makes_folders_and_moves_files_inside_its_own() {
    let programs = example("archive");
    for target in Profile::ALL {
        let Some(rig) = rig_for(target, Proving::Full).await else {
            continue;
        };
        let what = format!("archive on {}", target.name());
        let program = written(&rig.session, programs.get(target)).await;
        let report = done(&what, &rig.session, &program).await;
        let mut expect = Expect::new(&what, &report);
        expect.that(
            report["tests"][0] == "main.s follows every rule and passed all 404 test runs",
            "404 test runs passed",
        );
        // The proof follows the folder it makes and the file it moves, for every text in range.
        expect.that(
            text(&report["proofs"][0]).contains("proven for every input"),
            "B1 proven for every input",
        );
        expect.finish();
        rig.close().await;
    }
}

/// A program lists its folder and deletes inside it; the kernel lists a folder in an order of its
/// own, which the proof doesn't take.
#[tokio::test]
async fn a_program_lists_its_folder_and_deletes_inside_it() {
    let programs = fixture("sweep");
    for target in Profile::ALL {
        let Some(rig) = rig_for(target, Proving::Full).await else {
            continue;
        };
        let what = format!("sweep on {}", target.name());
        let program = written(&rig.session, programs.get(target)).await;
        let report = done(&what, &rig.session, &program).await;
        let mut expect = Expect::new(&what, &report);
        expect.that(
            report["tests"][0] == "main.s follows every rule and passed all 202 test runs",
            "202 test runs passed",
        );
        expect.that(
            text(&report["proofs"][0]).contains("tested, not proven: listing a folder"),
            "B1 tested, not proven: listing a folder",
        );
        expect.finish();
        rig.close().await;
    }
}

/// nap sleeps 30 seconds, more than a test may run: the edge ends a test's sleep at once, so the
/// test passes in a moment, under QEMU's user mode too.
#[tokio::test]
async fn a_test_answers_a_sleep_at_once() {
    let programs = fixture("nap");
    for target in Profile::ALL {
        let Some(rig) = rig_for(target, Proving::No).await else {
            continue;
        };
        let what = format!("nap on {}", target.name());
        let program = written(&rig.session, programs.get(target)).await;
        let started = Instant::now();
        let report = done(&what, &rig.session, &program).await;
        assert!(
            started.elapsed() < Duration::from_secs(10),
            "{what}: {report}"
        );
        rig.close().await;
    }
}

/// A program gets only the environment its test or its run gives it.
#[tokio::test]
async fn a_program_gets_only_the_environment_its_test_or_run_gives() {
    let programs = example("greet");
    for target in Profile::ALL {
        let Some(rig) = rig_for(target, Proving::Full).await else {
            continue;
        };
        let session = &rig.session;
        let what = format!("greet on {}", target.name());
        let program = written(session, programs.get(target)).await;
        let report = done(&what, session, &program).await;
        let mut expect = Expect::new(&what, &report);
        expect.that(
            report["tests"][0] == "main.s follows every rule and passed all 303 test runs",
            "303 test runs passed",
        );
        // Each name the proof can draw is greeted, and every run kept to the memory rules.
        expect.that(
            text(&report["proofs"][0]).contains("proven for every input in range"),
            "B1 proven for every input in range",
        );
        expect.finish();
        let mut arguments = program.arguments();
        arguments["env"] = json!({"NAME": "Grace Hopper"});
        let run = session.call_ok("run", arguments).await;
        assert_eq!(run["stdout"], "Hello, Grace Hopper!\n", "{what}: {run}");
        // Nothing else reaches it: without NAME it shows how to use it.
        let run = session.call_ok("run", program.arguments()).await;
        assert_eq!(run["stderr"], "usage: NAME=<who> greet\n", "{what}: {run}");
        rig.close().await;
    }
}

/// A die that's off by one, in each CPU's instructions: the digit is added from `0`, not `1`.
const OFF_BY_ONE: PerCpu<&[Edit]> = PerCpu {
    x86_64: &[("add edx, '1'", "add edx, '0'")],
    aarch64: &[("add w4, w4, #'1'", "add w4, w4, #'0'")],
};

/// The tests supply the random bytes, so each roll is exact: 200 runs, plus the proof that every
/// pair of first bytes rolls as the idea says.
#[tokio::test]
async fn the_dice_example_rolls_with_the_tests_random_bytes() {
    let programs = example("dice");
    let proving = if solvers_installed() {
        Proving::Full
    } else {
        Proving::No
    };
    for target in Profile::ALL {
        let Some(rig) = rig_for(target, proving).await else {
            continue;
        };
        let session = &rig.session;
        let what = format!("dice on {}", target.name());
        let files = programs.get(target);
        let program = written(session, files).await;
        done(&what, session, &program).await;
        if rig.way == Way::Native {
            // A die that's off by one: the tests catch it with the bytes they supply.
            let wrong = files.code_changed(OFF_BY_ONE.get(target));
            program.write("main.s", &wrong.code());
            let problems = failed(&what, session, &program).await;
            assert!(
                problems.contains(r#"stdout should be "rolled 1\n", but was "rolled 0\n""#),
                "{what}: {problems}"
            );
            program.write("main.s", &files.code());
        } else {
            lengua::skiplog::note("QEMU's user mode can't supply the tests' random bytes");
        }
        let report = done(&what, session, &program).await;
        if proving == Proving::Full {
            assert!(every_proof_says_proven(&report), "{what}: {report}");
        }
        rig.close().await;
    }
}

/// What each target's proofs of absdiff say: all four blocks are proven for every input in range,
/// and memory with them. ARM64's first block was not until the prover's model of AArch64 read the
/// overflow flag, `tbnz` on the top bit and `msub` as x86-64's reads them (it ran out of time
/// after 153 of 196 input combinations in 60 seconds, run 41912245262, and "a path couldn't be
/// settled" in 240, run 41914652006); now it settles by its terms alone, with no solver question
/// (196 input combinations, 420 paths, run 41942392800).
const ABSDIFF: PerCpu<Proofs> = PerCpu {
    x86_64: Proofs {
        blocks: &[Verdict::Proven; 4],
        memory: "on every run the proofs of B1, B2, B3 and B4 cover",
    },
    aarch64: Proofs {
        blocks: &[Verdict::Proven; 4],
        memory: "on every run the proofs of B1, B2, B3 and B4 cover",
    },
};

/// The absolute difference picks the size of a - b with a conditional move, which the prover
/// settles by writing both the program's choice and abs(a - b) as one term.
#[tokio::test]
async fn the_absolute_difference_example_is_proven() {
    let programs = example("absdiff");
    for target in Profile::ALL {
        let Some(rig) = rig_for(target, Proving::Full).await else {
            continue;
        };
        let what = format!("absdiff on {}", target.name());
        let program = written(&rig.session, programs.get(target)).await;
        let report = done(&what, &rig.session, &program).await;
        // B3 and B4 draw bad arguments with choice(…): each one is proven with every number.
        let mut expect = Expect::new(&what, &report);
        ABSDIFF.get(target).held_by(&report, &mut expect);
        expect.finish();
        rig.close().await;
    }
}

// The template, narrowed; the jump table; the words and the pairs.

/// The template's idea with its range narrowed to `-100` to `100`, and the tests drawing from it.
const NARROWED: &[Edit] = &[
    (
        "between `-9223372036854775807`\nand `9223372036854775807`",
        "between `-100`\nand `100`",
    ),
    (
        "int(-9223372036854775807, 9223372036854775807)",
        "int(-100, 100)",
    ),
];

/// A range of 201 values is small enough that the tests try every one.
#[tokio::test]
async fn small_ranges_run_every_value() {
    let templates = template(
        "template-idea.md",
        "main-template.s",
        "linux-aarch64/main-template.s",
    );
    let programs = PerCpu {
        x86_64: templates.x86_64.idea_changed(NARROWED),
        aarch64: templates.aarch64.idea_changed(NARROWED),
    };
    for target in Profile::ALL {
        let Some(rig) = rig_for(target, Proving::No).await else {
            continue;
        };
        let what = format!("the template on {}", target.name());
        let program = written(&rig.session, programs.get(target)).await;
        let report = done(&what, &rig.session, &program).await;
        assert_eq!(
            report["tests"],
            json!([
                "main.s follows every rule and passed all 456 test runs",
                "generated B1 tried every value of n in its range (all 201)",
                "generated B4 tried every value of n in its range (all 201)"
            ]),
            "{what}: {report}"
        );
        rig.close().await;
    }
}

/// What a session refuses of code for another CPU, in each CPU's name.
const CODE_IS: PerCpu<&str> = PerCpu {
    x86_64: "main.s is x86-64 code, and",
    aarch64: "main.s is AArch64 code, and",
};

/// A `switch` on an operator, as a checked jump table (dialect 4): it follows every rule, passes
/// its tests, and each case runs. Code for the other CPU isn't what a session checks.
#[tokio::test]
async fn a_checked_jump_table_passes_its_checks_and_runs_each_case() {
    let programs = fixture("calc");
    let runs = [
        prints(&["6", "+", "3"], "9\n"),
        prints(&["6", "-", "3"], "3\n"),
        prints(&["6", "*", "3"], "18\n"),
        prints(&["2", "*", "3"], "6\n"),
        prints(&["2", "+", "3"], "5\n"),
        prints(&["2", "-", "3"], "-1\n"),
        prints(
            &["-1000000000", "*", "1000000000"],
            "-1000000000000000000\n",
        ),
        // `,` is in the table's range, as its unknown case.
        fails(&["6", ",", "3"], "error: unknown operator\n", 3),
        fails(&["6", "/", "3"], "error: unknown operator\n", 3),
        fails(&["6", "", "3"], "error: unknown operator\n", 3),
    ];
    for target in Profile::ALL {
        let Some(rig) = rig_for(target, Proving::No).await else {
            continue;
        };
        let session = &rig.session;
        let what = format!("calc on {}", target.name());
        let program = written(session, programs.get(target)).await;
        let report = done(&what, session, &program).await;
        assert!(passed_all(&report), "{what}: {report}");
        run_all(&what, session, &program, &runs).await;

        // Another CPU's code is not what this session checks.
        for other in Profile::ALL {
            if other.cpu() == target.cpu() {
                continue;
            }
            let theirs = written(session, programs.get(other)).await;
            let report = session.check(&theirs).await;
            assert_eq!(report["state"], "code_problems", "{what}: {report}");
            assert!(
                any_line(&report["problems"], CODE_IS.get(other)),
                "{what}: {report}"
            );
        }
        rig.close().await;
    }
}

/// The word count's buffer made small, so the proof's base finishes in the test's time.
const SMALL_BUFFER: &[Edit] = &[(".equ BUF_LEN, 4096\n", ".equ BUF_LEN, 128\n")];

/// A word count, whose count for each buffer-full depends on whether the one before ended inside
/// a word, goes through check to done with its proof for every length: two buffer-fulls at a
/// time, the second counted after the first's last byte.
#[tokio::test]
async fn a_word_count_is_proven_for_every_length() {
    let words = fixture("words");
    let programs = PerCpu {
        x86_64: words.x86_64.code_changed(SMALL_BUFFER),
        aarch64: words.aarch64.code_changed(SMALL_BUFFER),
    };
    for target in Profile::ALL {
        let Some(rig) = rig_for(target, Proving::AloneFor(120)).await else {
            continue;
        };
        let session = &rig.session;
        let what = format!("words on {}", target.name());
        let program = written(session, programs.get(target)).await;
        let report = done(&what, session, &program).await;
        let proof = text(&report["proofs"][0]);
        assert!(
            proof.starts_with("generated B1: proven for every input in range")
                && proof.contains("two more buffer-fulls of 128 bytes")
                && proof.contains("after the first's last 1 byte"),
            "{what}: {report}"
        );
        let run = run_with_stdin(session, &program, "one two\nthree\n").await;
        assert_eq!(run["stdout"], "2 3\n", "{what}: {run}");
        rig.close().await;
    }
}

/// A count of a part that overlaps itself, `aa`, goes through check to done proven for every
/// length: the program keeps, across each buffer-full, whether its last byte is an `a` that
/// starts no pair yet, and the proof shows every time round keeps that so.
#[tokio::test]
async fn a_count_of_a_part_that_overlaps_itself_is_proven_for_every_length() {
    let programs = fixture("pairs");
    for target in Profile::ALL {
        let Some(rig) = rig_for(target, Proving::AloneFor(120)).await else {
            continue;
        };
        let session = &rig.session;
        let what = format!("pairs on {}", target.name());
        let program = written(session, programs.get(target)).await;
        let report = done(&what, session, &program).await;
        let proof = text(&report["proofs"][0]);
        assert!(
            proof.starts_with("generated B1: proven for every input in range")
                && proof.contains("one more buffer-full of 256 bytes"),
            "{what}: {report}"
        );
        let run = run_with_stdin(session, &program, "aaa baaaa\naa").await;
        assert_eq!(run["stdout"], "4\n", "{what}: {run}");
        rig.close().await;
    }
}

// Programs built on programs, and the programs Claude Code wrote.

/// What starts the comment that names the part a function comes from, in each CPU's dialect.
const FROM: PerCpu<&str> = PerCpu {
    x86_64: "# from: ",
    aarch64: "// from: ",
};

/// Programs stack like nodes of a graph: one idea links to another it builds on, and its main.s
/// takes the part's checked functions word for word, which check holds to the part's current
/// code.
#[tokio::test]
async fn a_program_builds_on_another() {
    let programs = example("sort");
    for target in Profile::ALL {
        let Some(rig) = rig_for(target, Proving::No).await else {
            continue;
        };
        let session = &rig.session;
        let what = format!("sorted on {}", target.name());
        let files = programs.get(target);
        let parent = tempfile::tempdir().expect("a folder for both");
        let part = Program::new_in(parent.path(), "sort");
        let node = Program::new_in(parent.path(), "sorted");
        part.write("idea.md", &files.idea());
        part.write("main.s", &files.code());
        let link = format!("../{}", part.name());
        node.write(
            "idea.md",
            &files
                .idea()
                .replace("title: sort", "title: sorted")
                .replace(
                    "- Uses: standard input",
                    &format!("- Uses: standard input\n- Builds on: [sort]({link}/idea.md)"),
                ),
        );
        let code = files.code();
        node.write("main.s", &code);
        let problems = |report: &Value| report["problems"].to_string();
        // A part it builds on but takes nothing from.
        let report = session.record(&node).await;
        assert_eq!(report["state"], "code_problems", "{what}: {report}");
        assert!(
            problems(&report).contains("takes nothing from it"),
            "{what}: {report}"
        );
        // A part that isn't checked yet.
        let taken = code.replace(
            ".type merge_sort, @function",
            &format!("{}{link}\n.type merge_sort, @function", FROM.get(target)),
        );
        node.write("main.s", &taken);
        let report = session.check(&node).await;
        assert!(
            problems(&report).contains("isn't checked on this computer"),
            "{what}: {report}"
        );
        // Once the part is done, so is the program built on it.
        session.record(&part).await;
        done(&what, session, &part).await;
        done(&what, session, &node).await;
        // A copy that drifts from the part's code.
        node.write(
            "main.s",
            &taken.replace(".Lmerge_sort_done", ".Lmerge_sort_finished"),
        );
        let report = session.check(&node).await;
        assert!(
            problems(&report).contains("merge_sort differs from")
                && problems(&report).contains("copy it again"),
            "{what}: {report}"
        );
        rig.close().await;
    }
}

/// Sorting lines as LC_ALL=C sort does, with -u: written through lengua from one plain request in
/// an interactive Claude Code session, with sort(s) and uniq(s) in its tests.
#[tokio::test]
async fn the_sort_claude_code_wrote_passes_its_checks() {
    pass("sort", example("sort")).await;
}

/// Written through lengua from a plain request in an interactive Claude Code session, and in the
/// baseline form that runs on every computer of its CPU.
#[tokio::test]
async fn the_word_count_claude_code_wrote_passes_its_checks() {
    pass("count", example("count")).await;
}

// The templates.

/// The vector template (SSE2 on x86-64, NEON on ARM64) passes its tests and is proven through the
/// server: every line it shouts, with no solver question.
#[tokio::test]
async fn the_vector_template_passes_its_tests_and_is_proven() {
    let programs = template(
        "vector-template-idea.md",
        "vector-template.s",
        "linux-aarch64/vector-template.s",
    );
    for target in Profile::ALL {
        let Some(rig) = rig_for(target, Proving::For(120)).await else {
            continue;
        };
        let session = &rig.session;
        let what = format!("the vector template on {}", target.name());
        let program = written(session, programs.get(target)).await;
        let report = done(&what, session, &program).await;
        let mut expect = Expect::new(&what, &report);
        expect.that(passed_all(&report), "a program that follows every rule");
        expect.that(
            text(&report["tests"][0]).ends_with("passed all 253 test runs"),
            "253 test runs passed",
        );
        expect.that(
            text(&report["proofs"][0]).starts_with("generated B1: proven for every input in range"),
            "B1 proven for every input in range",
        );
        expect.finish();
        let texts = [
            ("lengua checks vectors\n", "LENGUA CHECKS VECTORS\n"),
            ("neon shouts too\n", "NEON SHOUTS TOO\n"),
        ];
        filters(&what, session, &program, &texts).await;
        rig.close().await;
    }
}

/// The streaming filter goes through check to done with its proof for every length, by induction
/// over its read loop, where a proof by length would reach a few thousand bytes.
#[tokio::test]
async fn the_streaming_filter_is_proven_for_every_length() {
    let programs = template(
        "stream-template-idea.md",
        "stream-template.s",
        "linux-aarch64/stream-template.s",
    );
    for target in Profile::ALL {
        // Alone: the induction needs the cores, and beside the file example's two minutes of
        // proofs it ran out of its own on a slower computer (run 41921857314).
        let Some(rig) = rig_for(target, Proving::AloneFor(120)).await else {
            continue;
        };
        let session = &rig.session;
        let what = format!("the streaming filter on {}", target.name());
        let program = written(session, programs.get(target)).await;
        let report = done(&what, session, &program).await;
        let proof = text(&report["proofs"][0]);
        let lock = program.read("lengua.lock").expect("the lock");
        let mut expect = Expect::new(&what, &report);
        expect.that(passed_all(&report), "a program that follows every rule");
        expect.that(
            proof.starts_with("generated B1: proven for every input in range"),
            "B1 proven for every input in range",
        );
        expect.that(
            proof.contains("every length by induction over its read loop"),
            "every length by induction over its read loop",
        );
        expect.that(
            proof.contains("one more buffer-full of 4096 bytes"),
            "one more buffer-full of 4096 bytes",
        );
        expect.that(
            lock.contains("proven = [\"B1\"]"),
            "the lock records B1 proven",
        );
        expect.finish();
        let texts = [
            ("stream me\n", "STREAM ME\n"),
            ("a stream\nof lines\n", "A STREAM\nOF LINES\n"),
        ];
        filters(&what, session, &program, &texts).await;
        rig.close().await;
    }
}

/// The word tally building block goes through check to done, every test passing, and runs. Its
/// proof, whose questions about where words' hashes meet are heavy for the solvers, is the
/// prover's own test's; here it gets a few seconds, so the machine stays free for the others.
#[tokio::test]
async fn the_tally_block_passes_its_checks() {
    let programs = template(
        "tally-template-idea.md",
        "tally-template.s",
        "linux-aarch64/tally-template.s",
    );
    for target in Profile::ALL {
        let Some(rig) = rig_for(target, Proving::For(30)).await else {
            continue;
        };
        let session = &rig.session;
        let what = format!("the tally block on {}", target.name());
        let program = written(session, programs.get(target)).await;
        let report = done(&what, session, &program).await;
        assert!(passed_all(&report), "{what}: {report}");
        assert!(
            text(&report["proofs"][0]).starts_with("generated B1: "),
            "{what}: {report}"
        );
        let texts = [(
            "the cat and the\tdog the cat\n",
            "3 the\n2 cat\n1 and\n1 dog\n",
        )];
        filters(&what, session, &program, &texts).await;
        rig.close().await;
    }
}

/// The parallel template capitalizes a megabyte on up to 16 threads, each doing its part.
#[tokio::test]
async fn the_parallel_template_runs_on_several_cores() {
    let programs = template(
        "parallel-template-idea.md",
        "parallel-template.s",
        "linux-aarch64/parallel-template.s",
    );
    for target in Profile::ALL {
        let Some(rig) = rig_for(target, Proving::For(60)).await else {
            continue;
        };
        let session = &rig.session;
        let what = format!("the parallel template on {}", target.name());
        let program = written(session, programs.get(target)).await;
        let report = done(&what, session, &program).await;
        assert!(passed_all(&report), "{what}: {report}");
        assert!(
            report["performance"].as_array().is_some_and(|costs| costs
                .iter()
                .all(|cost| text(cost).contains("µs on the clock"))),
            "{what}: {report}"
        );
        // A megabyte comes back capitalized: sixteen threads each did their part.
        let megabyte: String = "Hello, threads! ".repeat(65536);
        let run = run_with_stdin(session, &program, &megabyte).await;
        assert_eq!(run["exit_code"], 0, "{what}: {run}");
        assert_eq!(run["stdout"], megabyte.to_ascii_uppercase(), "{what}");
        let texts = [("threads, too\n", "THREADS, TOO\n")];
        filters(&what, session, &program, &texts).await;
        rig.close().await;
    }
}

// The benchmark's programs.

/// The benchmark's own programs: the skill's fast programs, each with its own idea and tests.
/// Each runs on any computer of its CPU, but the tier of the word count, which sits beside its
/// baseline form, needs AVX-512BW on x86-64 and SVE on ARM64.
fn benchmark_programs() -> PerCpu<Vec<Variant>> {
    PerCpu {
        x86_64: vec![
            variant("wc", fast_x86_64("wc"), Needs::Nothing),
            variant("crc32c", fast_x86_64("crc32c"), Needs::Nothing),
            variant("histogram", fast_x86_64("histogram"), Needs::Nothing),
            variant("wc-avx512", fast_x86_64("wc-avx512"), Needs::Avx512bw),
        ],
        aarch64: vec![
            variant("wc", fast_aarch64("wc", "wc"), Needs::Nothing),
            variant("crc32c", fast_aarch64("crc32c", "crc32c"), Needs::Nothing),
            variant(
                "histogram",
                fast_aarch64("histogram", "histogram"),
                Needs::Nothing,
            ),
            variant("wc-sve", fast_aarch64("wc", "wc-sve"), Needs::Sve),
        ],
    }
}

/// The benchmark's shout programs: the parallel template's idea, with the vectors the CPU's
/// extension gives (x86-64's AVX-512 on 16 threads and on one, ARM64's NEON on one).
fn shout_tiers() -> PerCpu<Vec<Variant>> {
    let idea = format!("{ASSETS}/parallel-template-idea.md");
    PerCpu {
        x86_64: vec![
            variant(
                "avx512 shout",
                Files::pair(&idea, "benches/programs/shout/avx512.s"),
                Needs::Avx512bw,
            ),
            variant(
                "avx512 shout, one thread",
                Files::pair(&idea, "benches/programs/shout/avx512-one.s"),
                Needs::Avx512bw,
            ),
        ],
        aarch64: vec![variant(
            "neon shout, one thread",
            Files::pair(&idea, "benches/programs/shout/neon-one.s"),
            Needs::Nothing,
        )],
    }
}

/// The benchmark's upcase programs: the example's idea and tests, 64 bytes at a time with AVX-512
/// on x86-64 and 16 at a time with NEON on ARM64.
fn upcase_tiers() -> PerCpu<Vec<Variant>> {
    let idea = "examples/upcase/idea.md";
    PerCpu {
        x86_64: vec![variant(
            "avx512 upcase",
            Files::pair(idea, "benches/programs/upcase/avx512.s"),
            Needs::Avx512bw,
        )],
        aarch64: vec![variant(
            "neon upcase",
            Files::pair(idea, "benches/programs/upcase/neon.s"),
            Needs::Nothing,
        )],
    }
}

/// The benchmark's word count: the word count Claude Code wrote through lengua from a plain
/// request, as it was before the baseline form (the example's idea and tests), a fast tier the
/// benchmark still measures: 128 bytes at a time with AVX-512 on x86-64, and on ARM64 the same
/// program with a vector at a time, as many bytes as the CPU's SVE vectors hold.
fn count_tiers() -> PerCpu<Vec<Variant>> {
    let idea = "examples/count/idea.md";
    PerCpu {
        x86_64: vec![variant(
            "avx512 count",
            Files::pair(idea, "benches/programs/count/avx512.s"),
            Needs::Avx512bw,
        )],
        aarch64: vec![variant(
            "sve count",
            Files::pair(idea, "benches/programs/count/sve.s"),
            Needs::Sve,
        )],
    }
}

/// Takes the programs of every target this computer runs through the server.
async fn tiers_pass(tiers: PerCpu<Vec<Variant>>) {
    for target in Profile::ALL {
        let Some(rig) = rig_for(target, Proving::No).await else {
            continue;
        };
        variants_pass(target, &rig, tiers.get(target)).await;
        rig.close().await;
    }
}

/// The skill's fast programs, which the benchmark measures on 64 MiB, each follow every rule and
/// pass their tests, and no proof finds one wrong in the time it has.
#[tokio::test]
async fn the_benchmarks_own_programs_pass_their_checks() {
    let programs = benchmark_programs();
    for target in Profile::ALL {
        let Some(rig) = rig_for(target, Proving::For(60)).await else {
            continue;
        };
        variants_pass(target, &rig, programs.get(target)).await;
        rig.close().await;
    }
}

/// The benchmark's shouts, the parallel template's idea with the CPU's wider vectors, pass their
/// checks where the CPU has what they use.
#[tokio::test]
async fn the_shouts_in_the_benchmark_pass_their_checks() {
    tiers_pass(shout_tiers()).await;
}

/// The benchmark's upcase is a lengua program too: the same idea and tests as the example, with
/// wider vectors.
#[tokio::test]
async fn the_upcase_in_the_benchmark_passes_its_checks() {
    tiers_pass(upcase_tiers()).await;
}

/// The benchmark's word count, in the form a fast tier takes.
#[tokio::test]
async fn the_word_count_in_the_benchmark_passes_its_checks() {
    tiers_pass(count_tiers()).await;
}

// The tables, read on any computer.

/// Every program table above, by case: the cells that are twins of one program.
fn twin_tables() -> Vec<(&'static str, PerCpu<Files>)> {
    let templates = template(
        "template-idea.md",
        "main-template.s",
        "linux-aarch64/main-template.s",
    );
    let words = fixture("words");
    vec![
        ("mean", example("mean")),
        ("hypotenuse", example("hypotenuse")),
        ("upcase", example("upcase")),
        ("archive", example("archive")),
        ("sweep", fixture("sweep")),
        ("nap", fixture("nap")),
        ("greet", example("greet")),
        ("dice", example("dice")),
        ("absdiff", example("absdiff")),
        (
            "small ranges",
            PerCpu {
                x86_64: templates.x86_64.idea_changed(NARROWED),
                aarch64: templates.aarch64.idea_changed(NARROWED),
            },
        ),
        ("calc", fixture("calc")),
        (
            "words",
            PerCpu {
                x86_64: words.x86_64.code_changed(SMALL_BUFFER),
                aarch64: words.aarch64.code_changed(SMALL_BUFFER),
            },
        ),
        ("pairs", fixture("pairs")),
        ("sort", example("sort")),
        ("count", example("count")),
        (
            "vector template",
            template(
                "vector-template-idea.md",
                "vector-template.s",
                "linux-aarch64/vector-template.s",
            ),
        ),
        (
            "stream template",
            template(
                "stream-template-idea.md",
                "stream-template.s",
                "linux-aarch64/stream-template.s",
            ),
        ),
        (
            "tally template",
            template(
                "tally-template-idea.md",
                "tally-template.s",
                "linux-aarch64/tally-template.s",
            ),
        ),
        (
            "parallel template",
            template(
                "parallel-template-idea.md",
                "parallel-template.s",
                "linux-aarch64/parallel-template.s",
            ),
        ),
    ]
}

/// Every tier table above, by case.
fn tier_tables() -> Vec<(&'static str, PerCpu<Vec<Variant>>)> {
    vec![
        ("benchmark programs", benchmark_programs()),
        ("shouts", shout_tiers()),
        ("upcase", upcase_tiers()),
        ("count", count_tiers()),
    ]
}

/// Every cell names files that exist and edits that find their text, read on any computer even
/// for a target it cannot run; and the two CPUs' twins of a program are compiled from the very
/// same idea.
#[test]
fn every_cell_names_files_and_edits_that_exist() {
    let mut read = 0;
    for (name, programs) in twin_tables() {
        let ideas: Vec<String> = programs.cells().map(|(_, files)| files.idea()).collect();
        assert!(
            ideas
                .iter()
                .all(|idea| !idea.is_empty() && *idea == ideas[0]),
            "{name}: the CPUs' programs are compiled from different ideas"
        );
        for (target, files) in programs.cells() {
            assert!(!files.code().is_empty(), "{name} on {}", target.name());
            read += 1;
        }
    }
    for (name, tiers) in tier_tables() {
        for (target, variants) in tiers.cells() {
            assert!(!variants.is_empty(), "{name} on {}", target.name());
            for variant in variants {
                assert!(!variant.files.idea().is_empty(), "{name}: {}", variant.name);
                assert!(!variant.files.code().is_empty(), "{name}: {}", variant.name);
                read += 1;
            }
        }
    }
    // The 32 cases the old tests held, and the programs of the CPU's own beside them.
    assert!(read >= 50, "{read} cells read");
}
