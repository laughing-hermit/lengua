//! The attempts of tests/adversarial.rs in AArch64's dialect: programs written to get past
//! lengua's own checks, each of which `check` must refuse. They are the AArch64 template with
//! one change that tries it; none is ever run, because the rules refuse it first. They run on an
//! ARM64 computer, or on an x86-64 one with QEMU's user mode set up for AArch64. Any attempt
//! that isn't refused is a bug in lengua to fix before anything else.

mod common;

use std::path::PathBuf;

use common::{Program, Protocol, Session};
use serde_json::Value;

fn asset(name: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("plugin/skills/compile/assets")
        .join(name);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("read {}: {error}", path.display()))
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

/// The AArch64 template changed once: `from`, which must be in it, becomes `to`.
fn changed(from: &str, to: &str) -> String {
    let template = asset("linux-aarch64/main-template.s");
    assert!(template.contains(from), "the template has no {from:?}");
    template.replacen(from, to, 1)
}

/// A session for linux-aarch64 here: none where neither this computer's CPU nor QEMU runs
/// AArch64 programs.
async fn aarch64_session() -> Option<Session> {
    if cfg!(target_arch = "aarch64") {
        Some(Session::start(Protocol::Discover).await)
    } else if common::qemu_runs_aarch64() {
        Some(Session::emulating(Protocol::Discover, "linux-aarch64").await)
    } else {
        None
    }
}

/// [`aarch64_session`] with its time for proofs, as x86-64's wrong-program test has: the proof is
/// what finds that bug, so a session with none would test the tests alone.
async fn proving_aarch64_session() -> Option<Session> {
    if cfg!(target_arch = "aarch64") {
        Some(Session::proving_alone(Protocol::Discover).await)
    } else if common::qemu_runs_aarch64() {
        Some(Session::emulating_for(Protocol::Discover, "linux-aarch64", 300).await)
    } else {
        None
    }
}

/// A program folder holding the template's spec, recorded.
async fn checked_template(session: &Session) -> Program {
    let program = Program::new();
    program.write("spec.md", &asset("template-spec.md"));
    let report = session.record(&program).await;
    assert_eq!(report["state"], "write_code", "{report}");
    program
}

/// Checks `code` as the program's main.s, which must be refused with a problem naming one of
/// `needles`.
async fn refused(session: &Session, what: &str, code: &str, needles: &[&str]) {
    let program = checked_template(session).await;
    program.write("main.s", code);
    let report = session.check(&program).await;
    assert_eq!(
        report["state"], "code_problems",
        "{what} wasn't refused: {report}"
    );
    assert!(
        needles
            .iter()
            .any(|needle| any_line(&report["problems"], needle)),
        "{what} was refused, but not for {needles:?}: {report}"
    );
}

#[tokio::test]
async fn a_system_call_the_spec_doesnt_allow_is_refused() {
    let Some(session) = aarch64_session().await else {
        return;
    };
    // The template's spec allows write and exit. Each of these is a call a program could use to
    // reach beyond its folder, its memory or its user.
    let calls = [
        ("openat", 56),
        ("mount", 40),
        ("unshare", 97),
        ("ptrace", 117),
        ("kill", 129),
        ("setuid", 146),
        ("prctl", 167),
        ("socket", 198),
        ("connect", 203),
        ("clone", 220),
        ("execve", 221),
        ("mmap", 222),
        ("mprotect", 226),
        ("execveat", 281),
        ("io_uring_setup", 425),
    ];
    for (name, number) in calls {
        let code = changed(
            "    mov x8, #SYS_EXIT\n    svc #0\n",
            &format!("    mov x8, #{number}\n    svc #0\n    mov x8, #SYS_EXIT\n    svc #0\n"),
        );
        refused(
            &session,
            name,
            &code,
            &["isn't one the spec allows", "never connects by itself"],
        )
        .await;
    }
    session.service.cancel().await.expect("close the session");
}

#[tokio::test]
async fn a_system_call_chosen_by_a_value_the_rules_cant_see_is_refused() {
    let Some(session) = aarch64_session().await else {
        return;
    };
    let code = changed(
        "    mov x8, #SYS_EXIT\n    svc #0\n",
        "    mov x8, x9\n    svc #0\n    mov x8, #SYS_EXIT\n    svc #0\n",
    );
    refused(
        &session,
        "a number from a register",
        &code,
        &["isn't chosen by a `mov"],
    )
    .await;
    let code = changed(
        "    mov x8, #SYS_EXIT\n    svc #0\n",
        "    mov x8, #SYS_EXIT\n.Lstart_sneak:\n    svc #0\n",
    );
    refused(
        &session,
        "a label between the number and the call",
        &code,
        &["isn't chosen by a `mov", "rule 1"],
    )
    .await;
    session.service.cancel().await.expect("close the session");
}

#[tokio::test]
async fn accesses_outside_their_owner_are_refused() {
    let Some(session) = aarch64_session().await else {
        return;
    };
    // One byte past the end of usage_msg, which is 18 bytes, and far past it.
    for offset in [18, 4095] {
        let code = changed(
            "    cbz x0, .Lstart_assert_argv\n",
            &format!(
                "    cbz x0, .Lstart_assert_argv\n    adrp x9, usage_msg\n    add x9, x9, \
                 :lo12:usage_msg\n    ldrb w10, [x9, #{offset}]\n"
            ),
        );
        refused(&session, "a read past a buffer", &code, &["rule M2"]).await;
    }
    // An index nothing has checked.
    let code = changed(
        "    cbz x0, .Lstart_assert_argv\n",
        "    cbz x0, .Lstart_assert_argv\n    ldr x11, [sp, #8]\n    ldrb w10, [x0, x11]\n",
    );
    refused(
        &session,
        "an unchecked index",
        &code,
        &["rule 9", "rule M2"],
    )
    .await;
    // An index checked after the access, not before it.
    let code = changed(
        "    cbz x0, .Lstart_assert_argv\n",
        "    cbz x0, .Lstart_assert_argv\n    ldr x11, [sp, #8]\n    ldrb w10, [x0, x11]\n    cmp \
         x11, #8\n    b.hs .Lstart_assert_argv\n",
    );
    refused(
        &session,
        "an index checked too late",
        &code,
        &["rule 9", "rule M2"],
    )
    .await;
    session.service.cancel().await.expect("close the session");
}

#[tokio::test]
async fn an_address_made_from_a_plain_number_is_refused() {
    let Some(session) = aarch64_session().await else {
        return;
    };
    let code = changed(
        "    cbz x0, .Lstart_assert_argv\n",
        "    cbz x0, .Lstart_assert_argv\n    mov x9, #4096\n    ldrb w10, [x9]\n",
    );
    refused(&session, "a fixed address", &code, &["rule M1"]).await;
    // A pointer read from memory the rules can't follow to an owner is never stamped done when
    // it's wrong: the sandbox kills the stray read.
    let code = changed(
        "    cbz x0, .Lstart_assert_argv\n",
        "    cbz x0, .Lstart_assert_argv\n    adrp x9, usage_msg\n    add x9, x9, \
         :lo12:usage_msg\n    ldr x10, [x9]\n    ldrb w11, [x10]\n",
    );
    let program = checked_template(&session).await;
    program.write("main.s", &code);
    let report = session.check(&program).await;
    assert_ne!(
        report["state"], "done",
        "a pointer loaded from memory was stamped done: {report}"
    );
    assert!(
        any_line(&report["problems"], "SIGSEGV") || report["state"] == "code_problems",
        "a pointer loaded from memory wasn't stopped: {report}"
    );
    // One that happens to be good (argv[0]) may be done, and then memory is tested, not proven.
    let code = changed(
        "    cbz x0, .Lstart_assert_argv\n",
        "    cbz x0, .Lstart_assert_argv\n    ldr x9, [sp, #8]\n    ldrb w10, [x9]\n",
    );
    let program = checked_template(&session).await;
    program.write("main.s", &code);
    let report = session.check(&program).await;
    assert!(
        report["memory"]
            .as_str()
            .is_some_and(|memory| memory.contains("tested, not proven")),
        "a pointer the rules can't follow was reported as proven: {report}"
    );
    session.service.cancel().await.expect("close the session");
}

#[tokio::test]
async fn writing_read_only_data_or_a_return_address_is_refused() {
    let Some(session) = aarch64_session().await else {
        return;
    };
    let code = changed(
        "    cbz x0, .Lstart_assert_argv\n",
        "    cbz x0, .Lstart_assert_argv\n    adrp x9, usage_msg\n    add x9, x9, \
         :lo12:usage_msg\n    strb wzr, [x9]\n",
    );
    refused(&session, "a write to .rodata", &code, &["rule M6"]).await;
    // A function that changes the register it returns through.
    let code = changed("write_all:\n", "write_all:\n    mov x30, xzr\n");
    refused(
        &session,
        "a changed return address",
        &code,
        &["return address", "rule M3", "rule 1", "rule 3", "dialect"],
    )
    .await;
    // The return address a function saved in its frame record, written through memory: x86-64's
    // `mov qword ptr [rsp], 0` at the start of a function.
    let code = changed(
        "write_i64:\n    stp x29, x30, [sp, #-16]!\n",
        "write_i64:\n    stp x29, x30, [sp, #-16]!\n    str xzr, [sp, #8]\n",
    );
    refused(
        &session,
        "a write over the saved return address",
        &code,
        &["return address", "rule M3"],
    )
    .await;
    // Reading below the stack pointer, where a signal handler or a callee could have left data.
    let code = changed("write_all:\n", "write_all:\n    ldr x9, [sp, #-8]\n");
    refused(
        &session,
        "a read below the stack pointer",
        &code,
        &["rule M3", "rule 3"],
    )
    .await;
    session.service.cancel().await.expect("close the session");
}

#[tokio::test]
async fn loops_the_rules_cant_count_are_refused() {
    let Some(session) = aarch64_session().await else {
        return;
    };
    let code = changed(
        ".Lwrite_all_loop:  // bound: 64\n",
        ".Lwrite_all_loop:  // bound: 63\n",
    );
    refused(&session, "a bound below the counter", &code, &["rule 2"]).await;
    let code = changed(
        "    subs x10, x10, #1\n    b.ne .Lwrite_all_loop\n",
        "    b .Lwrite_all_loop\n",
    );
    refused(&session, "a loop with no counter", &code, &["rule 2"]).await;
    let code = changed(
        "    subs x2, x2, x0\n    b.eq .Lwrite_all_done\n    subs x10, x10, #1\n",
        "    subs x2, x2, x0\n    b.eq .Lwrite_all_done\n    mov x10, #MAX_WRITES\n    subs x10, \
         x10, #1\n",
    );
    refused(&session, "a counter reset in the body", &code, &["rule 2"]).await;
    // A backward jump that isn't a loop's end.
    let code = changed(
        "    mov w0, #EXIT_WRITE_FAILED\n.Lstart_exit:\n",
        "    mov w0, #EXIT_WRITE_FAILED\n    b .Lstart_usage\n.Lstart_exit:\n",
    );
    refused(&session, "a backward jump", &code, &["rule 2", "rule 1"]).await;
    let code = changed(".Lwrite_all_loop:  // bound: 64\n", ".Lwrite_all_loop:\n");
    refused(&session, "a loop with no bound", &code, &["rule 2"]).await;
    session.service.cancel().await.expect("close the session");
}

#[tokio::test]
async fn recursion_calls_through_registers_and_a_missing_return_are_refused() {
    let Some(session) = aarch64_session().await else {
        return;
    };
    let code = changed("write_all:\n", "write_all:\n    bl write_all\n");
    refused(
        &session,
        "recursion",
        &code,
        &["rule 1", "rule 4", "rule 3"],
    )
    .await;
    let code = changed(
        "    cbz x0, .Lstart_assert_argv\n",
        "    cbz x0, .Lstart_assert_argv\n    adrp x9, parse_i64\n    add x9, x9, \
         :lo12:parse_i64\n    blr x9\n",
    );
    refused(
        &session,
        "a call through a register",
        &code,
        &["rule 9", "dialect", "rule 1"],
    )
    .await;
    let code = changed(
        ".Lwrite_all_return:\n    ret\n",
        ".Lwrite_all_return:\n    nop\n",
    );
    refused(
        &session,
        "a function with no return",
        &code,
        &["rule 1", "rule 4", "dialect"],
    )
    .await;
    session.service.cancel().await.expect("close the session");
}

#[tokio::test]
async fn a_tampered_assertion_handler_is_refused() {
    let Some(session) = aarch64_session().await else {
        return;
    };
    // The handler must be the template's, word for word; this one exits quietly with code 0.
    let code = changed(
        "    mov x0, #134\n    mov x8, #94\n    svc #0\n",
        "    mov x0, #0\n    mov x8, #94\n    svc #0\n",
    );
    refused(
        &session,
        "an assertion handler that exits with 0",
        &code,
        &["assert_fail", "rule 5"],
    )
    .await;
    // And one with its message changed.
    let code = changed(
        "    .ascii \"lengua: assertion 000 failed\\n\"\n",
        "    .ascii \"lengua: assertion 000 passed\\n\"\n",
    );
    refused(
        &session,
        "an assertion message that says passed",
        &code,
        &["assert_message", "assert_fail", "rule 5"],
    )
    .await;
    session.service.cancel().await.expect("close the session");
}

#[tokio::test]
async fn code_that_ignores_a_failed_result_is_refused() {
    let Some(session) = aarch64_session().await else {
        return;
    };
    let code = changed(
        "    bl parse_i64\n    tbnz x0, #63, .Lstart_not_a_number\n",
        "    bl parse_i64\n    mov x9, x0\n    tbnz x0, #63, .Lstart_not_a_number\n",
    );
    refused(
        &session,
        "a result used before it's tested",
        &code,
        &["rule 7"],
    )
    .await;
    // A system call's result that is never tested.
    let code = changed(
        "    mov x8, #SYS_WRITE\n    svc #0\n    tbnz x0, #63, .Lwrite_all_return\n",
        "    mov x8, #SYS_WRITE\n    svc #0\n    mov x11, x0\n    tbnz x0, #63, \
         .Lwrite_all_return\n",
    );
    refused(&session, "a write result used first", &code, &["rule 7"]).await;
    session.service.cancel().await.expect("close the session");
}

#[tokio::test]
async fn a_wrong_program_is_never_stamped_done() {
    // The negative of a number comes out one too big when it is above 2^62: a part of the range
    // the tests' samples may miss and the proof covers. Whichever finds it, the program must not
    // be done. Finding it takes the solvers most of their time, so no other proof runs beside it.
    let installed = std::process::Command::new("z3")
        .arg("--version")
        .output()
        .is_ok();
    if !installed {
        lengua::skiplog::note("z3 isn't installed");
        return;
    }
    let Some(session) = proving_aarch64_session().await else {
        return;
    };
    let program = checked_template(&session).await;
    let buggy = asset("linux-aarch64/main-template.s")
        .replacen(
            ".equ OUT_BUF_LEN, 21\n",
            ".equ OUT_BUF_LEN, 21\n.equ BIG, 0x4000000000000000\n",
            1,
        )
        .replacen(
            "    negs x0, x1\n    b.vs .Lstart_assert_negate\n",
            "    negs x0, x1\n    b.vs .Lstart_assert_negate\n    mov x9, #BIG\n    cmp x0, x9\n    \
             b.le .Lstart_fine\n    add x0, x0, #1\n.Lstart_fine:\n",
            1,
        );
    assert_ne!(buggy, asset("linux-aarch64/main-template.s"));
    program.write("main.s", &buggy);
    let report = session.settle(&program).await;
    assert_eq!(
        report["state"], "tests_failed",
        "a wrong program got past the tests and the proof: {report}"
    );
    session.service.cancel().await.expect("close the session");
}
