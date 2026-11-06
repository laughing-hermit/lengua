//! The server's flow on every target: how `check` builds, tests and proves a program, what it
//! says to each way of going wrong, and what `run` allows after it. Each case is written once,
//! with a cell for every target (`PerCpu`): the program it starts from, or the edit of its
//! text that breaks or fixes something (the `from` and `to` strings differ, since the x86-64 and
//! AArch64 sources do). The assertions on the server's answers are the same on every target, so
//! a case that one CPU passes and the other fails is a finding about lengua, never a different
//! test.
//!
//! A target is tried where this computer runs its programs, natively or through QEMU's user mode,
//! which on a GitHub runner is its own CPU: each case runs on both runners. The cells are checked
//! against their programs on any computer, whichever target it is.

mod common;
mod flow;
mod per_target;

use common::{Program, Protocol, Session};
use flow::{Edits, any_line, done, failed, text, with_args};
use lengua::platform::Profile;
use lengua::targets::{PerCpu, PerTarget};
use per_target::Way;
use serde_json::{Value, json};

/// A program that always prints -7: right for the example, wrong for the rest.
static HARDCODED: PerCpu<Edits> = PerCpu {
    x86_64: &[(
        "    js .Lmain_not_a_number\n    neg rdx\n",
        "    js .Lmain_not_a_number\n    mov rdx, 7\n    neg rdx\n",
    )],
    aarch64: &[(
        "    tbnz x0, #63, .Lmain_not_a_number\n    negs x0, x1\n",
        "    tbnz x0, #63, .Lmain_not_a_number\n    mov x1, #7\n    negs x0, x1\n",
    )],
};

/// An edit that trips an assertion, and the assertion.
#[derive(Debug, Clone, Copy)]
struct Trip {
    /// The edit: the bounds check of the number's text now trips on its second character.
    edits: Edits,
    /// The line of the program that sets the failed assertion's number, which the report names.
    sets_the_number: &'static str,
    /// The assertion's number.
    number: u32,
}

/// Each target's tripped assertion.
static TRIPS: PerCpu<Trip> = PerCpu {
    x86_64: Trip {
        edits: &[("    cmp rcx, MAX_CHARS\n", "    cmp rcx, 1\n")],
        sets_the_number: "    mov edi, 4",
        number: 4,
    },
    aarch64: Trip {
        edits: &[("    cmp x2, #MAX_CHARS\n", "    cmp x2, #1\n")],
        sets_the_number: "    mov w0, #5",
        number: 5,
    },
};

/// An edit that code cannot be built with, and what the refusal must say.
#[derive(Debug, Clone, Copy)]
struct Refusal {
    /// The edit of the template.
    edits: Edits,
    /// Words the refusal must have, each in some problem the report lists.
    says: &'static [&'static str],
}

/// The ways each target's code is refused for not building. A register of the wrong size is a
/// mistake of the same kind on both CPUs, but not refused at the same stage. On x86-64 lengua
/// reads machine code, so the assembler is asked first and its message is what the report
/// carries. On ARM64 lengua reads the dialect itself, each instruction's operand forms, before
/// the assembler is asked, so the same mistake is a problem of the dialect, naming the line.
/// What reaches the assembler there is an instruction of an extension the program has not
/// asked for with `.arch_extension`, which lengua's reader accepts and the assembler does not.
static BUILD_REFUSALS: PerCpu<&[Refusal]> = PerCpu {
    x86_64: &[Refusal {
        edits: &[("    xor r9d, r9d\n", "    mov r9d, rbx\n")],
        says: &["doesn't build: assembling failed"],
    }],
    aarch64: &[
        Refusal {
            edits: &[("    mov x9, #0\n", "    mov w9, x1\n")],
            says: &[
                "dialect main.s:",
                "`mov w9, x1`: its registers aren't all one width",
            ],
        },
        Refusal {
            edits: &[(
                "    mov x9, #0\n",
                "    crc32cx w9, w9, x10\n    mov x9, #0\n",
            )],
            says: &["doesn't build: assembling failed"],
        },
    ],
};

/// The target another computer compiled main.s for: another OS on the other CPU.
static ELSEWHERE: PerTarget<&str> = PerTarget {
    linux_x86_64: "macos-aarch64",
    linux_aarch64: "windows-x86_64",
};

/// The negative of 1234567 comes out one too big: one input in 18 quintillion, which the tests'
/// samples miss and the proof finds.
static UNLUCKY: PerCpu<Edits> = PerCpu {
    x86_64: &[
        (
            ".equ OUT_BUF_LEN, 21\n",
            ".equ OUT_BUF_LEN, 21\n.equ UNLUCKY, -1234567\n",
        ),
        (
            "    neg rdx\n    jo .Lmain_assert_negate\n",
            concat!(
                "    neg rdx\n",
                "    jo .Lmain_assert_negate\n",
                "    cmp rdx, UNLUCKY\n",
                "    jne .Lmain_lucky\n",
                "    inc rdx\n",
                ".Lmain_lucky:\n"
            ),
        ),
    ],
    aarch64: &[(
        "    negs x0, x1\n    b.vs .Lmain_assert_negate\n",
        concat!(
            "    negs x0, x1\n",
            "    b.vs .Lmain_assert_negate\n",
            "    movn x9, #0xd686\n",
            "    movk x9, #0xffed, lsl #16\n",
            "    cmp x0, x9\n",
            "    b.ne .Lmain_lucky\n",
            "    add x0, x0, #1\n",
            ".Lmain_lucky:\n"
        ),
    )],
};

/// A change that leaves what a program does as it was, and makes it a new build: a `nop` first.
/// The tests and the proofs must be made again for it, and a proof of the old build must not
/// land on it. Both CPUs have a `nop`, and `main` starts every program.
const NOP: flow::Edit = ("main:\n", "main:\n    nop\n");

/// The template, with its spec recorded, passes every rule and all 274 test runs, is stamped
/// with lengua's line, and the lock says what built it.
#[tokio::test]
async fn the_template_passes_its_tests_and_is_stamped() {
    for target in Profile::ALL {
        let Some((session, way)) = per_target::session_with_way(target, Protocol::Discover).await
        else {
            continue;
        };
        let name = target.name();
        let program = flow::checked_template(&session).await;
        program.write("main.s", &flow::template(target));
        let report = done(&session, &program).await;
        flow::assert_passed(&report, 274, way, name);
        // Through QEMU the runs are not timed.
        if way == Way::Native {
            let costs = report["performance"]
                .as_array()
                .expect("what each test cost");
            assert_eq!(
                costs.len(),
                7,
                "{name}: 4 examples and 3 generated blocks: {report}"
            );
            assert!(
                text(&costs[0]).starts_with("example B1: 1 run")
                    && text(&costs[0]).contains("its input, run alone 5 times: median "),
                "{name}: {report}"
            );
            // The times come from the block's largest input, run alone: the minus sign and 19
            // digits.
            assert!(
                text(&costs[4]).starts_with("generated B1: 200 runs")
                    && text(&costs[4]).contains("its largest input, 20 bytes, run alone 5 times"),
                "{name}: {report}"
            );
        }
        assert_eq!(
            report["example"],
            "example B1: given arguments [\"7\"], the program printed \"-7\\n\", exited with \
             code 0",
            "{name}: {report}"
        );
        flow::assert_stamped(&program, target, way, &flow::asset("template-spec.md"));
        session.service.cancel().await.expect("close the session");
    }
}

/// The streaming filter passes every rule and test with no time for proofs, carries lengua's
/// line and a lock that says what ran the tests, and runs on a long input. Through QEMU the
/// reply and the run say what held the runs, and a session for this computer itself runs none
/// of it.
#[tokio::test]
async fn a_streaming_program_is_checked_tested_and_stamped() {
    for target in Profile::ALL {
        let Some((session, way)) = per_target::session_with_way(target, Protocol::Discover).await
        else {
            continue;
        };
        let name = target.name();
        let cpu = target.cpu();
        let emulated = way == Way::Emulated;
        let program = flow::shipped(&session, target, "cat-stream").await;
        let report = done(&session, &program).await;
        assert!(
            any_line(&report["tests"], "follows every rule and passed all"),
            "{name}: {report}"
        );
        assert_eq!(
            any_line(&report["tests"], &format!("test runs under qemu-{cpu}")),
            emulated,
            "{name}: {report}"
        );
        assert_eq!(
            any_line(
                &report["tests"],
                "the kernel couldn't hold its system calls to the spec, which only lengua's \
                 rules did"
            ),
            emulated,
            "{name}: {report}"
        );
        // With no time for proofs the prover still models the CPU's code.
        assert!(
            report["proofs"]
                .as_array()
                .is_some_and(|proofs| !proofs.is_empty()
                    && proofs.iter().all(|proof| !text(proof).contains("no model"))),
            "{name}: {report}"
        );
        assert!(
            text(&report["memory"]).contains("the rest is tested, not proven"),
            "{name}: {report}"
        );
        let spec = program.read("spec.md").expect("spec.md");
        flow::assert_stamped(&program, target, way, &spec);
        let mut arguments = program.arguments();
        arguments["stdin"] = Value::from("hello, from far away\n".repeat(400));
        let run = session.call_ok("run", arguments).await;
        assert_eq!(
            run["stdout"],
            "hello, from far away\n".repeat(400),
            "{name}: {run}"
        );
        assert_eq!(run["exit_code"], 0, "{name}: {run}");
        assert_eq!(
            text(&run["emulated"]).starts_with(&format!("ran under qemu-{cpu}")),
            emulated,
            "{name}: {run}"
        );
        if emulated {
            assert!(run["instructions"].is_null(), "{name}: {run}");
            another_session_refuses(session, target, &program).await;
        } else {
            session.service.cancel().await.expect("close the session");
        }
    }
}

/// A session for this computer itself runs nothing tested under QEMU, and the other CPU's code
/// isn't its own.
async fn another_session_refuses(session: Session, target: Profile, program: &Program) {
    let name = target.name();
    let session = session.restart_for(None).await;
    let error = session.call_error("run", program.arguments()).await;
    assert!(
        error.contains(&format!(
            "was tested as {name} code under qemu-{}",
            target.cpu()
        )) && error.contains("code natively"),
        "{name}: {error}"
    );
    let report = session.check(program).await;
    assert_eq!(report["state"], "code_problems", "{name}: {report}");
    assert!(
        any_line(
            &report["problems"],
            &format!(
                "main.s is {} code, and this computer is",
                flow::isa_name(target)
            )
        ),
        "{name}: {report}"
    );
    session.service.cancel().await.expect("close the session");
}

/// A program that prints the same answer for every number passes its example and fails the
/// generated tests, with the seed that found it, and is not stamped.
#[tokio::test]
async fn hardcoded_answers_fail_generated_tests() {
    for target in Profile::ALL {
        let Some((session, _)) = per_target::session_with_way(target, Protocol::Discover).await
        else {
            continue;
        };
        let name = target.name();
        let program = flow::checked_template(&session).await;
        let code = flow::template_changed(target, HARDCODED.get(target), "hardcoded answers");
        program.write("main.s", &code);
        let problems = failed(&session, &program).await;
        assert!(problems.contains("generated B1"), "{name}: {problems}");
        assert!(problems.contains("(seed "), "{name}: {problems}");
        assert!(problems.contains("but was \"-7\\n\""), "{name}: {problems}");
        assert!(
            !program
                .read("lengua.lock")
                .expect("the lock")
                .contains("[code]"),
            "{name}"
        );
        // Calling again with nothing changed runs the tests again, with new inputs.
        assert!(
            failed(&session, &program).await.contains("(seed "),
            "{name}"
        );
        session.service.cancel().await.expect("close the session");
    }
}

/// A failed assertion names its line in main.s.
#[tokio::test]
async fn failed_assertions_name_their_line() {
    for target in Profile::ALL {
        let Some((session, _)) = per_target::session_with_way(target, Protocol::Discover).await
        else {
            continue;
        };
        let name = target.name();
        let trip = TRIPS.get(target);
        let program = flow::checked_template(&session).await;
        let code = flow::template_changed(target, trip.edits, "a tripped assertion");
        program.write("main.s", &code);
        let problems = failed(&session, &program).await;
        let line = code
            .lines()
            .position(|line| line == trip.sets_the_number)
            .expect("the assertion's number")
            + 1;
        assert!(
            problems.contains(&format!(
                "assertion {} failed at main.s:{line}",
                trip.number
            )),
            "{name}: {problems}"
        );
        session.service.cancel().await.expect("close the session");
    }
}

/// `run` needs the files `check` tested and the build it tested, and refuses a lock that was
/// edited.
#[tokio::test]
async fn run_needs_matching_files_and_the_tested_build() {
    for target in Profile::ALL {
        let Some((session, _)) = per_target::session_with_way(target, Protocol::Discover).await
        else {
            continue;
        };
        let name = target.name();
        let program = flow::checked_template(&session).await;
        let code = flow::template(target);
        program.write("main.s", &code);
        let error = session.call_error("run", program.arguments()).await;
        assert!(
            error.contains("doesn't"),
            "{name}: run refuses unchecked code: {error}"
        );
        done(&session, &program).await;

        let report = session.call_ok("run", with_args(&program, &["7"])).await;
        assert_eq!(report["stdout"], "-7\n", "{name}: {report}");
        assert_eq!(report["exit_code"], 0, "{name}: {report}");
        assert_eq!(report["ended"], "exit code 0", "{name}: {report}");
        let report = session.call_ok("run", with_args(&program, &[])).await;
        assert_eq!(report["stderr"], "usage: negate <n>\n", "{name}: {report}");
        assert_eq!(report["exit_code"], 1, "{name}: {report}");

        // A build that differs from the tested one (as after an LLVM upgrade) doesn't run.
        let key_bytes: [u8; 32] = std::fs::read(session.data_dir.path().join("key"))
            .expect("the signing key")
            .try_into()
            .expect("32 bytes");
        let key = lengua::lock::Key::from_bytes(key_bytes);
        let mut lock = lengua::lock::read(program.path(), Some(&key))
            .expect("a valid lock")
            .expect("a lock");
        let stamp = lock.code.as_mut().expect("code stamp");
        stamp.executable_sha256 = lengua::hash::Sha256Hex::of(b"another build");
        stamp.toolchain = "llvm-mc 21.0.0, ld.lld 21.0.0".into();
        lock.proofs = None;
        lengua::lock::write(program.path(), &lock, &key).expect("write the lock");
        let error = session.call_error("run", with_args(&program, &["7"])).await;
        assert!(
            error.contains("differs from the one check tested"),
            "{name}: {error}"
        );
        assert!(
            error.contains("built with llvm-mc 21.0.0"),
            "{name}: {error}"
        );
        done(&session, &program).await;
        session.call_ok("run", with_args(&program, &["7"])).await;

        // Any edit stops run until it's checked again.
        program.write("main.s", &format!("{code}\n"));
        let error = session.call_error("run", with_args(&program, &["7"])).await;
        assert!(error.contains("call check"), "{name}: {error}");
        program.write("main.s", &code);
        let lock_text = program.read("lengua.lock").expect("the lock");
        program.write(
            "lengua.lock",
            &lock_text.replace("max_stack_bytes = ", "max_stack_bytes = 1"),
        );
        let error = session.call_error("run", with_args(&program, &["7"])).await;
        assert!(error.contains("can't be used"), "{name}: {error}");
        session.service.cancel().await.expect("close the session");
    }
}

/// A rule's violation comes with the rule, the line and a fix.
#[tokio::test]
async fn violations_come_with_rule_line_and_fix() {
    for target in Profile::ALL {
        let Some((session, _)) = per_target::session_with_way(target, Protocol::Discover).await
        else {
            continue;
        };
        let name = target.name();
        let offence = flow::MOVES_THE_STACK_POINTER_IN_EDGE_FORM.get(target);
        let program = flow::checked_template(&session).await;
        let code = offence.applied_to(&flow::template(target));
        program.write("main.s", &code);
        let report = session.check(&program).await;
        assert_eq!(report["state"], "code_problems", "{name}: {report}");
        let line = code
            .lines()
            .position(|line| line == offence.line)
            .expect("the inserted line")
            + 1;
        assert!(
            any_line(
                &report["problems"],
                &format!("rule 3 (fixed resources) main.s:{line}: {}", offence.says)
            ),
            "{name}: {report}"
        );
        assert!(
            text(&report["next_step"]).contains("compile skill"),
            "{name}: {report}"
        );
        session.service.cancel().await.expect("close the session");
    }
}

/// A label left from older versions of lengua is only a comment, and code that does not build
/// is refused with the assembler's message and its line.
#[tokio::test]
async fn main_s_needs_no_label_and_must_build() {
    for target in Profile::ALL {
        let Some((session, _)) = per_target::session_with_way(target, Protocol::Discover).await
        else {
            continue;
        };
        let name = target.name();
        let program = flow::checked_template(&session).await;
        let code = flow::template(target);
        program.write(
            "main.s",
            &format!(
                "{} lengua: spec sha256:{}\n{code}",
                flow::kit(target).comment,
                "0".repeat(64)
            ),
        );
        done(&session, &program).await;

        let refusals: &[Refusal] = BUILD_REFUSALS.get(target);
        for refusal in refusals {
            let broken = flow::template_changed(target, refusal.edits, "a refused build");
            program.write("main.s", &broken);
            let report = session.check(&program).await;
            assert_eq!(report["state"], "code_problems", "{name}: {report}");
            for words in refusal.says {
                assert!(any_line(&report["problems"], words), "{name}: {report}");
            }
            assert!(
                any_line(&report["problems"], "main.s:"),
                "{name}: the message names the line: {report}"
            );
        }
        session.service.cancel().await.expect("close the session");
    }
}

/// Code waits for a spec whose tests check: a spec with a test that cannot run is refused first,
/// and nothing is locked.
#[tokio::test]
async fn code_waits_for_a_spec_whose_tests_check() {
    for target in Profile::ALL {
        let Some((session, _)) = per_target::session_with_way(target, Protocol::Discover).await
        else {
            continue;
        };
        let name = target.name();
        let program = Program::new();
        program.write(
            "spec.md",
            &flow::asset("template-spec.md").replace("| 50 |", "| 0 |"),
        );
        program.write("main.s", &flow::template(target));
        let report = session.record(&program).await;
        assert_eq!(report["state"], "spec_problems", "{name}: {report}");
        assert!(
            any_line(&report["problems"], "runs must be from 1"),
            "{name}: {report}"
        );
        assert!(program.read("lengua.lock").is_none(), "{name}");
        session.service.cancel().await.expect("close the session");
    }
}

/// Code compiled for another computer is compiled again for this one.
#[tokio::test]
async fn code_compiled_for_another_computer_is_compiled_again() {
    for target in Profile::ALL {
        let Some((session, _)) = per_target::session_with_way(target, Protocol::Discover).await
        else {
            continue;
        };
        let name = target.name();
        let kit = flow::kit(target);
        let elsewhere = *ELSEWHERE.get(target);
        let program = flow::checked_template(&session).await;
        let first = format!("{}\n", kit.first_line);
        let code = flow::template(target);
        assert!(code.starts_with(&first), "{name}: {code}");
        program.write(
            "main.s",
            &code.replacen(
                &first,
                &format!(
                    "{first}{} lengua: compiled from spec.md sha256:00 for {elsewhere}\n",
                    kit.comment
                ),
                1,
            ),
        );
        let report = session.check(&program).await;
        assert_eq!(report["state"], "code_problems", "{name}: {report}");
        assert!(
            any_line(&report["problems"], &format!("compiled for {elsewhere}")),
            "{name}: {report}"
        );
        assert!(
            text(&report["next_step"]).contains("for this computer with the compile skill"),
            "{name}: {report}"
        );
        session.service.cancel().await.expect("close the session");
    }
}

/// Tests can be fixed after code ran: the change is checked like any version of the spec, shown
/// with lengua's own marks on what tests less, and approved by nobody.
#[tokio::test]
async fn tests_can_be_fixed_after_code_ran() {
    for target in Profile::ALL {
        let Some((session, _)) = per_target::session_with_way(target, Protocol::Discover).await
        else {
            continue;
        };
        let name = target.name();
        let program = flow::checked_template(&session).await;
        program.write("main.s", &flow::template(target));
        let report = done(&session, &program).await;
        assert!(report.get("changes").is_none(), "{name}: {report}");
        let spec = flow::asset("template-spec.md");

        program.write("spec.md", &spec.replace("| 200 |", "| 10 |"));
        let report = done(&session, &program).await;
        let changed = text(&report["changes"]["spec_changed"]);
        assert!(
            changed.contains("- ") && changed.contains("+ "),
            "{name}: {report}"
        );
        assert!(changed.contains("| 10 |"), "{name}: {report}");
        assert_eq!(
            report["changes"]["tests_less"],
            json!(["generated B1: fewer runs, 10 instead of 200"]),
            "{name}: {report}"
        );

        // A change that tests more gets no mark, and going back to the first version shows
        // nothing.
        program.write("spec.md", &spec.replace("| 200 |", "| 300 |"));
        let report = done(&session, &program).await;
        assert!(
            report["changes"].get("tests_less").is_none(),
            "{name}: {report}"
        );
        assert!(
            report["changes"]["spec_changed"].is_string(),
            "{name}: {report}"
        );
        program.write("spec.md", &spec);
        let report = done(&session, &program).await;
        assert!(report.get("changes").is_none(), "{name}: {report}");
        session.service.cancel().await.expect("close the session");
    }
}

/// New words in the middle of a build keep the first spec as what changes are shown against.
#[tokio::test]
async fn new_words_in_the_middle_of_a_build_never_hide_a_change() {
    for target in Profile::ALL {
        let Some((session, _)) = per_target::session_with_way(target, Protocol::Discover).await
        else {
            continue;
        };
        let name = target.name();
        let program = flow::checked_template(&session).await;
        // The build isn't finished: no code has passed yet. Changing the spec and recording it
        // with other words keeps the first spec as what changes are shown against.
        let spec = flow::asset("template-spec.md");
        let edited = spec.replacen("# Purpose\n", "# Purpose\nIt is quick. ", 1);
        assert_ne!(spec, edited, "the edit applies");
        program.write("spec.md", &edited);
        session
            .call_ok("check", program.request("Also make it quick"))
            .await;
        program.write("main.s", &flow::template(target));
        let report = done(&session, &program).await;
        assert_eq!(
            report["changes"]["requests"],
            json!([common::REQUEST, "Also make it quick"]),
            "{name}: {report}"
        );
        assert!(
            text(&report["changes"]["spec_changed"]).contains("It is quick."),
            "{name}: {report}"
        );
        session.service.cancel().await.expect("close the session");
    }
}

/// The template is proven for every input: each block of generated tests, and the lock keeps
/// the proof for exactly that build.
#[tokio::test]
async fn the_template_is_proven_for_every_input() {
    if !flow::solvers_installed() {
        return;
    }
    for target in Profile::ALL {
        let Some((session, _)) = flow::proving(target, None).await else {
            continue;
        };
        let name = target.name();
        let program = flow::checked_template(&session).await;
        program.write("main.s", &flow::template(target));
        let report = done(&session, &program).await;
        // B3's bad inputs come from choice(…), each proven in turn.
        let proofs = report["proofs"].as_array().expect("proofs");
        assert_eq!(proofs.len(), 3, "{name}: {report}");
        assert!(
            proofs
                .iter()
                .all(|proof| text(proof).contains(": proven for every input in range")),
            "{name}: {report}"
        );
        let lock = program.read("lengua.lock").expect("the lock");
        assert!(
            lock.contains("proven = [\"B1\", \"B3\", \"B4\"]"),
            "{name}: {lock}"
        );
        // Every step settles by the terms alone: the program's parse and the formula's number
        // are one sum.
        assert!(
            lock.contains(
                "solvers = \"none: the tests tried every value, or the terms settled every step\""
            ),
            "{name}: {lock}"
        );
        // Checking the same files again answers at once from what was found.
        let again = session.check(&program).await;
        assert_eq!(again["state"], "done", "{name}: {again}");
        assert_eq!(again["proofs"], report["proofs"], "{name}: {again}");
        // A new build is tested and proven again by itself.
        program.write("main.s", &flow::template(target).replacen(NOP.0, NOP.1, 1));
        let report = done(&session, &program).await;
        assert!(
            any_line(&report["proofs"], "proven for every input in range"),
            "{name}: {report}"
        );
        let relocked = program.read("lengua.lock").expect("the lock");
        assert_ne!(lock, relocked, "{name}");
        assert!(
            relocked.contains("proven = [\"B1\", \"B3\", \"B4\"]"),
            "{name}: {relocked}"
        );
        session.service.cancel().await.expect("close the session");
    }
}

/// The streaming filter is proven for every length through the server, by induction over its
/// read loop: natively on a computer of its CPU, and through QEMU on the other.
#[tokio::test]
async fn a_streaming_program_is_proven_for_every_length() {
    if !flow::solvers_installed() {
        return;
    }
    for target in Profile::ALL {
        let Some((session, _)) = flow::proving(target, Some(120)).await else {
            continue;
        };
        let name = target.name();
        let program = flow::shipped(&session, target, "cat-stream").await;
        let report = done(&session, &program).await;
        let proof = text(&report["proofs"][0]);
        assert!(
            proof.starts_with("generated B1: proven for every input in range"),
            "{name}: {report}"
        );
        session.service.cancel().await.expect("close the session");
    }
}

/// A bug the tests' samples miss is found by the proof, and a real run with the input it found
/// confirms it; lengua refuses to run that build from then on, after a restart too.
#[tokio::test]
async fn a_bug_the_samples_miss_is_found_by_the_proof_and_run_for_real() {
    // Finding it takes the solvers most of their time for a question, so no other proof runs
    // beside it.
    if !flow::solvers_installed() {
        return;
    }
    for target in Profile::ALL {
        let Some((session, _)) = flow::proving_alone(target).await else {
            continue;
        };
        let name = target.name();
        let program = flow::checked_template(&session).await;
        let buggy = flow::template_changed(target, UNLUCKY.get(target), "an unlucky number");
        assert_ne!(buggy, flow::template(target), "{name}");
        program.write("main.s", &buggy);
        let problems = failed(&session, &program).await;
        assert!(
            problems.contains(
                "the proof found inputs the tests didn't try where main.s is wrong: with n = \
                 1234567"
            ),
            "{name}: {problems}"
        );
        // The tests passed, but a build the proof found wrong doesn't run.
        let refused = session
            .call_error("run", with_args(&program, &["1234567"]))
            .await;
        assert!(
            refused.contains("The proof found inputs where main.s is wrong"),
            "{name}: {refused}"
        );
        // Nor after lengua restarts: lengua.lock records what the proof found, and check shows
        // it.
        let restarted = session.restart().await;
        let refused = restarted
            .call_error("run", with_args(&program, &["1234567"]))
            .await;
        assert!(
            refused.contains("The proof found inputs where main.s is wrong")
                && refused.contains("n = 1234567"),
            "{name}: {refused}"
        );
        let problems = failed(&restarted, &program).await;
        assert!(problems.contains("with n = 1234567"), "{name}: {problems}");
        restarted.service.cancel().await.expect("close the session");
    }
}

/// Done comes once the tests pass, while the proofs go on, and the program runs meanwhile.
#[tokio::test]
async fn done_comes_once_the_tests_pass_while_the_proofs_go_on() {
    if !flow::solvers_installed() {
        return;
    }
    for target in Profile::ALL {
        if !flow::natively(target) {
            continue;
        }
        let name = target.name();
        // add's proof takes most of a minute, so done comes first, with the proofs still
        // running.
        let session = Session::proving_with_grace(Protocol::Discover, 0).await;
        let program = flow::folder_of(flow::program_files(target, "add"));
        let report = session.record(&program).await;
        assert_eq!(report["state"], "done", "{name}: {report}");
        assert!(any_line(&report["proofs"], ": proving"), "{name}: {report}");
        assert!(
            text(&report["next_step"]).contains("still running"),
            "{name}: {report}"
        );
        // The program runs while its proofs go on.
        let run = session
            .call_ok("run", with_args(&program, &["2", "3"]))
            .await;
        assert_eq!(run["stdout"], "5\n", "{name}: {run}");
        session.service.cancel().await.expect("close the session");
    }
}

/// Proofs survive a server restart: a new server runs the tests again and takes the proofs the
/// lock records about this very build.
#[tokio::test]
async fn proofs_survive_a_server_restart() {
    if !flow::solvers_installed() {
        return;
    }
    for target in Profile::ALL {
        let Some((session, _)) = flow::proving(target, None).await else {
            continue;
        };
        let name = target.name();
        let program = flow::checked_template(&session).await;
        program.write("main.s", &flow::template(target));
        done(&session, &program).await;
        // A new server has no job in memory: it runs the tests again and takes the proofs the
        // lock records about this very build.
        let restarted = session.restart().await;
        let report = done(&restarted, &program).await;
        assert!(
            any_line(&report["proofs"], "as lengua.lock records"),
            "{name}: {report}"
        );
        restarted.service.cancel().await.expect("close the session");
    }
}

/// The last line of a proof goes final only once lengua.lock records the proofs: a caller that
/// sees every proof settled, or a server restarted at that moment, finds them there. The write is
/// made slow (600 ms) and each check waits 50 ms at most, so a gap between the last line and the
/// lock would be seen.
#[tokio::test]
async fn the_proofs_are_in_the_lock_before_the_last_line_goes_final() {
    if !flow::solvers_installed() {
        return;
    }
    for target in Profile::ALL {
        if !flow::natively(target) {
            continue;
        }
        let name = target.name();
        let session = Session::proving_slow_to_record(Protocol::Discover, 50, 600).await;
        let program = flow::shipped(&session, target, "cat").await;
        let started = std::time::Instant::now();
        loop {
            let report = session.check(&program).await;
            assert_ne!(report["state"], "tests_failed", "{name}: {report}");
            assert!(
                started.elapsed() < std::time::Duration::from_secs(600),
                "{name}: the proofs never settled: {report}"
            );
            let settled = report["proofs"]
                .as_array()
                .is_some_and(|lines| !lines.is_empty())
                && !any_line(&report["proofs"], ": proving")
                && !any_line(&report["proofs"], "waiting for its turn");
            if settled {
                let lock = program.read("lengua.lock").unwrap_or_default();
                assert!(
                    lock.contains("[proofs]"),
                    "{name}: all proof lines final, but the lock had no proofs: {report}\n{lock}"
                );
                break;
            }
        }
        done(&session, &program).await;
        session.service.cancel().await.expect("close the session");
    }
}

/// An edit in the middle of a proof starts it over, and the old proof never lands on the new
/// build.
#[tokio::test]
async fn an_edit_mid_proof_starts_over_and_the_old_proof_never_lands() {
    if !flow::solvers_installed() {
        return;
    }
    for target in Profile::ALL {
        if !flow::natively(target) {
            continue;
        }
        let name = target.name();
        // Each check waits 50 ms at most, so it sees the proof running: cat's, of every text up
        // to 4096 bytes, takes over a second even on an idle computer.
        let session = Session::proving_waiting(Protocol::Discover, 50).await;
        let program = flow::shipped(&session, target, "cat").await;
        loop {
            let report = session.check(&program).await;
            match report["state"].as_str() {
                Some("proving") => break,
                Some("testing") => {}
                _ => panic!("{name}: {report}"),
            }
        }
        program.write(
            "main.s",
            &flow::program_file(target, "cat", "main.s").replacen(NOP.0, NOP.1, 1),
        );
        let report = done(&session, &program).await;
        assert!(
            any_line(&report["proofs"], "proven for every input"),
            "{name}: {report}"
        );
        let lock = program.read("lengua.lock").expect("the lock");
        let executable = |table: &str| {
            lock.split(&format!("[{table}]"))
                .nth(1)
                .and_then(|rest| rest.split("executable_sha256 = \"").nth(1))
                .and_then(|rest| rest.split('"').next())
                .map(str::to_owned)
        };
        assert!(executable("code").is_some(), "{name}: {lock}");
        assert_eq!(executable("code"), executable("proofs"), "{name}: {lock}");
        session.service.cancel().await.expect("close the session");
    }
}

/// Long text is proven up to the bound the proof reaches, and the lock says it apart from full
/// proofs.
#[tokio::test]
async fn long_text_is_proven_up_to_the_bound() {
    if !flow::solvers_installed() {
        return;
    }
    for target in Profile::ALL {
        let Some((session, _)) = flow::proving(target, None).await else {
            continue;
        };
        let name = target.name();
        let program = flow::shipped(&session, target, "cat").await;
        let report = done(&session, &program).await;
        // Texts run to 5000 bytes, read with read, and a pipe write holds 4096: proving further
        // can't help, so the line doesn't offer it.
        let only = text(&report["proofs"][0]);
        assert!(
            only.starts_with(
                "generated B1: proven for every input with text up to 4096 of 5000 bytes ("
            ) && only.ends_with(
                "; longer texts are tested, not proven: standard input past 4096 bytes is more \
                 than one pipe write, which the proof reads whole; read it with fill to be \
                 proven for any length"
            ),
            "{name}: {only}"
        );
        // The lock records it apart from full proofs.
        let lock = program.read("lengua.lock").expect("the lock");
        assert!(lock.contains("proven = []"), "{name}: {lock}");
        for id in ["B1", "B2"] {
            assert!(
                lock.contains(&format!(
                    "{{ id = \"{id}\", bytes = 4096, of = 5000, more = false }}"
                )),
                "{name}: {lock}"
            );
        }
        session.service.cancel().await.expect("close the session");
    }
}

/// Every cell's edit changes text its program has, on every target and on any computer, and the
/// programs the cases start from are shipped for every target: a mistake in the cells of a
/// target this computer cannot run shows on every runner.
#[test]
fn every_cell_edits_text_its_program_has() {
    for target in Profile::ALL {
        let name = target.name();
        let template = flow::template(target);
        let tables: [(&str, Edits); 3] = [
            ("hardcoded answers", *HARDCODED.get(target)),
            ("a tripped assertion", TRIPS.get(target).edits),
            ("an unlucky number", *UNLUCKY.get(target)),
        ];
        for (what, edits) in tables {
            let code = flow::changed(&template, edits, &format!("{what} on {name}"));
            assert_ne!(code, template, "{what} on {name} changes nothing");
        }
        let refusals: &[Refusal] = BUILD_REFUSALS.get(target);
        assert!(!refusals.is_empty(), "{name} is refused for nothing");
        for refusal in refusals {
            let code = flow::changed(&template, refusal.edits, &format!("a refusal on {name}"));
            assert_ne!(code, template, "a refusal on {name} changes nothing");
            assert!(!refusal.says.is_empty(), "a refusal on {name} says nothing");
        }
        // The assertion a tripped program names is one the program has.
        let trip = TRIPS.get(target);
        assert!(
            template.lines().any(|line| line == trip.sets_the_number),
            "{name} has no {:?}",
            trip.sets_the_number
        );
        // The instruction that moves the stack pointer goes before one the template has.
        let offence = flow::MOVES_THE_STACK_POINTER_IN_EDGE_FORM.get(target);
        assert_ne!(offence.applied_to(&template), template, "{name}");
        // A `nop` goes first in the template and in cat, and every program starts with the line
        // that says which CPU's code it is.
        assert!(template.contains(NOP.0), "the template on {name}");
        assert!(
            flow::program_file(target, "cat", "main.s").contains(NOP.0),
            "cat on {name}"
        );
        assert!(template.starts_with(flow::kit(target).first_line), "{name}");
        // The programs the cases start from are shipped, whole.
        for program in ["add", "cat", "cat-stream"] {
            for file in ["spec.md", "main.s"] {
                assert!(
                    !flow::program_file(target, program, file).is_empty(),
                    "{program}/{file} on {name}"
                );
            }
            let code = flow::program_file(target, program, "main.s");
            assert!(
                code.starts_with(flow::kit(target).first_line),
                "{program} on {name}"
            );
        }
    }
}
