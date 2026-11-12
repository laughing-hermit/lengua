//! main.wat as the checker reads it: the module lengua built from it, read back with
//! `wasmparser` (its types, imports, functions with each operator's offset and line, globals,
//! memories, exports, data segments and the names of its name section); the regions its globals
//! name; where each of main.wat's own fields is, read from its text with `wast`; and what its
//! comments say beside the code.

use std::collections::BTreeMap;

use wasmparser::{
    BlockType, CompositeInnerType, ConstExpr, ContType, DataKind, ExternalKind, FrameKind,
    FuncType, KnownCustom, MemoryType, ModuleArity, Name, NameMap, NameSectionReader, Operator,
    Parser, Payload, RefType, SubType, TypeRef, ValType,
};
use wast::Wat;
use wast::core::{
    DataVal, FuncKind, GlobalKind, ItemKind, MemoryKind, ModuleField, ModuleKind, TableKind,
};
use wast::parser::{self, ParseBuffer};
use wast::token::Span;

use crate::checker::UnsafeBlock;
use crate::wasm::text::Built;

/// The first address a program's region may start at: lower ones are lengua's, its library's
/// scratch bytes among them.
pub const FIRST_ADDRESS: u64 = 1024;

/// A WebAssembly page, in bytes.
pub const PAGE: u64 = 65_536;

/// The most pages main.wat's memory may have: 1 GiB. A program's memory is the worker's own, and
/// every test of a program runs in a worker, several at once, so its size has a stated end. The
/// largest memory of a program lengua ships has 1,538 pages; a C, C++ or Rust toolchain's module
/// that the benchmark runs is no main.wat, and isn't held to this.
pub const MAX_PAGES: u64 = 16_384;

/// The most locals the checker reads of a function: the validator's own limit.
const MAX_LOCALS: usize = 50_000;

/// Whose a function is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// Imported by main.wat.
    Import,
    /// Imported by lengua's library, for its own functions.
    LibraryImport,
    /// main.wat's own.
    Own,
    /// lengua's library's, merged in after main.wat's.
    Library,
}

/// One operator of a function's body.
#[derive(Debug, Clone)]
pub struct Op<'a> {
    /// The operator.
    pub operator: Operator<'a>,
    /// Its offset in the module.
    pub offset: u64,
    /// Its line in main.wat, for main.wat's own functions.
    pub line: Option<usize>,
}

/// A function of the module, in its index space: imports first, then the definitions.
#[derive(Debug, Clone)]
pub struct Function<'a> {
    /// Its index.
    pub index: u32,
    /// Whose it is.
    pub kind: Kind,
    /// Its name, without the `$`, from the name section.
    pub name: Option<String>,
    /// Its type's index.
    pub type_index: u32,
    /// For an import, the module and the name it's imported by.
    pub import: Option<(String, String)>,
    /// Its line in main.wat: the `func` of its definition or of its import.
    pub line: Option<usize>,
    /// What it takes.
    pub params: Vec<ValType>,
    /// What it gives.
    pub results: Vec<ValType>,
    /// The locals it declares, after its parameters.
    pub locals: Vec<ValType>,
    /// Its locals' names, its parameters' first, by index.
    pub local_names: BTreeMap<u32, String>,
    /// Its operators, the final `end` included; empty for an import.
    pub body: Vec<Op<'a>>,
}

impl Function<'_> {
    /// How a report names it: `$name`, or its index when it has no name.
    #[must_use]
    pub fn shown(&self) -> String {
        self.name.as_ref().map_or_else(
            || format!("function {}", self.index),
            |name| format!("${name}"),
        )
    }

    /// How a report names its local `index`.
    #[must_use]
    pub fn local_shown(&self, index: u32) -> String {
        self.local_names
            .get(&index)
            .map_or_else(|| format!("local {index}"), |name| format!("${name}"))
    }

    /// The type of its local `index`, its parameters counted first.
    #[must_use]
    pub fn local_type(&self, index: u32) -> Option<ValType> {
        let index = usize::try_from(index).ok()?;
        match index.checked_sub(self.params.len()) {
            None => self.params.get(index).copied(),
            Some(declared) => self.locals.get(declared).copied(),
        }
    }

    /// How many locals it has, its parameters included.
    #[must_use]
    pub fn local_count(&self) -> usize {
        self.params.len() + self.locals.len()
    }

    /// Whether it's the WASI function `name`, imported by main.wat or by lengua's library.
    #[must_use]
    pub fn is_host(&self, name: &str) -> bool {
        matches!(self.kind, Kind::Import | Kind::LibraryImport)
            && self
                .import
                .as_ref()
                .is_some_and(|(module, field)| module == crate::wasm::wasi::MODULE && field == name)
    }

    /// Whether it's one of lengua's library's functions called `name`.
    #[must_use]
    pub fn is_library(&self, name: &str) -> bool {
        self.kind == Kind::Library && self.name.as_deref() == Some(name)
    }

    /// The function of lengua's host it imports, when it imports one: WASI's, or one of
    /// lengua's own network functions.
    #[must_use]
    pub fn host(&self) -> Option<&'static crate::wasm::wasi::Function> {
        let (module, field) = self.import.as_ref()?;
        crate::wasm::wasi::imported(module, field)
    }

    /// Whether a call of it never returns: `proc_exit`, or lengua's `$assert_fail`.
    #[must_use]
    pub fn never_returns(&self) -> bool {
        self.is_host("proc_exit") || self.is_library("assert_fail")
    }
}

/// A global of the module.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Global {
    /// Its index.
    pub index: u32,
    /// Its name, without the `$`.
    pub name: Option<String>,
    /// Its type.
    pub ty: ValType,
    /// Whether the program may change it.
    pub mutable: bool,
    /// Whether it's imported.
    pub imported: bool,
    /// Its value, when it's an integer constant: an i32's sign-extended.
    pub value: Option<i64>,
    /// Its line in main.wat.
    pub line: Option<usize>,
}

impl Global {
    /// Whether it's a named constant: immutable, defined and set to an integer constant.
    #[must_use]
    pub fn constant(&self) -> bool {
        !self.mutable && !self.imported && self.value.is_some()
    }

    /// Its value as an unsigned number of its width, when it's a constant.
    #[must_use]
    pub fn unsigned(&self) -> Option<u64> {
        let value = self.value.filter(|_| self.constant())?;
        match self.ty {
            ValType::I32 => Some(u64::from(i32::try_from(value).ok()?.cast_unsigned())),
            ValType::I64 => Some(value.cast_unsigned()),
            _ => None,
        }
    }

    /// How a report names it.
    #[must_use]
    pub fn shown(&self) -> String {
        self.name.as_ref().map_or_else(
            || format!("global {}", self.index),
            |name| format!("${name}"),
        )
    }
}

/// A memory of the module.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Memory {
    /// Its type.
    pub ty: MemoryType,
    /// Whether it's imported.
    pub imported: bool,
    /// Its line in main.wat.
    pub line: Option<usize>,
}

impl Memory {
    /// How many bytes it holds when the program starts, which with its maximum equal to its
    /// minimum is all it ever holds.
    #[must_use]
    pub fn bytes(&self) -> u64 {
        self.ty.initial.saturating_mul(PAGE)
    }
}

/// An export of the module.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Export {
    /// Its name.
    pub name: String,
    /// What it exports.
    pub kind: ExternalKind,
    /// The index of what it exports.
    pub index: u32,
    /// Its line in main.wat.
    pub line: Option<usize>,
}

/// One value of a data segment as main.wat writes it: a string, or numbers such as `(i8 1 2)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Piece {
    /// For a string, the line it starts on; `None` for numbers.
    pub line: Option<usize>,
    /// How many bytes it gives the segment.
    pub len: u64,
}

/// A data segment of the module.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Data {
    /// Its index.
    pub index: u32,
    /// Whether it's main.wat's, not lengua's library's.
    pub own: bool,
    /// For an active segment, its memory and the address it's written at when that's a constant;
    /// `None` for a passive one.
    pub at: Option<(u32, Option<u64>)>,
    /// How many bytes it holds.
    pub len: u64,
    /// Its line in main.wat.
    pub line: Option<usize>,
    /// Its values in order, as main.wat writes them, which together are its bytes; none for
    /// lengua's library's, and none when main.wat's strings can't be told apart.
    pub pieces: Vec<Piece>,
}

/// A region of the program's memory: an immutable i32 global `$x` holding its address, with
/// `$x.size` (writable) or `$x.rosize` (read-only) holding its size.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Region {
    /// Its name, without the `$`.
    pub name: String,
    /// The global holding its address.
    pub global: u32,
    /// The global holding its size.
    pub size_global: u32,
    /// Its first byte's address.
    pub start: u64,
    /// Its size in bytes.
    pub size: u64,
    /// Whether it's read-only: its size is `$x.rosize`.
    pub read_only: bool,
    /// The line of its address global.
    pub line: Option<usize>,
    /// The reason after `;; shared:` on that line, when it has one.
    pub shared: Option<String>,
}

impl Region {
    /// The address just past it.
    #[must_use]
    pub fn end(&self) -> u64 {
        self.start.saturating_add(self.size)
    }
}

/// What main.wat's text says beside its code.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Notes {
    /// The text of each line comment after its `;;`, trimmed, by line.
    pub comments: BTreeMap<usize, Vec<String>>,
    /// The lines with an annotation, `(@…`.
    pub annotations: Vec<usize>,
}

impl Notes {
    /// What follows `key` (such as `bound:`) in a comment on `line`, trimmed.
    #[must_use]
    pub fn marker(&self, line: usize, key: &str) -> Option<&str> {
        self.comments
            .get(&line)?
            .iter()
            .find_map(|comment| comment.strip_prefix(key).map(str::trim))
    }
}

/// A problem with the comments that mark unsafe blocks: its line, what's wrong and the fix.
pub type Mark = (usize, String, String);

/// main.wat's module as the checker reads it.
#[derive(Debug, Clone)]
pub struct Module<'a> {
    /// The module's bytes.
    pub wasm: &'a [u8],
    /// Its types, by index.
    pub sub_types: Vec<SubType>,
    /// Its functions, by index.
    pub functions: Vec<Function<'a>>,
    /// Its globals, by index.
    pub globals: Vec<Global>,
    /// Its memories, by index.
    pub memories: Vec<Memory>,
    /// Its tables' lines, imported and defined.
    pub tables: Vec<Option<usize>>,
    /// Its element segments' lines.
    pub elements: Vec<Option<usize>>,
    /// Its start function, with the line that names it.
    pub start: Option<(u32, Option<usize>)>,
    /// Its exports.
    pub exports: Vec<Export>,
    /// Its data segments.
    pub data: Vec<Data>,
    /// The names of its custom sections.
    pub customs: Vec<String>,
    /// The non-function imports main.wat makes, as (module, name, what, line).
    pub other_imports: Vec<(String, String, &'static str, Option<usize>)>,
    /// The regions its globals name.
    pub regions: Vec<Region>,
    /// The names its name section gives besides functions', locals' and globals': what each
    /// names, its index there, and the name.
    pub other_names: Vec<(Named, u32, String)>,
    /// What its text says beside the code.
    pub notes: Notes,
    /// Its `;; unsafe:` … `;; end unsafe` blocks.
    pub unsafe_blocks: Vec<UnsafeBlock>,
    /// What's wrong with the comments that mark them.
    pub unsafe_marks: Vec<Mark>,
}

/// Where each of main.wat's fields is, by line, in the order of their index spaces.
#[derive(Debug, Default)]
struct Places {
    /// Function imports.
    func_imports: Vec<usize>,
    /// Function definitions.
    funcs: Vec<usize>,
    /// Global imports.
    global_imports: Vec<usize>,
    /// Global definitions.
    globals: Vec<usize>,
    /// Memories, imported and defined.
    memories: Vec<usize>,
    /// Tables, imported and defined.
    tables: Vec<usize>,
    /// Data segments.
    data: Vec<usize>,
    /// Each data segment's values, as [`Data::pieces`] keeps them.
    pieces: Vec<Vec<Piece>>,
    /// Element segments.
    elems: Vec<usize>,
    /// Exports, by name.
    exports: Vec<(String, usize)>,
    /// The start function's index.
    start: Option<usize>,
}

/// What a name of the name section names, besides a function, a local or a global.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Named {
    /// The module.
    Module,
    /// A type.
    Type,
    /// A table.
    Table,
    /// A memory.
    Memory,
    /// An element segment.
    Element,
    /// A tag.
    Tag,
    /// A block's label in the function of this index.
    Label(u32),
}

impl Named {
    /// How a report says what it is.
    #[must_use]
    pub fn shown(self) -> &'static str {
        match self {
            Self::Module => "the module",
            Self::Type => "a type",
            Self::Table => "a table",
            Self::Memory => "a memory",
            Self::Element => "an element segment",
            Self::Tag => "a tag",
            Self::Label(_) => "a label",
        }
    }
}

/// The names of the module's name section.
#[derive(Debug, Default)]
struct Names {
    /// Functions'.
    functions: BTreeMap<u32, String>,
    /// Each function's locals', by function.
    locals: BTreeMap<u32, BTreeMap<u32, String>>,
    /// Globals'.
    globals: BTreeMap<u32, String>,
    /// The others: what each names, its index there, and the name.
    others: Vec<(Named, u32, String)>,
}

impl Names {
    /// Reads the name section `section`; a subsection it can't read names nothing.
    fn read(&mut self, section: NameSectionReader<'_>) {
        for subsection in section.flatten() {
            match subsection {
                Name::Function(map) => {
                    for naming in map.flatten() {
                        self.functions.insert(naming.index, naming.name.to_owned());
                    }
                }
                Name::Local(map) => {
                    for function in map.flatten() {
                        let names = function
                            .names
                            .flatten()
                            .map(|naming| (naming.index, naming.name.to_owned()))
                            .collect();
                        self.locals.insert(function.index, names);
                    }
                }
                Name::Global(map) => {
                    for naming in map.flatten() {
                        self.globals.insert(naming.index, naming.name.to_owned());
                    }
                }
                Name::Label(map) => {
                    for function in map.flatten() {
                        self.other(Named::Label(function.index), function.names);
                    }
                }
                Name::Module { name, .. } => {
                    self.others.push((Named::Module, 0, name.to_owned()));
                }
                Name::Type(map) => self.other(Named::Type, map),
                Name::Table(map) => self.other(Named::Table, map),
                Name::Memory(map) => self.other(Named::Memory, map),
                Name::Element(map) => self.other(Named::Element, map),
                Name::Tag(map) => self.other(Named::Tag, map),
                _ => {}
            }
        }
    }

    /// Keeps the names `map` gives to what it names, each `named`.
    fn other(&mut self, named: Named, map: NameMap<'_>) {
        for naming in map.flatten() {
            self.others
                .push((named, naming.index, naming.name.to_owned()));
        }
    }
}

/// `value` as a `usize`, saturating.
#[must_use]
pub fn to_usize(value: u32) -> usize {
    usize::try_from(value).unwrap_or(usize::MAX)
}

/// `value` as a `u32`, saturating.
#[must_use]
pub fn to_u32(value: usize) -> u32 {
    u32::try_from(value).unwrap_or(u32::MAX)
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

/// The value of a constant expression that is one integer constant.
fn constant(expression: &ConstExpr<'_>) -> Option<i64> {
    let mut reader = expression.get_operators_reader();
    let value = match reader.read().ok()? {
        Operator::I32Const { value } => i64::from(value),
        Operator::I64Const { value } => value,
        _ => return None,
    };
    matches!(reader.read().ok()?, Operator::End).then_some(value)
}

/// The names an inline export gives to what it exports at `line`.
fn exported(names: &[&str], line: usize) -> Vec<(String, usize)> {
    names
        .iter()
        .map(|name| ((*name).to_owned(), line))
        .collect()
}

/// The line of each string between the data field at `from` and the parenthesis that closes
/// it, in order, skipping comments: `from` is where the field's keyword starts, or its opening
/// parenthesis.
fn string_lines(text: &str, starts: &[usize], from: usize) -> Vec<usize> {
    let bytes = text.as_bytes();
    let mut found = Vec::new();
    let mut at = from;
    // How deep inside parentheses: the field's own is open already at its keyword.
    let mut depth = usize::from(bytes.get(from) != Some(&b'('));
    // How deep inside block comments, `(; … ;)`, which nest.
    let mut comment = 0_usize;
    while at < bytes.len() {
        let rest = &bytes[at..];
        if comment > 0 {
            if rest.starts_with(b";)") {
                comment -= 1;
                at += 2;
            } else if rest.starts_with(b"(;") {
                comment += 1;
                at += 2;
            } else {
                at += 1;
            }
        } else if rest.starts_with(b";;") {
            at = text[at..].find('\n').map_or(text.len(), |found| at + found);
        } else if rest.starts_with(b"(;") {
            comment += 1;
            at += 2;
        } else if bytes[at] == b'"' {
            found.push(line_of(starts, at));
            at += 1;
            while at < bytes.len() && bytes[at] != b'"' {
                if bytes[at] == b'\\' {
                    at += 1;
                }
                at += 1;
            }
            at += 1;
        } else if bytes[at] == b'(' {
            depth += 1;
            at += 1;
        } else if bytes[at] == b')' {
            depth = depth.saturating_sub(1);
            at += 1;
            if depth == 0 {
                break;
            }
        } else {
            at += 1;
        }
    }
    found
}

/// The values of the data field `data` of `text`, each with its bytes, and each string with its
/// line. Nothing when the strings in the text aren't one for one the strings `wast` read, as
/// for a segment with a name annotation, which holds a string of its own: then no string of
/// the segment can be named.
fn pieces(text: &str, starts: &[usize], data: &wast::core::Data<'_>) -> Vec<Piece> {
    if data.name.is_some() {
        return Vec::new();
    }
    let lines = string_lines(text, starts, data.span.offset());
    let strings = data
        .data
        .iter()
        .filter(|value| matches!(value, DataVal::String(_)))
        .count();
    if lines.len() != strings {
        return Vec::new();
    }
    let mut lines = lines.into_iter();
    data.data
        .iter()
        .map(|value| {
            let (line, len) = match value {
                DataVal::String(bytes) => (lines.next(), bytes.len()),
                DataVal::Integral(bytes) => (None, bytes.len()),
            };
            Piece {
                line,
                len: u64::try_from(len).unwrap_or(u64::MAX),
            }
        })
        .collect()
}

/// Where each of main.wat's fields is, read from its text; nothing when the text doesn't parse,
/// which a text the build took always does.
fn places(text: &str, starts: &[usize]) -> Places {
    let mut places = Places::default();
    let Ok(buffer) = ParseBuffer::new(text) else {
        return places;
    };
    let Ok(Wat::Module(module)) = parser::parse::<Wat>(&buffer) else {
        return places;
    };
    let ModuleKind::Text(fields) = &module.kind else {
        return places;
    };
    let line = |span: Span| line_of(starts, span.offset());
    for field in fields {
        match field {
            ModuleField::Import(imports) => {
                for sig in imports.item_sigs() {
                    let at = line(sig.span);
                    match sig.kind {
                        ItemKind::Func(_) | ItemKind::FuncExact(_) => places.func_imports.push(at),
                        ItemKind::Global(_) => places.global_imports.push(at),
                        ItemKind::Memory(_) => places.memories.push(at),
                        ItemKind::Table(_) => places.tables.push(at),
                        ItemKind::Tag(_) => {}
                    }
                }
            }
            ModuleField::Func(func) => {
                let at = line(func.span);
                if matches!(func.kind, FuncKind::Import(..)) {
                    places.func_imports.push(at);
                } else {
                    places.funcs.push(at);
                }
                places.exports.extend(exported(&func.exports.names, at));
            }
            ModuleField::Global(global) => {
                let at = line(global.span);
                if matches!(global.kind, GlobalKind::Import(_)) {
                    places.global_imports.push(at);
                } else {
                    places.globals.push(at);
                }
                places.exports.extend(exported(&global.exports.names, at));
            }
            ModuleField::Memory(memory) => {
                let at = line(memory.span);
                places.memories.push(at);
                // Inline data becomes a segment of its own, written at 0.
                if matches!(memory.kind, MemoryKind::Inline { .. }) {
                    places.data.push(at);
                    places.pieces.push(Vec::new());
                }
                places.exports.extend(exported(&memory.exports.names, at));
            }
            ModuleField::Table(table) => {
                let at = line(table.span);
                places.tables.push(at);
                if matches!(table.kind, TableKind::Inline { .. }) {
                    places.elems.push(at);
                }
                places.exports.extend(exported(&table.exports.names, at));
            }
            ModuleField::Data(data) => {
                places.data.push(line(data.span));
                places.pieces.push(pieces(text, starts, data));
            }
            ModuleField::Elem(elem) => places.elems.push(line(elem.span)),
            ModuleField::Export(export) => {
                places
                    .exports
                    .push((export.name.to_owned(), line(export.span)));
            }
            ModuleField::Start(index) => places.start = Some(line(index.span())),
            _ => {}
        }
    }
    places
}

/// What main.wat's text says beside its code: its line comments and where its annotations are,
/// skipping strings and block comments.
fn notes(text: &str) -> Notes {
    let mut notes = Notes::default();
    let bytes = text.as_bytes();
    let mut line = 1;
    let mut at = 0;
    // How deep inside block comments, `(; … ;)`, which nest.
    let mut depth = 0_usize;
    while at < bytes.len() {
        let rest = &bytes[at..];
        if bytes[at] == b'\n' {
            line += 1;
            at += 1;
        } else if depth > 0 {
            if rest.starts_with(b";)") {
                depth -= 1;
                at += 2;
            } else if rest.starts_with(b"(;") {
                depth += 1;
                at += 2;
            } else {
                at += 1;
            }
        } else if rest.starts_with(b";;") {
            let end = text[at..].find('\n').map_or(text.len(), |found| at + found);
            let comment = text[at + 2..end].trim().to_owned();
            notes.comments.entry(line).or_default().push(comment);
            at = end;
        } else if rest.starts_with(b"(;") {
            depth += 1;
            at += 2;
        } else if rest.starts_with(b"(@") {
            notes.annotations.push(line);
            at += 2;
        } else if bytes[at] == b'"' {
            at += 1;
            while at < bytes.len() && bytes[at] != b'"' {
                if bytes[at] == b'\\' {
                    at += 1;
                }
                if bytes.get(at) == Some(&b'\n') {
                    line += 1;
                }
                at += 1;
            }
            at += 1;
        } else {
            at += 1;
        }
    }
    notes
}

/// The `;; unsafe: <reason>` … `;; end unsafe` blocks the comments mark, and what's wrong with
/// the marks.
fn unsafe_blocks(notes: &Notes) -> (Vec<UnsafeBlock>, Vec<Mark>) {
    let mut blocks = Vec::new();
    let mut marks = Vec::new();
    let mut open: Option<(usize, String)> = None;
    for (line, comments) in &notes.comments {
        for comment in comments {
            if let Some(reason) = comment.strip_prefix("unsafe:") {
                let reason = reason.trim();
                if open.is_some() {
                    marks.push((
                        *line,
                        "unsafe blocks can't nest".to_owned(),
                        "close the first block with `;; end unsafe` before starting another"
                            .to_owned(),
                    ));
                    continue;
                }
                if reason.is_empty() {
                    marks.push((
                        *line,
                        "`;; unsafe:` needs a reason".to_owned(),
                        "say after `;; unsafe:` why the unchecked accesses stay in bounds"
                            .to_owned(),
                    ));
                }
                open = Some((*line, reason.to_owned()));
            } else if comment == "end unsafe" {
                match open.take() {
                    Some((start, reason)) => blocks.push(UnsafeBlock {
                        start,
                        end: *line,
                        reason,
                    }),
                    None => marks.push((
                        *line,
                        "`;; end unsafe` has no `;; unsafe:` before it".to_owned(),
                        "start the block with `;; unsafe: <reason>`".to_owned(),
                    )),
                }
            }
        }
    }
    if let Some((start, _)) = open {
        marks.push((
            start,
            "this `;; unsafe:` block has no `;; end unsafe`".to_owned(),
            "close the block with `;; end unsafe` after the accesses it covers".to_owned(),
        ));
    }
    (blocks, marks)
}

/// The regions the globals name: for each `$x.size` or `$x.rosize` that is an immutable i32
/// constant, the immutable i32 constant `$x`. What's wrong with the convention is the structure
/// pass's to report; a region named twice keeps its first size.
fn regions(globals: &[Global], notes: &Notes) -> Vec<Region> {
    let by_name: BTreeMap<&str, &Global> = globals
        .iter()
        .filter_map(|global| Some((global.name.as_deref()?, global)))
        .collect();
    let mut regions: Vec<Region> = Vec::new();
    for size in globals {
        let Some(name) = size.name.as_deref() else {
            continue;
        };
        let (base, read_only) = match (name.strip_suffix(".size"), name.strip_suffix(".rosize")) {
            (Some(base), _) => (base, false),
            (None, Some(base)) => (base, true),
            (None, None) => continue,
        };
        let Some(address) = by_name.get(base) else {
            continue;
        };
        let (Some(start), Some(bytes)) = (address.unsigned(), size.unsigned()) else {
            continue;
        };
        if address.ty != ValType::I32 || size.ty != ValType::I32 {
            continue;
        }
        if regions.iter().any(|region| region.global == address.index) {
            continue;
        }
        regions.push(Region {
            name: base.to_owned(),
            global: address.index,
            size_global: size.index,
            start,
            size: bytes,
            read_only,
            line: address.line,
            shared: address
                .line
                .and_then(|line| notes.marker(line, "shared:"))
                .map(str::to_owned),
        });
    }
    regions
}

impl<'a> Module<'a> {
    /// Reads `built`, the module lengua built from main.wat's `text`: as wast encoded it, with
    /// its names ([`Built::named`]), whose bytes before the names are the module that runs.
    ///
    /// # Errors
    ///
    /// What couldn't be read back, which a module the build validated never has.
    #[allow(clippy::too_many_lines)]
    pub fn read(text: &str, built: &'a Built) -> Result<Self, String> {
        let starts = line_starts(text);
        let places = places(text, &starts);
        let notes = notes(text);
        let wasm = built.named.as_slice();
        let failed = |error: wasmparser::BinaryReaderError| {
            format!("lengua couldn't read the module back: {error}")
        };
        let mut sub_types = Vec::new();
        let mut imports: Vec<(String, String, TypeRef)> = Vec::new();
        let mut defined: Vec<u32> = Vec::new();
        let mut defined_memories = Vec::new();
        let mut defined_tables = 0_usize;
        let mut element_count = 0_usize;
        let mut defined_globals = Vec::new();
        let mut raw_exports = Vec::new();
        let mut start = None;
        let mut segments = Vec::new();
        let mut bodies = Vec::new();
        let mut customs = Vec::new();
        let mut names = Names::default();
        for payload in Parser::new(0).parse_all(wasm) {
            match payload.map_err(failed)? {
                Payload::TypeSection(reader) => {
                    for group in reader {
                        sub_types.extend(group.map_err(failed)?.into_types());
                    }
                }
                Payload::ImportSection(reader) => {
                    for import in reader.into_imports() {
                        let import = import.map_err(failed)?;
                        imports.push((import.module.to_owned(), import.name.to_owned(), import.ty));
                    }
                }
                Payload::FunctionSection(reader) => {
                    for type_index in reader {
                        defined.push(type_index.map_err(failed)?);
                    }
                }
                Payload::TableSection(reader) => defined_tables += to_usize(reader.count()),
                Payload::MemorySection(reader) => {
                    for memory in reader {
                        defined_memories.push(memory.map_err(failed)?);
                    }
                }
                Payload::GlobalSection(reader) => {
                    for global in reader {
                        let global = global.map_err(failed)?;
                        defined_globals.push((global.ty, constant(&global.init_expr)));
                    }
                }
                Payload::ExportSection(reader) => {
                    for export in reader {
                        let export = export.map_err(failed)?;
                        raw_exports.push((export.name.to_owned(), export.kind, export.index));
                    }
                }
                Payload::StartSection { func, .. } => start = Some(func),
                Payload::ElementSection(reader) => element_count += to_usize(reader.count()),
                Payload::DataSection(reader) => {
                    for segment in reader {
                        let segment = segment.map_err(failed)?;
                        let len = u64::try_from(segment.data.len()).unwrap_or(u64::MAX);
                        let at = match segment.kind {
                            DataKind::Passive => None,
                            DataKind::Active {
                                memory_index,
                                offset_expr,
                            } => {
                                let address = constant(&offset_expr)
                                    .and_then(|value| i32::try_from(value).ok())
                                    .map(|value| u64::from(value.cast_unsigned()));
                                Some((memory_index, address))
                            }
                        };
                        segments.push((at, len));
                    }
                }
                Payload::CodeSectionEntry(body) => bodies.push(body),
                Payload::CustomSection(reader) => {
                    if let KnownCustom::Name(section) = reader.as_known() {
                        names.read(section);
                    }
                    customs.push(reader.name().to_owned());
                }
                _ => {}
            }
        }

        let signature = |type_index: u32| -> (Vec<ValType>, Vec<ValType>) {
            match sub_types
                .get(to_usize(type_index))
                .map(|sub| &sub.composite_type.inner)
            {
                Some(CompositeInnerType::Func(func)) => {
                    (func.params().to_vec(), func.results().to_vec())
                }
                _ => (Vec::new(), Vec::new()),
            }
        };

        // Functions: the library's imports first, then main.wat's, then main.wat's definitions
        // and the library's.
        let function_imports: Vec<(String, String, u32)> = imports
            .iter()
            .filter_map(|(module, field, ty)| match ty {
                TypeRef::Func(type_index) | TypeRef::FuncExact(type_index) => {
                    Some((module.clone(), field.clone(), *type_index))
                }
                _ => None,
            })
            .collect();
        let library_imports = function_imports
            .len()
            .saturating_sub(places.func_imports.len());
        let mut functions: Vec<Function<'a>> = Vec::new();
        for (position, (module, field, type_index)) in function_imports.into_iter().enumerate() {
            let index = to_u32(functions.len());
            let own = position >= library_imports;
            let (params, results) = signature(type_index);
            functions.push(Function {
                index,
                kind: if own {
                    Kind::Import
                } else {
                    Kind::LibraryImport
                },
                name: names.functions.get(&index).cloned(),
                type_index,
                import: Some((module, field)),
                line: if own {
                    places.func_imports.get(position - library_imports).copied()
                } else {
                    None
                },
                params,
                results,
                locals: Vec::new(),
                local_names: names.locals.get(&index).cloned().unwrap_or_default(),
                body: Vec::new(),
            });
        }
        let mut own_count = 0_usize;
        for (type_index, body) in defined.iter().copied().zip(bodies) {
            let index = to_u32(functions.len());
            let range = body.range();
            let own = built
                .bodies
                .iter()
                .any(|(start, end)| *start == range.start && *end == range.end);
            let mut locals = Vec::new();
            for declared in body.get_locals_reader().map_err(failed)? {
                let (count, ty) = declared.map_err(failed)?;
                let room = MAX_LOCALS.saturating_sub(locals.len());
                locals.extend(std::iter::repeat_n(ty, to_usize(count).min(room)));
            }
            let mut reader = body.get_operators_reader().map_err(failed)?;
            let mut ops = Vec::new();
            while !reader.eof() {
                let (operator, offset) = reader.read_with_offset().map_err(failed)?;
                ops.push(Op {
                    operator,
                    offset,
                    line: if own { built.line_at(offset) } else { None },
                });
            }
            let line = if own {
                own_count += 1;
                places.funcs.get(own_count - 1).copied()
            } else {
                None
            };
            let (params, results) = signature(type_index);
            functions.push(Function {
                index,
                kind: if own { Kind::Own } else { Kind::Library },
                name: names.functions.get(&index).cloned(),
                type_index,
                import: None,
                line,
                params,
                results,
                locals,
                local_names: names.locals.get(&index).cloned().unwrap_or_default(),
                body: ops,
            });
        }

        // Globals, memories and tables: the imported first, each in its index space.
        let mut globals = Vec::new();
        let mut memories = Vec::new();
        let mut tables = Vec::new();
        let mut other_imports = Vec::new();
        let mut global_lines = places.global_imports.iter().chain(&places.globals).copied();
        let mut memory_lines = places.memories.iter().copied();
        let mut table_lines = places.tables.iter().copied();
        for (module, field, ty) in &imports {
            let (what, line) = match ty {
                TypeRef::Func(_) | TypeRef::FuncExact(_) => continue,
                TypeRef::Global(global) => {
                    let index = to_u32(globals.len());
                    let line = global_lines.next();
                    globals.push(Global {
                        index,
                        name: names.globals.get(&index).cloned(),
                        ty: global.content_type,
                        mutable: global.mutable,
                        imported: true,
                        value: None,
                        line,
                    });
                    ("a global", line)
                }
                TypeRef::Memory(memory) => {
                    let line = memory_lines.next();
                    memories.push(Memory {
                        ty: *memory,
                        imported: true,
                        line,
                    });
                    ("a memory", line)
                }
                TypeRef::Table(_) => {
                    let line = table_lines.next();
                    tables.push(line);
                    ("a table", line)
                }
                TypeRef::Tag(_) => ("a tag", None),
            };
            other_imports.push((module.clone(), field.clone(), what, line));
        }
        for (ty, value) in defined_globals {
            let index = to_u32(globals.len());
            globals.push(Global {
                index,
                name: names.globals.get(&index).cloned(),
                ty: ty.content_type,
                mutable: ty.mutable,
                imported: false,
                value,
                line: global_lines.next(),
            });
        }
        for ty in defined_memories {
            memories.push(Memory {
                ty,
                imported: false,
                line: memory_lines.next(),
            });
        }
        tables.extend((0..defined_tables).map(|_| table_lines.next()));
        let elements = (0..element_count)
            .map(|position| places.elems.get(position).copied())
            .collect();

        let exports = raw_exports
            .into_iter()
            .map(|(name, kind, index)| Export {
                line: places
                    .exports
                    .iter()
                    .find(|(exported, _)| *exported == name)
                    .map(|(_, line)| *line),
                name,
                kind,
                index,
            })
            .collect();
        let data = segments
            .into_iter()
            .enumerate()
            .map(|(position, (at, len))| Data {
                index: to_u32(position),
                own: position < places.data.len(),
                at,
                len,
                line: places.data.get(position).copied(),
                // The values main.wat writes, when they add up to the segment's bytes.
                pieces: places
                    .pieces
                    .get(position)
                    .filter(|pieces| pieces.iter().map(|piece| piece.len).sum::<u64>() == len)
                    .cloned()
                    .unwrap_or_default(),
            })
            .collect();
        let regions = regions(&globals, &notes);
        let (unsafe_blocks, unsafe_marks) = unsafe_blocks(&notes);
        Ok(Self {
            wasm,
            sub_types,
            functions,
            globals,
            memories,
            tables,
            elements,
            start: start.map(|function| (function, places.start)),
            exports,
            data,
            customs,
            other_imports,
            regions,
            other_names: names.others,
            notes,
            unsafe_blocks,
            unsafe_marks,
        })
    }

    /// The function `index`.
    #[must_use]
    pub fn function(&self, index: u32) -> Option<&Function<'a>> {
        self.functions.get(to_usize(index))
    }

    /// The global `index`.
    #[must_use]
    pub fn global(&self, index: u32) -> Option<&Global> {
        self.globals.get(to_usize(index))
    }

    /// main.wat's own functions, in order.
    pub fn own(&self) -> impl Iterator<Item = &Function<'a>> {
        self.functions
            .iter()
            .filter(|function| function.kind == Kind::Own)
    }

    /// The function `_start` exports.
    #[must_use]
    pub fn entry(&self) -> Option<u32> {
        self.exports
            .iter()
            .find(|export| export.name == "_start" && export.kind == ExternalKind::Func)
            .map(|export| export.index)
    }

    /// The region whose address global `global` is.
    #[must_use]
    pub fn region_of(&self, global: u32) -> Option<usize> {
        self.regions
            .iter()
            .position(|region| region.global == global)
    }

    /// The region whose size global `global` is.
    #[must_use]
    pub fn sized_by(&self, global: u32) -> Option<usize> {
        self.regions
            .iter()
            .position(|region| region.size_global == global)
    }

    /// The parameters and results a block of type `ty` takes and gives.
    #[must_use]
    pub fn block_arity(&self, ty: BlockType) -> (usize, usize) {
        match ty {
            BlockType::Empty => (0, 0),
            BlockType::Type(_) => (0, 1),
            BlockType::FuncType(index) => match self
                .sub_types
                .get(to_usize(index))
                .map(|sub| &sub.composite_type.inner)
            {
                Some(CompositeInnerType::Func(func)) => (func.params().len(), func.results().len()),
                _ => (0, 0),
            },
        }
    }

    /// The types of the parameters a block of type `ty` takes.
    #[must_use]
    pub fn block_params(&self, ty: BlockType) -> Vec<ValType> {
        match ty {
            BlockType::Empty | BlockType::Type(_) => Vec::new(),
            BlockType::FuncType(index) => match self
                .sub_types
                .get(to_usize(index))
                .map(|sub| &sub.composite_type.inner)
            {
                Some(CompositeInnerType::Func(func)) => func.params().to_vec(),
                _ => Vec::new(),
            },
        }
    }

    /// How many values `operator` takes and gives inside `frames` (outermost first, the
    /// function's own first), or none when the checker can't say.
    #[must_use]
    pub fn arity(
        &self,
        frames: &[(BlockType, FrameKind)],
        operator: &Operator<'_>,
    ) -> (usize, usize) {
        let context = Arity {
            module: self,
            frames,
        };
        operator
            .operator_arity(&context)
            .map_or((0, 0), |(takes, gives)| (to_usize(takes), to_usize(gives)))
    }

    /// The frame of function `function`'s own body, for [`Self::arity`].
    #[must_use]
    pub fn function_frame(function: &Function<'_>) -> (BlockType, FrameKind) {
        (BlockType::FuncType(function.type_index), FrameKind::Block)
    }

    /// Whether `line` is inside an `;; unsafe:` block.
    #[must_use]
    pub fn in_unsafe_block(&self, line: Option<usize>) -> bool {
        line.is_some_and(|line| {
            self.unsafe_blocks
                .iter()
                .any(|block| (block.start..=block.end).contains(&line))
        })
    }
}

/// What `operator_arity` needs: the module's types and functions, and the blocks around an
/// operator.
struct Arity<'m, 'a> {
    /// The module.
    module: &'m Module<'a>,
    /// The blocks around the operator, outermost first.
    frames: &'m [(BlockType, FrameKind)],
}

impl ModuleArity for Arity<'_, '_> {
    fn sub_type_at(&self, type_idx: u32) -> Option<&SubType> {
        self.module.sub_types.get(to_usize(type_idx))
    }

    fn tag_type_arity(&self, _at: u32) -> Option<(u32, u32)> {
        None
    }

    fn type_index_of_function(&self, function_idx: u32) -> Option<u32> {
        self.module
            .function(function_idx)
            .map(|function| function.type_index)
    }

    fn func_type_of_cont_type(&self, _c: &ContType) -> Option<&FuncType> {
        None
    }

    fn sub_type_of_ref_type(&self, _rt: &RefType) -> Option<&SubType> {
        None
    }

    fn control_stack_height(&self) -> u32 {
        to_u32(self.frames.len())
    }

    fn label_block(&self, depth: u32) -> Option<(BlockType, FrameKind)> {
        let at = self
            .frames
            .len()
            .checked_sub(to_usize(depth).checked_add(1)?)?;
        self.frames.get(at).copied()
    }
}
