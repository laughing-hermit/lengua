//! `lengua.lock`: the signed record of the user's request and what was checked against it.
//!
//! Only the lengua server writes it. The file is canonical TOML ending in a `[signature]` table
//! that holds an HMAC-SHA256 of every byte before it, keyed with a per-user secret, so any edit
//! made outside lengua is detected.

use std::fmt::{self, Write as _};
use std::io::{self, Read as _, Write as _};
use std::path::{Path, PathBuf};

use hmac::{Hmac, KeyInit, Mac};
use serde::Deserialize;
use sha2::Sha256;

use crate::data_dir::DataDir;
use crate::hash::{Sha256Hex, hex};

/// The lock's file name inside a program folder.
pub const LOCK_FILE: &str = "lengua.lock";

/// The lock format this server reads and writes.
pub const VERSION: u32 = 4;

/// The lengua dialect this server checks.
pub const DIALECT: u32 = 4;

const HEADER: &str = "# lengua.lock: written only by the lengua MCP server. Do not edit.\n";
const SIGNATURE_DOMAIN: &[u8] = b"lengua-lock-v1\n";
const FINGERPRINT_DOMAIN: &str = "lengua-fingerprint-v1\n";
const SIGNATURE_START: &str = "\n[signature]\nhmac_sha256 = \"";
const SIGNATURE_END: &str = "\"\n";
const KEY_FILE: &str = "key";
const KEY_LEN: usize = 32;

/// Locks are a few hundred bytes; anything much bigger wasn't written by lengua.
const MAX_LOCK_BYTES: u64 = 64 * 1024;

/// What `lengua.lock` records.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Lock {
    /// The user's request the spec was written for, and what was first recorded under it.
    pub request: RequestStamp,
    /// The recorded spec, tests included.
    pub spec: SpecStamp,
    /// The checked code, once stamped.
    pub code: Option<CodeStamp>,
    /// What `prove` proved about that code's build, once it has run. Requires `code`.
    pub proofs: Option<Box<ProofStamp>>,
}

/// The user's requests for the build in progress, and the versions every later change is shown
/// against.
///
/// A build starts with the first request and lasts until its code passes its tests. Words
/// recorded in the middle of a build join it rather than start over, so no change can drop out
/// of what is shown.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestStamp {
    /// The user's own words asking for the program or a change, as the agent quoted them, oldest
    /// first: the build's first request, then the latest ones.
    pub words: Vec<String>,
    /// SHA-256 of spec.md when the build's first request was recorded.
    pub spec: Sha256Hex,
}

/// The most request words a build keeps: its first and the latest ones.
pub const MAX_REQUESTS: usize = 6;

/// The recorded spec.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpecStamp {
    /// SHA-256 of spec.md.
    pub sha256: Sha256Hex,
}

/// The checked code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodeStamp {
    /// SHA-256 of main.s.
    pub sha256: Sha256Hex,
    /// SHA-256 of the linked executable that passed the checks.
    pub executable_sha256: Sha256Hex,
    /// The assembler and linker versions that built it.
    pub toolchain: String,
    /// The deepest stack the program can use.
    pub max_stack_bytes: u64,
    /// How many `# unsafe:` blocks the code has.
    pub unsafe_blocks: u32,
    /// The target it was compiled and checked for, such as `linux-x86_64`.
    pub target: String,
    /// The CPU features its instructions need beyond the target's baseline, in order.
    pub features: Vec<String>,
    /// What ran its tests when this computer runs the target's programs only through QEMU,
    /// such as `qemu-aarch64 10.2.2`; none when they ran natively.
    pub emulator: Option<String>,
}

/// A generated test proven for every input with texts up to a length.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bounded {
    /// Its id, such as `B1`.
    pub id: String,
    /// The longest text proven, with every shorter one, in bytes.
    pub bytes: usize,
    /// The longest the spec lets its texts be.
    pub of: usize,
    /// Whether proving on could reach further, as `check` with `prove_further` does.
    pub more: bool,
}

/// What `prove` proved about one build of the code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProofStamp {
    /// SHA-256 of the executable the proofs are about, which must be the checked code's.
    pub executable_sha256: Sha256Hex,
    /// The ids of the generated tests proven for every input in range, in the spec's order.
    pub proven: Vec<String>,
    /// The generated tests proven only for texts up to a length. Never also in `proven`.
    pub bounded: Vec<Bounded>,
    /// Which solvers checked them: `z3 and cvc5`, or one of them alone.
    pub solvers: String,
    /// The generated tests a proof found wrong, each id with what it found: `run` refuses this
    /// build for them however often lengua has restarted since.
    pub wrong: Vec<(String, String)>,
}

impl Lock {
    /// A lock that records only a new request and its spec; tests and code must be checked
    /// again.
    #[must_use]
    pub fn requested(spec: Sha256Hex, words: String) -> Self {
        Self {
            request: RequestStamp {
                words: vec![words],
                spec: spec.clone(),
            },
            spec: SpecStamp { sha256: spec },
            code: None,
            proofs: None,
        }
    }

    /// One hash over the spec and the code, once both are stamped.
    #[must_use]
    pub fn fingerprint(&self) -> Option<Sha256Hex> {
        let code = self.code.as_ref()?;
        let input = format!(
            "{FINGERPRINT_DOMAIN}spec:{}\ncode:{}\n",
            self.spec.sha256, code.sha256
        );
        Some(Sha256Hex::of(input.as_bytes()))
    }

    /// The complete file text, signed with `key`.
    #[must_use]
    pub fn to_signed_text(&self, key: &Key) -> String {
        sign_body(&self.body(), key)
    }

    /// Verifies a lock file's signature and parses it.
    ///
    /// # Errors
    ///
    /// Fails when the text isn't exactly what this server writes, or when the signature doesn't
    /// match `key`.
    pub fn from_signed_text(text: &str, key: &Key) -> Result<Self, LockError> {
        let damaged = || LockError::Malformed("its [signature] table is missing or damaged".into());
        let start = text.find(SIGNATURE_START).ok_or_else(damaged)?;
        let body = &text[..start];
        let tail = &text[start + SIGNATURE_START.len()..];
        let signature = tail.strip_suffix(SIGNATURE_END).ok_or_else(damaged)?;
        let signature = Sha256Hex::parse(signature).ok_or_else(damaged)?;
        if !key.verifies(body, &signature) {
            return Err(LockError::BadSignature);
        }
        parse_body(body)
    }

    /// The canonical text before the `[signature]` table.
    fn body(&self) -> String {
        let mut text = String::from(HEADER);
        // Writing to a String can't fail.
        let _ = writeln!(text, "version = {VERSION}");
        let _ = writeln!(text, "dialect = {DIALECT}");
        if let Some(fingerprint) = self.fingerprint() {
            let _ = writeln!(text, "fingerprint = \"{fingerprint}\"");
        }
        let words: Vec<String> = self
            .request
            .words
            .iter()
            .map(|words| toml_string(words))
            .collect();
        let _ = write!(
            text,
            "\n[request]\nwords = [{}]\nspec = \"{}\"\n",
            words.join(", "),
            self.request.spec
        );
        let _ = write!(text, "\n[spec]\nsha256 = \"{}\"\n", self.spec.sha256);
        if let Some(code) = &self.code {
            let features: Vec<String> =
                code.features.iter().map(|name| toml_string(name)).collect();
            let _ = write!(
                text,
                "\n[code]\nsha256 = \"{}\"\nexecutable_sha256 = \"{}\"\ntoolchain = {}\n\
                 max_stack_bytes = {}\nunsafe_blocks = {}\ntarget = {}\nfeatures = [{}]\n",
                code.sha256,
                code.executable_sha256,
                toml_string(&code.toolchain),
                code.max_stack_bytes,
                code.unsafe_blocks,
                toml_string(&code.target),
                features.join(", ")
            );
            if let Some(emulator) = &code.emulator {
                let _ = writeln!(text, "emulator = {}", toml_string(emulator));
            }
        }
        if let Some(proofs) = &self.proofs {
            let proven: Vec<String> = proofs.proven.iter().map(|id| toml_string(id)).collect();
            let _ = write!(
                text,
                "\n[proofs]\nexecutable_sha256 = \"{}\"\nproven = [{}]\n",
                proofs.executable_sha256,
                proven.join(", "),
            );
            if !proofs.bounded.is_empty() {
                let bounded: Vec<String> = proofs
                    .bounded
                    .iter()
                    .map(|bounded| {
                        format!(
                            "{{ id = {}, bytes = {}, of = {}, more = {} }}",
                            toml_string(&bounded.id),
                            bounded.bytes,
                            bounded.of,
                            bounded.more
                        )
                    })
                    .collect();
                let _ = writeln!(text, "bounded = [{}]", bounded.join(", "));
            }
            let _ = writeln!(text, "solvers = {}", toml_string(&proofs.solvers));
            if !proofs.wrong.is_empty() {
                let wrong: Vec<String> = proofs
                    .wrong
                    .iter()
                    .map(|(id, found)| {
                        format!(
                            "{{ id = {}, found = {} }}",
                            toml_string(id),
                            toml_string(found)
                        )
                    })
                    .collect();
                let _ = writeln!(text, "wrong = [{}]", wrong.join(", "));
            }
        }
        text
    }
}

fn sign_body(body: &str, key: &Key) -> String {
    format!("{body}{SIGNATURE_START}{}{SIGNATURE_END}", key.sign(body))
}

#[derive(Deserialize)]
struct VersionOnly {
    version: Option<i64>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LockFile {
    /// Already checked through `VersionOnly`; declared so `deny_unknown_fields` accepts it.
    #[serde(rename = "version")]
    _version: u32,
    dialect: u32,
    fingerprint: Option<String>,
    request: RequestTable,
    spec: SpecTable,
    code: Option<CodeTable>,
    proofs: Option<ProofsTable>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ProofsTable {
    executable_sha256: String,
    proven: Vec<String>,
    #[serde(default)]
    bounded: Vec<BoundedTable>,
    solvers: String,
    #[serde(default)]
    wrong: Vec<WrongTable>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WrongTable {
    id: String,
    found: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BoundedTable {
    id: String,
    bytes: usize,
    #[serde(default)]
    of: usize,
    #[serde(default)]
    more: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RequestTable {
    words: Vec<String>,
    spec: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SpecTable {
    sha256: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CodeTable {
    sha256: String,
    executable_sha256: String,
    toolchain: String,
    max_stack_bytes: u64,
    unsafe_blocks: u32,
    target: String,
    features: Vec<String>,
    #[serde(default)]
    emulator: Option<String>,
}

pub(crate) fn parse_body(body: &str) -> Result<Lock, LockError> {
    let malformed = |detail: &str| LockError::Malformed(detail.to_owned());
    let version = toml::from_str::<VersionOnly>(body)
        .map_err(|_| malformed("it isn't valid TOML"))?
        .version;
    if version != Some(i64::from(VERSION)) {
        return Err(LockError::UnsupportedVersion(
            version.map_or_else(|| "none".to_owned(), |version| version.to_string()),
        ));
    }
    let file: LockFile =
        toml::from_str(body).map_err(|_| malformed("it has fields lengua doesn't write"))?;
    if file.dialect != DIALECT {
        return Err(LockError::UnsupportedDialect(file.dialect));
    }
    let digest = |text: &str| Sha256Hex::parse(text).ok_or_else(|| malformed("a hash is damaged"));
    if file.request.words.is_empty() || file.request.words.len() > MAX_REQUESTS {
        return Err(malformed("its [request] table is damaged"));
    }
    let lock = Lock {
        request: RequestStamp {
            words: file.request.words,
            spec: digest(&file.request.spec)?,
        },
        spec: SpecStamp {
            sha256: digest(&file.spec.sha256)?,
        },
        code: file
            .code
            .map(|code| {
                Ok::<_, LockError>(CodeStamp {
                    sha256: digest(&code.sha256)?,
                    executable_sha256: digest(&code.executable_sha256)?,
                    toolchain: code.toolchain,
                    max_stack_bytes: code.max_stack_bytes,
                    unsafe_blocks: code.unsafe_blocks,
                    target: code.target,
                    features: code.features,
                    emulator: code.emulator,
                })
            })
            .transpose()?,
        proofs: file
            .proofs
            .map(|proofs| {
                Ok::<_, LockError>(Box::new(ProofStamp {
                    executable_sha256: digest(&proofs.executable_sha256)?,
                    proven: proofs.proven,
                    bounded: proofs
                        .bounded
                        .into_iter()
                        .map(|bounded| Bounded {
                            id: bounded.id,
                            bytes: bounded.bytes,
                            of: bounded.of,
                            more: bounded.more,
                        })
                        .collect(),
                    solvers: proofs.solvers,
                    wrong: proofs
                        .wrong
                        .into_iter()
                        .map(|wrong| (wrong.id, wrong.found))
                        .collect(),
                }))
            })
            .transpose()?,
    };
    if let Some(proofs) = &lock.proofs
        && lock
            .code
            .as_ref()
            .is_none_or(|code| code.executable_sha256 != proofs.executable_sha256)
    {
        return Err(malformed("its proofs are about a build it doesn't record"));
    }
    let fingerprint = file.fingerprint.as_deref().map(digest).transpose()?;
    if fingerprint != lock.fingerprint() {
        return Err(malformed("its fingerprint doesn't match its hashes"));
    }
    if lock.body() != body {
        return Err(malformed("it isn't laid out the way lengua writes it"));
    }
    Ok(lock)
}

/// A TOML basic string.
fn toml_string(text: &str) -> String {
    let mut quoted = String::with_capacity(text.len() + 2);
    quoted.push('"');
    for character in text.chars() {
        match character {
            '"' => quoted.push_str("\\\""),
            '\\' => quoted.push_str("\\\\"),
            '\n' => quoted.push_str("\\n"),
            '\t' => quoted.push_str("\\t"),
            '\r' => quoted.push_str("\\r"),
            control if control.is_control() => {
                let _ = write!(quoted, "\\u{:04X}", u32::from(control));
            }
            other => quoted.push(other),
        }
    }
    quoted.push('"');
    quoted
}

/// Why a lock can't be trusted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LockError {
    /// This machine has no signing key, so no lock can be verified.
    NoKey,
    /// The file couldn't be read.
    Unreadable(String),
    /// The file isn't in the format this server writes.
    Malformed(String),
    /// The signature doesn't match: something other than this server changed the file.
    BadSignature,
    /// The file uses a lock format this server doesn't read.
    UnsupportedVersion(String),
    /// The file was checked against a dialect this server doesn't check.
    UnsupportedDialect(u32),
}

impl fmt::Display for LockError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoKey => f.write_str(
                "this machine has no lengua signing key to verify it (was it copied from another \
                 machine?)",
            ),
            Self::Unreadable(error) => write!(f, "it can't be read: {error}"),
            Self::Malformed(detail) => write!(f, "{detail}"),
            Self::BadSignature => f.write_str(
                "its signature doesn't match: it was signed on another machine or changed outside \
                 lengua",
            ),
            Self::UnsupportedVersion(version) => write!(
                f,
                "it uses lock format version {version}; this lengua reads version {VERSION}"
            ),
            Self::UnsupportedDialect(dialect) => write!(
                f,
                "it was checked against dialect {dialect}; this lengua checks dialect {DIALECT}"
            ),
        }
    }
}

/// Reads and verifies `<folder>/lengua.lock`. `Ok(None)` means there is no lock yet.
///
/// # Errors
///
/// Fails when a lock exists but can't be trusted.
pub fn read(folder: &Path, key: Option<&Key>) -> Result<Option<Lock>, LockError> {
    let path = folder.join(LOCK_FILE);
    let file = match std::fs::File::open(&path) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(LockError::Unreadable(error.to_string())),
    };
    let mut bytes = Vec::new();
    file.take(MAX_LOCK_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| LockError::Unreadable(error.to_string()))?;
    if bytes.len() as u64 > MAX_LOCK_BYTES {
        return Err(LockError::Malformed("it's far too large".into()));
    }
    let text =
        String::from_utf8(bytes).map_err(|_| LockError::Malformed("it isn't UTF-8 text".into()))?;
    let key = key.ok_or(LockError::NoKey)?;
    Lock::from_signed_text(&text, key).map(Some)
}

/// Writes `<folder>/lengua.lock` atomically: readers see the old lock or the new one, never a
/// partial file.
///
/// # Errors
///
/// Fails when the folder isn't writable.
pub fn write(folder: &Path, lock: &Lock, key: &Key) -> io::Result<()> {
    let mut file = tempfile::NamedTempFile::new_in(folder)?;
    file.write_all(lock.to_signed_text(key).as_bytes())?;
    file.as_file().sync_all()?;
    file.persist(folder.join(LOCK_FILE))
        .map_err(|error| error.error)?;
    Ok(())
}

/// The per-user secret that signs locks.
#[derive(Clone)]
pub struct Key([u8; KEY_LEN]);

impl fmt::Debug for Key {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Key(<secret>)")
    }
}

/// Why the signing key can't be used.
#[derive(Debug)]
pub enum KeyError {
    /// The key file or its folder couldn't be read or written.
    Io {
        /// The file or folder involved.
        path: PathBuf,
        /// What went wrong.
        error: io::Error,
    },
    /// The key file exists but isn't a key.
    Damaged(PathBuf),
}

impl fmt::Display for KeyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { path, error } => {
                write!(
                    f,
                    "can't use the signing key at {}: {error}",
                    path.display()
                )
            }
            Self::Damaged(path) => write!(
                f,
                "the signing key at {} is damaged; delete it, then record each program's spec \
                 again",
                path.display()
            ),
        }
    }
}

impl std::error::Error for KeyError {}

impl Key {
    /// A key from raw bytes.
    #[must_use]
    pub fn from_bytes(bytes: [u8; KEY_LEN]) -> Self {
        Self(bytes)
    }

    /// The path of the key file in `data_dir`.
    #[must_use]
    pub fn path(data_dir: &DataDir) -> PathBuf {
        data_dir.path().join(KEY_FILE)
    }

    /// Loads this user's key, if one exists.
    ///
    /// # Errors
    ///
    /// Fails when the key file exists but can't be read or is damaged.
    pub fn load(data_dir: &DataDir) -> Result<Option<Self>, KeyError> {
        let path = Self::path(data_dir);
        match std::fs::read(&path) {
            Ok(bytes) => <[u8; KEY_LEN]>::try_from(bytes.as_slice())
                .map(|bytes| Some(Self(bytes)))
                .map_err(|_| KeyError::Damaged(path)),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(KeyError::Io { path, error }),
        }
    }

    /// Loads this user's key, creating it (32 random bytes, readable only by the user) on
    /// first use.
    ///
    /// # Errors
    ///
    /// Fails when the data folder can't be created or the key can't be read or written.
    pub fn load_or_create(data_dir: &DataDir) -> Result<Self, KeyError> {
        if let Some(key) = Self::load(data_dir)? {
            return Ok(key);
        }
        let folder = data_dir.path();
        create_private_dir(folder).map_err(|error| KeyError::Io {
            path: folder.to_owned(),
            error,
        })?;
        let mut bytes = [0_u8; KEY_LEN];
        getrandom::fill(&mut bytes).map_err(|error| KeyError::Io {
            path: folder.to_owned(),
            error: io::Error::other(error.to_string()),
        })?;
        let path = Self::path(data_dir);
        let io_error = |error| KeyError::Io {
            path: path.clone(),
            error,
        };
        // tempfile creates the file readable only by its owner.
        let mut file = tempfile::NamedTempFile::new_in(folder).map_err(io_error)?;
        file.write_all(&bytes).map_err(io_error)?;
        file.as_file().sync_all().map_err(io_error)?;
        match file.persist_noclobber(&path) {
            Ok(_) => Ok(Self(bytes)),
            // Another lengua process created the key first; use that one.
            Err(error) if error.error.kind() == io::ErrorKind::AlreadyExists => {
                Self::load(data_dir)?.ok_or(KeyError::Damaged(path))
            }
            Err(error) => Err(io_error(error.error)),
        }
    }

    fn sign(&self, body: &str) -> Sha256Hex {
        let tag = self.mac(body).finalize().into_bytes();
        Sha256Hex::parse(&hex(&tag)).expect("an HMAC-SHA256 tag is 32 bytes")
    }

    fn verifies(&self, body: &str, signature: &Sha256Hex) -> bool {
        let Some(tag) = unhex(signature.as_str()) else {
            return false;
        };
        // Constant-time comparison.
        self.mac(body).verify_slice(&tag).is_ok()
    }

    fn mac(&self, body: &str) -> Hmac<Sha256> {
        let mut mac = <Hmac<Sha256> as KeyInit>::new_from_slice(&self.0)
            .expect("HMAC accepts any key length");
        mac.update(SIGNATURE_DOMAIN);
        mac.update(body.as_bytes());
        mac
    }
}

fn unhex(text: &str) -> Option<Vec<u8>> {
    let digit = |byte: u8| char::from(byte).to_digit(16);
    text.as_bytes()
        .chunks(2)
        .map(|pair| match pair {
            [high, low] => Some(u8::try_from(digit(*high)? * 16 + digit(*low)?).ok()?),
            _ => None,
        })
        .collect()
}

#[cfg(unix)]
fn create_private_dir(path: &Path) -> io::Result<()> {
    use std::os::unix::fs::DirBuilderExt as _;
    std::fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(path)
}

#[cfg(not(unix))]
fn create_private_dir(path: &Path) -> io::Result<()> {
    std::fs::create_dir_all(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(byte: u8) -> Key {
        Key::from_bytes([byte; KEY_LEN])
    }

    fn full_lock() -> Lock {
        Lock {
            request: RequestStamp {
                words: vec![
                    "Make me a program that adds two numbers".into(),
                    "Also take \"negative\" numbers".into(),
                ],
                spec: Sha256Hex::of(b"first spec"),
            },
            spec: SpecStamp {
                sha256: Sha256Hex::of(b"spec"),
            },
            code: Some(CodeStamp {
                sha256: Sha256Hex::of(b"code"),
                executable_sha256: Sha256Hex::of(b"executable"),
                toolchain: "llvm-mc 22.1.8, ld.lld 22.1.8 \"quoted\" \\ tab\t".into(),
                max_stack_bytes: 96,
                unsafe_blocks: 1,
                target: "linux-x86_64".into(),
                features: vec!["avx2".into(), "bmi2".into()],
                emulator: None,
            }),
            proofs: None,
        }
    }

    /// Signs arbitrary body text, as if a server had written it.
    fn forge(body: &str, key: &Key) -> String {
        sign_body(body, key)
    }

    #[test]
    fn round_trips_a_spec_only_lock() {
        let lock = Lock::requested(Sha256Hex::of(b"spec"), "Make it say \"hi\"\nplease".into());
        let text = lock.to_signed_text(&key(1));
        assert!(text.starts_with(HEADER), "{text}");
        assert!(!text.contains("fingerprint"), "{text}");
        assert!(
            text.contains("words = [\"Make it say \\\"hi\\\"\\nplease\"]\n"),
            "{text}"
        );
        assert!(!text.contains("[code]"), "{text}");
        assert!(text.ends_with("\"\n"), "{text}");
        assert_eq!(Lock::from_signed_text(&text, &key(1)), Ok(lock));
    }

    #[test]
    fn round_trips_proofs_about_the_checked_build() {
        let mut lock = full_lock();
        lock.proofs = Some(Box::new(ProofStamp {
            executable_sha256: Sha256Hex::of(b"executable"),
            proven: vec!["B1".into(), "B\"2".into()],
            bounded: Vec::new(),
            solvers: "z3 and cvc5".into(),
            wrong: Vec::new(),
        }));
        let text = lock.to_signed_text(&key(1));
        assert!(text.contains("proven = [\"B1\", \"B\\\"2\"]\n"), "{text}");
        assert!(!text.contains("bounded"), "{text}");
        assert_eq!(Lock::from_signed_text(&text, &key(1)), Ok(lock.clone()));
        // A proof that holds only up to a length is recorded with it, apart from full ones.
        let mut bounded = lock.clone();
        if let Some(proofs) = &mut bounded.proofs {
            proofs.bounded = vec![Bounded {
                id: "B3".into(),
                bytes: 4096,
                of: 5000,
                more: true,
            }];
        }
        let bounded_text = bounded.to_signed_text(&key(1));
        assert!(
            bounded_text
                .contains("bounded = [{ id = \"B3\", bytes = 4096, of = 5000, more = true }]\n"),
            "{bounded_text}"
        );
        assert_eq!(Lock::from_signed_text(&bounded_text, &key(1)), Ok(bounded));
        // What a proof found wrong is recorded with what it found, so it outlasts restarts.
        let mut wrong = lock.clone();
        if let Some(proofs) = &mut wrong.proofs {
            proofs.wrong = vec![(
                "B2".into(),
                "with n = 1234567: it printed \"-1234566\"".into(),
            )];
        }
        let wrong_text = wrong.to_signed_text(&key(1));
        assert!(
            wrong_text.contains(
                "wrong = [{ id = \"B2\", found = \"with n = 1234567: it printed \\\"-1234566\\\"\" }]\n"
            ),
            "{wrong_text}"
        );
        assert_eq!(Lock::from_signed_text(&wrong_text, &key(1)), Ok(wrong));
        // Proofs about some other build can't be in a lock lengua wrote.
        let other = text
            .split("\n[signature]")
            .next()
            .expect("a body")
            .replacen(
                &format!(
                    "executable_sha256 = \"{}\"\nproven",
                    Sha256Hex::of(b"executable")
                ),
                &format!(
                    "executable_sha256 = \"{}\"\nproven",
                    Sha256Hex::of(b"other")
                ),
                1,
            );
        assert!(matches!(
            Lock::from_signed_text(&forge(&other, &key(1)), &key(1)),
            Err(LockError::Malformed(_))
        ));
    }

    #[test]
    fn round_trips_a_full_lock_with_its_fingerprint() {
        let lock = full_lock();
        let text = lock.to_signed_text(&key(1));
        let fingerprint = lock.fingerprint().expect("both are stamped");
        assert!(
            text.contains(&format!("fingerprint = \"{fingerprint}\"\n")),
            "{text}"
        );
        assert!(
            text.contains("target = \"linux-x86_64\"\nfeatures = [\"avx2\", \"bmi2\"]\n"),
            "{text}"
        );
        assert_eq!(Lock::from_signed_text(&text, &key(1)), Ok(lock));
    }

    /// Code tested under QEMU says so, and reads back as emulated; a native lock says nothing
    /// of it.
    #[test]
    fn round_trips_what_ran_the_tests() {
        let mut lock = full_lock();
        let native = lock.to_signed_text(&key(1));
        assert!(!native.contains("emulator"), "{native}");
        if let Some(code) = lock.code.as_mut() {
            code.target = "linux-aarch64".into();
            code.features = Vec::new();
            code.emulator = Some("qemu-aarch64 10.2.2".into());
        }
        let text = lock.to_signed_text(&key(1));
        assert!(
            text.contains(
                "target = \"linux-aarch64\"\nfeatures = []\nemulator = \"qemu-aarch64 10.2.2\"\n"
            ),
            "{text}"
        );
        assert_eq!(Lock::from_signed_text(&text, &key(1)), Ok(lock));
    }

    #[test]
    fn fingerprint_needs_the_code() {
        let mut lock = full_lock();
        assert!(lock.fingerprint().is_some());
        lock.code = None;
        assert_eq!(lock.fingerprint(), None);
    }

    #[test]
    fn any_flipped_byte_is_detected() {
        let text = full_lock().to_signed_text(&key(1));
        for index in 0..text.len() {
            let mut bytes = text.clone().into_bytes();
            bytes[index] ^= 0x01;
            let Ok(flipped) = String::from_utf8(bytes) else {
                continue;
            };
            assert!(
                Lock::from_signed_text(&flipped, &key(1)).is_err(),
                "flipping byte {index} went unnoticed"
            );
        }
    }

    #[test]
    fn a_different_key_is_detected() {
        let text = full_lock().to_signed_text(&key(1));
        assert_eq!(
            Lock::from_signed_text(&text, &key(2)),
            Err(LockError::BadSignature)
        );
    }

    #[test]
    fn truncation_and_trailing_bytes_are_detected() {
        let text = full_lock().to_signed_text(&key(1));
        for cut in [1, 10, 80] {
            assert!(matches!(
                Lock::from_signed_text(&text[..text.len() - cut], &key(1)),
                Err(LockError::Malformed(_))
            ));
        }
        assert!(matches!(
            Lock::from_signed_text(&format!("{text}# more\n"), &key(1)),
            Err(LockError::Malformed(_))
        ));
        assert!(matches!(
            Lock::from_signed_text("", &key(1)),
            Err(LockError::Malformed(_))
        ));
    }

    #[test]
    fn unknown_versions_and_dialects_are_reported() {
        let body = full_lock().body();
        // Version 3 recorded tests.toml apart from the spec.
        let older = forge(&body.replace("version = 4", "version = 3"), &key(1));
        assert_eq!(
            Lock::from_signed_text(&older, &key(1)),
            Err(LockError::UnsupportedVersion("3".into()))
        );
        // Dialect 3 had no jump tables.
        let dialect = forge(&body.replace("dialect = 4", "dialect = 3"), &key(1));
        assert_eq!(
            Lock::from_signed_text(&dialect, &key(1)),
            Err(LockError::UnsupportedDialect(3))
        );
    }

    #[test]
    fn signed_but_unexpected_content_is_rejected() {
        let lock = full_lock();
        let body = lock.body();
        let cases = [
            body.replace("[spec]", "[spec]\nextra = 1"),
            body.replace("max_stack_bytes = 96", "max_stack_bytes = 96 # note"),
            body.replace("\n\n[spec]", "\n\n\n[spec]"),
            body.replace(
                &lock.fingerprint().expect("fingerprint").to_string(),
                Sha256Hex::of(b"other").as_str(),
            ),
            body.replace("words = ", "approval = "),
            {
                let start = body.find("words = [").expect("words");
                let end = body[start..].find('\n').expect("end of words") + start;
                format!("{}words = []{}", &body[..start], &body[end..])
            },
            body.replace("[code]\nsha256", "[code]\nfrozen = true\nsha256"),
            body.replace(
                &format!(
                    "fingerprint = \"{}\"\n",
                    lock.fingerprint().expect("fingerprint")
                ),
                "",
            ),
            body.replace(
                "\n[code]",
                &format!(
                    "\n[tests]\nsha256 = \"{}\"\n\n[code]",
                    Sha256Hex::of(b"tests")
                ),
            ),
            body.replace("[\"avx2\", \"bmi2\"]", "[\"avx2\",\"bmi2\"]"),
            body.replace("target = \"linux-x86_64\"\n", ""),
        ];
        for body in cases {
            assert!(
                matches!(
                    Lock::from_signed_text(&forge(&body, &key(1)), &key(1)),
                    Err(LockError::Malformed(_))
                ),
                "accepted:\n{body}"
            );
        }
    }

    #[test]
    fn writes_and_reads_a_lock_file() {
        let folder = tempfile::tempdir().expect("temporary folder");
        assert_eq!(read(folder.path(), Some(&key(1))), Ok(None));
        let lock = full_lock();
        write(folder.path(), &lock, &key(1)).expect("write the lock");
        assert_eq!(read(folder.path(), Some(&key(1))), Ok(Some(lock.clone())));
        assert_eq!(read(folder.path(), None), Err(LockError::NoKey));
        let leftovers: Vec<_> = std::fs::read_dir(folder.path())
            .expect("list the folder")
            .map(|entry| {
                entry
                    .expect("entry")
                    .file_name()
                    .into_string()
                    .expect("UTF-8")
            })
            .collect();
        assert_eq!(leftovers, [LOCK_FILE], "no temporary files are left behind");
        std::fs::write(folder.path().join(LOCK_FILE), vec![b'#'; 70_000]).expect("overwrite");
        assert!(matches!(
            read(folder.path(), Some(&key(1))),
            Err(LockError::Malformed(_))
        ));
    }

    #[test]
    fn creates_a_private_key_once() {
        let home = tempfile::tempdir().expect("temporary folder");
        let data_dir = DataDir::new(home.path().join("data/lengua"));
        assert!(Key::load(&data_dir).expect("no key yet").is_none());
        let created = Key::load_or_create(&data_dir).expect("create the key");
        let loaded = Key::load_or_create(&data_dir).expect("load the key");
        assert_eq!(created.0, loaded.0);
        assert_ne!(created.0, [0; KEY_LEN]);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            let mode = |path: &Path| {
                std::fs::metadata(path)
                    .expect("metadata")
                    .permissions()
                    .mode()
                    & 0o777
            };
            assert_eq!(mode(&Key::path(&data_dir)), 0o600);
            assert_eq!(mode(data_dir.path()), 0o700);
        }
    }

    #[test]
    fn reports_a_damaged_key() {
        let home = tempfile::tempdir().expect("temporary folder");
        let data_dir = DataDir::new(home.path().to_owned());
        std::fs::write(Key::path(&data_dir), b"short").expect("write a bad key");
        assert!(matches!(Key::load(&data_dir), Err(KeyError::Damaged(_))));
        assert!(matches!(
            Key::load_or_create(&data_dir),
            Err(KeyError::Damaged(_))
        ));
    }

    #[test]
    fn debug_output_hides_the_key() {
        assert_eq!(format!("{:?}", key(7)), "Key(<secret>)");
    }
}
