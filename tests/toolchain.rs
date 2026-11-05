//! The LLVM toolchain wrapper builds real executables, and reports assembler errors.

use lengua::platform::Profile;
use lengua::toolchain::{BuildStep, Toolchain};

/// A program that exits with 42, for this computer's CPU.
#[cfg(target_arch = "x86_64")]
const EXIT_42: (Profile, &str) = (
    Profile::LinuxX86_64,
    ".intel_syntax noprefix\n.globl _start\n.text\n_start:\n    mov eax, 60\n    mov edi, 42\n    \
     syscall\n",
);
#[cfg(target_arch = "aarch64")]
const EXIT_42: (Profile, &str) = (
    Profile::LinuxAarch64,
    ".arch armv8-a\n.globl _start\n.text\n_start:\n    mov x8, #93\n    mov x0, #42\n    svc #0\n",
);

#[tokio::test]
async fn builds_a_program_that_exits_with_42() {
    let (profile, program) = EXIT_42;
    let toolchain = Toolchain::discover()
        .await
        .expect("llvm-mc and ld.lld are installed");
    let dir = tempfile::tempdir().expect("temporary folder");
    let source = dir.path().join("main.s");
    let object = dir.path().join("main.o");
    let executable = dir.path().join("main");
    std::fs::write(&source, program).expect("write main.s");

    toolchain
        .assemble(profile, &source, &object)
        .await
        .expect("assemble");
    toolchain
        .link(profile, &object, &executable)
        .await
        .expect("link");

    // The executable is Linux's, so it runs on Linux alone; elsewhere it is built, which is what
    // the toolchain does.
    if cfg!(target_os = "linux") {
        let status = std::process::Command::new(&executable)
            .status()
            .expect("run the program");
        assert_eq!(status.code(), Some(42));
    }
}

#[tokio::test]
async fn reports_assembler_errors_with_the_tool_output() {
    let toolchain = Toolchain::discover()
        .await
        .expect("llvm-mc and ld.lld are installed");
    let dir = tempfile::tempdir().expect("temporary folder");
    let source = dir.path().join("main.s");
    std::fs::write(
        &source,
        ".intel_syntax noprefix\n.text\n    not_an_instruction rax\n",
    )
    .expect("write main.s");

    let error = toolchain
        .assemble(Profile::LinuxX86_64, &source, &dir.path().join("main.o"))
        .await
        .expect_err("an unknown instruction must fail");
    assert_eq!(error.step, BuildStep::Assemble);
    assert!(error.message.contains("main.s:3"), "{}", error.message);
}

#[tokio::test]
async fn treats_assembler_warnings_as_errors() {
    let toolchain = Toolchain::discover()
        .await
        .expect("llvm-mc and ld.lld are installed");
    let dir = tempfile::tempdir().expect("temporary folder");
    let source = dir.path().join("main.s");
    std::fs::write(
        &source,
        ".intel_syntax noprefix\n.text\n.warning \"careful\"\n",
    )
    .expect("write main.s");

    let error = toolchain
        .assemble(Profile::LinuxX86_64, &source, &dir.path().join("main.o"))
        .await
        .expect_err("a warning must fail the build");
    assert!(error.message.contains("careful"), "{}", error.message);
}
