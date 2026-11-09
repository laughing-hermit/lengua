//! main.wat as lengua builds it: the text parsed with each instruction's place in it kept, the
//! functions of lengua's library that it calls and doesn't define merged in, the module encoded,
//! and validated with exactly the engine's features. A build keeps, for every operator of
//! main.wat's own functions, its offset in the module and its line in main.wat, so a trap, a
//! validator's complaint or a failed assertion can name the line.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use wast::Wat;
use wast::core::{
    ElemPayload, ExportKind, FuncKind, GlobalKind, Instruction, ModuleField, ModuleKind, TableKind,
};
use wast::parser::{self, ParseBuffer};
use wast::token::{Id, Index, Span};

/// lengua's library: the functions a main.wat may call without defining them.
pub const LIBRARY: &str = include_str!("library.wat");

/// The library's functions, by the names main.wat calls them by.
pub const LIBRARY_FUNCTIONS: [&str; 3] = ["assert_fail", "fill", "write_all"];

/// Memory's first bytes, which the library uses while a call of it lasts: a buffer given to it
/// starts here or later.
pub const SCRATCH: u32 = 16;

/// The prefix of the names lengua keeps for its library's imports and data.
pub const RESERVED: &str = "lengua.";

/// What decides the bytes [`build`] makes of a main.wat: lengua's version, this file, lengua's
/// library and Cargo.lock, which pins wast and wasm-encoder exactly; a validation or a line map
/// changes none of them. A 64-bit FNV-1a over each, its length first, computed as lengua is
/// compiled. check keeps the module whose tests passed under its 16 hex digits, and a run from
/// a shell takes a kept module only from its own builder's folder, so a lengua whose build may
/// differ builds main.wat instead. It names a build and secures nothing: what runs is held to the
/// module hash in the lock that lengua's key signed.
#[allow(long_running_const_eval)]
pub const BUILDER: u64 = {
    let mut hash = FNV_OFFSET;
    hash = fnv_with_length(hash, env!("CARGO_PKG_VERSION").as_bytes());
    hash = fnv_with_length(hash, include_bytes!("text.rs"));
    hash = fnv_with_length(hash, include_bytes!("library.wat"));
    fnv_with_length(hash, include_bytes!("../../Cargo.lock"))
};

/// FNV-1a's 64-bit offset basis.
const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;

/// FNV-1a's 64-bit prime.
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

/// `hash` taken on over `bytes` by FNV-1a: each byte xored in, then the product with the prime,
/// modulo 2^64. The product is taken in 128 bits, where it can't overflow, so that the compiler
/// works it out with no call a byte.
const fn fnv(mut hash: u64, bytes: &[u8]) -> u64 {
    let length = bytes.len();
    let mut at = 0;
    while at < length {
        hash ^= bytes[at] as u64;
        hash = (hash as u128 * FNV_PRIME as u128) as u64;
        at += 1;
    }
    hash
}

/// `hash` taken on over the length of `bytes`, its eight bytes from the least significant, then
/// over `bytes`, so that no two inputs one after another hash as another two do.
const fn fnv_with_length(hash: u64, bytes: &[u8]) -> u64 {
    let length = (bytes.len() as u64).to_le_bytes();
    fnv(fnv(hash, &length), bytes)
}

/// What each of the library's functions needs besides itself, by its names in the library.
fn needs(function: &str) -> &'static [&'static str] {
    match function {
        "assert_fail" => &["lengua.fd_write", "lengua.proc_exit", "lengua.assertion"],
        "fill" => &["assert_fail", "lengua.fd_read"],
        "write_all" => &["assert_fail", "lengua.fd_write"],
        _ => &[],
    }
}

/// What's wrong with a main.wat, on a line of it when it has one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Problem {
    /// The line, from 1.
    pub line: Option<usize>,
    /// What's wrong, in words.
    pub message: String,
}

impl fmt::Display for Problem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.line {
            Some(line) => write!(f, "main.wat line {line}: {}", self.message),
            None => write!(f, "main.wat: {}", self.message),
        }
    }
}

impl std::error::Error for Problem {}

/// A module built from a main.wat, ready for the engine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Built {
    /// The module: main.wat with the library's functions it calls, a standard WASI preview 1
    /// module that any WebAssembly runtime runs.
    pub wasm: Vec<u8>,
    /// main.wat's line of each operator of its own functions, by the operator's offset in
    /// [`Self::wasm`].
    pub lines: BTreeMap<u64, usize>,
    /// Where main.wat's own functions' bodies are in [`Self::wasm`], each from its start to its
    /// end.
    pub bodies: Vec<(u64, u64)>,
    /// The line of each assertion, by its number: where main.wat passes the number to
    /// `$assert_fail`.
    pub assertions: BTreeMap<i64, usize>,
    /// The library's functions merged in, by name.
    pub library: Vec<String>,
}

impl Built {
    /// main.wat's line of the operator at `offset` in the module, or of the nearest before it in
    /// its function, when it is in one of main.wat's own functions. An offset before a function's
    /// first operator, among its locals, is that operator's line.
    #[must_use]
    pub fn line_at(&self, offset: u64) -> Option<usize> {
        line_at(&self.lines, &self.bodies, offset)
    }
}

/// [`Built::line_at`], from the parts of a build.
#[must_use]
pub fn line_at(lines: &BTreeMap<u64, usize>, bodies: &[(u64, u64)], offset: u64) -> Option<usize> {
    let (start, end) = bodies
        .iter()
        .copied()
        .find(|(start, end)| (*start..*end).contains(&offset))?;
    lines
        .range(start..=offset)
        .next_back()
        .or_else(|| lines.range(start..end).next())
        .map(|(_, line)| *line)
}

/// Where each line of `text` starts.
fn line_starts(text: &str) -> Vec<usize> {
    std::iter::once(0)
        .chain(text.match_indices('\n').map(|(at, _)| at + 1))
        .collect()
}

/// The line, from 1, of the byte at `offset`.
fn line_of(starts: &[usize], offset: usize) -> usize {
    starts.partition_point(|start| *start <= offset).max(1)
}

/// A problem the text parser found in main.wat.
fn parse_problem(text: &str, starts: &[usize], error: &wast::Error) -> Problem {
    let offset = error.span().offset();
    Problem {
        line: (offset <= text.len()).then(|| line_of(starts, offset)),
        message: error.message(),
    }
}

/// A problem in lengua's own library, which its tests rule out.
fn library_problem(error: &dyn fmt::Display) -> Problem {
    Problem {
        line: None,
        message: format!("lengua's library didn't build: {error}"),
    }
}

/// The names a module field gives, with where it gives them.
fn ids<'a>(field: &ModuleField<'a>) -> Vec<(Id<'a>, Span)> {
    match field {
        ModuleField::Func(func) => func.id.map(|id| (id, func.span)).into_iter().collect(),
        ModuleField::Data(data) => data.id.map(|id| (id, data.span)).into_iter().collect(),
        ModuleField::Import(import) => import
            .item_sigs()
            .into_iter()
            .filter_map(|sig| sig.id.map(|id| (id, sig.span)))
            .collect(),
        _ => Vec::new(),
    }
}

/// The functions main.wat defines or imports, by name.
fn defined(fields: &[ModuleField<'_>]) -> BTreeSet<String> {
    fields
        .iter()
        .filter(|field| matches!(field, ModuleField::Func(_) | ModuleField::Import(_)))
        .flat_map(ids)
        .map(|(id, _)| id.name().to_owned())
        .collect()
}

/// The functions main.wat's code calls or takes a reference to, by name.
fn called(fields: &[ModuleField<'_>]) -> BTreeSet<String> {
    let mut called = BTreeSet::new();
    for field in fields {
        let ModuleField::Func(func) = field else {
            continue;
        };
        let FuncKind::Inline { expression, .. } = &func.kind else {
            continue;
        };
        for instruction in expression.instrs.iter() {
            if let Instruction::call(Index::Id(id))
            | Instruction::return_call(Index::Id(id))
            | Instruction::ref_func(Index::Id(id)) = instruction
            {
                called.insert(id.name().to_owned());
            }
        }
    }
    called
}

/// The number `index` gives, and where, when it gives one rather than a name.
fn number_of(index: &Index<'_>) -> Option<(u32, Span)> {
    match index {
        Index::Num(number, span) => Some((*number, *span)),
        Index::Id(_) => None,
    }
}

/// The first function `expression` calls or takes a reference to by its number.
fn numbered_in(expression: &wast::core::Expression<'_>) -> Option<(u32, Span)> {
    expression
        .instrs
        .iter()
        .find_map(|instruction| match instruction {
            Instruction::call(index)
            | Instruction::return_call(index)
            | Instruction::ref_func(index) => number_of(index),
            _ => None,
        })
}

/// The first function a table's elements name by its number.
fn numbered_elements(payload: &ElemPayload<'_>) -> Option<(u32, Span)> {
    match payload {
        ElemPayload::Indices(indices) => indices.iter().find_map(number_of),
        ElemPayload::Exprs { exprs, .. } => exprs.iter().find_map(numbered_in),
    }
}

/// The first place main.wat names a function by its number, and the number: in its code, an
/// export, its start, a table's elements or a global's value.
fn numbered(fields: &[ModuleField<'_>]) -> Option<(u32, Span)> {
    fields.iter().find_map(|field| match field {
        ModuleField::Func(func) => match &func.kind {
            FuncKind::Inline { expression, .. } => numbered_in(expression),
            FuncKind::Import(..) => None,
        },
        ModuleField::Export(export) if export.kind == ExportKind::Func => number_of(&export.item),
        ModuleField::Start(index) => number_of(index),
        ModuleField::Elem(elem) => numbered_elements(&elem.payload),
        ModuleField::Table(table) => match &table.kind {
            TableKind::Inline { payload, .. } => numbered_elements(payload),
            TableKind::Normal {
                init_expr: Some(expression),
                ..
            } => numbered_in(expression),
            TableKind::Normal { .. } | TableKind::Import { .. } => None,
        },
        ModuleField::Global(global) => match &global.kind {
            GlobalKind::Inline(expression) => numbered_in(expression),
            GlobalKind::Import(_) => None,
        },
        _ => None,
    })
}

/// The line of each assertion: an `i32.const` of its number just before a `call $assert_fail`.
fn assertions(fields: &[ModuleField<'_>], starts: &[usize]) -> BTreeMap<i64, usize> {
    let mut lines = BTreeMap::new();
    for field in fields {
        let ModuleField::Func(func) = field else {
            continue;
        };
        let FuncKind::Inline { expression, .. } = &func.kind else {
            continue;
        };
        let Some(spans) = expression.instr_spans.as_deref() else {
            continue;
        };
        for (index, pair) in expression.instrs.windows(2).enumerate() {
            if let [
                Instruction::i32_const(number),
                Instruction::call(Index::Id(id)),
            ] = pair
                && id.name() == "assert_fail"
                && let Some(span) = spans.get(index)
            {
                lines.insert(i64::from(*number), line_of(starts, span.offset()));
            }
        }
    }
    lines
}

/// The name a library field gives: a function's, an import's or a data segment's.
fn library_name<'a>(field: &ModuleField<'a>) -> Option<&'a str> {
    ids(field).first().map(|(id, _)| id.name())
}

/// Builds main.wat's `text` into a module: the library's functions it calls and doesn't define
/// merged in after its own, encoded, and validated as the engine takes it.
///
/// # Errors
///
/// What's wrong with the text, on its line when it has one.
pub fn build(text: &str) -> Result<Built, Problem> {
    let starts = line_starts(text);
    let mut main_buffer =
        ParseBuffer::new(text).map_err(|error| parse_problem(text, &starts, &error))?;
    main_buffer.track_instr_spans(true);
    let library_buffer = ParseBuffer::new(LIBRARY).map_err(|error| library_problem(&error))?;
    let parsed =
        parser::parse::<Wat>(&main_buffer).map_err(|error| parse_problem(text, &starts, &error))?;
    let Wat::Module(mut module) = parsed else {
        return Err(Problem {
            line: None,
            message: "this is a component; lengua takes a module, `(module …)`".to_owned(),
        });
    };
    let Ok(Wat::Module(library)) = parser::parse::<Wat>(&library_buffer) else {
        return Err(library_problem(&"it isn't a module"));
    };
    let ModuleKind::Text(library_fields) = library.kind else {
        return Err(library_problem(&"it isn't text"));
    };
    let ModuleKind::Text(fields) = &mut module.kind else {
        return Err(Problem {
            line: None,
            message: "this module is given as bytes; lengua checks a module's text".to_owned(),
        });
    };
    for field in fields.iter() {
        for (id, span) in ids(field) {
            if id.name().starts_with(RESERVED) {
                return Err(Problem {
                    line: Some(line_of(&starts, span.offset())),
                    message: format!(
                        "${} starts with `${RESERVED}`, which lengua keeps for its library's \
                         names: name it otherwise",
                        id.name()
                    ),
                });
            }
        }
    }
    let defined = defined(fields);
    let called = called(fields);
    let assertions = assertions(fields, &starts);

    // The library's functions main.wat calls and doesn't define, and what they need; one that
    // main.wat defines itself is its own, for the library's functions too.
    let mut wanted: BTreeSet<&'static str> = BTreeSet::new();
    let mut pending: Vec<&'static str> = LIBRARY_FUNCTIONS
        .iter()
        .copied()
        .filter(|name| called.contains(*name))
        .collect();
    while let Some(name) = pending.pop() {
        if !defined.contains(name) && wanted.insert(name) {
            pending.extend(needs(name).iter().copied());
        }
    }
    let mut imports = Vec::new();
    let mut rest = Vec::new();
    let mut library = Vec::new();
    for field in library_fields {
        let Some(name) = library_name(&field) else {
            continue;
        };
        if !wanted.contains(name) {
            continue;
        }
        match field {
            ModuleField::Import(_) => imports.push(field),
            ModuleField::Func(_) => {
                library.push(name.to_owned());
                rest.push(field);
            }
            _ => rest.push(field),
        }
    }
    // The library's imports go before main.wat's fields and take the first functions' numbers,
    // so a number main.wat gives a function by would name another, maybe of the same type.
    if !imports.is_empty()
        && let Some((number, span)) = numbered(fields)
    {
        return Err(Problem {
            line: Some(line_of(&starts, span.offset())),
            message: format!(
                "function {number} is named by its number, which merging lengua's library would \
                 change, since its imports come first: name the function, as in `(call $name)`"
            ),
        });
    }
    let own = std::mem::take(fields);
    let own_functions = own
        .iter()
        .filter(|field| {
            matches!(
                field,
                ModuleField::Func(func) if matches!(func.kind, FuncKind::Inline { .. })
            )
        })
        .count();
    // The library's imports first, as imports precede every definition, and its functions last,
    // so main.wat's functions keep their places in the code.
    let mut merged = Vec::with_capacity(imports.len() + own.len() + rest.len());
    merged.extend(imports);
    merged.extend(own);
    merged.extend(rest);
    *fields = merged;

    let wasm = module
        .encode()
        .map_err(|error| parse_problem(text, &starts, &error))?;

    // Each of main.wat's functions' instructions, by place, with where it is in the text.
    let ModuleKind::Text(fields) = &module.kind else {
        return Err(library_problem(&"the merged module lost its text"));
    };
    let spans: Vec<Option<&[Span]>> = fields
        .iter()
        .filter_map(|field| match field {
            ModuleField::Func(func) => match &func.kind {
                FuncKind::Inline { expression, .. } => Some(expression.instr_spans.as_deref()),
                FuncKind::Import(..) => None,
            },
            _ => None,
        })
        .take(own_functions)
        .collect();

    let mut lines = BTreeMap::new();
    let mut bodies = Vec::new();
    let mut library_bodies = Vec::new();
    let mut function = 0;
    for payload in wasmparser::Parser::new(0).parse_all(&wasm) {
        let body = match payload {
            Ok(wasmparser::Payload::CodeSectionEntry(body)) => body,
            Ok(_) => continue,
            Err(error) => return Err(library_problem(&error)),
        };
        let range = body.range();
        let Some(own_spans) = spans.get(function) else {
            library_bodies.push((range.start, range.end));
            function += 1;
            continue;
        };
        bodies.push((range.start, range.end));
        function += 1;
        let Some(own_spans) = own_spans else {
            continue;
        };
        let mut reader = body
            .get_operators_reader()
            .map_err(|error| library_problem(&error))?;
        let mut offsets = Vec::new();
        while !reader.eof() {
            let (_, offset) = reader
                .read_with_offset()
                .map_err(|error| library_problem(&error))?;
            offsets.push(offset);
        }
        for (span, offset) in own_spans.iter().zip(offsets) {
            lines.insert(offset, line_of(&starts, span.offset()));
        }
    }

    if let Err(error) = crate::wasm::engine::validate(&wasm) {
        let offset = error.offset();
        let in_library = library_bodies
            .iter()
            .position(|(start, end)| (*start..*end).contains(&offset))
            .and_then(|at| library.get(at));
        let message = match in_library {
            Some(name) if error.message().contains("memory") => format!(
                "{}, in lengua's library function ${name}, which needs a memory: declare one \
                 and export it as \"memory\"",
                error.message()
            ),
            Some(name) => format!(
                "{}, in lengua's library function ${name}: a function main.wat defines with a \
                 library function's name must take what the library's takes",
                error.message()
            ),
            None => error.message().to_owned(),
        };
        return Err(Problem {
            line: line_at(&lines, &bodies, offset),
            message,
        });
    }
    Ok(Built {
        wasm,
        lines,
        bodies,
        assertions,
        library,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A module that writes "hi" with the library's `$write_all` and asserts on the way.
    const HELLO: &str = r#"(module
  (import "wasi_snapshot_preview1" "proc_exit" (func $exit (param i32)))
  (memory (export "memory") 1)
  (data (i32.const 64) "hi")
  (func (export "_start")
    (if (i32.ne (call $write_all (i32.const 1) (i32.const 64) (i32.const 2)) (i32.const 0))
      (then (call $assert_fail (i32.const 7))))
    (call $exit (i32.const 0))))
"#;

    #[test]
    fn the_library_functions_main_wat_calls_are_merged_with_what_they_need() {
        let built = build(HELLO).expect("it builds");
        assert_eq!(built.library, ["assert_fail", "write_all"]);
        // The assertion's number is on line 7.
        assert_eq!(built.assertions, BTreeMap::from([(7, 7)]));
        // main.wat's one function, every operator on its lines 6 to 8.
        assert_eq!(built.bodies.len(), 1);
        assert!(!built.lines.is_empty());
        assert!(
            built.lines.values().all(|line| (6..=8).contains(line)),
            "{:?}",
            built.lines
        );
        // Imported twice, as `$exit` and as `$lengua.proc_exit`, is legal.
        assert!(crate::wasm::engine::validate(&built.wasm).is_ok());
    }

    #[test]
    fn each_library_function_builds_alone() {
        for name in LIBRARY_FUNCTIONS {
            let call = match name {
                "assert_fail" => "(call $assert_fail (i32.const 1))",
                "fill" => {
                    "(drop (call $fill (i32.const 0) (i32.const 64) (i32.const 8) (i32.const -1)))"
                }
                _ => "(drop (call $write_all (i32.const 1) (i32.const 64) (i32.const 8)))",
            };
            let text = format!(
                "(module (memory (export \"memory\") 1) (func (export \"_start\") {call}))"
            );
            let built = build(&text).unwrap_or_else(|problem| panic!("{name}: {problem}"));
            assert!(built.library.iter().any(|merged| merged == name), "{name}");
        }
    }

    #[test]
    fn a_module_that_calls_no_library_function_is_built_as_it_is() {
        let text = "(module (memory (export \"memory\") 1) (func (export \"_start\")))";
        let built = build(text).expect("it builds");
        assert!(built.library.is_empty());
        assert!(built.assertions.is_empty());
    }

    #[test]
    fn a_function_main_wat_defines_is_its_own() {
        let text = "(module (memory (export \"memory\") 1)
  (func $assert_fail (param i32) (unreachable))
  (func (export \"_start\") (call $assert_fail (i32.const 3))))";
        let built = build(text).expect("it builds");
        assert!(built.library.is_empty(), "{:?}", built.library);
        // The library's $fill then calls main.wat's own $assert_fail.
        let both = "(module (memory (export \"memory\") 1)
  (func $assert_fail (param i32) (unreachable))
  (func (export \"_start\")
    (drop (call $fill (i32.const 0) (i32.const 64) (i32.const 8) (i32.const -1)))))";
        let built = build(both).expect("it builds");
        assert_eq!(built.library, ["fill"]);
    }

    #[test]
    fn problems_name_their_lines() {
        let unknown = build("(module\n  (func (export \"_start\")\n    (call $nowhere)))")
            .expect_err("an unknown function");
        assert_eq!(unknown.line, Some(3), "{unknown}");
        let mismatch =
            build("(module\n  (func (export \"_start\") (result i32)\n    (i64.const 1)))")
                .expect_err("a type mismatch");
        assert_eq!(mismatch.line, Some(3), "{mismatch}");
        let reserved = build("(module\n  (func $lengua.mine))").expect_err("a reserved name");
        assert!(reserved.message.contains("lengua keeps"), "{reserved}");
        assert_eq!(reserved.line, Some(2));
        let unparsed = build("(module (func (i32.nothing)))").expect_err("no such instruction");
        assert_eq!(unparsed.line, Some(1));
        let no_memory = build(
            "(module (func (export \"_start\") (call $assert_fail (i32.const \
                               1))))",
        )
        .expect_err("the library needs a memory");
        assert!(no_memory.message.contains("needs a memory"), "{no_memory}");
    }

    #[test]
    fn a_line_is_found_only_in_main_wat_s_own_code() {
        let built = build(HELLO).expect("it builds");
        let (start, end) = built.bodies[0];
        assert!(built.line_at(start).is_some() || built.line_at(start + 1).is_some());
        assert_eq!(built.line_at(end), None);
        assert_eq!(built.line_at(0), None);
    }

    #[test]
    fn a_place_among_a_functions_locals_is_its_own_first_line() {
        let text = "(module\n  (func $first\n    (nop))\n  (func $second (local i32)\n    (nop)))";
        let built = build(text).expect("it builds");
        // The second body starts with its locals, before its first operator on line 5: that
        // line, not the first function's last.
        let (start, _) = built.bodies[1];
        assert_eq!(built.line_at(start), Some(5));
        let (start, _) = built.bodies[0];
        assert_eq!(built.line_at(start), Some(3));
    }

    #[test]
    fn a_function_named_by_its_number_is_refused_where_the_library_merges_in() {
        // Alone, a number names the function it says.
        let alone = "(module (func $first) (func (export \"_start\") (call 0)))";
        assert!(build(alone).is_ok());
        // With the library's imports first, `call 0` would call its fd_write, which takes what
        // fd_read takes: a read would become a write.
        let merged = "(module
  (import \"wasi_snapshot_preview1\" \"fd_read\" (func (param i32 i32 i32 i32) (result i32)))
  (memory (export \"memory\") 1)
  (func (export \"_start\")
    (drop (call 0 (i32.const 0) (i32.const 64) (i32.const 1) (i32.const 8)))
    (drop (call $write_all (i32.const 1) (i32.const 64) (i32.const 1)))))";
        let refused = build(merged).expect_err("a number the library would change");
        assert_eq!(refused.line, Some(5), "{refused}");
        assert!(refused.message.contains("by its number"), "{refused}");
        // An export by number is refused the same way.
        let exported = "(module (memory (export \"memory\") 1)
  (func (call $assert_fail (i32.const 1)))
  (export \"_start\" (func 0)))";
        let refused = build(exported).expect_err("an export by number");
        assert_eq!(refused.line, Some(3), "{refused}");
    }
}
