use std::fs;
use std::path::Path;
use std::process::{Command, ExitCode};

pub(crate) const MARK: &str = "LENGUA_RUSTC_WRAPPER";

fn value<'a>(args: &'a [String], flag: &str) -> Vec<&'a str> {
    let mut out = Vec::new();
    let joined = format!("{flag}=");
    let mut at = 0;
    while at < args.len() {
        let arg = args[at].as_str();
        if arg == flag {
            if let Some(next) = args.get(at + 1) {
                out.push(next.as_str());
            }
            at += 2;
            continue;
        }
        if let Some(rest) = arg.strip_prefix(&joined) {
            out.push(rest);
        }
        at += 1;
    }
    out
}

fn wasm_target(args: &[String]) -> bool {
    value(args, "--target")
        .iter()
        .any(|t| t.starts_with("wasm32-"))
}

pub(crate) fn metadata(args: &[String], version: &str) -> String {
    let mut parts: Vec<String> = Vec::new();
    for flag in ["--crate-name", "--crate-type", "--edition", "--target"] {
        for found in value(args, flag) {
            parts.push(format!("{flag} {found}"));
        }
    }
    let mut cfgs: Vec<&str> = value(args, "--cfg");
    cfgs.sort_unstable();
    parts.extend(cfgs.iter().map(|c| format!("--cfg {c}")));
    let mut externs: Vec<&str> = value(args, "--extern")
        .iter()
        .map(|e| e.split_once('=').map_or(*e, |(name, _)| name))
        .collect();
    externs.sort_unstable();
    parts.extend(externs.iter().map(|e| format!("--extern {e}")));
    parts.push(format!("version {version}"));
    let hex = lengua_idea::sha256_hex(parts.join("\n").as_bytes());
    hex.get(..16).unwrap_or(&hex).to_string()
}

fn codegen(args: &[String], key: &str) -> Option<String> {
    let prefix = format!("{key}=");
    let mut found = None;
    for (at, arg) in args.iter().enumerate() {
        let option = if arg == "-C" || arg == "--codegen" {
            args.get(at + 1).map(String::as_str)
        } else {
            arg.strip_prefix("-C")
        };
        if let Some(text) = option.and_then(|o| o.strip_prefix(&prefix)) {
            found = Some(text.to_string());
        }
    }
    found
}

fn replaced(args: &[String], key: &str, new: &str) -> Vec<String> {
    let prefix = format!("{key}=");
    let mut out = Vec::with_capacity(args.len());
    let mut at = 0;
    while at < args.len() {
        let arg = &args[at];
        let split = (arg == "-C" || arg == "--codegen")
            && args.get(at + 1).is_some_and(|n| n.starts_with(&prefix));
        if split {
            out.push(arg.clone());
            out.push(format!("{prefix}{new}"));
            at += 2;
            continue;
        }
        if arg.starts_with(&format!("-C{prefix}")) {
            out.push(format!("-C{prefix}{new}"));
        } else {
            out.push(arg.clone());
        }
        at += 1;
    }
    out
}

pub(crate) fn rewritten(args: &[String], version: &str) -> Vec<String> {
    if !wasm_target(args) {
        return args.to_vec();
    }
    let fixed = metadata(args, version);
    let out = replaced(args, "metadata", &fixed);
    if codegen(args, "extra-filename").is_some() {
        replaced(&out, "extra-filename", &format!("-{fixed}"))
    } else {
        out
    }
}

pub(crate) fn rename_outputs(folder: &Path, from: &str, to: &str) -> std::io::Result<()> {
    if from == to || from.is_empty() {
        return Ok(());
    }
    for entry in fs::read_dir(folder)?.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if !name.contains(from) {
            continue;
        }
        let target = folder.join(name.replace(from, to));
        if Path::new(&name).extension().is_some_and(|e| e.eq_ignore_ascii_case("d")) {
            let text = fs::read_to_string(entry.path())?;
            fs::write(&target, text.replace(from, to))?;
            fs::remove_file(entry.path())?;
        } else {
            fs::rename(entry.path(), &target)?;
        }
    }
    Ok(())
}

pub(crate) fn run(args: &[String]) -> ExitCode {
    let Some((program, rest)) = args.split_first() else {
        eprintln!("{MARK}: no compiler to run");
        return ExitCode::from(2);
    };
    let version = std::env::var("CARGO_PKG_VERSION").unwrap_or_default();
    let arguments = rewritten(rest, &version);
    let status = Command::new(program)
        .args(&arguments)
        .env_remove(MARK)
        .status();
    let code = match status {
        Ok(status) => status.code().and_then(|c| u8::try_from(c).ok()),
        Err(error) => {
            eprintln!("{MARK}: could not start {program}: {error}");
            return ExitCode::FAILURE;
        }
    };
    let cargo_suffix = codegen(rest, "extra-filename").unwrap_or_default();
    let our_suffix = codegen(&arguments, "extra-filename").unwrap_or_default();
    if code == Some(0)
        && let Some(folder) = value(rest, "--out-dir").first()
        && let Err(error) = rename_outputs(Path::new(folder), &our_suffix, &cargo_suffix)
    {
        eprintln!("{MARK}: could not give the outputs Cargo's names: {error}");
        return ExitCode::FAILURE;
    }
    code.map_or(ExitCode::FAILURE, ExitCode::from)
}

#[cfg(test)]
mod tests {
    use super::{metadata, rewritten};

    fn args(text: &str) -> Vec<String> {
        text.split_whitespace().map(str::to_string).collect()
    }

    #[test]
    fn a_wasm_crate_gets_a_metadata_of_its_own_identity_only() {
        let linux = args(
            "--crate-name wasip2 --edition=2021 src/lib.rs --crate-type lib -C opt-level=3 -C metadata=a1b2 -C extra-filename=-a1b2 --target wasm32-wasip2 --cfg feature=\"std\" --extern wit_bindgen=/home/runner/x/libwit_bindgen-a1.rlib -L dependency=/home/runner/x",
        );
        let windows = args(
            "--crate-name wasip2 --edition=2021 src\\lib.rs --crate-type lib -C opt-level=3 -C metadata=ffee -C extra-filename=-ffee --target wasm32-wasip2 --cfg feature=\"std\" --extern wit_bindgen=D:\\x\\libwit_bindgen-ff.rlib -L dependency=D:\\x",
        );
        let a = rewritten(&linux, "2.0.1");
        let b = rewritten(&windows, "2.0.1");
        let pick = |all: &[String]| {
            all.iter()
                .find(|x| x.starts_with("metadata="))
                .cloned()
                .unwrap_or_default()
        };
        assert_eq!(pick(&a), pick(&b));
        assert_ne!(pick(&a), "metadata=a1b2");
        let suffix = |all: &[String]| {
            all.iter()
                .find(|x| x.starts_with("extra-filename="))
                .cloned()
                .unwrap_or_default()
        };
        assert_eq!(suffix(&a), suffix(&b));
        assert_ne!(suffix(&a), "extra-filename=-a1b2");
        assert_ne!(metadata(&linux, "2.0.1"), metadata(&linux, "2.0.2"));
        let other =
            args("--crate-name wasip2 --crate-type lib -C metadata=a1b2 --target wasm32-wasip2");
        assert_ne!(metadata(&linux, "2.0.1"), metadata(&other, "2.0.1"));
    }

    #[test]
    fn a_crate_for_the_host_keeps_cargo_s_metadata() {
        let host = args("--crate-name proc --crate-type proc-macro -C metadata=a1b2");
        assert_eq!(rewritten(&host, "1.0.0"), host);
    }

    #[test]
    fn the_outputs_get_cargo_s_names_back() {
        let folder =
            std::env::temp_dir().join(format!("lengua-mcp-{}-renamed", std::process::id()));
        std::fs::create_dir_all(&folder).expect("a folder");
        std::fs::write(folder.join("libwasip2-ours.rlib"), "code").expect("a file");
        std::fs::write(
            folder.join("wasip2-ours.d"),
            "libwasip2-ours.rlib: src/lib.rs\n",
        )
        .expect("a file");
        super::rename_outputs(&folder, "-ours", "-cargo").expect("renamed");
        assert!(folder.join("libwasip2-cargo.rlib").is_file());
        assert_eq!(
            std::fs::read_to_string(folder.join("wasip2-cargo.d")).expect("the dep-info"),
            "libwasip2-cargo.rlib: src/lib.rs\n"
        );
        assert!(!folder.join("libwasip2-ours.rlib").exists());
    }
}
