//! Calls between modules, which an idea declares in `# Constraints`:
//!
//! - `- Calls: shapes.wasm, built from Rust with rustc; its functions: area(f64, f64) -> f64`:
//!   the program calls functions of an outside module, a WebAssembly module that a compiler made
//!   from another language, which stands beside main.wat as `shapes.wasm`. lengua didn't build
//!   it and proves nothing of it: it is trusted, not proven.
//! - `- Calls: [shapes](../shapes/idea.md); its functions: area(f64, f64) -> f64`: the program
//!   calls functions of another lengua program of the same bundle, which lengua checked on this
//!   computer and whose idea offers them with an `- Exports:` line.
//! - `- Exports: area(f64, f64) -> f64`: the functions the program's main.wat exports for
//!   another program to call.
//!
//! A called module runs as an instance of its own beside the program, with a memory of its own
//! and no function of lengua's host. Numbers pass in a call; bytes pass by copy, through lengua's
//! `give` and `take` ([`crate::wasm::host::call`]). main.wat imports each function it calls from
//! the module named as the called module is: `(import "shapes" "area" (func $area (param f64
//! f64) (result f64)))`.

use serde::{Deserialize, Serialize};

/// The most modules an idea may call: each is an instance with a memory of its own, of up to
/// 1 GiB, for as long as the program runs.
pub const MAX_MODULES: usize = 4;

/// The most functions a `- Calls:` line or an `- Exports:` line may name.
pub const MAX_FUNCTIONS: usize = 32;

/// The most parameters a function that crosses between modules may take.
pub const MAX_PARAMS: usize = 8;

/// The most characters a called module's name may have.
pub const MAX_NAME: usize = 32;

/// The most characters the name of a function that crosses between modules may have.
pub const MAX_FUNCTION_NAME: usize = 64;

/// What says in every reply that a called module is an outside module.
pub const TRUSTED: &str = "trusted, not proven";

/// What says in every reply that a called module is a lengua program checked on this computer.
pub const CHECKED: &str = "checked";

/// How an idea writes a `- Calls:` line.
pub const CALLS_USAGE: &str = "write `- Calls: shapes.wasm, built from Rust with rustc; its \
                               functions: area(f64, f64) -> f64, count() -> i32` for an outside \
                               module that stands beside main.wat, or `- Calls: \
                               [shapes](../shapes/idea.md); its functions: area(f64, f64) -> f64` \
                               for a lengua program of the same bundle. A function takes up to 8 \
                               numbers, each `i32`, `i64`, `f32` or `f64`, and gives one or none";

/// How an idea writes an `- Exports:` line.
pub const EXPORTS_USAGE: &str = "write `- Exports: area(f64, f64) -> f64, count() -> i32`: each \
                                 function main.wat exports for another program to call, with \
                                 the numbers it takes, each `i32`, `i64`, `f32` or `f64`, up to \
                                 8, and the one it gives, or none";

/// The type of a number that crosses between modules.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Type {
    /// 32 bits, a whole number.
    I32,
    /// 64 bits, a whole number.
    I64,
    /// 32 bits, a float.
    F32,
    /// 64 bits, a float.
    F64,
}

impl Type {
    /// Its name, as WebAssembly text writes it.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::I32 => "i32",
            Self::I64 => "i64",
            Self::F32 => "f32",
            Self::F64 => "f64",
        }
    }

    /// The type `text` names.
    #[must_use]
    pub fn read(text: &str) -> Option<Self> {
        match text.trim() {
            "i32" => Some(Self::I32),
            "i64" => Some(Self::I64),
            "f32" => Some(Self::F32),
            "f64" => Some(Self::F64),
            _ => None,
        }
    }
}

/// A function that crosses between modules: its name as the module exports it, the numbers it
/// takes and the one it gives.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Signature {
    /// Its name, as the module that defines it exports it.
    pub name: String,
    /// The numbers it takes.
    pub params: Vec<Type>,
    /// The number it gives, when it gives one.
    pub result: Option<Type>,
}

impl Signature {
    /// The function as an idea's line writes it: `area(f64, f64) -> f64`.
    #[must_use]
    pub fn shown(&self) -> String {
        let params: Vec<&str> = self.params.iter().map(|ty| ty.name()).collect();
        let result = self
            .result
            .map_or_else(String::new, |ty| format!(" -> {}", ty.name()));
        format!("{}({}){result}", self.name, params.join(", "))
    }

    /// Its type as WebAssembly text writes it after a function's name: ` (param f64 f64)
    /// (result f64)`, with nothing for no parameter or no result.
    #[must_use]
    pub fn wat_type(&self) -> String {
        let mut text = String::new();
        if !self.params.is_empty() {
            let params: Vec<&str> = self.params.iter().map(|ty| ty.name()).collect();
            text.push_str(&format!(" (param {})", params.join(" ")));
        }
        if let Some(result) = self.result {
            text.push_str(&format!(" (result {})", result.name()));
        }
        text
    }
}

/// What a called module is.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Kind {
    /// An outside module, `<name>.wasm` beside main.wat, with what the idea says it was built
    /// from, in its own words: trusted, not proven.
    Outside {
        /// What the idea says the module was built from. lengua records the words and can't
        /// check them.
        built_from: String,
    },
    /// A lengua program of the same bundle, by the link to its idea.md, relative to the folder
    /// of the idea that calls it: checked on this computer.
    Program {
        /// The link to its idea.md, as written: `../shapes/idea.md`.
        idea: String,
    },
}

/// A module an idea's `- Calls:` line names, and the functions of it the program may call.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Called {
    /// Its name: what main.wat imports its functions from. An outside module's file is this name
    /// with `.wasm`; a program's folder has this name.
    pub name: String,
    /// What it is.
    pub kind: Kind,
    /// The functions of it the program may call.
    pub functions: Vec<Signature>,
}

/// Whether `name` may name a called module: a lowercase letter, then lowercase letters, digits,
/// `-` and `_`, up to [`MAX_NAME`] characters, and none of the names lengua and WASI keep.
#[must_use]
pub fn module_name(name: &str) -> bool {
    let mut characters = name.chars();
    let first = characters.next().is_some_and(|c| c.is_ascii_lowercase());
    first
        && name.len() <= MAX_NAME
        && characters.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
        && !name.starts_with("wasi")
        && !name.starts_with("lengua")
        && name != "main"
        && name != "env"
}

/// Whether `name` may name a function that crosses between modules: a letter or `_`, then
/// letters, digits and `_`, up to [`MAX_FUNCTION_NAME`] characters, and none of the names a
/// module's other exports have.
#[must_use]
pub fn function_name(name: &str) -> bool {
    let mut characters = name.chars();
    let first = characters
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_');
    first
        && name.len() <= MAX_FUNCTION_NAME
        && characters.all(|c| c.is_ascii_alphanumeric() || c == '_')
        && !matches!(name, "_start" | "_initialize" | "memory")
        && !name.starts_with("lengua")
        && !name.starts_with("__")
}

/// The functions `text` lists, as `area(f64, f64) -> f64, count() -> i32`: `usage` says how the
/// line is written.
fn functions(text: &str, usage: &str) -> Result<Vec<Signature>, String> {
    let cleaned = text.replace('`', "");
    let mut rest = cleaned.trim().trim_end_matches('.').trim();
    let mut found: Vec<Signature> = Vec::new();
    if rest.is_empty() {
        return Err(format!("the line names no function; {usage}"));
    }
    loop {
        let open = rest
            .find('(')
            .ok_or_else(|| format!("`{rest}` isn't a function with its numbers; {usage}"))?;
        let name = rest[..open].trim();
        if !function_name(name) {
            return Err(format!(
                "`{name}` can't name a function that another module calls: a name is letters, \
                 digits and `_`, starts with no digit, with no `__` and not `lengua`, and isn't \
                 `_start`, `_initialize` or `memory`; {usage}"
            ));
        }
        let close = rest[open..]
            .find(')')
            .map(|at| open + at)
            .ok_or_else(|| format!("`{name}(` has no `)`; {usage}"))?;
        let mut params = Vec::new();
        for param in rest[open + 1..close]
            .split(',')
            .map(str::trim)
            .filter(|param| !param.is_empty())
        {
            let ty = Type::read(param)
                .ok_or_else(|| format!("`{param}` isn't a number's type, in `{name}`; {usage}"))?;
            params.push(ty);
        }
        if params.len() > MAX_PARAMS {
            return Err(format!(
                "`{name}` takes {} numbers, and a function that crosses between modules takes at \
                 most {MAX_PARAMS}",
                params.len()
            ));
        }
        rest = rest[close + 1..].trim_start();
        let result = match rest.strip_prefix("->") {
            Some(after) => {
                let after = after.trim_start();
                let end = after
                    .find(|c: char| !c.is_ascii_alphanumeric())
                    .unwrap_or(after.len());
                let ty = Type::read(&after[..end]).ok_or_else(|| {
                    format!("`{name}` gives something that isn't one number; {usage}")
                })?;
                rest = after[end..].trim_start();
                Some(ty)
            }
            None => None,
        };
        if found.iter().any(|function| function.name == name) {
            return Err(format!("the function `{name}` is listed twice"));
        }
        if found.len() == MAX_FUNCTIONS {
            return Err(format!(
                "the line names more than {MAX_FUNCTIONS} functions, the most one line may"
            ));
        }
        found.push(Signature {
            name: name.to_owned(),
            params,
            result,
        });
        if rest.is_empty() {
            return Ok(found);
        }
        let after = rest
            .strip_prefix(',')
            .map(str::trim_start)
            .or_else(|| rest.strip_prefix("and "))
            .ok_or_else(|| format!("`{rest}` follows `{name}`, and isn't a function; {usage}"))?;
        rest = after.strip_prefix("and ").unwrap_or(after).trim_start();
        if rest.is_empty() {
            return Err(format!("a function is missing after `{name}`; {usage}"));
        }
    }
}

/// The functions an `- Exports:` line lists, after `- Exports:`.
///
/// # Errors
///
/// What's wrong with the words, and what to write instead.
pub fn exports(rest: &str) -> Result<Vec<Signature>, String> {
    functions(rest, EXPORTS_USAGE)
}

impl Called {
    /// The module a line's words name, after `- Calls:`.
    ///
    /// # Errors
    ///
    /// What's wrong with the words, and what to write instead.
    pub fn read(rest: &str) -> Result<Self, String> {
        let rest = rest.trim();
        let lower = rest.to_ascii_lowercase();
        let marker = "; its functions";
        let at = lower
            .find(marker)
            .ok_or_else(|| format!("the line doesn't say `; its functions: …`; {CALLS_USAGE}"))?;
        let head = rest[..at].trim();
        let tail = rest[at + marker.len()..].trim_start();
        let tail = tail.strip_prefix(':').unwrap_or(tail);
        let functions = functions(tail, CALLS_USAGE)?;
        let (name, kind) = if let Some(link) = head.strip_prefix('[') {
            let target = link
                .split_once("](")
                .and_then(|(_, after)| after.strip_suffix(')'))
                .map(str::trim)
                .ok_or_else(|| format!("`{head}` isn't a link to an idea; {CALLS_USAGE}"))?;
            let portable = !target.starts_with('/')
                && !target.contains("://")
                && !target.contains('\\')
                && target.ends_with("idea.md");
            if !portable {
                return Err(format!(
                    "the program `{head}` is named by a link to its idea, by a path relative to \
                     this one that ends in idea.md, such as `[shapes](../shapes/idea.md)`"
                ));
            }
            let folder = crate::idea::folder_of_link(target);
            let name = folder.rsplit('/').next().unwrap_or_default().to_owned();
            (
                name,
                Kind::Program {
                    idea: target.to_owned(),
                },
            )
        } else {
            let (file, after) = head
                .split_once(',')
                .ok_or_else(|| format!("`{head}` doesn't say `built from …`; {CALLS_USAGE}"))?;
            let name = file
                .trim()
                .strip_suffix(".wasm")
                .ok_or_else(|| {
                    format!(
                        "`{}` isn't a module's file, `<name>.wasm`; {CALLS_USAGE}",
                        file.trim()
                    )
                })?
                .to_owned();
            let words = after.trim();
            let built_from = words
                .get(.."built from ".len())
                .filter(|prefix| prefix.eq_ignore_ascii_case("built from "))
                .map(|_| words["built from ".len()..].trim())
                .filter(|words| {
                    !words.is_empty()
                        && words.chars().count() <= crate::idea::MAX_BUILT_FROM
                        && !words.chars().any(char::is_control)
                })
                .ok_or_else(|| {
                    format!(
                        "after `{name}.wasm,` the line says `built from …`, with the language \
                         and the compiler in up to {} characters; {CALLS_USAGE}",
                        crate::idea::MAX_BUILT_FROM
                    )
                })?;
            (
                name,
                Kind::Outside {
                    built_from: built_from.to_owned(),
                },
            )
        };
        if !module_name(&name) {
            return Err(format!(
                "`{name}` can't name a called module: a name is lowercase letters, digits, `-` \
                 and `_`, starts with a letter, has up to {MAX_NAME} characters, and isn't \
                 `main`, `env` or a name that starts with `wasi` or `lengua`"
            ));
        }
        Ok(Self {
            name,
            kind,
            functions,
        })
    }

    /// Whether it is a lengua program checked on this computer; otherwise it is an outside
    /// module, trusted and not proven.
    #[must_use]
    pub const fn checked(&self) -> bool {
        matches!(self.kind, Kind::Program { .. })
    }

    /// An outside module's file beside main.wat, `<name>.wasm`; a program's folder, relative to
    /// the folder of the idea that calls it.
    #[must_use]
    pub fn place(&self) -> String {
        match &self.kind {
            Kind::Outside { .. } => format!("{}.wasm", self.name),
            Kind::Program { idea } => crate::idea::folder_of_link(idea),
        }
    }

    /// The function `name` of it that the idea lets the program call.
    #[must_use]
    pub fn function(&self, name: &str) -> Option<&Signature> {
        self.functions.iter().find(|function| function.name == name)
    }

    /// Its functions as a line lists them.
    #[must_use]
    pub fn listed(&self) -> String {
        let shown: Vec<String> = self.functions.iter().map(Signature::shown).collect();
        shown.join(", ")
    }

    /// The call in words, for the user who is about to run the program.
    #[must_use]
    pub fn describe(&self) -> String {
        match &self.kind {
            Kind::Outside { built_from } => format!(
                "it calls {}.wasm, an outside module that is {TRUSTED}, which its idea says was \
                 built from {built_from}: the module runs beside the program with a memory of \
                 its own and reaches nothing of this computer",
                self.name
            ),
            Kind::Program { .. } => format!(
                "it calls the lengua program {}, which lengua {CHECKED} on this computer: it \
                 runs beside the program with a memory of its own and reaches nothing of this \
                 computer",
                self.place()
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A line names an outside module with what it was built from, or a program by its link,
    /// and each function with its numbers.
    #[test]
    fn a_line_names_a_module_and_its_functions() {
        let outside = Called::read(
            "shapes.wasm, built from Rust with rustc 1.90; its functions: `area(f64, f64) -> \
             f64`, count() -> i32 and reset()",
        )
        .expect("an outside module");
        assert_eq!(outside.name, "shapes");
        assert_eq!(
            outside.kind,
            Kind::Outside {
                built_from: "Rust with rustc 1.90".to_owned()
            }
        );
        assert!(!outside.checked());
        assert_eq!(outside.place(), "shapes.wasm");
        assert_eq!(
            outside.listed(),
            "area(f64, f64) -> f64, count() -> i32, reset()"
        );
        assert_eq!(
            outside.function("area").map(Signature::wat_type),
            Some(" (param f64 f64) (result f64)".to_owned())
        );
        assert_eq!(
            outside.function("reset").map(Signature::wat_type),
            Some(String::new())
        );
        assert!(outside.describe().contains(TRUSTED));
        let program =
            Called::read("[shapes](../shapes/idea.md); its functions: total(i64, i32) -> i64.")
                .expect("a program");
        assert_eq!(program.name, "shapes");
        assert!(program.checked());
        assert_eq!(program.place(), "../shapes");
        assert!(program.describe().contains(CHECKED));
    }

    /// A line in other words, a name lengua keeps, a type that is no number's and a function
    /// listed twice are refused, each with what to write.
    #[test]
    fn a_line_in_other_words_is_refused() {
        for line in [
            "shapes.wasm",
            "shapes.wasm; its functions: area() -> f64",
            "shapes.wasm, made in Rust; its functions: area() -> f64",
            "shapes.wasm, built from Rust; its functions:",
            "shapes.wasm, built from Rust; its functions: area",
            "shapes.wasm, built from Rust; its functions: area(v128)",
            "shapes.wasm, built from Rust; its functions: area() -> f64 f64",
            "shapes.wasm, built from Rust; its functions: area(), area()",
            "shapes.wasm, built from Rust; its functions: _start()",
            "shapes.wasm, built from Rust; its functions: memory()",
            "shapes.wasm, built from Rust; its functions: lengua_stop_reached()",
            "shapes.wasm, built from Rust; its functions: f(i32, i32, i32, i32, i32, i32, i32, \
             i32, i32)",
            "main.wasm, built from Rust; its functions: area()",
            "wasi_snapshot_preview1.wasm, built from Rust; its functions: fd_write()",
            "lengua_call_1.wasm, built from Rust; its functions: give()",
            "Shapes.wasm, built from Rust; its functions: area()",
            "[shapes](/abs/shapes/idea.md); its functions: area()",
            "[shapes](../shapes/main.wat); its functions: area()",
            "[shapes](../shapes/idea.md)",
        ] {
            let refused = Called::read(line);
            assert!(refused.is_err(), "{line}: {refused:?}");
        }
    }

    /// An `- Exports:` line lists functions as a `- Calls:` line does.
    #[test]
    fn an_exports_line_lists_functions() {
        let found = exports("`text() -> i32`, `crc32(i32) -> i32`").expect("two functions");
        assert_eq!(found.len(), 2);
        assert_eq!(found[1].shown(), "crc32(i32) -> i32");
        assert!(exports("").is_err());
        assert!(exports("crc32(i32) -> text").is_err());
    }
}
