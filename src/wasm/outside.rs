//! An outside module: `main.wasm`, a WebAssembly module that a compiler made from Rust, C or
//! another language, which an idea names as its program with a `- Module:` line. lengua didn't
//! build it, holds it to none of main.wat's rules and proves nothing of it: it is "trusted, not
//! proven" ([`TRUST`]), and every reply and every run says so. What holds for it is what the
//! host holds for any module by itself ([`crate::wasm::host`]) and what [`read`] refuses here,
//! before anything of it is compiled or run:
//!
//! - a file past [`MAX_BYTES`];
//! - a module the engine's validator doesn't take, with exactly the engine's features
//!   ([`crate::wasm::engine::FEATURES`]): no threads, tail calls, exceptions, 64-bit or multiple
//!   memories, GC, relaxed SIMD, wide arithmetic or components;
//! - an import that isn't a function, isn't one of WASI preview 1 or of lengua's own modules, or
//!   has another type; and a function of lengua's own modules that the idea doesn't grant. A
//!   function of WASI preview 1 that the idea doesn't grant links, as a toolchain's startup
//!   imports several whatever the program does, and the host refuses each call of it;
//! - a memory that isn't exported as `memory`, which every host call reads and writes through
//!   and whose declared size the host holds the program to, or one with no maximum, or with a
//!   maximum past [`MAX_PAGES`], main.wat's own;
//! - no `_start` that takes and gives nothing, which is all lengua ever calls.

use std::collections::BTreeMap;

use wasmparser::{CompositeInnerType, ExternalKind, Parser, Payload, TypeRef, ValType};

use crate::checker::wasm::model::{MAX_PAGES, PAGE};
use crate::checker::wasm::structure::needs;
use crate::wasm::Policy;
use crate::wasm::engine;
use crate::wasm::text::Built;
use crate::wasm::wasi::{self, Unasked};

/// An outside module's file in its program folder.
pub const FILE: &str = "main.wasm";

/// What lengua says of an outside module, in every reply and before every run.
pub const TRUST: &str = "trusted, not proven";

/// The state `check` gives an outside module whose tests pass, where main.wat gets `done`.
pub const STATE: &str = "trusted_not_proven";

/// The most bytes an outside module's file may hold: 16 MiB. A worker compiles the module
/// before a test's first run, and the pool gives up on a worker 20 seconds past a test's time,
/// so the size bounds what a module can cost before anything holds its time. The number was
/// chosen, not measured against real modules (todo.md).
pub const MAX_BYTES: usize = 16 * 1024 * 1024;

/// The most imports of the idea's ungranted ones that a reply names; it counts the rest.
const SHOWN: usize = 8;

/// An outside module that lengua may test and run, and what it read of it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Outside {
    /// The module as the workers and a run take it: its bytes as they are, with no line of a
    /// main.wat and no assertion.
    pub built: Built,
    /// The WebAssembly features it uses beyond WebAssembly 1.0.
    pub features: Vec<String>,
    /// The pages its memory starts with and the most it may grow to, when it has a memory.
    pub pages: Option<(u64, u64)>,
    /// The functions of WASI preview 1 it imports that its idea doesn't grant, which the host
    /// refuses at each call, or answers harmlessly.
    pub ungranted: Vec<&'static str>,
    /// The functions of WASI preview 1 it imports that lengua doesn't give at all.
    pub ungiven: Vec<&'static str>,
}

/// How a problem names a WebAssembly value type.
const fn type_name(ty: ValType) -> &'static str {
    match ty {
        ValType::I32 => "i32",
        ValType::I64 => "i64",
        ValType::F32 => "f32",
        ValType::F64 => "f64",
        ValType::V128 => "v128",
        ValType::Ref(_) => "a reference",
    }
}

/// How a problem shows a function's type.
fn signature_text(params: &[ValType], results: &[ValType]) -> String {
    let names = |list: &[ValType]| {
        list.iter()
            .map(|ty| type_name(*ty))
            .collect::<Vec<_>>()
            .join(" ")
    };
    format!("({}) -> ({})", names(params), names(results))
}

/// A WASI value's type as WebAssembly's.
const fn value_type(value: wasi::Value) -> ValType {
    match value {
        wasi::Value::I32 => ValType::I32,
        wasi::Value::I64 => ValType::I64,
    }
}

/// What [`read`] takes from the module's sections.
#[derive(Default)]
struct Sections {
    /// Each function type's parameters and results, by its index; `None` for a type that isn't a
    /// function's.
    types: Vec<Option<(Vec<ValType>, Vec<ValType>)>>,
    /// The imports: each one's module, name, and type index when it is a function.
    imports: Vec<(String, String, Option<u32>)>,
    /// The type index of each function the module defines.
    defined: Vec<u32>,
    /// Each memory the module defines: its first pages and its maximum.
    memories: Vec<(u64, Option<u64>)>,
    /// The exports: name, kind and index.
    exports: Vec<(String, ExternalKind, u32)>,
}

impl Sections {
    /// The sections of `wasm`, a module the validator took.
    fn of(wasm: &[u8]) -> Result<Self, String> {
        let failed = |error: wasmparser::BinaryReaderError| {
            format!("{FILE} can't be read as a WebAssembly module: {error}")
        };
        let mut sections = Self::default();
        for payload in Parser::new(0).parse_all(wasm) {
            match payload.map_err(failed)? {
                Payload::TypeSection(reader) => {
                    for group in reader {
                        for sub in group.map_err(failed)?.into_types() {
                            sections.types.push(match &sub.composite_type.inner {
                                CompositeInnerType::Func(func) => {
                                    Some((func.params().to_vec(), func.results().to_vec()))
                                }
                                _ => None,
                            });
                        }
                    }
                }
                Payload::ImportSection(reader) => {
                    for import in reader.into_imports() {
                        let import = import.map_err(failed)?;
                        let function = match import.ty {
                            TypeRef::Func(index) | TypeRef::FuncExact(index) => Some(index),
                            _ => None,
                        };
                        sections.imports.push((
                            import.module.to_owned(),
                            import.name.to_owned(),
                            function,
                        ));
                    }
                }
                Payload::FunctionSection(reader) => {
                    for index in reader {
                        sections.defined.push(index.map_err(failed)?);
                    }
                }
                Payload::MemorySection(reader) => {
                    for memory in reader {
                        let memory = memory.map_err(failed)?;
                        sections.memories.push((memory.initial, memory.maximum));
                    }
                }
                Payload::ExportSection(reader) => {
                    for export in reader {
                        let export = export.map_err(failed)?;
                        sections
                            .exports
                            .push((export.name.to_owned(), export.kind, export.index));
                    }
                }
                _ => {}
            }
        }
        Ok(sections)
    }

    /// The parameters and results of function `index` of the module, an import or one it
    /// defines.
    fn function(&self, index: u32) -> Option<&(Vec<ValType>, Vec<ValType>)> {
        let index = usize::try_from(index).ok()?;
        let imported: Vec<u32> = self
            .imports
            .iter()
            .filter_map(|(_, _, function)| *function)
            .collect();
        let ty = match imported.get(index) {
            Some(ty) => *ty,
            None => *self.defined.get(index.checked_sub(imported.len())?)?,
        };
        self.types.get(usize::try_from(ty).ok()?)?.as_ref()
    }
}

/// Reads `wasm`, the bytes of an outside module, for a program whose idea allows `policy`: what
/// lengua may test and run, or every reason it won't, each saying how to build the module so
/// that it will. Nothing of the module is compiled or run here.
///
/// # Errors
///
/// What lengua refuses of the module (the module's doc lists it), one line each.
pub fn read(wasm: Vec<u8>, policy: &Policy) -> Result<Outside, Vec<String>> {
    if wasm.len() > MAX_BYTES {
        return Err(vec![format!(
            "{FILE} holds {} bytes, and an outside module holds at most {} MiB: build it for \
             size, with its names and debug sections stripped",
            wasm.len(),
            MAX_BYTES / 1024 / 1024
        )]);
    }
    if let Err(error) = engine::validate(&wasm) {
        return Err(vec![format!(
            "{FILE} isn't a WebAssembly module lengua's engine takes: {error}. The engine has \
             WebAssembly 1.0 with {}, and no threads, tail calls, exceptions, 64-bit or multiple \
             memories, GC, relaxed SIMD, wide arithmetic or components: build a plain module \
             for wasm32-wasip1 without them",
            engine::OPTIONAL_FEATURES.join(", ")
        )]);
    }
    let sections = Sections::of(&wasm).map_err(|problem| vec![problem])?;
    let mut problems = Vec::new();
    let mut ungranted = Vec::new();
    let mut ungiven = Vec::new();
    for (module, name, function) in &sections.imports {
        let Some(type_index) = function else {
            problems.push(format!(
                "{FILE} imports {module}.{name}, which isn't a function: an outside module \
                 imports functions alone, and defines its own memory, table and globals"
            ));
            continue;
        };
        let Some(host) = wasi::imported(module, name) else {
            problems.push(format!(
                "{FILE} imports the function {module}.{name}, which lengua's host doesn't have: \
                 it gives the functions of {} and of lengua's own modules alone. Build the \
                 module for wasm32-wasip1, with nothing left for another host to give",
                wasi::MODULE
            ));
            continue;
        };
        let wanted_params: Vec<ValType> = host.params.iter().map(|v| value_type(*v)).collect();
        let wanted_results: Vec<ValType> = host.results.iter().map(|v| value_type(*v)).collect();
        let found = usize::try_from(*type_index)
            .ok()
            .and_then(|index| sections.types.get(index))
            .and_then(Option::as_ref);
        if found
            .is_none_or(|(params, results)| *params != wanted_params || *results != wanted_results)
        {
            let shown = found.map_or_else(
                || "another type".to_owned(),
                |(params, results)| signature_text(params, results),
            );
            problems.push(format!(
                "{FILE} imports {module}.{name} as {shown}, and it is {}",
                signature_text(&wanted_params, &wanted_results)
            ));
            continue;
        }
        if policy.allows(host) {
            if !host.given {
                ungiven.push(host.name);
            }
        } else if module == wasi::MODULE {
            // A toolchain's startup imports these whatever the program does: the host refuses
            // each call, or answers it harmlessly.
            if host.unasked == Unasked::Refused {
                ungranted.push(host.name);
            }
        } else {
            problems.push(format!(
                "{FILE} imports {module}.{name}, which its idea doesn't grant: it needs {}",
                needs(host)
            ));
        }
    }
    let exported_memory = sections
        .exports
        .iter()
        .find(|(name, ..)| name == "memory")
        .map(|(_, kind, index)| (*kind, *index));
    let pages = match sections.memories.as_slice() {
        [] => {
            if exported_memory.is_some() {
                problems.push(format!(
                    "{FILE} exports something named `memory` that isn't a memory it defines"
                ));
            }
            None
        }
        [(first, maximum)] => {
            if exported_memory != Some((ExternalKind::Memory, 0)) {
                problems.push(format!(
                    "{FILE} doesn't export its memory as `memory`, which every call of the host \
                     reads and writes through, and which lengua holds to the size it declares: a \
                     module built for wasm32-wasip1 exports it"
                ));
            }
            match maximum {
                None => problems.push(format!(
                    "{FILE}'s memory declares no maximum, and lengua holds a program to the \
                     memory its module declares: build it with one of at most {MAX_PAGES} pages \
                     (1 GiB), such as `-Wl,--max-memory=67108864` for clang or `-C \
                     link-arg=--max-memory=67108864` for rustc (64 MiB)"
                )),
                Some(most) if *most > MAX_PAGES || *first > MAX_PAGES => problems.push(format!(
                    "{FILE}'s memory may grow to {most} pages, and a program's memory holds at \
                     most {MAX_PAGES} pages (1 GiB): build it with a lower maximum, such as \
                     `-Wl,--max-memory=1073741824`"
                )),
                Some(_) => {}
            }
            Some((*first, maximum.unwrap_or(*first)))
        }
        _ => {
            problems.push(format!("{FILE} defines more than one memory"));
            None
        }
    };
    let start = sections
        .exports
        .iter()
        .find(|(name, kind, _)| name == "_start" && *kind == ExternalKind::Func)
        .map(|(.., index)| *index);
    match start.map(|index| sections.function(index)) {
        Some(Some((params, results))) if params.is_empty() && results.is_empty() => {}
        Some(_) => problems.push(format!(
            "{FILE} exports a `_start` that isn't a function that takes and gives nothing"
        )),
        None => problems.push(format!(
            "{FILE} exports no function `_start`, which is what lengua runs: build a program \
             with a main function for wasm32-wasip1 (a command), not a library"
        )),
    }
    if !problems.is_empty() {
        return Err(problems);
    }
    let features = engine::features_used(&wasm);
    ungranted.sort_unstable();
    ungranted.dedup();
    ungiven.sort_unstable();
    ungiven.dedup();
    Ok(Outside {
        built: Built {
            named: wasm.clone(),
            wasm,
            lines: BTreeMap::new(),
            bodies: Vec::new(),
            assertions: BTreeMap::new(),
            library: Vec::new(),
        },
        features,
        pages,
        ungranted,
        ungiven,
    })
}

/// An outside module that a program calls ([`crate::call`]), as lengua may start it beside the
/// program.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CalledOutside {
    /// The module's bytes, as they are.
    pub wasm: Vec<u8>,
    /// The WebAssembly features it uses beyond WebAssembly 1.0.
    pub features: Vec<String>,
    /// The pages its memory starts with and the most it may grow to, when it has a memory.
    pub pages: Option<(u64, u64)>,
}

/// A type of the idea's, as WebAssembly's.
const fn called_type(ty: crate::call::Type) -> ValType {
    match ty {
        crate::call::Type::I32 => ValType::I32,
        crate::call::Type::I64 => ValType::I64,
        crate::call::Type::F32 => ValType::F32,
        crate::call::Type::F64 => ValType::F64,
    }
}

/// Reads `wasm`, the bytes of the outside module that an idea's `- Calls:` line names as
/// `called`: what lengua may start beside the program, or every reason it won't, each saying
/// how to build the module so that it will. Nothing of the module is compiled or run here. It
/// refuses what it refuses of an outside module that is the program ([`read`]), but for the
/// `_start` a library doesn't have, and besides:
///
/// - any import: a called module gets no function of lengua's host, so nothing the program's
///   idea grants is the called module's;
/// - a function the idea's line names that the module doesn't export with the type the line
///   gives it.
///
/// Its memory, when it has one, is exported as `memory`, which `give` and `take` copy through,
/// with a maximum of at most [`MAX_PAGES`].
///
/// # Errors
///
/// What lengua refuses of the module, one line each.
pub fn read_called(
    wasm: Vec<u8>,
    called: &crate::call::Called,
) -> Result<CalledOutside, Vec<String>> {
    let file = format!("{}.wasm", called.name);
    if wasm.len() > MAX_BYTES {
        return Err(vec![format!(
            "{file} holds {} bytes, and an outside module holds at most {} MiB: build it for \
             size, with its names and debug sections stripped",
            wasm.len(),
            MAX_BYTES / 1024 / 1024
        )]);
    }
    if let Err(error) = engine::validate(&wasm) {
        return Err(vec![format!(
            "{file} isn't a WebAssembly module lengua's engine takes: {error}. The engine has \
             WebAssembly 1.0 with {}, and no threads, tail calls, exceptions, 64-bit or multiple \
             memories, GC, relaxed SIMD, wide arithmetic or components: build a plain module \
             without them",
            engine::OPTIONAL_FEATURES.join(", ")
        )]);
    }
    let sections = Sections::of(&wasm).map_err(|problem| vec![problem.replace(FILE, &file)])?;
    let mut problems = Vec::new();
    for (module, name, _) in &sections.imports {
        problems.push(format!(
            "{file} imports {module}.{name}, and a module that a program calls imports nothing: \
             it gets no function of lengua's host, so that nothing the program's idea grants is \
             the module's. Build it as a library with nothing left for a host to give: for \
             Rust, `--target wasm32-unknown-unknown` with `--crate-type cdylib`; for C, \
             `-nostdlib -Wl,--no-entry` with each function exported"
        ));
    }
    let exported_memory = sections
        .exports
        .iter()
        .find(|(name, ..)| name == "memory")
        .map(|(_, kind, index)| (*kind, *index));
    let pages = match sections.memories.as_slice() {
        [] => {
            if exported_memory.is_some() {
                problems.push(format!(
                    "{file} exports something named `memory` that isn't a memory it defines"
                ));
            }
            None
        }
        [(first, maximum)] => {
            if exported_memory != Some((ExternalKind::Memory, 0)) {
                problems.push(format!(
                    "{file} doesn't export its memory as `memory`, which the program's `give` \
                     and `take` copy bytes through, and which lengua holds to the size it \
                     declares: export it, as a toolchain's linker does by default"
                ));
            }
            match maximum {
                None => problems.push(format!(
                    "{file}'s memory declares no maximum, and lengua holds a module to the \
                     memory it declares: build it with one of at most {MAX_PAGES} pages (1 GiB), \
                     such as `-Wl,--max-memory=16777216` for clang or `-C \
                     link-arg=--max-memory=16777216` for rustc (16 MiB)"
                )),
                Some(most) if *most > MAX_PAGES || *first > MAX_PAGES => problems.push(format!(
                    "{file}'s memory may grow to {most} pages, and a module's memory holds at \
                     most {MAX_PAGES} pages (1 GiB): build it with a lower maximum"
                )),
                Some(_) => {}
            }
            Some((*first, maximum.unwrap_or(*first)))
        }
        _ => {
            problems.push(format!("{file} defines more than one memory"));
            None
        }
    };
    for signature in &called.functions {
        let wanted_params: Vec<ValType> =
            signature.params.iter().copied().map(called_type).collect();
        let wanted_results: Vec<ValType> = signature.result.into_iter().map(called_type).collect();
        let exported = sections
            .exports
            .iter()
            .find(|(name, kind, _)| *name == signature.name && *kind == ExternalKind::Func)
            .map(|(.., index)| *index);
        match exported.map(|index| sections.function(index)) {
            Some(Some((params, results)))
                if *params == wanted_params && *results == wanted_results => {}
            Some(Some((params, results))) => problems.push(format!(
                "{file} exports `{}` as {}, and the idea's `- Calls:` line says `{}`: make the \
                 two the same",
                signature.name,
                signature_text(params, results),
                signature.shown()
            )),
            Some(None) => problems.push(format!(
                "{file} exports a `{}` that isn't a function lengua can read",
                signature.name
            )),
            None => problems.push(format!(
                "{file} exports no function `{}`, which the idea's `- Calls:` line names: \
                 export it under that name (`#[unsafe(no_mangle)] pub extern \"C\" fn` in Rust, \
                 `__attribute__((export_name(\"{0}\")))` in C), or take it off the line",
                signature.name
            )),
        }
    }
    if false && !problems.is_empty() {
        return Err(problems);
    }
    let features = engine::features_used(&wasm);
    Ok(CalledOutside {
        wasm,
        features,
        pages,
    })
}

/// What `check` says of an outside module that the program calls, in one line: that it is
/// trusted, not proven, what the idea says it was built from, and what holds it.
#[must_use]
pub fn called_line(called: &crate::call::Called, module: &CalledOutside) -> String {
    let built_from = match &called.kind {
        crate::call::Kind::Outside { built_from } => built_from.as_str(),
        crate::call::Kind::Program { .. } => "",
    };
    let memory = module.pages.map_or_else(
        || "it has no memory".to_owned(),
        |(first, most)| {
            format!(
                "its memory is its own, starts at {first} pages and grows to at most {most} ({} \
                 MiB)",
                most.saturating_mul(PAGE) / 1024 / 1024
            )
        },
    );
    format!(
        "{}.wasm: {TRUST}: an outside module, which the idea says was built from {built_from}; \
         lengua didn't build it, holds it to none of main.wat's rules and proves nothing of it. \
         The program calls {} of it. Held, by lengua's host: it runs beside the program as an \
         instance of its own; {memory}; it imports nothing and gets no function of the host, so \
         it reaches no file, no network and nothing else the idea grants the program; the \
         run's deadline and the engine's stack bound it with the program. Not held: what its \
         functions give back, which the program's tests show for the inputs they tried and no \
         proof covers",
        called.name,
        called.listed()
    )
}

/// `names` as a reply lists them: the first [`SHOWN`], and how many more.
fn listed(names: &[&str]) -> String {
    let mut shown: Vec<String> = names
        .iter()
        .take(SHOWN)
        .map(|name| (*name).to_owned())
        .collect();
    if names.len() > SHOWN {
        shown.push(format!("{} more", names.len() - SHOWN));
    }
    shown.join(", ")
}

/// `count` functions, in the plural unless there's one.
fn functions(count: usize) -> String {
    if count == 1 {
        "1 function".to_owned()
    } else {
        format!("{count} functions")
    }
}

/// What lengua says of a program that is an outside module, in one line, before every run of it
/// and in every reply about it: `built_from` is what its idea says it was built from.
#[must_use]
pub fn words(built_from: &str) -> String {
    format!(
        "{TRUST}: {FILE} is an outside module, which its idea says was built from {built_from}; \
         lengua didn't build it and holds it to none of main.wat's rules, and nothing of it is \
         proven. lengua's host holds it to what its idea grants"
    )
}

/// What `check` says of an outside module, a line each: that it is trusted, not proven, each
/// guarantee of main.wat's that does not hold for it, and what does hold, which is the host's.
/// `module` is what lengua read of it, once it has.
#[must_use]
pub fn said(built_from: &str, module: Option<&Outside>) -> Vec<String> {
    let mut lines = vec![
        format!(
            "{TRUST}: {FILE} is an outside module, which its idea says was built from \
             {built_from}; lengua didn't build it and can't tell what it was built from"
        ),
        "not held: main.wat's rules. No memory rule (M1 to M8) follows its addresses, so it may \
         read or write the wrong place inside its own memory, use what it freed, or run off an \
         array, as a C program can"
            .to_owned(),
        "not held: bounded loops and no recursion. Nothing says it ends: a test and the run \
         tool stop it at their deadline, and a run from a shell has none, so there the user \
         stops it"
            .to_owned(),
        "not held: the secret rule. An idea with a `- Secret:` line is refused for an outside \
         module, since lengua can't say what its code does with a secret"
            .to_owned(),
        "not held: assertions, checked results and the other rules of the lengua Power of 10"
            .to_owned(),
        "not proven: nothing. A generated test shows only the runs it made, and no proof covers \
         an input the tests didn't try"
            .to_owned(),
        "not attested: lengua writes no entry in idea.md's `verified` for it, and `lengua-mcp \
         check` doesn't attest it"
            .to_owned(),
    ];
    let memory = module.and_then(|module| module.pages).map_or_else(
        || "it has no memory".to_owned(),
        |(first, most)| {
            format!(
                "its memory stays its own, starts at {first} pages and grows to at most {most} \
                 ({} MiB)",
                most.saturating_mul(PAGE) / 1024 / 1024
            )
        },
    );
    lines.push(format!(
        "held, by lengua's host: it reaches only what its idea grants, on the network, among \
         the files, by the programs it starts and at the terminal; {memory}; its calls stay \
         within the engine's stack; and what it prints and writes has the limits of any \
         program's"
    ));
    if let Some(module) = module {
        if !module.ungranted.is_empty() {
            lines.push(format!(
                "it imports {} of WASI preview 1 that its idea doesn't grant, as a toolchain's \
                 startup does: the host answers each call that it isn't allowed ({})",
                functions(module.ungranted.len()),
                listed(&module.ungranted)
            ));
        }
        if !module.ungiven.is_empty() {
            lines.push(format!(
                "it imports {} of WASI preview 1 that lengua doesn't give, which answer that \
                 they aren't there ({})",
                functions(module.ungiven.len()),
                listed(&module.ungiven)
            ));
        }
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::idea::Use;

    /// The module `text` builds, as a compiler's output stands in a test.
    fn wasm(text: &str) -> Vec<u8> {
        crate::wasm::text::build(text)
            .unwrap_or_else(|problem| panic!("{problem}"))
            .wasm
    }

    /// What lengua refuses of `text`, in one text.
    fn refused(text: &str, policy: &Policy) -> String {
        read(wasm(text), policy)
            .expect_err("lengua takes it")
            .join("\n")
    }

    const EXIT: &str = r#"(import "wasi_snapshot_preview1" "proc_exit" (func $exit (param i32)))"#;

    /// A module as a toolchain makes one: its memory exported with a maximum, a table, a
    /// `_start`, and imports its idea doesn't grant, which link.
    #[test]
    fn a_toolchain_s_module_is_taken() {
        let text = format!(
            r#"(module {EXIT}
  (import "wasi_snapshot_preview1" "path_open"
    (func (param i32 i32 i32 i32 i32 i64 i64 i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "environ_sizes_get" (func (param i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "fd_sync" (func (param i32) (result i32)))
  (type $t (func))
  (table 2 2 funcref)
  (memory (export "memory") 17 1024)
  (global (export "__stack_pointer") (mut i32) (i32.const 1048576))
  (func (export "_start") (call_indirect (type $t) (i32.const 0)))
  (func $f)
  (elem (i32.const 0) $f))"#
        );
        let module = read(wasm(&text), &Policy::default()).unwrap_or_else(|p| panic!("{p:?}"));
        assert_eq!(module.pages, Some((17, 1024)));
        assert_eq!(module.ungranted, ["path_open"]);
        assert_eq!(module.ungiven, ["fd_sync"]);
        assert!(module.built.assertions.is_empty() && module.built.lines.is_empty());
        assert_eq!(module.built.wasm, wasm(&text));
        let lines = said("C with clang", Some(&module)).join("\n");
        assert!(
            lines.starts_with("trusted, not proven: main.wasm"),
            "{lines}"
        );
        assert!(lines.contains("grows to at most 1024 (64 MiB)"), "{lines}");
        assert!(lines.contains("1 function of WASI preview 1 that its idea doesn't grant"));
        assert!(lines.contains("(path_open)"), "{lines}");
        // With files named, path_open is granted.
        let files = Policy::of(&[Use::Files], &crate::idea::Network::default());
        let module = read(wasm(&text), &files).unwrap_or_else(|p| panic!("{p:?}"));
        assert!(module.ungranted.is_empty(), "{:?}", module.ungranted);
    }

    /// What isn't a module the engine takes is refused before anything reads it further: other
    /// bytes, and each feature the engine has off.
    #[test]
    fn what_the_engine_doesn_t_take_is_refused() {
        let none = Policy::default();
        let problem = read(b"not a module".to_vec(), &none).expect_err("it isn't one");
        assert!(problem[0].contains("isn't a WebAssembly module lengua's engine takes"));
        // A component's header: version 13, layer 1.
        let component = b"\0asm\x0d\0\x01\0".to_vec();
        assert!(read(component, &none).is_err());
        // The text's builder validates with the engine's features too, so each of these is
        // written as the bytes a compiler would make.
        // Two memories: the memory section with two entries.
        let two_memories = [
            b"\0asm\x01\0\0\0".as_slice(),
            &[0x05, 0x05, 0x02, 0x00, 0x01, 0x00, 0x01],
        ]
        .concat();
        assert!(read(two_memories, &none).is_err());
        // A shared memory, threads': limits flag 3.
        let shared = [
            b"\0asm\x01\0\0\0".as_slice(),
            &[0x05, 0x04, 0x01, 0x03, 0x01, 0x01],
        ]
        .concat();
        assert!(read(shared, &none).is_err());
        // A 64-bit memory: limits flag 4.
        let wide = [
            b"\0asm\x01\0\0\0".as_slice(),
            &[0x05, 0x03, 0x01, 0x04, 0x01],
        ]
        .concat();
        assert!(read(wide, &none).is_err());
        // A tail call: one function () -> () whose body is `return_call 0`.
        let tail_call = [
            b"\0asm\x01\0\0\0".as_slice(),
            &[0x01, 0x04, 0x01, 0x60, 0x00, 0x00],
            &[0x03, 0x02, 0x01, 0x00],
            &[0x0a, 0x06, 0x01, 0x04, 0x00, 0x12, 0x00, 0x0b],
        ]
        .concat();
        assert!(read(tail_call, &none).is_err());
        // Exceptions: a tag section.
        let tag = [
            b"\0asm\x01\0\0\0".as_slice(),
            &[0x01, 0x04, 0x01, 0x60, 0x00, 0x00],
            &[0x0d, 0x03, 0x01, 0x00, 0x00],
        ]
        .concat();
        assert!(read(tag, &none).is_err());
        // A file past the size isn't read at all.
        let large = read(vec![0; MAX_BYTES + 1], &none).expect_err("too large");
        assert!(large[0].contains("holds at most 16 MiB"), "{large:?}");
    }

    /// An import is a function of lengua's host with its own type; one of lengua's own modules
    /// links only when the idea grants it.
    #[test]
    fn an_import_is_a_function_of_the_host_that_the_idea_grants() {
        let none = Policy::default();
        let module = |imports: &str| {
            format!(r#"(module {imports} (memory (export "memory") 1 1) (func (export "_start")))"#)
        };
        let other = refused(
            &module(r#"(import "env" "system" (func (param i32)))"#),
            &none,
        );
        assert!(
            other.contains("imports the function env.system, which lengua's host doesn't have"),
            "{other}"
        );
        let preview2 = refused(
            &module(r#"(import "wasi:cli/environment@0.2.0" "get-arguments" (func))"#),
            &none,
        );
        assert!(
            preview2.contains("lengua's host doesn't have"),
            "{preview2}"
        );
        let typed = refused(
            &module(r#"(import "wasi_snapshot_preview1" "fd_close" (func (param i64)))"#),
            &none,
        );
        assert!(
            typed.contains(
                "imports wasi_snapshot_preview1.fd_close as (i64) -> (), and it is (i32) -> (i32)"
            ),
            "{typed}"
        );
        let global = refused(&module(r#"(import "env" "g" (global i32))"#), &none);
        assert!(global.contains("which isn't a function"), "{global}");
        let table = refused(&module(r#"(import "env" "t" (table 1 funcref))"#), &none);
        assert!(table.contains("which isn't a function"), "{table}");
        let opens = module(
            r#"(import "lengua_net_1" "tcp_open" (func (param i32 i32 i32 i32 i32) (result i32)))"#,
        );
        let ungranted = refused(&opens, &none);
        assert!(
            ungranted.contains("imports lengua_net_1.tcp_open, which its idea doesn't grant"),
            "{ungranted}"
        );
        let (_, network) =
            crate::idea::constraints(&[], "opens TCP connections to port 443 on any computer")
                .unwrap_or_else(|problems| panic!("{problems:?}"));
        let granted = Policy::of(&[], &network);
        assert!(read(wasm(&opens), &granted).is_ok());
        let starts = module(
            r#"(import "lengua_run_1" "program_run"
                 (func (param i32 i32 i32 i32 i32 i32 i32 i32) (result i32)))"#,
        );
        assert!(refused(&starts, &none).contains("which its idea doesn't grant"));
    }

    /// The memory is the one the host reads through and holds to its declared size: exported as
    /// `memory`, with a maximum of at most main.wat's.
    #[test]
    fn the_memory_is_exported_and_bounded() {
        let none = Policy::default();
        let hidden = refused(r#"(module (memory 1 1) (func (export "_start")))"#, &none);
        assert!(
            hidden.contains("doesn't export its memory as `memory`"),
            "{hidden}"
        );
        let renamed = refused(
            r#"(module (memory (export "mem") 1 1) (func (export "_start")))"#,
            &none,
        );
        assert!(
            renamed.contains("doesn't export its memory as `memory`"),
            "{renamed}"
        );
        let unbounded = refused(
            r#"(module (memory (export "memory") 1) (func (export "_start")))"#,
            &none,
        );
        assert!(unbounded.contains("declares no maximum"), "{unbounded}");
        let large = refused(
            r#"(module (memory (export "memory") 1 65536) (func (export "_start")))"#,
            &none,
        );
        assert!(large.contains("may grow to 65536 pages"), "{large}");
        let most = r#"(module (memory (export "memory") 1 16384) (func (export "_start")))"#;
        assert!(read(wasm(most), &none).is_ok());
        // A function named `memory` is no memory.
        let lie = refused(
            r#"(module (func (export "memory")) (func (export "_start")))"#,
            &none,
        );
        assert!(lie.contains("isn't a memory it defines"), "{lie}");
        // A module with no memory at all runs: it can call nothing of the host that takes one.
        let bare = read(wasm(r#"(module (func (export "_start")))"#), &none);
        assert_eq!(bare.map(|module| module.pages), Ok(None));
    }

    /// What lengua runs is `_start`, a function that takes and gives nothing.
    #[test]
    fn a_module_without_a_start_is_no_program() {
        let none = Policy::default();
        let library = refused(
            r#"(module (memory (export "memory") 1 1) (func (export "add") (param i32 i32)))"#,
            &none,
        );
        assert!(
            library.contains("exports no function `_start`"),
            "{library}"
        );
        let typed = refused(
            r#"(module (memory (export "memory") 1 1)
                 (func (export "_start") (param i32) (result i32) (local.get 0)))"#,
            &none,
        );
        assert!(
            typed.contains("isn't a function that takes and gives nothing"),
            "{typed}"
        );
        let global = refused(
            r#"(module (memory (export "memory") 1 1) (global (export "_start") i32 (i32.const 0)))"#,
            &none,
        );
        assert!(global.contains("exports no function `_start`"), "{global}");
        // An imported function exported as `_start` has the host's type, never () -> ().
        let imported = refused(
            &format!(
                r#"(module {EXIT} (memory (export "memory") 1 1) (export "_start" (func $exit)))"#
            ),
            &none,
        );
        assert!(
            imported.contains("isn't a function that takes and gives nothing"),
            "{imported}"
        );
    }

    /// The words of every reply and run start with the trust, and name what the idea says.
    #[test]
    fn the_words_say_trusted_not_proven_first() {
        let line = words("Rust with rustc");
        assert!(line.starts_with("trusted, not proven: main.wasm is an outside module"));
        assert!(line.contains("built from Rust with rustc"), "{line}");
        let lines = said("Rust with rustc", None);
        assert!(lines[0].starts_with(TRUST));
        for missing in [
            "memory rule",
            "bounded loops",
            "secret rule",
            "not proven",
            "not attested",
        ] {
            assert!(
                lines.iter().any(|line| line.contains(missing)),
                "{missing}: {lines:?}"
            );
        }
        assert!(!lines.iter().any(|line| line.contains("done")), "{lines:?}");
    }
}
