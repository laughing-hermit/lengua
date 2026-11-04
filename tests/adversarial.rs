//! Programs written to get past lengua's own checks, each of which `check` must refuse: a system
//! call the spec doesn't allow, an access past a buffer or into read-only data, an address made
//! from a plain number, a return address overwritten, a loop the rules miscount, recursion, a
//! call through a register, and a tampered assertion handler. They are the template with one
//! change that tries it; none is ever run, because the rules refuse it first. Any attempt that
//! isn't refused is a bug in lengua to fix before anything else.

// Its programs are x86-64 code, which runs only on an x86-64 computer; the AArch64 suite
// (tests/adversarial_aarch64.rs) tries the same attempts in AArch64's dialect.
#![cfg(target_arch = "x86_64")]

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

/// The template changed once: `from`, which must be in it, becomes `to`.
fn changed(from: &str, to: &str) -> String {
    let template = asset("main-template.s");
    assert!(template.contains(from), "the template has no {from:?}");
    template.replacen(from, to, 1)
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
    // The template's spec allows write and exit. Each of these is a call a program could use to
    // reach beyond its folder, its memory or its user; the kernel's filter kills it too (see
    // tests/runner.rs), but the rules refuse the program first.
    let calls = [
        ("open", 2),
        ("mmap", 9),
        ("mprotect", 10),
        ("clone", 56),
        ("fork", 57),
        ("execve", 59),
        ("kill", 62),
        ("ptrace", 101),
        ("setuid", 105),
        ("prctl", 157),
        ("mount", 165),
        ("unshare", 272),
        ("execveat", 322),
        ("io_uring_setup", 425),
        ("socket", 41),
        ("connect", 42),
    ];
    let session = Session::start(Protocol::Discover).await;
    for (name, number) in calls {
        let code = changed(
            "    mov eax, SYS_EXIT\n    syscall\n",
            &format!("    mov eax, {number}\n    syscall\n    mov eax, SYS_EXIT\n    syscall\n"),
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
    let session = Session::start(Protocol::Discover).await;
    // The number comes from a register that holds anything.
    let code = changed(
        "    mov eax, SYS_EXIT\n    syscall\n",
        "    mov eax, edi\n    syscall\n    mov eax, SYS_EXIT\n    syscall\n",
    );
    refused(
        &session,
        "a number from a register",
        &code,
        &["isn't chosen by a `mov eax, SYS_NAME`"],
    )
    .await;
    // The number is set, then a label lets other paths in before the call.
    let code = changed(
        "    mov eax, SYS_EXIT\n    syscall\n",
        "    mov eax, SYS_EXIT\n.Lstart_sneak:\n    syscall\n",
    );
    refused(
        &session,
        "a label between the number and the call",
        &code,
        &["isn't chosen by a `mov eax, SYS_NAME`", "rule 1"],
    )
    .await;
    session.service.cancel().await.expect("close the session");
}

#[tokio::test]
async fn accesses_outside_their_owner_are_refused() {
    let session = Session::start(Protocol::Discover).await;
    // One byte past the end of usage_msg, which is 18 bytes.
    let code = changed(
        "    mov rdi, qword ptr [rsp + 16]\n",
        "    movzx ecx, byte ptr [rip + usage_msg + 18]\n    mov rdi, qword ptr [rsp + 16]\n",
    );
    refused(
        &session,
        "a read past a buffer",
        &code,
        &["rule M2", "past the end of"],
    )
    .await;
    // Far past it, where another symbol or nothing lies.
    let code = changed(
        "    mov rdi, qword ptr [rsp + 16]\n",
        "    movzx ecx, byte ptr [rip + usage_msg + 4096]\n    mov rdi, qword ptr [rsp + 16]\n",
    );
    refused(
        &session,
        "a read far past a buffer",
        &code,
        &["rule M2", "past the end of"],
    )
    .await;
    // An index nothing has checked.
    let code = changed(
        "    mov rdi, qword ptr [rsp + 16]\n    test rdi, rdi\n",
        "    mov rdi, qword ptr [rsp + 16]\n    mov rcx, qword ptr [rsp + 8]\n    movzx edx, \
         byte ptr [rdi + rcx]\n    test rdi, rdi\n",
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
        "    mov rdi, qword ptr [rsp + 16]\n    test rdi, rdi\n",
        "    mov rdi, qword ptr [rsp + 16]\n    mov rcx, qword ptr [rsp + 8]\n    movzx edx, \
         byte ptr [rdi + rcx]\n    cmp rcx, 8\n    jae .Lstart_assert_argv\n    test rdi, rdi\n",
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
    let session = Session::start(Protocol::Discover).await;
    let code = changed(
        "    mov rdi, qword ptr [rsp + 16]\n",
        "    mov esi, 4096\n    movzx ecx, byte ptr [rsi]\n    mov rdi, qword ptr [rsp + 16]\n",
    );
    refused(&session, "a fixed address", &code, &["rule M1"]).await;
    let code = changed(
        "    mov rdi, qword ptr [rsp + 16]\n",
        "    lea rax, [rip + usage_msg]\n    mov rsi, qword ptr [rax]\n    movzx ecx, byte ptr \
         [rsi]\n    mov rdi, qword ptr [rsp + 16]\n",
    );
    // A pointer read from memory has no owner the rules can follow, so it's tested, not proven:
    // the sandbox kills the stray read, and the program is never stamped done.
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
    session.service.cancel().await.expect("close the session");
}

#[tokio::test]
async fn writing_read_only_data_or_a_return_address_is_refused() {
    let session = Session::start(Protocol::Discover).await;
    let code = changed(
        "    mov rdi, qword ptr [rsp + 16]\n",
        "    mov byte ptr [rip + usage_msg], 0\n    mov rdi, qword ptr [rsp + 16]\n",
    );
    refused(
        &session,
        "a write to .rodata",
        &code,
        &["rule M6", "read-only"],
    )
    .await;
    // write_all's return address is at [rsp] when it starts.
    let code = changed("write_all:\n", "write_all:\n    mov qword ptr [rsp], 0\n");
    refused(
        &session,
        "a write over a return address",
        &code,
        &["rule M3", "rule 3"],
    )
    .await;
    // Reading below the stack pointer, where a signal handler or a callee could have left data.
    let code = changed(
        "write_all:\n",
        "write_all:\n    mov rax, qword ptr [rsp - 8]\n",
    );
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
    let session = Session::start(Protocol::Discover).await;
    // A bound smaller than the counter's start.
    let code = changed(
        ".Lwrite_all_loop:  # bound: 64\n",
        ".Lwrite_all_loop:  # bound: 63\n",
    );
    refused(&session, "a bound below the counter", &code, &["rule 2"]).await;
    // A loop that never counts down.
    let code = changed(
        "    dec r8\n    jnz .Lwrite_all_loop\n",
        "    jmp .Lwrite_all_loop\n",
    );
    refused(&session, "a loop with no counter", &code, &["rule 2"]).await;
    // A counter the body changes.
    let code = changed(
        "    add rsi, rax\n    sub rdx, rax\n    jz .Lwrite_all_done\n    dec r8\n",
        "    add rsi, rax\n    sub rdx, rax\n    jz .Lwrite_all_done\n    mov r8, MAX_WRITES\n    \
         dec r8\n",
    );
    refused(&session, "a counter reset in the body", &code, &["rule 2"]).await;
    // A backward jump that isn't a loop's end.
    let code = changed(
        "    mov edi, EXIT_WRITE_FAILED\n.Lstart_exit:\n",
        "    mov edi, EXIT_WRITE_FAILED\n    jmp .Lstart_usage\n.Lstart_exit:\n",
    );
    refused(&session, "a backward jump", &code, &["rule 2", "rule 1"]).await;
    // A loop with no bound written at all.
    let code = changed(".Lwrite_all_loop:  # bound: 64\n", ".Lwrite_all_loop:\n");
    refused(&session, "a loop with no bound", &code, &["rule 2"]).await;
    session.service.cancel().await.expect("close the session");
}

#[tokio::test]
async fn recursion_calls_through_registers_and_a_missing_return_are_refused() {
    let session = Session::start(Protocol::Discover).await;
    let code = changed("write_all:\n", "write_all:\n    call write_all\n");
    refused(&session, "recursion", &code, &["rule 1", "rule 4"]).await;
    let code = changed(
        "    mov rdi, qword ptr [rsp + 16]\n",
        "    lea rax, [rip + parse_i64]\n    call rax\n    mov rdi, qword ptr [rsp + 16]\n",
    );
    refused(
        &session,
        "a call through a register",
        &code,
        &["rule 9", "dialect", "rule M1", "rule 1"],
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
        &["rule 1", "rule 4"],
    )
    .await;
    session.service.cancel().await.expect("close the session");
}

#[tokio::test]
async fn a_tampered_assertion_handler_is_refused() {
    let session = Session::start(Protocol::Discover).await;
    // The handler must be the template's, word for word; this one exits quietly with code 0.
    let code = changed(
        "    mov eax, 231\n    mov edi, 134\n    syscall\n",
        "    mov eax, 231\n    mov edi, 0\n    syscall\n",
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
    let session = Session::start(Protocol::Discover).await;
    // The result of a call that can fail is used before it's tested.
    let code = changed(
        "    call parse_i64\n    test rax, rax\n    js .Lstart_not_a_number\n",
        "    call parse_i64\n    mov rbx, rax\n    test rax, rax\n    js .Lstart_not_a_number\n",
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
        "    mov eax, SYS_WRITE\n    syscall\n    test rax, rax\n    js .Lwrite_all_return\n",
        "    mov eax, SYS_WRITE\n    syscall\n    mov rbx, rax\n    test rax, rax\n    js \
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
        eprintln!("z3 isn't installed");
        return;
    }
    let session = Session::proving_alone(Protocol::Discover).await;
    let program = checked_template(&session).await;
    let buggy = asset("main-template.s")
        .replacen(
            ".equ OUT_BUF_LEN, 21\n",
            ".equ OUT_BUF_LEN, 21\n.equ BIG, 0x4000000000000000\n",
            1,
        )
        .replacen(
            "    neg rdx\n    jo .Lstart_assert_negate\n",
            "    neg rdx\n    jo .Lstart_assert_negate\n    mov rcx, BIG\n    cmp rdx, rcx\n    jle \
             .Lstart_fine\n    inc rdx\n.Lstart_fine:\n",
            1,
        );
    assert_ne!(buggy, asset("main-template.s"));
    program.write("main.s", &buggy);
    let report = session.settle(&program).await;
    assert_eq!(
        report["state"], "tests_failed",
        "a wrong program got past the tests and the proof: {report}"
    );
    session.service.cancel().await.expect("close the session");
}
