//! `lengua.lock`: the signed record of the user's request and what was checked against it.
//!
//! Only the lengua server writes it. The file is canonical TOML ending in a `[signature]` table
//! that holds an HMAC-SHA256 of every byte before it, keyed with a per-user secret, so any edit
//! made outside lengua is detected.

use std::fmt::{self, Write as _};
use std::io::{self, Read as _, Write as _};
use std::path::{Path, PathBuf};

use hmac::{Hmac, Mac, digest::KeyInit};
use serde::Deserialize;
use sha2::Sha256;

use crate::data_dir::DataDir;
use crate::hash::{Sha256Hex, hex};

/// The lock's file name inside a program folder.
pub const LOCK_FILE: &str = "lengua.lock";

/// The lock format this server writes. Version 5 names the idea file (`[idea]`, from `idea.md`)
/// where version 4 named a spec; version 6 adds the stamp of WebAssembly code, main.wat's; version
/// 7 records main.wat alone, with the module's hash in `[proofs]` named `module_sha256`.
pub const VERSION: u32 = 7;

/// The oldest lock format this server still reads: version 6, whose stamp of main.wat version 7
/// keeps as it was, so a program checked before stays checked. lengua writes it again as 7. Every
/// format before it came before main.wat: only the native lengua wrote them, with its dialect 4,
/// so a lock of one is the native lengua's ([`LockError::Native`]).
pub const OLDEST_VERSION: u32 = 6;

/// The dialect a lock of checked main.wat records: the WebAssembly dialect 1, which
/// `plugin/skills/compile/references/power-of-10-webassembly.md` writes down.
pub const WASM_DIALECT: &str = "webassembly 1";

/// The dialect lock format 6 wrote in a lock whose code wasn't checked yet: the native lengua's
/// number, which the lock records no code against.
const FORMAT_6_UNCHECKED_DIALECT: u32 = 4;

/// The target a stamp of main.wat names: one module for every computer.
pub const WASM_TARGET: &str = "webassembly";

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
    /// The user's request the idea was written for, and what was first recorded under it.
    pub request: RequestStamp,
    /// The recorded idea, tests included.
    pub idea: IdeaStamp,
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
    /// SHA-256 of idea.md when the build's first request was recorded.
    pub idea: Sha256Hex,
}

/// The most request words a build keeps: its first and the latest ones.
pub const MAX_REQUESTS: usize = 6;

/// The recorded idea.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdeaStamp {
    /// SHA-256 of idea.md.
    pub sha256: Sha256Hex,
}

/// The checked code: main.wat's, for every computer ([`WASM_TARGET`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodeStamp {
    /// SHA-256 of main.wat as it is.
    pub sha256: Sha256Hex,
    /// SHA-256 of what passed the checks: the module lengua built from main.wat.
    pub module_sha256: Sha256Hex,
    /// The engine that runs the module, such as `wasmtime 50.0.1`.
    pub engine: String,
    /// The deepest stack the program can use, as the rules count it.
    pub max_stack_bytes: u64,
    /// How many `;; unsafe:` blocks main.wat has.
    pub unsafe_blocks: u32,
    /// The WebAssembly features its module uses beyond WebAssembly 1.0, in order.
    pub features: Vec<String>,
}

/// A generated test proven for every input with texts up to a length.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bounded {
    /// Its id, such as `B1`.
    pub id: String,
    /// The longest text proven, with every shorter one, in bytes.
    pub bytes: usize,
    /// The longest the idea lets its texts be.
    pub of: usize,
    /// Whether proving on could reach further, as `check` with `prove_further` does.
    pub more: bool,
}

/// What `prove` proved about one build of the code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProofStamp {
    /// SHA-256 of the module the proofs are about, which must be the checked code's.
    pub module_sha256: Sha256Hex,
    /// The ids of the generated tests proven for every input in range, in the idea's order.
    pub proven: Vec<String>,
    /// The generated tests proven only for texts up to a length. Never also in `proven`.
    pub bounded: Vec<Bounded>,
    /// Which solvers checked them: `z3 and cvc5`, or one of them alone.
    pub solvers: String,
    /// The generated tests a proof found wrong, each id with what it found: `run` refuses this
    /// build for them however often lengua has restarted since.
    pub wrong: Vec<(String, String)>,
    /// The other generated tests the proofs ended without proving, each id with why they are
    /// tested, not proven: what the model doesn't follow, the time running out, or what a real
    /// run made of what a proof found. A check after a restart says why again.
    pub unproven: Vec<(String, String)>,
}

impl Lock {
    /// A lock that records only a new request and its idea; tests and code must be checked
    /// again.
    #[must_use]
    pub fn requested(idea: Sha256Hex, words: String) -> Self {
        Self {
            request: RequestStamp {
                words: vec![words],
                idea: idea.clone(),
            },
            idea: IdeaStamp { sha256: idea },
            code: None,
            proofs: None,
        }
    }

    /// One hash over the idea and the code, once both are stamped.
    #[must_use]
    pub fn fingerprint(&self) -> Option<Sha256Hex> {
        let code = self.code.as_ref()?;
        let input = format!(
            "{FINGERPRINT_DOMAIN}idea:{}\ncode:{}\n",
            self.idea.sha256, code.sha256
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

    /// The canonical text before the `[signature]` table, in the format this server writes.
    fn body(&self) -> String {
        self.body_in(VERSION)
    }

    /// The dialect the checked code was held to, as lock format `version` writes it: the
    /// WebAssembly dialect's name, which format 6 wrote only once code was checked.
    fn dialect_line(&self, version: u32) -> String {
        if version > 6 || self.code.is_some() {
            format!("dialect = {}", toml_string(WASM_DIALECT))
        } else {
            format!("dialect = {FORMAT_6_UNCHECKED_DIALECT}")
        }
    }

    /// The canonical text before the `[signature]` table, in lock format `version`.
    fn body_in(&self, version: u32) -> String {
        let mut text = String::from(HEADER);
        // Writing to a String can't fail.
        let _ = writeln!(text, "version = {version}");
        let _ = writeln!(text, "{}", self.dialect_line(version));
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
            "\n[request]\nwords = [{}]\nidea = \"{}\"\n",
            words.join(", "),
            self.request.idea
        );
        let _ = write!(text, "\n[idea]\nsha256 = \"{}\"\n", self.idea.sha256);
        if let Some(code) = &self.code {
            let features: Vec<String> =
                code.features.iter().map(|name| toml_string(name)).collect();
            let _ = write!(
                text,
                "\n[code]\nsha256 = \"{}\"\nmodule_sha256 = \"{}\"\nengine = {}\n\
                 max_stack_bytes = {}\nunsafe_blocks = {}\ntarget = {}\nfeatures = [{}]\n",
                code.sha256,
                code.module_sha256,
                toml_string(&code.engine),
                code.max_stack_bytes,
                code.unsafe_blocks,
                toml_string(WASM_TARGET),
                features.join(", ")
            );
        }
        if let Some(proofs) = &self.proofs {
            let proven: Vec<String> = proofs.proven.iter().map(|id| toml_string(id)).collect();
            // Format 6 named the module's hash as a native executable's.
            let built = if version > 6 {
                "module_sha256"
            } else {
                "executable_sha256"
            };
            let _ = write!(
                text,
                "\n[proofs]\n{built} = \"{}\"\nproven = [{}]\n",
                proofs.module_sha256,
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
            if !proofs.unproven.is_empty() {
                let unproven: Vec<String> = proofs
                    .unproven
                    .iter()
                    .map(|(id, reason)| {
                        format!(
                            "{{ id = {}, why = {} }}",
                            toml_string(id),
                            toml_string(reason)
                        )
                    })
                    .collect();
                let _ = writeln!(text, "unproven = [{}]", unproven.join(", "));
            }
        }
        text
    }
}

fn sign_body(body: &str, key: &Key) -> String {
    format!("{body}{SIGNATURE_START}{}{SIGNATURE_END}", key.sign(body))
}

/// The `dialect` a lock says: main.wat's, a name, or the native lengua's, a number.
#[derive(Deserialize)]
#[serde(untagged)]
enum DialectField {
    /// The native dialect's number.
    Number(u32),
    /// The WebAssembly dialect's name.
    Name(String),
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LockFile {
    /// Already checked, from the body's table; declared so `deny_unknown_fields` accepts it.
    #[serde(rename = "version")]
    _version: u32,
    dialect: DialectField,
    fingerprint: Option<String>,
    request: RequestTable,
    idea: IdeaTable,
    code: Option<CodeTable>,
    proofs: Option<ProofsTable>,
}

/// The `[proofs]` table: format 7 names the module's hash `module_sha256`, format 6
/// `executable_sha256`.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ProofsTable {
    #[serde(default)]
    module_sha256: Option<String>,
    #[serde(default)]
    executable_sha256: Option<String>,
    proven: Vec<String>,
    #[serde(default)]
    bounded: Vec<BoundedTable>,
    solvers: String,
    #[serde(default)]
    wrong: Vec<WrongTable>,
    #[serde(default)]
    unproven: Vec<UnprovenTable>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WrongTable {
    id: String,
    found: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct UnprovenTable {
    id: String,
    why: String,
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
    idea: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct IdeaTable {
    sha256: String,
}

/// The `[code]` table: main.wat's names its module and engine; the native lengua's named an
/// executable and a toolchain, a CPU's target, and the emulator that ran its tests, if any.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CodeTable {
    sha256: String,
    #[serde(default)]
    module_sha256: Option<String>,
    #[serde(default)]
    engine: Option<String>,
    #[serde(default)]
    executable_sha256: Option<String>,
    #[serde(default)]
    toolchain: Option<String>,
    max_stack_bytes: u64,
    unsafe_blocks: u32,
    target: String,
    features: Vec<String>,
    #[serde(default)]
    emulator: Option<String>,
}

impl CodeTable {
    /// Whether it stamps code of the native lengua's.
    fn native(&self) -> bool {
        self.target != WASM_TARGET
            || self.executable_sha256.is_some()
            || self.toolchain.is_some()
            || self.emulator.is_some()
    }
}

/// The lock whose body, its signature verified, is `body`. A body exactly as this lengua writes
/// it, as every lock a run reads is, is read line by line with no TOML ([`as_written`]); any other
/// text, a lock of format 6 among them, is read as TOML ([`parse_toml`]), which says what is
/// wrong with one lengua can't take. Either way the lock is the one TOML reads.
pub(crate) fn parse_body(body: &str) -> Result<Lock, LockError> {
    if let Some(lock) = as_written(body) {
        return Ok(lock);
    }
    parse_toml(body)
}

/// [`parse_body`] by TOML: a body of either format this lengua reads, checked for its version,
/// its fields, its dialect, its hashes and fingerprint, and its layout, which must be the one
/// [`Lock::body_in`] writes.
pub(crate) fn parse_toml(body: &str) -> Result<Lock, LockError> {
    let malformed = |detail: &str| LockError::Malformed(detail.to_owned());
    // The body's TOML is parsed once: its version is read from the table, which then gives the
    // whole, so an unsupported version is said before any field lengua doesn't write.
    let table: toml::Table = body.parse().map_err(|_| malformed("it isn't valid TOML"))?;
    let found = match table.get("version") {
        None => None,
        Some(toml::Value::Integer(version)) => Some(*version),
        Some(_) => return Err(malformed("it isn't valid TOML")),
    };
    let version = found.and_then(|version| u32::try_from(version).ok());
    // A format from before main.wat is the native lengua's, whatever else it records.
    if version.is_some_and(|version| (1..OLDEST_VERSION).contains(&version)) {
        return Err(LockError::Native);
    }
    let Some(version) = version.filter(|version| (OLDEST_VERSION..=VERSION).contains(version))
    else {
        return Err(LockError::UnsupportedVersion(
            found.map_or_else(|| "none".to_owned(), |version| version.to_string()),
        ));
    };
    let file: LockFile = table
        .try_into()
        .map_err(|_| malformed("it has fields lengua doesn't write"))?;
    // The native lengua's code, or its dialect beside code, is what this lengua no longer checks.
    if file.code.as_ref().is_some_and(CodeTable::native) {
        return Err(LockError::Native);
    }
    match &file.dialect {
        DialectField::Number(number)
            if version == 6 && *number == FORMAT_6_UNCHECKED_DIALECT && file.code.is_none() => {}
        DialectField::Number(_) => return Err(LockError::Native),
        DialectField::Name(name) if name != WASM_DIALECT => {
            return Err(LockError::UnsupportedWasmDialect(name.clone()));
        }
        DialectField::Name(_) => {}
    }
    let digest = |text: &str| Sha256Hex::parse(text).ok_or_else(|| malformed("a hash is damaged"));
    if file.request.words.is_empty() || file.request.words.len() > MAX_REQUESTS {
        return Err(malformed("its [request] table is damaged"));
    }
    let lock = Lock {
        request: RequestStamp {
            words: file.request.words,
            idea: digest(&file.request.idea)?,
        },
        idea: IdeaStamp {
            sha256: digest(&file.idea.sha256)?,
        },
        code: file
            .code
            .map(|code| {
                let (Some(module), Some(engine)) = (code.module_sha256, code.engine) else {
                    return Err(malformed("its [code] table is damaged"));
                };
                Ok::<_, LockError>(CodeStamp {
                    sha256: digest(&code.sha256)?,
                    module_sha256: digest(&module)?,
                    engine,
                    max_stack_bytes: code.max_stack_bytes,
                    unsafe_blocks: code.unsafe_blocks,
                    features: code.features,
                })
            })
            .transpose()?,
        proofs: file
            .proofs
            .map(|proofs| {
                let built = match (version, proofs.module_sha256, proofs.executable_sha256) {
                    (7, Some(built), None) | (6, None, Some(built)) => built,
                    _ => return Err(malformed("its [proofs] table is damaged")),
                };
                Ok::<_, LockError>(Box::new(ProofStamp {
                    module_sha256: digest(&built)?,
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
                    unproven: proofs
                        .unproven
                        .into_iter()
                        .map(|unproven| (unproven.id, unproven.why))
                        .collect(),
                }))
            })
            .transpose()?,
    };
    if let Some(proofs) = &lock.proofs
        && lock
            .code
            .as_ref()
            .is_none_or(|code| code.module_sha256 != proofs.module_sha256)
    {
        return Err(malformed("its proofs are about a build it doesn't record"));
    }
    let fingerprint = file.fingerprint.as_deref().map(digest).transpose()?;
    if fingerprint != lock.fingerprint() {
        return Err(malformed("its fingerprint doesn't match its hashes"));
    }
    if lock.body_in(version) != body {
        return Err(malformed("it isn't laid out the way lengua writes it"));
    }
    Ok(lock)
}

/// The lock whose body in format [`VERSION`] is exactly `body`, read with no TOML: `None` for any
/// other text. It reads the lines [`Lock::body_in`] writes, in their order, and takes the lock
/// only when `body_in` writes `body` back byte for byte and the checks [`parse_toml`] makes of a
/// lock's content hold: one to [`MAX_REQUESTS`] request words, and proofs only about the module
/// the code's stamp names. So the lines it passes over (the dialect, the fingerprint and the
/// target, which the version and the hashes decide), every string's escapes and every number's
/// digits are lengua's own, and its numbers are within TOML's integers, an `i64`'s. A lock it
/// takes is the very lock [`parse_toml`] takes from the same text, since `body_in` writes TOML
/// that reads back as the lock it was written from.
pub(crate) fn as_written(body: &str) -> Option<Lock> {
    let mut text = Written(body);
    text.eat(HEADER)?;
    text.eat("version = ")?;
    if text.number::<u32>()? != VERSION {
        return None;
    }
    text.eat("\n")?;
    // The dialect, which the version decides.
    text.line()?;
    // The fingerprint, which the hashes decide, once code is stamped.
    if text.0.starts_with("fingerprint = ") {
        text.line()?;
    }
    text.eat("\n[request]\nwords = ")?;
    let words = text.list(Written::string)?;
    text.eat("\nidea = ")?;
    let first = text.sha256()?;
    text.eat("\n\n[idea]\nsha256 = ")?;
    let idea = text.sha256()?;
    text.eat("\n")?;
    let code = if text.eat("\n[code]\nsha256 = ").is_some() {
        let sha256 = text.sha256()?;
        text.eat("\nmodule_sha256 = ")?;
        let module_sha256 = text.sha256()?;
        text.eat("\nengine = ")?;
        let engine = text.string()?;
        text.eat("\nmax_stack_bytes = ")?;
        let max_stack_bytes = text.number()?;
        text.eat("\nunsafe_blocks = ")?;
        let unsafe_blocks = text.number()?;
        text.eat("\n")?;
        // The target, one module for every computer.
        text.line()?;
        text.eat("features = ")?;
        let features = text.list(Written::string)?;
        text.eat("\n")?;
        Some(CodeStamp {
            sha256,
            module_sha256,
            engine,
            max_stack_bytes,
            unsafe_blocks,
            features,
        })
    } else {
        None
    };
    let proofs = if text.eat("\n[proofs]\nmodule_sha256 = ").is_some() {
        let module_sha256 = text.sha256()?;
        text.eat("\nproven = ")?;
        let proven = text.list(Written::string)?;
        text.eat("\n")?;
        let bounded = if text.eat("bounded = ").is_some() {
            let bounded = text.list(Written::bounded)?;
            text.eat("\n")?;
            bounded
        } else {
            Vec::new()
        };
        text.eat("solvers = ")?;
        let solvers = text.string()?;
        text.eat("\n")?;
        let wrong = text.pairs("wrong", "found")?;
        let unproven = text.pairs("unproven", "why")?;
        Some(Box::new(ProofStamp {
            module_sha256,
            proven,
            bounded,
            solvers,
            wrong,
            unproven,
        }))
    } else {
        None
    };
    if !text.0.is_empty() || !(1..=MAX_REQUESTS).contains(&words.len()) {
        return None;
    }
    let about_the_stamp = match (&code, &proofs) {
        (_, None) => true,
        (Some(code), Some(proofs)) => code.module_sha256 == proofs.module_sha256,
        (None, Some(_)) => false,
    };
    let lock = Lock {
        request: RequestStamp { words, idea: first },
        idea: IdeaStamp { sha256: idea },
        code,
        proofs,
    };
    (about_the_stamp && lock.body_in(VERSION) == body).then_some(lock)
}

/// What is left to read of a lock's body, read as [`Lock::body_in`] writes it ([`as_written`]).
/// Each reader takes what it reads from the front and gives `None` when the text there isn't
/// that; then nothing is taken, or what is left is never read again.
struct Written<'b>(&'b str);

impl Written<'_> {
    /// Takes `literal`, when what is left starts with it.
    fn eat(&mut self, literal: &str) -> Option<()> {
        self.0 = self.0.strip_prefix(literal)?;
        Some(())
    }

    /// Takes the rest of a line, its newline too.
    fn line(&mut self) -> Option<()> {
        let (_, rest) = self.0.split_once('\n')?;
        self.0 = rest;
        Some(())
    }

    /// A TOML basic string, on one line, with the escapes [`toml_string`] writes: `\"`, `\\`,
    /// `\n`, `\t`, `\r` and `\u` with four hex digits. Whether they are written as `toml_string`
    /// writes them, [`as_written`]'s comparison says.
    fn string(&mut self) -> Option<String> {
        let mut rest = self.0.strip_prefix('"')?;
        let mut text = String::new();
        loop {
            let mut chars = rest.chars();
            match chars.next()? {
                '"' => {
                    self.0 = chars.as_str();
                    return Some(text);
                }
                '\\' => match chars.next()? {
                    '"' => text.push('"'),
                    '\\' => text.push('\\'),
                    'n' => text.push('\n'),
                    't' => text.push('\t'),
                    'r' => text.push('\r'),
                    'u' => {
                        let after = chars.as_str();
                        let code = u32::from_str_radix(after.get(..4)?, 16).ok()?;
                        text.push(char::from_u32(code)?);
                        chars = after.get(4..)?.chars();
                    }
                    _ => return None,
                },
                '\n' => return None,
                other => text.push(other),
            }
            rest = chars.as_str();
        }
    }

    /// A SHA-256, as a string of its 64 lowercase hex digits.
    fn sha256(&mut self) -> Option<Sha256Hex> {
        let (digits, rest) = self.0.strip_prefix('"')?.split_once('"')?;
        self.0 = rest;
        Sha256Hex::parse(digits)
    }

    /// A whole number in decimal digits, within TOML's integers, an `i64`'s, and within `T`.
    fn number<T: TryFrom<i64>>(&mut self) -> Option<T> {
        let end = self
            .0
            .bytes()
            .position(|byte| !byte.is_ascii_digit())
            .unwrap_or(self.0.len());
        let (digits, rest) = self.0.split_at_checked(end)?;
        self.0 = rest;
        T::try_from(digits.parse::<i64>().ok()?).ok()
    }

    /// `true` or `false`.
    fn boolean(&mut self) -> Option<bool> {
        if self.eat("true").is_some() {
            Some(true)
        } else {
            self.eat("false").map(|()| false)
        }
    }

    /// A list on one line, `[]` or `[a, b]`, of what `item` reads.
    fn list<T>(&mut self, mut item: impl FnMut(&mut Self) -> Option<T>) -> Option<Vec<T>> {
        self.eat("[")?;
        let mut items = Vec::new();
        if self.eat("]").is_some() {
            return Some(items);
        }
        loop {
            items.push(item(&mut *self)?);
            if self.eat("]").is_some() {
                return Some(items);
            }
            self.eat(", ")?;
        }
    }

    /// One of `[proofs]`'s `bounded` tables: `{ id = "B1", bytes = 4096, of = 5000, more = true }`.
    fn bounded(&mut self) -> Option<Bounded> {
        self.eat("{ id = ")?;
        let id = self.string()?;
        self.eat(", bytes = ")?;
        let bytes = self.number()?;
        self.eat(", of = ")?;
        let of = self.number()?;
        self.eat(", more = ")?;
        let more = self.boolean()?;
        self.eat(" }")?;
        Some(Bounded {
            id,
            bytes,
            of,
            more,
        })
    }

    /// `[proofs]`'s list `key` of tables of an id and its `field`, such as `wrong = [{ id = "B2",
    /// found = "…" }]`, on a line of its own: none when that line isn't there.
    fn pairs(&mut self, key: &str, field: &str) -> Option<Vec<(String, String)>> {
        if self.eat(key).is_none() {
            return Some(Vec::new());
        }
        self.eat(" = ")?;
        let pairs = self.list(|text| {
            text.eat("{ id = ")?;
            let id = text.string()?;
            text.eat(", ")?;
            text.eat(field)?;
            text.eat(" = ")?;
            let value = text.string()?;
            text.eat(" }")?;
            Some((id, value))
        })?;
        self.eat("\n")?;
        Some(pairs)
    }
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
    /// The file uses a lock format this server doesn't read: a later lengua's, or one no lengua
    /// wrote.
    UnsupportedVersion(String),
    /// The file records main.s, the native lengua's code, which this lengua doesn't check: its
    /// stamp, its dialect, or a format from before main.wat, which only the native lengua wrote.
    Native,
    /// The file's main.wat was checked against a WebAssembly dialect this server doesn't check.
    UnsupportedWasmDialect(String),
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
                "it uses lock format version {version}; this lengua reads versions \
                 {OLDEST_VERSION} to {VERSION}"
            ),
            Self::Native => f.write_str(
                "it records main.s, which the native lengua checked: lengua compiles an idea to \
                 main.wat since 0.2.0, and the native lengua is kept at the tag native-final",
            ),
            Self::UnsupportedWasmDialect(dialect) => write!(
                f,
                "its main.wat was checked against the dialect {dialect}; this lengua checks \
                 {WASM_DIALECT}"
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
    // The file's size, as the open file gives it, makes room for it at once: one read takes it
    // all, where a read with no room doubles its buffer, and reads again, from 32 bytes up.
    let size = file
        .metadata()
        .map_or(0, |metadata| metadata.len())
        .min(MAX_LOCK_BYTES + 1);
    let mut bytes = Vec::with_capacity(usize::try_from(size).unwrap_or(0));
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
                "the signing key at {} is damaged; delete it, then record each program's idea \
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

/// Makes the folder `path`, with its parents, readable only by this user where the system
/// says who may read a folder.
#[cfg(unix)]
pub(crate) fn create_private_dir(path: &Path) -> io::Result<()> {
    use std::os::unix::fs::DirBuilderExt as _;
    std::fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(path)
}

/// Makes the folder `path`, with its parents.
#[cfg(not(unix))]
pub(crate) fn create_private_dir(path: &Path) -> io::Result<()> {
    std::fs::create_dir_all(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(byte: u8) -> Key {
        Key::from_bytes([byte; KEY_LEN])
    }

    /// A lock with main.wat's stamp, as `check` writes it once its tests pass.
    fn full_lock() -> Lock {
        Lock {
            request: RequestStamp {
                words: vec![
                    "Make me a program that adds two numbers".into(),
                    "Also take \"negative\" numbers".into(),
                ],
                idea: Sha256Hex::of(b"first idea"),
            },
            idea: IdeaStamp {
                sha256: Sha256Hex::of(b"idea"),
            },
            code: Some(CodeStamp {
                sha256: Sha256Hex::of(b"main.wat"),
                module_sha256: Sha256Hex::of(b"module"),
                engine: "wasmtime 50.0.1 \"quoted\" \\ tab\t".into(),
                max_stack_bytes: 1184,
                unsafe_blocks: 1,
                features: vec!["bulk-memory".into(), "simd".into()],
            }),
            proofs: None,
        }
    }

    /// The proofs of the full lock's module: B1 proven.
    fn proofs() -> Box<ProofStamp> {
        Box::new(ProofStamp {
            module_sha256: Sha256Hex::of(b"module"),
            proven: vec!["B1".into()],
            bounded: Vec::new(),
            solvers: "z3 and cvc5".into(),
            wrong: Vec::new(),
            unproven: Vec::new(),
        })
    }

    /// Signs arbitrary body text, as if a server had written it.
    fn forge(body: &str, key: &Key) -> String {
        sign_body(body, key)
    }

    #[test]
    fn round_trips_a_idea_only_lock() {
        let lock = Lock::requested(Sha256Hex::of(b"idea"), "Make it say \"hi\"\nplease".into());
        let text = lock.to_signed_text(&key(1));
        assert!(text.starts_with(HEADER), "{text}");
        assert!(!text.contains("fingerprint"), "{text}");
        assert!(
            text.contains("version = 7\ndialect = \"webassembly 1\"\n"),
            "{text}"
        );
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
        let mut stamp = proofs();
        stamp.proven = vec!["B1".into(), "B\"2".into()];
        lock.proofs = Some(stamp);
        let text = lock.to_signed_text(&key(1));
        assert!(
            text.contains(&format!(
                "\n[proofs]\nmodule_sha256 = \"{}\"\nproven = [\"B1\", \"B\\\"2\"]\n",
                Sha256Hex::of(b"module")
            )),
            "{text}"
        );
        assert!(!text.contains("executable_sha256"), "{text}");
        assert!(!text.contains("bounded"), "{text}");
        assert!(!text.contains("unproven"), "{text}");
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
        // Why a test is tested, not proven is recorded too, so a check after a restart says it.
        let mut unproven = lock.clone();
        if let Some(proofs) = &mut unproven.proofs {
            proofs.unproven = vec![("B4".into(), "the proof ran out of time".into())];
        }
        let unproven_text = unproven.to_signed_text(&key(1));
        assert!(
            unproven_text
                .contains("unproven = [{ id = \"B4\", why = \"the proof ran out of time\" }]\n"),
            "{unproven_text}"
        );
        assert_eq!(
            Lock::from_signed_text(&unproven_text, &key(1)),
            Ok(unproven)
        );
        // Proofs about some other build can't be in a lock lengua wrote.
        let other = text
            .split("\n[signature]")
            .next()
            .expect("a body")
            .replacen(
                &format!("module_sha256 = \"{}\"\nproven", Sha256Hex::of(b"module")),
                &format!("module_sha256 = \"{}\"\nproven", Sha256Hex::of(b"other")),
                1,
            );
        assert!(matches!(
            Lock::from_signed_text(&forge(&other, &key(1)), &key(1)),
            Err(LockError::Malformed(_))
        ));
    }

    /// main.wat's stamp names a module and an engine, and the lock the WebAssembly dialect.
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
            text.contains(&format!(
                "\n[code]\nsha256 = \"{}\"\nmodule_sha256 = \"{}\"\nengine = \"wasmtime 50.0.1 \
                 \\\"quoted\\\" \\\\ tab\\t\"\nmax_stack_bytes = 1184\nunsafe_blocks = 1\ntarget = \
                 \"webassembly\"\nfeatures = [\"bulk-memory\", \"simd\"]\n",
                Sha256Hex::of(b"main.wat"),
                Sha256Hex::of(b"module")
            )),
            "{text}"
        );
        assert!(!text.contains("toolchain"), "{text}");
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

    /// A lock of format 6, as 0.2.0 wrote it, still reads: its stamp of main.wat stays checked,
    /// its proofs name the module `executable_sha256`, and one whose code wasn't checked yet says
    /// `dialect = 4`. lengua writes each again as format 7.
    #[test]
    fn a_lock_of_format_6_still_reads() {
        let lock = Lock {
            proofs: Some(proofs()),
            ..full_lock()
        };
        let six = lock.body_in(6);
        assert!(
            six.contains("version = 6\ndialect = \"webassembly 1\"\n"),
            "{six}"
        );
        assert!(
            six.contains(&format!(
                "\n[proofs]\nexecutable_sha256 = \"{}\"\n",
                Sha256Hex::of(b"module")
            )),
            "{six}"
        );
        let read = Lock::from_signed_text(&forge(&six, &key(1)), &key(1));
        assert_eq!(read, Ok(lock.clone()));
        let again = lock.to_signed_text(&key(1));
        assert!(again.contains("version = 7\n"), "{again}");
        assert!(
            again.contains("\n[proofs]\nmodule_sha256 = ") && !again.contains("executable_sha256"),
            "{again}"
        );
        let unchecked = Lock::requested(Sha256Hex::of(b"idea"), "an adder".into());
        let six = unchecked.body_in(6);
        assert!(six.contains("version = 6\ndialect = 4\n"), "{six}");
        let read = Lock::from_signed_text(&forge(&six, &key(1)), &key(1));
        assert_eq!(read, Ok(unchecked));
        // Each format names the module's hash in [proofs] its own way only.
        let mixed = lock.body().replace("version = 7", "version = 6");
        assert!(matches!(
            Lock::from_signed_text(&forge(&mixed, &key(1)), &key(1)),
            Err(LockError::Malformed(_))
        ));
    }

    /// A lock that records the native lengua's code, its stamp of main.s or its dialect beside
    /// code, or is of a format from before main.wat, which only the native lengua wrote, can't be
    /// used, and says where the native lengua is.
    #[test]
    fn a_lock_of_the_native_lengua_is_stale() {
        let body = full_lock().body_in(6);
        let native = body
            .replace("dialect = \"webassembly 1\"", "dialect = 4")
            .replace("module_sha256 = ", "executable_sha256 = ")
            .replace("engine = ", "toolchain = ")
            .replace("target = \"webassembly\"", "target = \"linux-x86_64\"");
        let cases = [
            native.clone(),
            native.replace("version = 6", "version = 7"),
            native.replace(
                "features = [\"bulk-memory\", \"simd\"]\n",
                "features = [\"bulk-memory\", \"simd\"]\nemulator = \"qemu-aarch64 10.2.2\"\n",
            ),
            body.replace("dialect = \"webassembly 1\"", "dialect = 4"),
            full_lock()
                .body()
                .replace("target = \"webassembly\"", "target = \"linux-aarch64\""),
            // Version 5 came before main.wat, version 4 named the program's spec.md, and 1 was
            // the first.
            body.replace("version = 6", "version = 5"),
            body.replace("version = 6", "version = 4"),
            body.replace("version = 6", "version = 1"),
        ];
        for body in cases {
            assert_eq!(
                Lock::from_signed_text(&forge(&body, &key(1)), &key(1)),
                Err(LockError::Native),
                "{body}"
            );
        }
        let said = LockError::Native.to_string();
        assert!(
            said.contains("main.wat since 0.2.0") && said.contains("native-final"),
            "{said}"
        );
    }

    #[test]
    fn unknown_versions_and_dialects_are_reported() {
        let body = full_lock().body();
        // No lengua wrote a format 0, and 8 is a later lengua's.
        for unknown in ["0", "8"] {
            let text = forge(
                &body.replace("version = 7", &format!("version = {unknown}")),
                &key(1),
            );
            assert_eq!(
                Lock::from_signed_text(&text, &key(1)),
                Err(LockError::UnsupportedVersion(unknown.into()))
            );
        }
        let later = forge(
            &body.replace("\"webassembly 1\"", "\"webassembly 2\""),
            &key(1),
        );
        assert_eq!(
            Lock::from_signed_text(&later, &key(1)),
            Err(LockError::UnsupportedWasmDialect("webassembly 2".into()))
        );
        // A later version is a later lengua's whatever fields it writes: its version is said
        // before any field this lengua doesn't write.
        let newer = body
            .replace("version = 7", "version = 8")
            .replace("[idea]", "[idea]\nextra = 1");
        assert_eq!(
            Lock::from_signed_text(&forge(&newer, &key(1)), &key(1)),
            Err(LockError::UnsupportedVersion("8".into()))
        );
    }

    #[test]
    fn signed_but_unexpected_content_is_rejected() {
        let lock = full_lock();
        let body = lock.body();
        let cases = [
            body.replace("[idea]", "[idea]\nextra = 1"),
            body.replace("max_stack_bytes = 1184", "max_stack_bytes = 1184 # note"),
            body.replace("\n\n[idea]", "\n\n\n[idea]"),
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
            body.replace("[\"bulk-memory\", \"simd\"]", "[\"bulk-memory\",\"simd\"]"),
            body.replace("target = \"webassembly\"\n", ""),
            body.replace(
                "engine = \"wasmtime 50.0.1 \\\"quoted\\\" \\\\ tab\\t\"\n",
                "",
            ),
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

    /// A lock exactly as lengua writes it reads with no TOML, as the very lock TOML reads from the
    /// same text: with no code, with code, with proofs of every kind, with words of every escape
    /// and of any character, and with numbers as large as TOML's integers. Any other text is
    /// TOML's to read or to refuse: another layout, digits lengua doesn't write, a number past
    /// TOML's integers, another dialect, an escape lengua doesn't write, a lock of format 6, and a
    /// body with a byte more or less.
    #[test]
    fn a_lock_as_lengua_writes_it_reads_without_toml() {
        let mut proven = Lock {
            proofs: Some(proofs()),
            ..full_lock()
        };
        if let Some(stamp) = &mut proven.proofs {
            stamp.bounded = vec![
                Bounded {
                    id: "B3".into(),
                    bytes: 4096,
                    of: 5000,
                    more: true,
                },
                Bounded {
                    id: "B4".into(),
                    bytes: 0,
                    of: 0,
                    more: false,
                },
            ];
            stamp.wrong = vec![(
                "B2".into(),
                "with n = 1234567: it printed \"-1234566\"".into(),
            )];
            stamp.unproven = vec![
                ("B5".into(), "the proof ran out of time".into()),
                ("B6".into(), String::new()),
            ];
        }
        let mut odd = full_lock();
        odd.request.words = vec![
            "tab\t, quote \", backslash \\, newline\n, return\r".into(),
            "\u{0}\u{1f}\u{7f}\u{80}\u{9f} é \u{1f600} \\u0041 \\\" [] , { }".into(),
            String::new(),
        ];
        let mut largest = full_lock();
        if let Some(code) = &mut largest.code {
            code.max_stack_bytes = u64::try_from(i64::MAX).expect("an i64's largest is a u64");
            code.unsafe_blocks = u32::MAX;
            code.features = Vec::new();
        }
        let locks = [
            Lock::requested(Sha256Hex::of(b"idea"), "an adder".into()),
            full_lock(),
            Lock {
                proofs: Some(proofs()),
                ..full_lock()
            },
            proven,
            odd,
            largest,
        ];
        for lock in locks {
            let body = lock.body();
            assert_eq!(as_written(&body).as_ref(), Some(&lock), "{body}");
            assert_eq!(parse_toml(&body).as_ref(), Ok(&lock), "{body}");
            assert_eq!(parse_body(&body).as_ref(), Ok(&lock), "{body}");
        }
        let body = full_lock().body();
        let past = body.replace(
            "max_stack_bytes = 1184",
            "max_stack_bytes = 9223372036854775808",
        );
        let others = [
            body.replace("\n\n[idea]", "\n\n\n[idea]"),
            body.replace("max_stack_bytes = 1184", "max_stack_bytes = 01184"),
            body.replace("max_stack_bytes = 1184", "max_stack_bytes = +1184"),
            past.clone(),
            body.replace("\"webassembly 1\"", "\"webassembly 2\""),
            body.replace("\"bulk-memory\"", "\"bulk\\u002Dmemory\""),
            full_lock().body_in(6),
            format!("{body}\n"),
            body[..body.len() - 1].to_owned(),
        ];
        for other in others {
            assert_eq!(as_written(&other), None, "{other}");
        }
        assert!(matches!(parse_body(&past), Err(LockError::Malformed(_))));
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

    /// SHA-256 of `parts`, one after another, as bytes.
    fn sha256(parts: &[&[u8]]) -> Vec<u8> {
        unhex(Sha256Hex::of(&parts.concat()).as_str()).expect("a digest is hex")
    }

    /// `Key::sign` is HMAC-SHA256, keyed with the 32-byte key, over the signature's domain and
    /// then the body, as RFC 2104 defines HMAC from SHA-256 alone: SHA-256 of the key padded with
    /// zeros to SHA-256's 64-byte block and xored with 0x5c, then of the inner hash, SHA-256 of
    /// the padded key xored with 0x36, then the domain, then the body. Every lock on a user's
    /// computer verifies only while that stays so, whichever crates compute it. `verifies` takes
    /// the tag, and refuses it with any one bit changed.
    #[test]
    fn a_signature_is_hmac_sha256_over_the_domain_and_the_body() {
        let body = "# lengua.lock: a body\nversion = 7\n";
        let mut padded = [0_u8; 64];
        padded[..KEY_LEN].copy_from_slice(&[1; KEY_LEN]);
        let pad = |byte: u8| padded.map(|each| each ^ byte);
        let inner = sha256(&[&pad(0x36), SIGNATURE_DOMAIN, body.as_bytes()]);
        let tag = sha256(&[&pad(0x5c), &inner]);
        let signed = key(1).sign(body);
        assert_eq!(signed.as_str(), hex(&tag));
        assert!(key(1).verifies(body, &signed));
        for bit in [0, 7, 100, 255] {
            let mut flipped = tag.clone();
            flipped[bit / 8] ^= 1 << (bit % 8);
            let flipped = Sha256Hex::parse(&hex(&flipped)).expect("a tag is hex");
            assert!(!key(1).verifies(body, &flipped), "bit {bit}");
        }
    }

    /// The MAC lengua signs with gives the standard's own answer: RFC 4231's test case 1 for
    /// HMAC-SHA256, a key of 20 bytes 0x0b over "Hi There". (SHA-256's own, of "abc", is
    /// src/hash.rs's.)
    #[test]
    fn hmac_sha256_gives_rfc_4231_s_first_answer() {
        let mut mac = <Hmac<Sha256> as KeyInit>::new_from_slice(&[0x0b; 20])
            .expect("HMAC takes a key of any length");
        mac.update(b"Hi There");
        assert_eq!(
            hex(&mac.finalize().into_bytes()),
            "b0344c61d8db38535ca8afceaf0bf12b881dc200c9833da726e9376c2e32cff7"
        );
    }
}
