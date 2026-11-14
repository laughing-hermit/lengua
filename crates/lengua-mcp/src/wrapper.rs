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

pub(crate) fn rewritten(args: &[String], version: &str) -> Vec<String> {
    if !wasm_target(args) {
        return args.to_vec();
    }
    let fixed = metadata(args, version);
    let mut out = Vec::with_capacity(args.len());
    let mut at = 0;
    while at < args.len() {
        let arg = &args[at];
        let next_is_metadata = (arg == "-C" || arg == "--codegen")
            && args.get(at + 1).is_some_and(|n| n.starts_with("metadata="));
        if next_is_metadata {
            out.push(arg.clone());
            out.push(format!("metadata={fixed}"));
            at += 2;
            continue;
        }
        if arg.starts_with("-Cmetadata=") {
            out.push(format!("-Cmetadata={fixed}"));
        } else {
            out.push(arg.clone());
        }
        at += 1;
    }
    out
}

pub(crate) fn run(args: &[String]) -> ExitCode {
    let Some((program, rest)) = args.split_first() else {
        eprintln!("{MARK}: no compiler to run");
        return ExitCode::from(2);
    };
    let version = std::env::var("CARGO_PKG_VERSION").unwrap_or_default();
    let status = Command::new(program)
        .args(rewritten(rest, &version))
        .env_remove(MARK)
        .status();
    match status {
        Ok(status) => status
            .code()
            .and_then(|c| u8::try_from(c).ok())
            .map_or(ExitCode::FAILURE, ExitCode::from),
        Err(error) => {
            eprintln!("{MARK}: could not start {program}: {error}");
            ExitCode::FAILURE
        }
    }
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
        assert!(a.contains(&"extra-filename=-a1b2".to_string()));
        assert_ne!(metadata(&linux, "2.0.1"), metadata(&linux, "2.0.2"));
        let other = args("--crate-name wasip2 --crate-type lib -C metadata=a1b2 --target wasm32-wasip2");
        assert_ne!(metadata(&linux, "2.0.1"), metadata(&other, "2.0.1"));
    }

    #[test]
    fn a_crate_for_the_host_keeps_cargo_s_metadata() {
        let host = args("--crate-name proc --crate-type proc-macro -C metadata=a1b2");
        assert_eq!(rewritten(&host, "1.0.0"), host);
    }
}
