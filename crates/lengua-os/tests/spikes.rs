//! Spikes: what lengua's planned executables, and the systems they run on, do, answered on the
//! real runners (todo.md, round 1) before the edge's design (round 2) relies on it. Each prints
//! `SPIKE <name>: <what it found>` lines, which ROADMAP.md records; run them with the ignored
//! tests and the filter `spike` (`ci.yml`: mode `ignored`).
//!
//! They are the toolchain's and the systems' own behavior: how LLVM's tools turn lengua's lowered
//! assembly into a Mach-O or PE executable with no SDK, and what each system does with the
//! simplest program, a pipe or a file name. Whatever a sandbox refuses is not tried here.

// Each system uses a different subset of these helpers.
#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// A fresh folder for one spike.
fn folder(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("lengua-spike-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&path);
    std::fs::create_dir_all(&path).expect("a spike folder");
    path
}

/// Runs `tool` with `args` in `dir`, printing what it said; returns its output.
fn run(dir: &Path, tool: &str, args: &[&str]) -> Output {
    let output = Command::new(tool)
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap_or_else(|error| panic!("{tool} didn't start: {error}"));
    println!(
        "SPIKE $ {tool} {} -> {:?}\n{}{}",
        args.join(" "),
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    output
}

/// Runs `tool`, which must succeed.
fn must(dir: &Path, tool: &str, args: &[&str]) {
    let output = run(dir, tool, args);
    assert!(output.status.success(), "{tool} {args:?} failed");
}

/// The bytes of a file name as hexadecimal, as the system stores them where it tells.
fn hex(name: &std::ffi::OsStr) -> String {
    #[cfg(unix)]
    let bytes = {
        use std::os::unix::ffi::OsStrExt as _;
        name.as_bytes().to_vec()
    };
    #[cfg(not(unix))]
    let bytes = name.to_string_lossy().into_owned().into_bytes();
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// What a system keeps of the names given to it, and which spellings find a file: the answer
/// decides how the broker can match names byte for byte (decision 2 of ROADMAP.md).
#[test]
#[ignore = "a spike: ci.yml mode ignored, filter spike"]
fn spike_names_keep_their_bytes() {
    let dir = folder("names");
    let composed = "caf\u{e9}";
    let decomposed = "cafe\u{301}";
    std::fs::write(dir.join(composed), b"composed").expect("a file");
    std::fs::write(dir.join("Readme"), b"readme").expect("a file");
    let mut listed: Vec<String> = std::fs::read_dir(&dir)
        .expect("a listing")
        .map(|entry| hex(&entry.expect("an entry").file_name()))
        .collect();
    listed.sort();
    println!("SPIKE names: listed as {listed:?}");
    println!(
        "SPIKE names: the composed spelling is {}",
        hex(std::ffi::OsStr::new(composed))
    );
    assert!(
        listed.contains(&hex(std::ffi::OsStr::new(composed))),
        "the listing gives the stored bytes back"
    );
    println!(
        "SPIKE names: the decomposed spelling opens the composed file: {}",
        std::fs::read(dir.join(decomposed)).is_ok()
    );
    println!(
        "SPIKE names: README opens the file made as Readme: {}",
        std::fs::read(dir.join("README")).is_ok()
    );
    std::fs::write(dir.join("readme"), b"again").expect("a write");
    let named_so = std::fs::read_dir(&dir)
        .expect("a listing")
        .filter(|entry| {
            entry.as_ref().is_ok_and(|entry| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .eq_ignore_ascii_case("readme")
            })
        })
        .count();
    println!("SPIKE names: creating `readme` over Readme leaves {named_so} entries named so");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Whether 4,096 bytes written to a pipe are read whole by one read of a larger buffer: what the
/// proofs' "input that one read returns whole" needs on every system (decision 12 of ROADMAP.md).
#[test]
#[ignore = "a spike: ci.yml mode ignored, filter spike"]
fn spike_a_4096_byte_write_is_read_whole() {
    use std::io::{Read as _, Write as _};
    let (mut reader, mut writer) = std::io::pipe().expect("a pipe");
    let sent = vec![7_u8; 4096];
    writer.write_all(&sent).expect("a write");
    let mut buffer = vec![0_u8; 65536];
    let got = reader.read(&mut buffer).expect("a read");
    println!("SPIKE pipe: one read of a 64 KiB buffer after one 4,096-byte write got {got} bytes");
    assert_eq!(got, 4096);
    // Written in two pieces before the read, they come together too.
    writer.write_all(&sent[..1000]).expect("a write");
    writer.write_all(&sent[1000..2000]).expect("a write");
    let got = reader.read(&mut buffer).expect("a read");
    println!("SPIKE pipe: two writes of 1,000 bytes, one read got {got} bytes");
}

#[cfg(target_os = "macos")]
mod macos {
    use super::*;

    const ARCH: &str = if cfg!(target_arch = "aarch64") {
        "arm64"
    } else {
        "x86_64"
    };
    const HELLO: &str = if cfg!(target_arch = "aarch64") {
        include_str!("../spikes/macos-aarch64-hello.s")
    } else {
        include_str!("../spikes/macos-x86_64-hello.s")
    };
    const CRASH: &str = if cfg!(target_arch = "aarch64") {
        include_str!("../spikes/macos-aarch64-crash.s")
    } else {
        include_str!("../spikes/macos-x86_64-crash.s")
    };
    const LIBSYSTEM: &str = include_str!("../spikes/libSystem.tbd");

    /// Builds `source` into `dir/name` with LLVM's tools alone and a hand-written stub of libSystem:
    /// no Xcode, no SDK.
    fn build(dir: &Path, name: &str, source: &str) -> PathBuf {
        std::fs::write(dir.join(format!("{name}.s")), source).expect("the source");
        std::fs::write(dir.join("libSystem.tbd"), LIBSYSTEM).expect("the stub");
        let triple = format!("{ARCH}-apple-macos11.0");
        must(
            dir,
            "llvm-mc",
            &[
                &format!("--triple={triple}"),
                "--filetype=obj",
                "--fatal-warnings",
                &format!("{name}.s"),
                "-o",
                &format!("{name}.o"),
            ],
        );
        must(
            dir,
            "ld64.lld",
            &[
                "-arch",
                ARCH,
                "-platform_version",
                "macos",
                "11.0",
                "11.0",
                "-e",
                "_main",
                "-fatal_warnings",
                "-o",
                name,
                &format!("{name}.o"),
                "libSystem.tbd",
            ],
        );
        dir.join(name)
    }

    /// Runs `program`; if the system refuses it as unsigned, signs it ad hoc as the system asks
    /// and says so.
    fn start(dir: &Path, program: &Path) -> Output {
        let first = Command::new(program).output().expect("a start");
        if first.status.code().is_some() || cfg!(target_arch = "x86_64") {
            return first;
        }
        println!(
            "SPIKE macOS: unsigned, it ended {:?}; signing it ad hoc",
            first.status
        );
        must(dir, "codesign", &["--force", "-s", "-", &program.to_string_lossy()]);
        Command::new(program).output().expect("a start")
    }

    /// LLVM's tools make a runnable Mach-O from lengua's kind of assembly: libSystem's `write` and
    /// `_exit` called through the GOT, a string in `__TEXT,__cstring`.
    #[test]
    #[ignore = "a spike: ci.yml mode ignored, filter spike"]
    fn spike_hello_through_the_got() {
        let dir = folder("macos-hello");
        let program = build(&dir, "hello", HELLO);
        let output = start(&dir, &program);
        println!(
            "SPIKE macOS hello: {:?}, stdout {:?}, stderr {:?}",
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        let _ = run(&dir, "otool", &["-l", "hello"]);
        let _ = run(&dir, "otool", &["-L", "hello"]);
        let _ = run(&dir, "ls", &["-l", "hello"]);
        assert_eq!(output.stdout, b"hello\n");
        assert!(output.status.success());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// How long a program that faults takes to be reported ended, against one that exits, and by
    /// which signal: the system's crash reporter may delay the end (decision 5 of ROADMAP.md's
    /// sandbox table says SIGBUS is shown as 11).
    #[test]
    #[ignore = "a spike: ci.yml mode ignored, filter spike"]
    fn spike_a_fault_ends_by_a_signal() {
        use std::os::unix::process::ExitStatusExt as _;
        let dir = folder("macos-crash");
        let crash = build(&dir, "crash", CRASH);
        let hello = build(&dir, "hello", HELLO);
        // An ARM64 program must be signed to run at all: signed here, so that what ends the
        // crashing one is its fault and not the missing signature.
        if cfg!(target_arch = "aarch64") {
            for program in [&crash, &hello] {
                must(&dir, "codesign", &["--force", "-s", "-", &program.to_string_lossy()]);
            }
        }
        for _ in 0..3 {
            let began = std::time::Instant::now();
            let status = Command::new(&crash).status().expect("a start");
            println!(
                "SPIKE macOS fault: ended {status:?} (signal {:?}) after {:?}",
                status.signal(),
                began.elapsed()
            );
            assert!(status.signal().is_some(), "{status:?}");
        }
        let began = std::time::Instant::now();
        let status = Command::new(&hello).status().expect("a start");
        println!(
            "SPIKE macOS fault: for comparison, a program that exits took {:?} ({status:?})",
            began.elapsed()
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[cfg(windows)]
mod windows {
    use super::*;

    const MACHINE: &str = if cfg!(target_arch = "aarch64") {
        "arm64"
    } else {
        "i386:x86-64"
    };
    const TRIPLE: &str = if cfg!(target_arch = "aarch64") {
        "aarch64-pc-windows-msvc"
    } else {
        "x86_64-pc-windows-msvc"
    };
    const HELLO: &str = if cfg!(target_arch = "aarch64") {
        include_str!("../spikes/windows-aarch64-hello.s")
    } else {
        include_str!("../spikes/windows-x86_64-hello.s")
    };
    const KERNEL32: &str = include_str!("../spikes/kernel32.def");

    /// LLVM's tools make a runnable PE from lengua's kind of assembly with no SDK: an import
    /// library for kernel32 built from a list of names, and `kernel32` called through the import
    /// address table, with symbols kept (`/debug:symtab`) for reading the executable back.
    #[test]
    #[ignore = "a spike: ci.yml mode ignored, filter spike"]
    fn spike_hello_through_the_iat() {
        let dir = folder("windows-hello");
        std::fs::write(dir.join("hello.s"), HELLO).expect("the source");
        std::fs::write(dir.join("kernel32.def"), KERNEL32).expect("the import list");
        must(
            &dir,
            "llvm-dlltool",
            &["-m", MACHINE, "-d", "kernel32.def", "-l", "kernel32.lib"],
        );
        must(
            &dir,
            "llvm-mc",
            &[
                &format!("--triple={TRIPLE}"),
                "--filetype=obj",
                "--fatal-warnings",
                "hello.s",
                "-o",
                "hello.obj",
            ],
        );
        must(
            &dir,
            "lld-link",
            &[
                "/entry:_start",
                "/subsystem:console",
                "/nodefaultlib",
                "/debug:symtab",
                "/out:hello.exe",
                "hello.obj",
                "kernel32.lib",
            ],
        );
        let output = Command::new(dir.join("hello.exe")).output().expect("a start");
        println!(
            "SPIKE Windows hello: {:?}, stdout {:?}, stderr {:?}",
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        let _ = run(&dir, "llvm-objdump", &["-p", "hello.exe"]);
        assert_eq!(output.stdout, b"hello\n");
        assert!(output.status.success());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
