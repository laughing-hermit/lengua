use std::path::Path;
use std::process::{Command, Stdio};

pub(crate) const TARGETS: [&str; 2] = ["wasm32-wasip2", "wasm32-unknown-unknown"];
const OLDEST: (u32, u32) = (1, 85);

pub(crate) struct Item {
    pub name: String,
    pub found: Option<String>,
    pub fix: Option<String>,
}

pub(crate) struct Doctor {
    pub items: Vec<Item>,
}

fn output(program: &str, args: &[&str], folder: &Path) -> Option<String> {
    let out = Command::new(program)
        .args(args)
        .current_dir(folder)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
}

fn install_rust() -> String {
    if cfg!(windows) {
        "winget install --id Rustlang.Rustup".to_string()
    } else {
        "curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh".to_string()
    }
}

pub(crate) fn version(text: &str) -> Option<(u32, u32)> {
    let number = text.split_whitespace().nth(1)?;
    let mut parts = number.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    Some((major, minor))
}

fn installed_targets(folder: &Path) -> Vec<String> {
    if let Some(list) = output("rustup", &["target", "list", "--installed"], folder) {
        return list.lines().map(|l| l.trim().to_string()).collect();
    }
    let Some(sysroot) = output("rustc", &["--print", "sysroot"], folder) else {
        return Vec::new();
    };
    let lib = Path::new(&sysroot).join("lib").join("rustlib");
    TARGETS
        .iter()
        .filter(|t| lib.join(t).join("lib").is_dir())
        .map(|t| (*t).to_string())
        .collect()
}

pub(crate) fn examine(folder: &Path) -> Doctor {
    let mut items = Vec::new();
    let cargo = output("cargo", &["--version"], folder);
    items.push(Item {
        name: "cargo".to_string(),
        fix: cargo.is_none().then(install_rust),
        found: cargo,
    });
    let rustc = output("rustc", &["--version"], folder);
    let fix = match rustc.as_deref().map(version) {
        None => Some(install_rust()),
        Some(Some(found)) if found >= OLDEST => None,
        Some(_) => Some("rustup update stable".to_string()),
    };
    items.push(Item {
        name: format!("rustc {}.{} or later", OLDEST.0, OLDEST.1),
        found: rustc,
        fix,
    });
    for component in ["rustfmt", "clippy"] {
        let command = if component == "rustfmt" {
            "fmt"
        } else {
            component
        };
        let found = output("cargo", &[command, "--version"], folder);
        items.push(Item {
            name: component.to_string(),
            fix: found
                .is_none()
                .then(|| format!("rustup component add {component}")),
            found,
        });
    }
    let installed = installed_targets(folder);
    for target in TARGETS {
        let have = installed.iter().any(|t| t == target);
        items.push(Item {
            name: format!("the target {target}"),
            found: have.then(|| "installed".to_string()),
            fix: (!have).then(|| format!("rustup target add {target}")),
        });
    }
    Doctor { items }
}

impl Doctor {
    pub(crate) fn rustc(&self) -> String {
        self.items
            .iter()
            .filter(|i| i.name.starts_with("rustc"))
            .find_map(|i| i.found.clone())
            .unwrap_or_else(|| "rustc, version not found".to_string())
    }

    pub(crate) fn ok(&self) -> bool {
        self.items.iter().all(|i| i.fix.is_none())
    }

    fn line(item: &Item) -> String {
        let found = item.found.as_deref().unwrap_or("not found");
        match &item.fix {
            None => format!("ok       {}: {found}", item.name),
            Some(fix) => format!("missing  {}: {found}; to install it, run: {fix}", item.name),
        }
    }

    pub(crate) fn text(&self) -> String {
        let mut lines: Vec<String> = self.items.iter().map(Self::line).collect();
        lines.push(if self.ok() {
            "lengua can build programs.".to_string()
        } else {
            "lengua can't build programs until the missing parts are installed.".to_string()
        });
        let mut text = lines.join("\n");
        text.push('\n');
        text
    }

    pub(crate) fn problems(&self) -> String {
        let lines: Vec<String> = self
            .items
            .iter()
            .filter(|i| i.fix.is_some())
            .map(Self::line)
            .collect();
        format!(
            "the Rust toolchain is not ready, so lengua can't build: {}",
            lines.join("; ")
        )
    }
}

#[cfg(test)]
mod tests {
    use super::{Doctor, Item, version};

    #[test]
    fn the_version_is_read_from_the_first_line() {
        assert_eq!(version("rustc 1.100.0 (abc 2026-11-12)"), Some((1, 100)));
        assert_eq!(version("cargo"), None);
    }

    #[test]
    fn a_missing_part_gives_its_command() {
        let doctor = Doctor {
            items: vec![
                Item {
                    name: "cargo".to_string(),
                    found: Some("cargo 1.100.0".to_string()),
                    fix: None,
                },
                Item {
                    name: "the target wasm32-wasip2".to_string(),
                    found: None,
                    fix: Some("rustup target add wasm32-wasip2".to_string()),
                },
            ],
        };
        assert!(!doctor.ok());
        assert_eq!(
            doctor.text(),
            "ok       cargo: cargo 1.100.0\nmissing  the target wasm32-wasip2: not found; to install it, run: rustup target add wasm32-wasip2\nlengua can't build programs until the missing parts are installed.\n"
        );
        assert!(doctor.problems().ends_with(
            "build: missing  the target wasm32-wasip2: not found; to install it, run: rustup target add wasm32-wasip2"
        ));
    }
}
