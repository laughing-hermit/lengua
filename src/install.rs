//! `lengua-mcp install`: lengua installed from its release archive, by the archive's own
//! lengua-mcp, the same way on every system. It checks that this computer is the system and CPU
//! the archive is for (its TARGET), the hardware's own CPU even where a shell runs under
//! macOS's Rosetta or Windows's x64 emulation, and on Linux that its glibc is new enough; copies
//! the plugin where Claude Code finds plugins, a marketplace folder of lengua's own, which it
//! adds to Claude Code when the claude command is installed (and says the commands that do, when
//! it isn't); then runs the installed lengua-mcp's doctor, which says whether lengua has what it
//! needs here, and exits with doctor's code: 0 when nothing is wrong. The archive ships every
//! part lengua needs, the solvers included, so nothing else is installed.
//!
//! A folder it would replace must be lengua's (an earlier install), or not there yet, and the
//! archive goes neither into a folder inside itself nor over the folder it runs from. A browser
//! marks what it downloads, and the archive isn't signed, so whether to trust it is the user's:
//! the install says how to clear the mark and clears nothing itself.

use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::data_dir::DataDir;

/// What a folder with no TARGET file says.
const NO_TARGET: &str =
    "This folder has no TARGET file: install lengua from its release archive, unpacked.";

/// What an install says when the system names no folder of the user's to install into.
#[cfg(windows)]
const NOWHERE: &str =
    "can't find where to install lengua: set LOCALAPPDATA, or give --prefix <folder>";

/// What an install says when the system names no folder of the user's to install into.
#[cfg(target_os = "macos")]
const NOWHERE: &str = "can't find where to install lengua: set HOME, or give --prefix <folder>";

/// What an install says when the system names no folder of the user's to install into.
#[cfg(not(any(windows, target_os = "macos")))]
const NOWHERE: &str =
    "can't find where to install lengua: set HOME or XDG_DATA_HOME, or give --prefix <folder>";

/// What `--link` on Windows says.
const LINK_ON_WINDOWS: &str =
    "--link is for Linux and macOS: on Windows, run lengua-mcp by the path the install prints";

/// What a macOS install says of the mark a browser puts on what it downloads.
const MARKED_MACOS: &str = "macOS marks these files as downloaded, and won't start the unsigned \
                            server so marked.\nIf you trust this archive, clear the mark with:";

/// What a Windows install says of the mark a browser puts on what it downloads.
const MARKED_WINDOWS: &str = "Windows marks these files as downloaded, and may stop the unsigned \
                              server so marked.\nIf you trust this archive, clear the mark with:";

/// What an install that added lengua to Claude Code says.
const REGISTERED: &str = "Claude Code has lengua now: start a new session, or run \
                          /reload-plugins in one.";

/// What every install says last, before doctor.
const DESCRIBE: &str = "Then describe a program: \"Use lengua to write a program that ...\".";

/// The oldest glibc lengua's Linux programs run with: Ubuntu 24.04's, Debian 13's and Fedora 40's.
const GLIBC: (u32, u32) = (2, 39);

/// What `lengua-mcp install` is asked to do.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Options {
    /// Only check this computer, and install nothing.
    pub check: bool,
    /// Also link lengua-mcp into ~/.local/bin, to run programs from a shell (Linux and macOS).
    pub link: bool,
    /// The marketplace folder to install into, the plugin in its lengua folder, in place of
    /// lengua's own.
    pub prefix: Option<PathBuf>,
}

impl Options {
    /// `install`'s own words: `--check`, `--link` and `--prefix <folder>` (or `--prefix=<folder>`;
    /// an empty folder is lengua's own).
    ///
    /// # Errors
    ///
    /// Says which word it doesn't take.
    pub fn parse(mut args: impl Iterator<Item = String>) -> Result<Self, String> {
        let mut options = Self::default();
        while let Some(arg) = args.next() {
            let folder = match arg.as_str() {
                "--check" => {
                    options.check = true;
                    continue;
                }
                "--link" if cfg!(windows) => return Err(LINK_ON_WINDOWS.to_owned()),
                "--link" => {
                    options.link = true;
                    continue;
                }
                "--prefix" => args.next().ok_or("--prefix needs a folder")?,
                other => match other.strip_prefix("--prefix=") {
                    Some(folder) => folder.to_owned(),
                    None => return Err(format!("install takes no {other:?}")),
                },
            };
            options.prefix = (!folder.is_empty()).then(|| PathBuf::from(folder));
        }
        Ok(options)
    }
}

/// A target's name as people say it.
#[must_use]
pub fn described(name: &str) -> &str {
    match name {
        "linux-x86_64" => "Linux on x86-64",
        "linux-aarch64" => "Linux on ARM64",
        "macos-aarch64" => "macOS on ARM64 (Apple silicon)",
        "macos-x86_64" => "macOS on x86-64 (an Intel Mac)",
        "windows-x86_64" => "Windows on x86-64",
        "windows-aarch64" => "Windows on ARM64",
        other => other,
    }
}

/// A CPU's name as a target names it: `aarch64` for ARM64 and `x86_64` for x86-64, whatever the
/// system calls them.
#[must_use]
pub fn cpu_named(name: &str) -> String {
    match name.trim() {
        "arm64" | "ARM64" | "aarch64" => "aarch64".to_owned(),
        "AMD64" | "amd64" | "x86_64" | "x64" => "x86_64".to_owned(),
        other => other.to_owned(),
    }
}

/// Why an archive for `target` won't install on `computer`, a computer of another system or
/// CPU; `None` when they are the same.
#[must_use]
pub fn refusal(target: &str, computer: &str) -> Option<String> {
    if target == computer {
        return None;
    }
    let (archive, this) = (described(target), described(computer));
    if crate::gate::TARGETS.contains(&computer) {
        Some(format!(
            "This archive is lengua for {archive}, and this computer is {this}: use the archive \
             for {computer}."
        ))
    } else {
        Some(format!(
            "This archive is lengua for {archive}, and this computer is {this}, which lengua \
             isn't built for: it is built for linux-x86_64, linux-aarch64, macos-aarch64, \
             windows-x86_64 and windows-aarch64."
        ))
    }
}

/// The glibc version that `ldd --version`'s first line, `line`, ends with, as `2.39`.
#[must_use]
pub fn glibc_version(line: &str) -> Option<(u32, u32)> {
    let line = line.trim_end();
    let before_minor = line.trim_end_matches(|c: char| c.is_ascii_digit());
    let minor = line[before_minor.len()..].parse().ok()?;
    let major_and_dot = before_minor.strip_suffix('.')?;
    let before_major = major_and_dot.trim_end_matches(|c: char| c.is_ascii_digit());
    let major = major_and_dot[before_major.len()..].parse().ok()?;
    Some((major, minor))
}

/// The marketplace.json that names lengua's marketplace and its one plugin, at `version`: with
/// a newline at its end on Linux and macOS, and none on Windows, as each system's install
/// script wrote it.
#[must_use]
pub fn marketplace(version: &str) -> String {
    let version = serde_json::to_string(version).unwrap_or_else(|_| "\"\"".to_owned());
    let text = format!(
        "{{\n  \"name\": \"lengua\",\n  \"owner\": {{\n    \"name\": \"lengua\"\n  }},\n  \
         \"plugins\": [\n    {{\n      \"name\": \"lengua\",\n      \
         \"source\": \"./lengua\",\n      \"version\": {version}\n    }}\n  ]\n}}"
    );
    if cfg!(windows) { text } else { text + "\n" }
}

/// Whether the install may replace what is in the marketplace folder `market`: its lengua
/// folder, and the one it stages in beside it, must be an earlier install, which holds its
/// TARGET file, or not there yet, and its marketplace.json must name lengua.
///
/// # Errors
///
/// Says what isn't lengua's, and what to do.
pub fn replaceable(market: &Path) -> Result<(), String> {
    for folder in [market.join("lengua"), market.join("lengua.new")] {
        if folder.exists() && !folder.join("TARGET").is_file() {
            let shown = folder.display();
            return Err(format!(
                "{shown} is there and isn't lengua (it has no TARGET file), so the install won't \
                 replace it: give --prefix a folder of lengua's own, or move {shown} away."
            ));
        }
    }
    let manifest = market.join(".claude-plugin").join("marketplace.json");
    if manifest.exists() {
        let named = std::fs::read_to_string(&manifest)
            .ok()
            .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
            .is_some_and(|read| read["name"] == "lengua");
        if !named {
            let shown = manifest.display();
            return Err(format!(
                "{shown} is another marketplace's, so the install won't replace it: give \
                 --prefix a folder of lengua's own."
            ));
        }
    }
    Ok(())
}

/// `path`, an absolute path, as the system finds it: the folders of it that are there with their
/// links followed (on Windows, in their own case), and those that aren't yet as named. The
/// install compares folders by it.
fn resolved(path: &Path) -> PathBuf {
    let mut missing = Vec::new();
    let mut existing = path;
    loop {
        if let Ok(mut found) = std::fs::canonicalize(existing) {
            for name in missing.iter().rev() {
                found.push(name);
            }
            return found;
        }
        match (existing.parent(), existing.file_name()) {
            (Some(parent), Some(name)) => {
                missing.push(name);
                existing = parent;
            }
            _ => return path.to_path_buf(),
        }
    }
}

/// Whether the unpacked archive `here` may be copied into the marketplace folder `market`,
/// compared as [`resolved`] gives them: not into a folder inside the archive, where the copy would
/// copy itself again and again, and not in place of the folder it is copied from, as the install
/// of an installed lengua's own lengua-mcp would be, which Windows won't let it remove as it
/// runs. When not, says why, and what to do.
fn apart(here: &Path, market: &Path) -> Result<(), String> {
    let (archive, folder) = (resolved(here), resolved(market));
    let staging = folder.join("lengua.new");
    if staging.starts_with(&archive) {
        return Err(format!(
            "{} is this archive's own folder or inside it, so the install would copy the archive \
             into itself: give --prefix a folder outside {}.",
            market.display(),
            here.display()
        ));
    }
    if archive.starts_with(folder.join("lengua")) || archive.starts_with(&staging) {
        return Err(format!(
            "This lengua-mcp runs from {}, which the install would replace: run the install from \
             the unpacked archive, in a folder of its own.",
            here.display()
        ));
    }
    Ok(())
}

/// Copies the unpacked archive `here` into the marketplace folder `market` as its lengua folder,
/// in place of an earlier install, without the archive's own INSTALL.md, and writes the
/// marketplace.json that names it at `version`: gives the plugin's folder.
///
/// # Errors
///
/// Says what couldn't be copied or written.
pub fn place(here: &Path, market: &Path, version: &str) -> Result<PathBuf, String> {
    let failed = |path: &Path, error: std::io::Error| format!("{}: {error}", path.display());
    let dest = market.join("lengua");
    let staging = market.join("lengua.new");
    let manifests = market.join(".claude-plugin");
    std::fs::create_dir_all(&manifests).map_err(|error| failed(&manifests, error))?;
    remove(&staging)?;
    copy_folder(here, &staging)?;
    remove(&staging.join("INSTALL.md"))?;
    remove(&dest)?;
    std::fs::rename(&staging, &dest).map_err(|error| failed(&dest, error))?;
    let manifest = manifests.join("marketplace.json");
    std::fs::write(&manifest, marketplace(version)).map_err(|error| failed(&manifest, error))?;
    Ok(dest)
}

/// Removes `path`, a folder with all it holds or a file: nothing there is no error.
fn remove(path: &Path) -> Result<(), String> {
    let removed = if path.is_dir() {
        std::fs::remove_dir_all(path)
    } else {
        std::fs::remove_file(path)
    };
    match removed {
        Err(error) if error.kind() != std::io::ErrorKind::NotFound => {
            Err(format!("{}: {error}", path.display()))
        }
        _ => Ok(()),
    }
}

/// Copies the folder `from` to `to`, everything in it, each file with its permissions.
fn copy_folder(from: &Path, to: &Path) -> Result<(), String> {
    let failed = |path: &Path, error: std::io::Error| format!("{}: {error}", path.display());
    std::fs::create_dir_all(to).map_err(|error| failed(to, error))?;
    let entries = std::fs::read_dir(from).map_err(|error| failed(from, error))?;
    for entry in entries {
        let entry = entry.map_err(|error| failed(from, error))?;
        let source = entry.path();
        let target = to.join(entry.file_name());
        if source.is_dir() {
            copy_folder(&source, &target)?;
        } else {
            std::fs::copy(&source, &target).map_err(|error| failed(&source, error))?;
        }
    }
    Ok(())
}

/// What `program` printed, when it ran and succeeded.
fn printed(program: &str, args: &[&str]) -> Option<String> {
    let output = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).into_owned())
}

/// This computer's system and CPU, as a target names them: the system this lengua-mcp runs on,
/// and the hardware's own CPU, as `uname -m` says it on Linux, `hw.optional.arm64` on macOS
/// (a shell under Rosetta says x86_64), and the system's own environment on Windows (an x64
/// program emulated on ARM64 is told AMD64); where the system's program doesn't run, the CPU
/// this lengua-mcp is built for.
#[must_use]
pub fn this_computer() -> String {
    let cpu = if cfg!(target_os = "macos") {
        // sysctl by its path, which a shell's PATH may lack; where it doesn't run, the CPU this
        // lengua-mcp is built for, which an arm64 lengua-mcp, running, shows is Apple silicon.
        let said = printed("/usr/sbin/sysctl", &["-n", "hw.optional.arm64"]);
        let arm = said.map(|said| said.trim() == "1");
        arm.map(|arm| if arm { "aarch64" } else { "x86_64" }.to_owned())
    } else if cfg!(windows) {
        let key = r"HKLM\SYSTEM\CurrentControlSet\Control\Session Manager\Environment";
        printed("reg", &["query", key, "/v", "PROCESSOR_ARCHITECTURE"]).and_then(|said| {
            said.lines()
                .find_map(|line| Some(line.split_once("REG_SZ")?.1.trim().to_owned()))
        })
    } else {
        printed("uname", &["-m"])
    };
    let cpu = cpu.unwrap_or_else(|| std::env::consts::ARCH.to_owned());
    format!("{}-{}", std::env::consts::OS, cpu_named(&cpu))
}

/// Linux's server and solvers are built against glibc 2.39: Ubuntu 24.04, Debian 13, Fedora 40
/// and later.
fn glibc() -> Result<(), String> {
    let said = Command::new("ldd")
        .arg("--version")
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .map(|output| String::from_utf8_lossy(&output.stdout).into_owned())
        .unwrap_or_default();
    let (major, minor) = GLIBC;
    match glibc_version(said.lines().next().unwrap_or_default()) {
        None => Err(format!(
            "lengua needs glibc {major}.{minor} or later, and this computer's C library isn't \
             glibc."
        )),
        Some((found_major, found_minor)) if (found_major, found_minor) < GLIBC => Err(format!(
            "lengua needs glibc {major}.{minor} or later, and this computer has \
             {found_major}.{found_minor}."
        )),
        Some(_) => Ok(()),
    }
}

/// `path` quoted as this system's shell takes it: in single quotes, or on Windows in double
/// quotes.
fn quoted(path: &Path) -> String {
    if cfg!(windows) {
        format!("\"{}\"", path.display())
    } else {
        format!("'{}'", path.display())
    }
}

/// On macOS, says how to clear the mark a browser puts on what it downloads, when `folder` has
/// it: macOS won't start the unsigned server so marked.
fn quarantine_note(folder: &Path) {
    if !cfg!(target_os = "macos") {
        return;
    }
    let at = folder.to_string_lossy();
    let marked =
        printed("xattr", &["-r", &at]).is_some_and(|said| said.contains("com.apple.quarantine"));
    if marked {
        println!("{MARKED_MACOS}");
        println!("  xattr -dr com.apple.quarantine {}", quoted(folder));
        println!();
    }
}

/// Whether a file at any depth of `folder` holds Windows's mark of a download, its
/// `Zone.Identifier` stream.
fn zone_marked(folder: &Path) -> bool {
    std::fs::read_dir(folder)
        .into_iter()
        .flatten()
        .flatten()
        .any(|entry| {
            let path = entry.path();
            if path.is_dir() {
                return zone_marked(&path);
            }
            let mut stream = OsString::from(path);
            stream.push(":Zone.Identifier");
            Path::new(&stream).exists()
        })
}

/// On Windows, says how to clear the mark a browser puts on what it downloads, when `folder`
/// has it: Windows may stop the unsigned server so marked.
fn zone_note(folder: &Path) {
    if cfg!(windows) && zone_marked(folder) {
        println!("{MARKED_WINDOWS}");
        println!(
            "  Get-ChildItem -Recurse -File -LiteralPath '{}' | Unblock-File",
            folder.display()
        );
        println!();
    }
}

/// Runs the claude command with `args`, quietly: whether it ran and succeeded. On Windows the
/// command may be the npm package's claude.cmd.
fn claude(args: &[&OsStr]) -> bool {
    let names: &[&str] = if cfg!(windows) {
        &["claude", "claude.cmd"]
    } else {
        &["claude"]
    };
    for name in names {
        let ran = Command::new(name)
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        match ran {
            Ok(status) => return status.success(),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return false,
        }
    }
    false
}

/// [`claude`] with the words of `line`.
fn claude_says(line: &str) -> bool {
    let words: Vec<&OsStr> = line.split(' ').map(OsStr::new).collect();
    claude(&words)
}

/// Claude Code installs a plugin from a marketplace it knows: lengua's, `market`, added once and
/// updated after, and then the plugin from it. Whether Claude Code has lengua now.
fn register(market: &Path) -> bool {
    let add = [
        OsStr::new("plugin"),
        OsStr::new("marketplace"),
        OsStr::new("add"),
        market.as_os_str(),
    ];
    let known = claude(&add) || claude_says("plugin marketplace update lengua");
    known
        && (claude_says("plugin update lengua@lengua")
            || claude_says("plugin install lengua@lengua"))
}

/// The installed server's doctor, with lengua's data folder `data_dir` when one was given: its
/// exit code.
fn installed_doctor(server: &Path, data_dir: Option<&DataDir>) -> u8 {
    let mut doctor = Command::new(server);
    if let Some(data_dir) = data_dir {
        doctor.arg("--data-dir").arg(data_dir.path());
    }
    match doctor.arg("doctor").status() {
        Ok(status) => status
            .code()
            .and_then(|code| u8::try_from(code).ok())
            .unwrap_or(1),
        Err(error) => {
            eprintln!("{} didn't start: {error}", server.display());
            1
        }
    }
}

/// This lengua-mcp's doctor, with lengua's data folder `data_dir`, or the one the environment
/// names, as `lengua-mcp doctor` finds it: its exit code.
fn own_doctor(data_dir: Option<&DataDir>) -> u8 {
    let found = data_dir.cloned().or_else(DataDir::from_env);
    let report = crate::doctor::doctor(found.as_ref());
    for line in report.lines() {
        println!("{line}");
    }
    u8::from(report.problems() != 0)
}

/// `lengua-mcp install`, run from the unpacked archive's server folder: what it says, and its
/// exit code, doctor's when it got that far, else 1.
#[must_use]
pub fn install(options: &Options, data_dir: Option<&DataDir>) -> u8 {
    match installed(options, data_dir) {
        Ok(code) => code,
        Err(message) => {
            eprintln!("{message}");
            1
        }
    }
}

/// [`install`]'s work.
fn installed(options: &Options, data_dir: Option<&DataDir>) -> Result<u8, String> {
    let program = std::env::current_exe()
        .map_err(|error| format!("lengua-mcp can't find its own file: {error}"))?;
    #[cfg(unix)]
    let program = std::fs::canonicalize(&program).unwrap_or(program);
    let here = program
        .parent()
        .and_then(Path::parent)
        .ok_or(NO_TARGET)?
        .to_path_buf();
    let target = std::fs::read_to_string(here.join("TARGET")).unwrap_or_default();
    let target = target.trim();
    if target.is_empty() {
        return Err(NO_TARGET.to_owned());
    }
    let version = std::fs::read_to_string(here.join("plugin.json"))
        .ok()
        .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
        .and_then(|read| read["version"].as_str().map(str::to_owned))
        .unwrap_or_default();
    let computer = this_computer();
    if let Some(refused) = refusal(target, &computer) {
        return Err(refused);
    }
    if cfg!(target_os = "linux") {
        glibc()?;
    }
    if options.check {
        quarantine_note(&here);
        let this = described(&computer);
        println!("This computer is {this}, which this archive is for.");
        println!();
        return Ok(own_doctor(data_dir));
    }

    // lengua's marketplace folder: the plugin in lengua, and .claude-plugin's marketplace.json,
    // which names it for Claude Code. A folder given is taken from the current one and named in
    // full from then on, as the link, the claude command and what the install prints need it.
    let own = DataDir::from_env().map(|data| data.path().join("plugins"));
    let market = match &options.prefix {
        Some(prefix) => absolute(prefix)?,
        None => own.ok_or(NOWHERE)?,
    };
    apart(&here, &market)?;
    replaceable(&market)?;
    let dest = place(&here, &market, &version)?;
    quarantine_note(&dest);
    zone_note(&dest);
    let server = dest
        .join("server")
        .join(format!("lengua-mcp{}", std::env::consts::EXE_SUFFIX));
    if options.link {
        link(&server)?;
    }

    let (said, shown) = (described(target), dest.display());
    println!("Installed lengua {version} for {said} in {shown}.");
    println!();
    if register(&market) {
        println!("{REGISTERED}");
    } else {
        let (market, folder) = (quoted(&market), quoted(&dest));
        println!("Add it to Claude Code:");
        println!("    claude plugin marketplace add {market}");
        println!("    claude plugin install lengua@lengua");
        println!("or load it for one session: claude --plugin-dir {folder}");
    }
    println!("Other agent harnesses load the plugin folder {shown}.");
    println!("{DESCRIBE}");
    println!();
    println!("Run a program it built from a shell, script or service:");
    let program = quoted(&server);
    if options.link {
        println!("    lengua-mcp run <program folder> -- <arguments>");
    } else if cfg!(windows) {
        println!("    & {program} run <program folder> -- <arguments>");
    } else {
        println!("    {program} run <program folder> -- <arguments>");
    }
    println!();
    Ok(installed_doctor(&server, data_dir))
}

/// `folder`, named from the current folder, named in full.
fn absolute(folder: &Path) -> Result<PathBuf, String> {
    std::path::absolute(folder).map_err(|error| format!("{}: {error}", folder.display()))
}

/// Links lengua-mcp, `server`, into ~/.local/bin, as `ln -sf` does.
#[cfg(unix)]
fn link(server: &Path) -> Result<(), String> {
    let home =
        std::env::var_os("HOME").ok_or("--link needs HOME, the folder ~/.local/bin is in")?;
    let bin = PathBuf::from(home).join(".local").join("bin");
    std::fs::create_dir_all(&bin).map_err(|error| format!("{}: {error}", bin.display()))?;
    let link = bin.join("lengua-mcp");
    let _ = std::fs::remove_file(&link);
    std::os::unix::fs::symlink(server, &link)
        .map_err(|error| format!("{}: {error}", link.display()))
}

/// Windows takes no link, which [`Options::parse`] refuses.
#[cfg(not(unix))]
fn link(server: &Path) -> Result<(), String> {
    let shown = server.display();
    Err(format!("--link is for Linux and macOS: run {shown} by its path"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `list` as a command line's words.
    fn words(list: &[&str]) -> impl Iterator<Item = String> {
        list.iter().map(|word| (*word).to_owned())
    }

    /// install takes --check, --link and --prefix, the folder after it or after `=`, and no
    /// other word; an empty folder is lengua's own.
    #[test]
    fn install_takes_its_own_words() {
        let parsed = Options::parse(words(&["--check", "--prefix", "/x"])).expect("parsed");
        assert!(parsed.check);
        assert_eq!(parsed.prefix, Some(PathBuf::from("/x")));
        let joined = Options::parse(words(&["--prefix=/y"])).expect("parsed");
        assert_eq!(joined.prefix, Some(PathBuf::from("/y")));
        let empty = Options::parse(words(&["--prefix="])).expect("parsed");
        assert_eq!(empty, Options::default());
        assert!(Options::parse(words(&["--prefix"])).is_err());
        assert!(Options::parse(words(&["--force"])).is_err());
        assert_eq!(Options::parse(words(&["--link"])).is_ok(), !cfg!(windows));
    }

    /// An archive for another system or CPU is refused, saying which archive to use or that
    /// lengua isn't built for this computer; the CPU's names are a target's.
    #[test]
    fn another_system_or_cpu_is_refused() {
        assert_eq!(refusal("linux-x86_64", "linux-x86_64"), None);
        let other = refusal("linux-aarch64", "linux-x86_64").expect("refused");
        assert_eq!(
            other,
            "This archive is lengua for Linux on ARM64, and this computer is Linux on x86-64: \
             use the archive for linux-x86_64."
        );
        let intel = refusal("macos-aarch64", "macos-x86_64").expect("refused");
        assert!(intel.contains("Intel Mac), which lengua isn't"), "{intel}");
        assert_eq!(cpu_named("arm64"), "aarch64");
        assert_eq!(cpu_named("AMD64\n"), "x86_64");
        assert_eq!(cpu_named("riscv64"), "riscv64");
    }

    /// glibc's version is the end of `ldd --version`'s first line; another C library has none.
    #[test]
    fn glibc_s_version_ends_ldd_s_first_line() {
        assert_eq!(
            glibc_version("ldd (Ubuntu GLIBC 2.39-0ubuntu8.6) 2.39"),
            Some((2, 39))
        );
        assert_eq!(glibc_version("ldd (GNU libc) 2.40\n"), Some((2, 40)));
        assert_eq!(glibc_version("musl libc (x86_64)"), None);
        assert!((2, 35) < GLIBC && (2, 40) > GLIBC);
    }

    /// A folder the install would replace must be an earlier install, and the marketplace.json
    /// lengua's; then the archive's folder goes in as lengua, without INSTALL.md, and an earlier
    /// install is replaced.
    #[test]
    fn only_lengua_s_folders_are_replaced() {
        let root = tempfile::tempdir().expect("a folder");
        let market = root.path().join("market");
        std::fs::create_dir_all(market.join("lengua")).expect("someone's folder");
        std::fs::write(market.join("lengua").join("notes.txt"), "mine").expect("their notes");
        let refused = replaceable(&market).expect_err("not lengua's");
        assert!(refused.contains("it has no TARGET file"), "{refused}");

        let here = root.path().join("lengua-0.2.0-linux-x86_64");
        std::fs::create_dir_all(here.join("server")).expect("the archive's folders");
        for (file, text) in [("TARGET", "linux-x86_64\n"), ("INSTALL.md", "# lengua\n")] {
            std::fs::write(here.join(file), text).expect("an archive's file");
        }
        std::fs::write(here.join("server").join("lengua-mcp"), "a server")
            .expect("the server");
        let ours = root.path().join("ours");
        assert!(replaceable(&ours).is_ok());
        for _ in 0..2 {
            let dest = place(&here, &ours, "0.2.0").expect("installed");
            assert!(dest.join("TARGET").is_file());
            assert!(dest.join("server").join("lengua-mcp").is_file());
            assert!(!dest.join("INSTALL.md").exists());
            assert!(replaceable(&ours).is_ok());
        }
        let manifest = ours.join(".claude-plugin").join("marketplace.json");
        let text = std::fs::read_to_string(&manifest).expect("marketplace.json");
        assert_eq!(text, marketplace("0.2.0"));
        let read: serde_json::Value = serde_json::from_str(&text).expect("JSON");
        assert_eq!(read["plugins"][0]["version"], "0.2.0");
        std::fs::write(&manifest, "{\"name\": \"someone\"}").expect("another marketplace");
        let refused = replaceable(&ours).expect_err("another marketplace's");
        assert!(refused.contains("is another marketplace's"), "{refused}");
    }

    /// The archive is copied into no folder of its own, there or not there yet or reached by a
    /// link, and not over the folder it runs from, as an installed lengua's own install would
    /// copy it; a marketplace folder beside the archive, or one that holds it, takes it.
    #[test]
    fn the_archive_is_copied_neither_into_itself_nor_over_itself() {
        let root = tempfile::tempdir().expect("a folder");
        let here = root.path().join("lengua-0.2.0-linux-x86_64");
        std::fs::create_dir_all(&here).expect("the archive's folder");
        for market in [here.clone(), here.join("new").join("market")] {
            let refused = apart(&here, &market).expect_err("inside the archive");
            assert!(refused.contains("give --prefix a folder outside"), "{refused}");
        }
        #[cfg(unix)]
        {
            let link = root.path().join("link");
            std::os::unix::fs::symlink(&here, &link).expect("a link to the archive");
            assert!(apart(&here, &link).is_err());
        }
        let market = root.path().join("market");
        assert!(apart(&here, &market).is_ok());
        assert!(apart(&here, root.path()).is_ok());
        let installed = market.join("lengua");
        let refused = apart(&installed, &market).expect_err("an installed lengua's own");
        assert!(refused.contains("which the install would replace"), "{refused}");
        assert!(apart(&market.join("lengua.new").join("server"), &market).is_err());
    }
}
