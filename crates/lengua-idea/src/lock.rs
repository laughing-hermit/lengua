use std::fmt::Write as _;
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use crate::{Trust, generate, sha256};

pub const LOCK: &str = "lengua.lock";
pub const HOME: &str = "LENGUA_HOME";
pub const KEY_BYTES: usize = 32;
const HEADER: &str = "lengua-lock 1";
const SIGNATURE: &str = "hmac-sha256 ";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Kind {
    #[default]
    Program,
    Library,
    Test,
}

impl Kind {
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Program => "program",
            Self::Library => "library",
            Self::Test => "test",
        }
    }

    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        [Self::Program, Self::Library, Self::Test]
            .into_iter()
            .find(|k| k.name() == text)
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct Key {
    bytes: [u8; KEY_BYTES],
}

#[derive(Debug)]
pub struct Sealed {
    bytes: Vec<u8>,
}

impl Sealed {
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

#[derive(Debug)]
pub struct Verified {
    pub kind: Kind,
    pub label: String,
    pub files: Vec<String>,
    sealed: Option<Sealed>,
    trusted: Vec<Sealed>,
}

impl Verified {
    #[must_use]
    pub fn sealed(&self) -> Option<&Sealed> {
        self.sealed.as_ref()
    }

    #[must_use]
    pub fn trusted(&self) -> &[Sealed] {
        &self.trusted
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Facts {
    pub kind: Kind,
    pub toolchain: String,
    pub trust: Trust,
    pub files: Vec<String>,
    pub libraries: Vec<String>,
    pub trusted: Vec<String>,
    pub component: Option<String>,
    pub sealed: Option<String>,
    pub sealed_trusted: Vec<String>,
}

impl Facts {
    #[must_use]
    pub fn test(sealed: &str, trusted: &[String]) -> Self {
        Self {
            kind: Kind::Test,
            sealed: Some(sealed.to_string()),
            sealed_trusted: trusted.to_vec(),
            ..Self::default()
        }
    }
}

fn env_path(name: &str) -> Option<PathBuf> {
    std::env::var_os(name)
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
}

pub fn data_folder() -> Result<PathBuf, String> {
    if let Some(home) = env_path(HOME) {
        return Ok(home);
    }
    let found = if cfg!(windows) {
        env_path("APPDATA").map(|p| p.join("lengua"))
    } else if cfg!(target_os = "macos") {
        env_path("HOME").map(|p| p.join("Library").join("Application Support").join("lengua"))
    } else {
        env_path("XDG_DATA_HOME")
            .filter(|p| p.is_absolute())
            .map(|p| p.join("lengua"))
            .or_else(|| env_path("HOME").map(|p| p.join(".local").join("share").join("lengua")))
    };
    found.ok_or_else(|| {
        format!("lengua could not find its data folder; set `{HOME}` to a folder for lengua's data")
    })
}

#[must_use]
pub fn key_file(folder: &Path) -> PathBuf {
    folder.join("key")
}

fn parse_key(text: &str) -> Option<Key> {
    let decoded = sha256::from_hex(text.trim())?;
    let bytes: [u8; KEY_BYTES] = decoded.try_into().ok()?;
    Some(Key { bytes })
}

pub fn read_key(folder: &Path) -> Result<Key, String> {
    let path = key_file(folder);
    let text = fs::read_to_string(&path).map_err(|e| {
        format!(
            "lengua could not read the user's key `{}`: {e}; call check, which makes it",
            path.display()
        )
    })?;
    parse_key(&text).ok_or_else(|| {
        format!(
            "the user's key `{}` is not 64 hexadecimal digits",
            path.display()
        )
    })
}

#[cfg(unix)]
fn write_private(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)?;
    file.write_all(bytes)?;
    file.sync_all()
}

#[cfg(not(unix))]
fn write_private(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?;
    file.write_all(bytes)?;
    file.sync_all()
}

#[cfg(unix)]
fn make_private_folder(folder: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::DirBuilderExt;
    fs::DirBuilder::new().mode(0o700).create(folder)
}

#[cfg(not(unix))]
fn make_private_folder(folder: &Path) -> std::io::Result<()> {
    fs::create_dir(folder)
}

fn make_folder(folder: &Path) -> std::io::Result<()> {
    if folder.is_dir() {
        return Ok(());
    }
    if let Some(parent) = folder.parent() {
        fs::create_dir_all(parent)?;
    }
    match make_private_folder(folder) {
        Err(e) if e.kind() == ErrorKind::AlreadyExists => Ok(()),
        other => other,
    }
}

pub fn create_key(folder: &Path, bytes: [u8; KEY_BYTES]) -> Result<Key, String> {
    let path = key_file(folder);
    if path.is_file() {
        return read_key(folder);
    }
    let fail = |e: std::io::Error| {
        format!(
            "lengua could not make the user's key `{}`: {e}; set `{HOME}` to a folder that lengua can write",
            path.display()
        )
    };
    make_folder(folder).map_err(fail)?;
    let temporary = folder.join(format!("key.{}.new", std::process::id()));
    let _ = fs::remove_file(&temporary);
    let text = format!("{}\n", sha256::to_hex(&bytes));
    write_private(&temporary, text.as_bytes()).map_err(fail)?;
    match fs::hard_link(&temporary, &path) {
        Ok(()) => {
            let _ = fs::remove_file(&temporary);
            Ok(Key { bytes })
        }
        Err(e) if e.kind() == ErrorKind::AlreadyExists => {
            let _ = fs::remove_file(&temporary);
            read_key(folder)
        }
        Err(_) => {
            fs::rename(&temporary, &path).map_err(fail)?;
            read_key(folder)
        }
    }
}

pub fn user_key() -> Result<Key, String> {
    read_key(&data_folder()?)
}

pub fn user_key_or_create(
    fill: impl FnOnce(&mut [u8; KEY_BYTES]) -> Result<(), String>,
) -> Result<Key, String> {
    let folder = data_folder()?;
    if key_file(&folder).is_file() {
        return read_key(&folder);
    }
    let mut bytes = [0u8; KEY_BYTES];
    fill(&mut bytes)?;
    create_key(&folder, bytes)
}

fn ideas(folder: &Path) -> Vec<String> {
    let mut out = Vec::new();
    if folder.join("idea.md").is_file() {
        out.push("idea.md".to_string());
    }
    for place in ["ideas", "ideas/ui"] {
        let Ok(entries) = fs::read_dir(folder.join(place)) else {
            continue;
        };
        let mut names: Vec<String> = entries
            .flatten()
            .filter(|e| e.path().is_file())
            .map(|e| e.file_name().to_string_lossy().to_string())
            .filter(|n| {
                Path::new(n)
                    .extension()
                    .is_some_and(|x| x.eq_ignore_ascii_case("md"))
            })
            .map(|n| format!("{place}/{n}"))
            .collect();
        names.sort();
        out.extend(names);
    }
    out
}

fn sources(folder: &Path) -> Vec<String> {
    let mut found = Vec::new();
    generate::collect(folder, "src", &mut found);
    let mut names: Vec<String> = found.into_iter().map(|(path, _)| path).collect();
    names.sort();
    names
}

#[must_use]
pub fn program_files(folder: &Path) -> Vec<String> {
    let mut out = ideas(folder);
    out.push("index.md".to_string());
    out.push("log.md".to_string());
    out.extend(sources(folder));
    out.push("Cargo.toml".to_string());
    out.push("wit/world.wit".to_string());
    if folder.join("Cargo.lock").is_file() {
        out.push("Cargo.lock".to_string());
    }
    out
}

fn one_line(text: &str) -> String {
    text.replace(['\r', '\n'], " ").trim().to_string()
}

pub fn text(folder: &Path, facts: &Facts, key: &Key) -> Result<String, String> {
    let mut out = format!(
        "{HEADER}\nkind {}\nlengua {}\n",
        facts.kind.name(),
        env!("CARGO_PKG_VERSION")
    );
    if facts.kind != Kind::Test {
        let score = match facts.trust.score {
            Some((caught, counted)) => format!("{caught} of {counted}"),
            None => "none".to_string(),
        };
        let _ = write!(
            out,
            "toolchain {}\nlabel {}\nscore {score}\n",
            one_line(&facts.toolchain),
            facts.trust.label.name()
        );
    }
    let mut entries: Vec<(&str, &str)> = facts.files.iter().map(|f| ("file", f.as_str())).collect();
    entries.extend(facts.libraries.iter().map(|l| ("library", l.as_str())));
    entries.extend(facts.trusted.iter().map(|t| ("trusted", t.as_str())));
    entries.extend(facts.component.as_deref().map(|c| ("component", c)));
    entries.extend(facts.sealed.as_deref().map(|s| ("sealed", s)));
    entries.extend(
        facts
            .sealed_trusted
            .iter()
            .map(|s| ("sealed-trusted", s.as_str())),
    );
    for (tag, path) in entries {
        if path.contains(['\r', '\n']) || path.is_empty() {
            return Err(format!(
                "lengua can't lock the file `{path}`: its name has a line end"
            ));
        }
        let bytes = fs::read(folder.join(path))
            .map_err(|e| format!("lengua could not read `{path}` to lock it: {e}"))?;
        let _ = writeln!(out, "{tag} {} {path}", sha256::hex(&bytes));
    }
    let signature = sha256::to_hex(&sha256::hmac(&key.bytes, out.as_bytes()));
    let _ = writeln!(out, "{SIGNATURE}{signature}");
    Ok(out)
}

pub fn write(folder: &Path, lock: &Path, facts: &Facts, key: &Key) -> Result<(), String> {
    let text = text(folder, facts, key)?;
    let fail = |e: std::io::Error| format!("lengua could not write `{}`: {e}", lock.display());
    if let Some(parent) = lock.parent() {
        fs::create_dir_all(parent).map_err(fail)?;
    }
    let temporary = lock.with_extension("lock-new");
    fs::write(&temporary, text).map_err(fail)?;
    fs::rename(&temporary, lock).map_err(fail)
}

fn shown(folder: &Path, lock: &Path) -> String {
    lock.strip_prefix(folder).map_or_else(
        |_| lock.display().to_string(),
        |p| p.to_string_lossy().replace('\\', "/"),
    )
}

fn signed<'a>(text: &'a str, key: &Key) -> Option<&'a str> {
    let body = text.strip_suffix('\n')?;
    let at = body.rfind('\n').map_or(0, |i| i + 1);
    let (signed, last) = text.split_at(at);
    let given = sha256::from_hex(last.trim_end().strip_prefix(SIGNATURE)?)?;
    let wanted = sha256::hmac(&key.bytes, signed.as_bytes());
    sha256::same(&given, &wanted).then_some(signed)
}

fn check_entry(
    folder: &Path,
    (tag, rest): (&str, &str),
    key: &Key,
    changed: &mut Vec<String>,
) -> Option<(String, Option<Sealed>)> {
    let (hash, path) = rest.split_once(' ')?;
    let keep = tag == "sealed" || tag == "sealed-trusted";
    let sealed = match fs::read(folder.join(path)) {
        Ok(bytes) if sha256::hex(&bytes) == hash => keep.then_some(Sealed { bytes }),
        Ok(_) => {
            changed.push(format!("`{path}` changed"));
            None
        }
        Err(_) => {
            changed.push(format!("`{path}` is not there"));
            None
        }
    };
    if tag == "library" && changed.is_empty() {
        let lock = folder.join(path);
        let place = lock.parent().unwrap_or(folder);
        if let Err(message) = verify(place, &lock, Kind::Library, key) {
            changed.push(format!(
                "the library of `{path}` is not as its lock signed it ({message})"
            ));
        }
    }
    Some((path.to_string(), sealed))
}

struct Reading {
    found: Option<Kind>,
    label: String,
    files: Vec<String>,
    sealed: Option<Sealed>,
    trusted: Vec<Sealed>,
    changed: Vec<String>,
}

fn read_lines(folder: &Path, body: &str, key: &Key) -> Reading {
    let mut out = Reading {
        found: None,
        label: String::new(),
        files: Vec::new(),
        sealed: None,
        trusted: Vec::new(),
        changed: Vec::new(),
    };
    for line in body.lines().skip(1) {
        let (tag, rest) = line.split_once(' ').unwrap_or((line, ""));
        match tag {
            "kind" => out.found = Kind::parse(rest),
            "label" => out.label = rest.to_string(),
            "file" | "component" | "sealed" | "library" | "trusted" | "sealed-trusted" => {
                match check_entry(folder, (tag, rest), key, &mut out.changed) {
                    Some((path, code)) => {
                        out.files.push(path);
                        if tag == "sealed-trusted" {
                            out.trusted.extend(code);
                        } else {
                            out.sealed = code.or(out.sealed.take());
                        }
                    }
                    None => out.changed.push(format!("the line `{line}` of the lock")),
                }
            }
            _ => {}
        }
    }
    out
}

pub fn verify(folder: &Path, lock: &Path, kind: Kind, key: &Key) -> Result<Verified, String> {
    let name = shown(folder, lock);
    let text = fs::read_to_string(lock).map_err(|_| {
        format!(
            "there is no lock `{name}`: check writes it when its job is `done`, so the program is not checked as it is; run `check`"
        )
    })?;
    let Some(body) = signed(&text, key) else {
        return Err(format!(
            "the lock `{name}` is not signed with this user's key, or it was changed by hand; run `check` to sign it again"
        ));
    };
    if body.lines().next() != Some(HEADER) {
        return Err(format!(
            "the lock `{name}` is not a lock this lengua reads; run `check`"
        ));
    }
    let mut read = read_lines(folder, body, key);
    if read.found != Some(kind) {
        return Err(format!(
            "the lock `{name}` is not a {} lock; run `check`",
            kind.name()
        ));
    }
    if kind != Kind::Test {
        let mut now = ideas(folder);
        now.extend(sources(folder));
        let files = &read.files;
        read.changed.extend(
            now.into_iter()
                .filter(|path| !files.contains(path))
                .map(|path| format!("`{path}` is new")),
        );
    }
    if !read.changed.is_empty() {
        return Err(format!(
            "the files are not as the lock `{name}` signed them: {}; run `check`",
            read.changed.join(", ")
        ));
    }
    Ok(Verified {
        kind,
        label: read.label,
        files: read.files,
        sealed: read.sealed,
        trusted: read.trusted,
    })
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};

    use super::{Facts, KEY_BYTES, Kind, create_key, key_file, read_key, text, verify, write};
    use crate::{Label, Trust};

    fn folder(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!("lengua-lock-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(path.join("src")).expect("a folder");
        path
    }

    fn program(path: &Path) -> Facts {
        for (file, content) in [
            ("idea.md", "# Purpose\n"),
            ("index.md", "index\n"),
            ("log.md", "# Log\n"),
            ("src/main.rs", "fn main() {}\n"),
            ("Cargo.toml", "[package]\n"),
            ("wit/world.wit", "world program {}\n"),
            ("program.wasm", "wasm"),
            ("program.cwasm", "sealed code"),
        ] {
            let at = path.join(file);
            fs::create_dir_all(at.parent().expect("a parent")).expect("a folder");
            fs::write(at, content).expect("a file");
        }
        Facts {
            kind: Kind::Program,
            toolchain: "rustc 1.100.0".to_string(),
            trust: Trust {
                label: Label::Checked,
                score: Some((3, 3)),
            },
            files: super::program_files(path),
            component: Some("program.wasm".to_string()),
            sealed: Some("program.cwasm".to_string()),
            ..Facts::default()
        }
    }

    #[test]
    fn a_key_is_made_once_and_read_back() {
        let path = folder("key");
        let home = path.join("home");
        let first = create_key(&home, [7; KEY_BYTES]).expect("a key");
        let again = create_key(&home, [9; KEY_BYTES]).expect("the same key");
        assert!(first == again);
        assert!(read_key(&home).expect("a key") == first);
        let text = fs::read_to_string(key_file(&home)).expect("the file");
        assert_eq!(text, format!("{}\n", "07".repeat(KEY_BYTES)));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let key_mode = fs::metadata(key_file(&home))
                .expect("a file")
                .permissions()
                .mode();
            assert_eq!(key_mode & 0o777, 0o600);
            let folder_mode = fs::metadata(&home).expect("a folder").permissions().mode();
            assert_eq!(folder_mode & 0o777, 0o700);
        }
        fs::write(key_file(&home), "short\n").expect("a file");
        assert!(read_key(&home).is_err());
    }

    #[test]
    fn a_lock_lists_every_file_and_is_signed() {
        let path = folder("format");
        let facts = program(&path);
        let key = create_key(&path.join("home"), [1; KEY_BYTES]).expect("a key");
        let written = text(&path, &facts, &key).expect("a lock");
        let lines: Vec<&str> = written.lines().collect();
        assert_eq!(lines.first().copied(), Some("lengua-lock 1"));
        assert_eq!(lines.get(1).copied(), Some("kind program"));
        assert!(lines.contains(&"label checked"), "{written}");
        assert!(lines.contains(&"score 3 of 3"), "{written}");
        assert!(lines.contains(&"toolchain rustc 1.100.0"), "{written}");
        for file in [
            "idea.md",
            "index.md",
            "log.md",
            "src/main.rs",
            "Cargo.toml",
            "wit/world.wit",
        ] {
            assert!(
                lines
                    .iter()
                    .any(|l| l.starts_with("file ") && l.ends_with(&format!(" {file}"))),
                "{file} in {written}"
            );
        }
        assert!(
            lines.iter().any(|l| l.starts_with("component ")),
            "{written}"
        );
        assert!(lines.iter().any(|l| l.starts_with("sealed ")), "{written}");
        assert!(
            lines
                .last()
                .is_some_and(|l| l.starts_with("hmac-sha256 ") && l.len() == 76)
        );
    }

    #[test]
    fn a_change_another_key_or_a_new_file_is_refused() {
        let path = folder("verify");
        let facts = program(&path);
        let home = path.join("home");
        let key = create_key(&home, [2; KEY_BYTES]).expect("a key");
        let lock = path.join("lengua.lock");
        write(&path, &lock, &facts, &key).expect("a lock");
        let verified = verify(&path, &lock, Kind::Program, &key).expect("a good lock");
        assert_eq!(verified.label, "checked");
        assert_eq!(
            verified.sealed().map(super::Sealed::bytes),
            Some(&b"sealed code"[..])
        );
        assert!(verify(&path, &lock, Kind::Test, &key).is_err());
        let other = create_key(&path.join("other"), [3; KEY_BYTES]).expect("a key");
        let refused = verify(&path, &lock, Kind::Program, &other).expect_err("another key");
        assert!(
            refused.contains("not signed with this user's key"),
            "{refused}"
        );
        fs::write(path.join("src/main.rs"), "fn main() { }\n").expect("a change");
        let refused = verify(&path, &lock, Kind::Program, &key).expect_err("a change");
        assert!(
            refused.contains("`src/main.rs` changed") && refused.contains("run `check`"),
            "{refused}"
        );
        fs::write(path.join("src/main.rs"), "fn main() {}\n").expect("undone");
        fs::write(path.join("src/extra.rs"), "\n").expect("a new file");
        let refused = verify(&path, &lock, Kind::Program, &key).expect_err("a new file");
        assert!(refused.contains("`src/extra.rs` is new"), "{refused}");
        fs::remove_file(path.join("src/extra.rs")).expect("removed");
        let mut tampered = fs::read_to_string(&lock).expect("the lock");
        tampered = tampered.replace("label checked", "label tested");
        fs::write(&lock, tampered).expect("a tampered lock");
        assert!(verify(&path, &lock, Kind::Program, &key).is_err());
        fs::remove_file(&lock).expect("removed");
        let refused = verify(&path, &lock, Kind::Program, &key).expect_err("no lock");
        assert!(refused.contains("there is no lock"), "{refused}");
    }

    #[test]
    fn a_test_lock_holds_only_the_sealed_code() {
        let path = folder("test");
        let _ = program(&path);
        let key = create_key(&path.join("home"), [4; KEY_BYTES]).expect("a key");
        let lock = path.join("test.lock");
        write(&path, &lock, &Facts::test("program.cwasm", &[]), &key).expect("a lock");
        let verified = verify(&path, &lock, Kind::Test, &key).expect("a good lock");
        assert_eq!(verified.files, ["program.cwasm"]);
        fs::write(path.join("program.cwasm"), "other code").expect("a change");
        let refused = verify(&path, &lock, Kind::Test, &key).expect_err("tampered code");
        assert!(refused.contains("`program.cwasm` changed"), "{refused}");
    }

    #[test]
    fn a_program_lock_names_its_library_and_its_trusted_parts() {
        let path = folder("library");
        let library = path.join("libraries/json");
        let _ = program(&library);
        let key = create_key(&path.join("home"), [5; KEY_BYTES]).expect("a key");
        let library_facts = Facts {
            kind: Kind::Library,
            files: super::program_files(&library),
            component: Some("program.wasm".to_string()),
            ..Facts::default()
        };
        write(&library, &library.join("lengua.lock"), &library_facts, &key).expect("a lock");
        let app = path.join("app");
        let mut facts = program(&app);
        fs::write(app.join("tax.wasm"), "a trusted part").expect("a part");
        fs::write(app.join("tax.cwasm"), "its sealed code").expect("sealed");
        facts.libraries = vec!["../libraries/json/lengua.lock".to_string()];
        facts.trusted = vec!["tax.wasm".to_string()];
        facts.sealed_trusted = vec!["tax.cwasm".to_string()];
        let lock = app.join("lengua.lock");
        write(&app, &lock, &facts, &key).expect("a lock");
        let text = fs::read_to_string(&lock).expect("the lock");
        assert!(text.contains(" ../libraries/json/lengua.lock\n"), "{text}");
        assert!(text.contains("\ntrusted "), "{text}");
        assert!(text.contains("\nsealed-trusted "), "{text}");
        let verified = verify(&app, &lock, Kind::Program, &key).expect("a good lock");
        assert_eq!(verified.trusted().len(), 1);
        assert_eq!(verified.trusted()[0].bytes(), b"its sealed code");
        fs::write(library.join("src/main.rs"), "pub fn changed() {}\n").expect("a change");
        let refused = verify(&app, &lock, Kind::Program, &key).expect_err("a changed library");
        assert!(
            refused.contains("the library of `../libraries/json/lengua.lock`")
                && refused.contains("`src/main.rs` changed"),
            "{refused}"
        );
    }
}
