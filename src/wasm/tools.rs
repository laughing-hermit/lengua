//! The tools of a run: what a programmer expects beside `check`. Each is off unless a run asks
//! for it, and a run that asks for none pays for none.
//!
//! - A trace ([`Trace`]): every call the program made to the host, in order, with the numbers it
//!   passed and what the host answered. It never holds a byte of the program's memory, and for
//!   an idea that names a secret it holds none of the numbers either ([`Detail::Names`]). It
//!   keeps the first [`TRACE_CALLS`] calls and counts the rest.
//! - A profile ([`Samples`]): where the run's time went, by function. The engine looks at its
//!   clock at the start of a function and at the head of a loop; a profiled run moves that clock
//!   on every [`PROFILE_TICK`], and each look notes the functions on the stack, with what
//!   Wasmtime's own backtrace gives.
//! - A stop ([`plan`], [`stopped`]): the run ends when it reaches a function or a line of
//!   main.wat, and says the values on hand. The engine lengua pins has no way to stop at a
//!   place that keeps lengua's rules, so lengua runs a copy of main.wat with the stop written in:
//!   at the place, each local goes to a global of the copy, then the copy traps. The copy is
//!   built for the one run, and `check` never tests or stamps it.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use wasmparser::{Operator, ValType};
use wasmtime::{Engine, FrameInfo, Instance, Store, Val};
use wast::Wat;
use wast::core::{FuncKind, ModuleField, ModuleKind};
use wast::parser::{self, ParseBuffer};

use crate::checker::wasm::model::Module;
use crate::wasm::engine::Setup;
use crate::wasm::host::{Host, Stop};
use crate::wasm::text::{self, Built};
use crate::wasm::wasi::errno;

/// The most calls a trace keeps: the first ones. The calls after them are counted.
pub const TRACE_CALLS: usize = 512;

/// How often a profiled run's clock moves on, which is how often a sample is taken while the
/// program's own code runs.
pub const PROFILE_TICK: Duration = Duration::from_millis(1);

/// The most functions a profile's report names: those with the most samples.
pub const PROFILE_FUNCTIONS: usize = 32;

/// The most lines a profile's report names: those with the most samples.
pub const PROFILE_LINES: usize = 16;

/// The most places in the module a profile keeps samples for.
const PLACES: usize = 4096;

/// The most bytes of memory a stop shows.
pub const SHOWN_BYTES: u32 = 1024;

/// A stop shows the locals and the globals whose index is below this.
pub const SHOWN_VALUES: u32 = 1024;

/// What starts the name of every global and export the copy with a stop adds to main.wat.
pub const STOP_PREFIX: &str = "lengua_stop_";

/// How much of a call a trace keeps.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Detail {
    /// Its name, the numbers the program passed and what the host answered.
    Arguments,
    /// Its name and the host's answer: for an idea that names a secret, since nothing says a
    /// number the program passes isn't the secret.
    Names,
}

/// What a stop shows beside the locals and the globals.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Show {
    /// Bytes of the program's memory: where they start, and how many.
    pub memory: Option<(u32, u32)>,
}

/// The tools a run is asked for. None by default.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tools {
    /// A trace of its calls to the host, and how much of each.
    #[serde(default)]
    pub trace: Option<Detail>,
    /// A profile of its time.
    #[serde(default)]
    pub profile: bool,
    /// That its module is a copy with a stop written in, whose values to read once it ends.
    #[serde(default)]
    pub stop: Option<Show>,
}

/// The tools a run is asked for, in its caller's words: the `run` tool's parameters, or the
/// options of a run from a shell.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Asked {
    /// A trace of the program's calls to the host.
    pub trace: bool,
    /// A profile of the run's time.
    pub profile: bool,
    /// Where to stop: a line of main.wat or one of its functions ([`Place::parse`]).
    pub stop_at: Option<String>,
    /// The bytes of memory to show at the stop ([`memory_range`]).
    pub show_memory: Option<String>,
}

impl Asked {
    /// Whether the run is asked for any tool.
    #[must_use]
    pub const fn any(&self) -> bool {
        self.trace || self.profile || self.stop_at.is_some() || self.show_memory.is_some()
    }
}

impl Tools {
    /// Whether the run is asked for any tool.
    #[must_use]
    pub const fn any(&self) -> bool {
        self.trace.is_some() || self.profile || self.stop.is_some()
    }
}

/// What a call of the host came to, as a trace says it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Gave {
    /// The host answered with this error number, 0 for none.
    Code(i32),
    /// The host answered with nothing.
    Nothing,
    /// The program exited with this code.
    Exit(i32),
    /// The host stopped the program.
    Stopped(Stop),
    /// The call failed in another way.
    Failed,
}

/// The name WASI gives the error `number`, for the errors lengua's host answers.
fn errno_name(number: u16) -> Option<&'static str> {
    Some(match number {
        errno::ACCES => "ACCES",
        errno::AGAIN => "AGAIN",
        errno::BADF => "BADF",
        errno::BUSY => "BUSY",
        errno::CONNREFUSED => "CONNREFUSED",
        errno::EXIST => "EXIST",
        errno::FBIG => "FBIG",
        errno::ILSEQ => "ILSEQ",
        errno::INVAL => "INVAL",
        errno::IO => "IO",
        errno::ISDIR => "ISDIR",
        errno::MFILE => "MFILE",
        errno::MSGSIZE => "MSGSIZE",
        errno::NAMETOOLONG => "NAMETOOLONG",
        errno::NOENT => "NOENT",
        errno::NOSYS => "NOSYS",
        errno::NOTDIR => "NOTDIR",
        errno::NOTEMPTY => "NOTEMPTY",
        errno::NOTSOCK => "NOTSOCK",
        errno::NOTTY => "NOTTY",
        errno::PIPE => "PIPE",
        errno::SPIPE => "SPIPE",
        errno::TIMEDOUT => "TIMEDOUT",
        errno::NOTCAPABLE => "NOTCAPABLE",
        _ => return None,
    })
}

/// The calls a program made to the host, as a run records them.
#[derive(Debug)]
pub struct Trace {
    /// How much of each call it keeps.
    detail: Detail,
    /// The calls, one line each, in order.
    calls: Vec<String>,
    /// How many calls came after the last it keeps.
    left_out: u64,
}

impl Trace {
    /// A trace with no call yet.
    #[must_use]
    pub const fn new(detail: Detail) -> Self {
        Self {
            detail,
            calls: Vec::new(),
            left_out: 0,
        }
    }

    /// Notes that the program calls `call` with `arguments`, and gives the call's place in the
    /// trace, for [`Self::end`]: none once the trace is full.
    pub fn begin(&mut self, call: &str, arguments: &[i64]) -> Option<usize> {
        if self.calls.len() >= TRACE_CALLS {
            self.left_out = self.left_out.saturating_add(1);
            return None;
        }
        let mut line = call.to_owned();
        if self.detail == Detail::Arguments {
            line.push('(');
            for (at, argument) in arguments.iter().enumerate() {
                if at > 0 {
                    line.push_str(", ");
                }
                let _ = write!(line, "{argument}");
            }
            line.push(')');
        }
        self.calls.push(line);
        Some(self.calls.len() - 1)
    }

    /// Notes what the call at `at` came to.
    pub fn end(&mut self, at: Option<usize>, gave: Gave) {
        let detail = self.detail;
        let Some(line) = at.and_then(|at| self.calls.get_mut(at)) else {
            return;
        };
        match gave {
            Gave::Code(code) => {
                let _ = write!(line, " = {code}");
                let name = u16::try_from(code)
                    .ok()
                    .filter(|number| *number != 0)
                    .and_then(errno_name);
                if let Some(name) = name {
                    let _ = write!(line, " ({name})");
                }
            }
            Gave::Nothing => {}
            Gave::Exit(code) if detail == Detail::Arguments => {
                let _ = write!(line, " ended the run with exit code {code}");
            }
            Gave::Exit(_) => line.push_str(" ended the run"),
            Gave::Stopped(stop) => {
                let _ = write!(line, " stopped the run: {stop}");
            }
            Gave::Failed => line.push_str(" stopped the run"),
        }
    }

    /// What the trace holds.
    #[must_use]
    pub fn found(&self) -> Traced {
        Traced {
            calls: self.calls.clone(),
            left_out: self.left_out,
        }
    }
}

/// A trace, as a worker hands it to the server.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Traced {
    /// The calls, one line each, in order.
    pub calls: Vec<String>,
    /// How many calls came after them.
    pub left_out: u64,
}

/// One function's share of a profile's samples.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Share {
    /// Its name, when the module that ran names it.
    pub name: Option<String>,
    /// The samples that fell in the function itself.
    pub own: u64,
    /// The samples that fell in it or in what it called.
    pub with_calls: u64,
}

/// A profile's samples, as a run takes them.
#[derive(Debug, Default)]
pub struct Samples {
    /// How many it took.
    taken: u64,
    /// Each function's share, by the function's index.
    functions: BTreeMap<u32, Share>,
    /// How many fell at each place, by the place's offset in the module.
    places: BTreeMap<u64, u64>,
}

impl Samples {
    /// Takes one sample: `frames` are the program's calls now, the innermost first.
    pub fn take(&mut self, frames: &[FrameInfo]) {
        let Some(innermost) = frames.first() else {
            return;
        };
        self.taken = self.taken.saturating_add(1);
        let mut seen: Vec<u32> = Vec::with_capacity(frames.len());
        for (depth, frame) in frames.iter().enumerate() {
            let index = frame.func_index();
            if seen.contains(&index) {
                continue;
            }
            seen.push(index);
            let share = self.functions.entry(index).or_insert_with(|| Share {
                name: frame.func_name().map(str::to_owned),
                ..Share::default()
            });
            share.with_calls = share.with_calls.saturating_add(1);
            if depth == 0 {
                share.own = share.own.saturating_add(1);
            }
        }
        let place = innermost
            .module_offset()
            .and_then(|offset| u64::try_from(offset).ok());
        if let Some(place) = place
            && (self.places.len() < PLACES || self.places.contains_key(&place))
        {
            let count = self.places.entry(place).or_default();
            *count = count.saturating_add(1);
        }
    }

    /// What the samples hold.
    #[must_use]
    pub fn found(&self) -> Sampled {
        Sampled {
            taken: self.taken,
            functions: self
                .functions
                .iter()
                .map(|(index, share)| (*index, share.clone()))
                .collect(),
            places: self
                .places
                .iter()
                .map(|(place, count)| (*place, *count))
                .collect(),
        }
    }
}

/// A profile's samples, as a worker hands them to the server.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Sampled {
    /// How many samples the run took.
    pub taken: u64,
    /// Each function's share, by the function's index.
    pub functions: Vec<(u32, Share)>,
    /// How many samples fell at each place, by the place's offset in the module.
    pub places: Vec<(u64, u64)>,
}

/// What a run of a copy with a stop came to.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Stopped {
    /// Whether the run reached the stop.
    pub reached: bool,
    /// Each value the copy kept, by the name of the export that holds it.
    pub values: Vec<(String, String)>,
    /// The bytes of memory asked for, in hex, or why they can't be shown.
    pub memory: Option<Result<String, String>>,
}

/// What a run's tools found, as a worker hands it to the server.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Found {
    /// The trace.
    #[serde(default)]
    pub trace: Option<Traced>,
    /// The profile's samples.
    #[serde(default)]
    pub profile: Option<Sampled>,
    /// What the stop came to.
    #[serde(default)]
    pub stopped: Option<Stopped>,
}

/// What a program's host holds for the run's tools. Empty for a run that asked for none.
#[derive(Debug, Default)]
pub struct Probe {
    /// The trace, which the host's functions write.
    pub trace: Option<Arc<Mutex<Trace>>>,
    /// The profile's samples, which the engine's look at its clock takes.
    pub samples: Option<Arc<Mutex<Samples>>>,
    /// What to read once a copy with a stop has ended.
    pub stop: Option<Show>,
    /// What the stop came to.
    pub stopped: Option<Stopped>,
}

impl Probe {
    /// What the tools found, taken out of the probe: nothing for a run that asked for none.
    pub fn found(&mut self) -> Option<Box<Found>> {
        let trace = self
            .trace
            .take()
            .and_then(|trace| trace.lock().ok().map(|trace| trace.found()));
        let profile = self
            .samples
            .take()
            .and_then(|samples| samples.lock().ok().map(|samples| samples.found()));
        let stopped = self.stopped.take();
        if trace.is_none() && profile.is_none() && stopped.is_none() {
            return None;
        }
        Some(Box::new(Found {
            trace,
            profile,
            stopped,
        }))
    }
}

/// Moves an engine's clock on every [`PROFILE_TICK`] until it is dropped, beside the engine's
/// own slower thread: a profiled run's samples.
#[derive(Debug)]
pub struct Ticker {
    /// Set once the run has ended.
    done: Arc<AtomicBool>,
}

impl Ticker {
    /// Starts moving `engine`'s clock on.
    #[must_use]
    pub fn start(engine: &Engine) -> Self {
        let done = Arc::new(AtomicBool::new(false));
        let (ended, engine) = (Arc::clone(&done), engine.clone());
        // Without the thread the engine's own clock still moves, and the profile has fewer
        // samples.
        let _ = std::thread::Builder::new()
            .name("lengua profile".to_owned())
            .spawn(move || {
                while !ended.load(Ordering::Relaxed) {
                    std::thread::sleep(PROFILE_TICK);
                    engine.increment_epoch();
                }
            });
        Self { done }
    }
}

impl Drop for Ticker {
    fn drop(&mut self) {
        self.done.store(true, Ordering::Relaxed);
    }
}

/// How a stop shows `value`.
fn shown(value: &Val) -> String {
    match value {
        Val::I32(number) => format!("i32 {number} (0x{number:08x})"),
        Val::I64(number) => format!("i64 {number} (0x{number:016x})"),
        Val::F32(bits) => format!("f32 {} (bits 0x{bits:08x})", f32::from_bits(*bits)),
        Val::F64(bits) => format!("f64 {} (bits 0x{bits:016x})", f64::from_bits(*bits)),
        Val::V128(bits) => format!("v128 0x{:032x}", bits.as_u128()),
        _ => "a reference".to_owned(),
    }
}

/// What the run of a copy with a stop came to, read from `instance` once the run has ended:
/// whether it reached the stop, and then each value the copy kept and the bytes `show` names.
pub fn stopped(store: &mut Store<Host>, instance: &Instance, show: Show) -> Stopped {
    let reached = instance
        .get_global(&mut *store, &format!("{STOP_PREFIX}reached"))
        .map(|global| global.get(&mut *store));
    if !matches!(reached, Some(Val::I32(1))) {
        return Stopped::default();
    }
    let mut values = Vec::new();
    for kind in ["local", "global"] {
        for index in 0..SHOWN_VALUES {
            let name = format!("{STOP_PREFIX}{kind}_{index}");
            if let Some(global) = instance.get_global(&mut *store, &name) {
                let value = global.get(&mut *store);
                values.push((name, shown(&value)));
            }
        }
    }
    let memory = show.memory.map(|(at, length)| {
        let Some(memory) = instance.get_memory(&mut *store, "memory") else {
            return Err("the module exports no memory named \"memory\"".to_owned());
        };
        let bytes = memory.data(&*store);
        let start = usize::try_from(at).unwrap_or(usize::MAX);
        let end = start.saturating_add(usize::try_from(length).unwrap_or(usize::MAX));
        bytes.get(start..end).map_or_else(
            || {
                Err(format!(
                    "bytes {at} to {} are outside the program's memory of {} bytes",
                    u64::from(at) + u64::from(length),
                    bytes.len()
                ))
            },
            |bytes| {
                let mut hex = String::with_capacity(bytes.len() * 3);
                for (index, byte) in bytes.iter().enumerate() {
                    if index > 0 {
                        hex.push(' ');
                    }
                    let _ = write!(hex, "{byte:02x}");
                }
                Ok(hex)
            },
        )
    });
    Stopped {
        reached: true,
        values,
        memory,
    }
}

/// Where a run is asked to stop.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Place {
    /// Where this function of main.wat starts, by its name without the `$`.
    Function(String),
    /// At the first instruction on this line of main.wat.
    Line(usize),
}

impl Place {
    /// The place `text` names: a line's number, or a function's name, with its `$` or without.
    ///
    /// # Errors
    ///
    /// When it names neither.
    pub fn parse(text: &str) -> Result<Self, String> {
        let text = text.trim();
        if let Ok(line) = text.parse::<usize>() {
            if line == 0 {
                return Err("main.wat's lines count from 1".to_owned());
            }
            return Ok(Self::Line(line));
        }
        let name = text.strip_prefix('$').unwrap_or(text);
        if name.is_empty() || name.chars().any(char::is_whitespace) {
            return Err(
                "a stop names a line of main.wat, as `42`, or one of its functions, as `$parse`"
                    .to_owned(),
            );
        }
        Ok(Self::Function(name.to_owned()))
    }
}

/// The bytes of memory `text` names, as `<address>:<length>`, such as `1024:64`.
///
/// # Errors
///
/// When it isn't two numbers, or names more than [`SHOWN_BYTES`] bytes.
pub fn memory_range(text: &str) -> Result<(u32, u32), String> {
    let usage = || {
        format!(
            "the memory to show is `<address>:<length>`, such as `1024:64`, of at most \
             {SHOWN_BYTES} bytes, not {text:?}"
        )
    };
    let (at, length) = text.trim().split_once(':').ok_or_else(usage)?;
    let at = at.trim().parse::<u32>().map_err(|_| usage())?;
    let length = length.trim().parse::<u32>().map_err(|_| usage())?;
    if length == 0 || length > SHOWN_BYTES {
        return Err(usage());
    }
    Ok((at, length))
}

/// A copy of main.wat with a stop written in.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Plan {
    /// The copy's text. Every line of it is the same line of main.wat.
    pub text: String,
    /// Where it stops, in words.
    pub at: String,
    /// What each export the copy adds holds, by the export's name: a local or a global, as
    /// main.wat names it.
    pub names: BTreeMap<String, String>,
    /// What the stop doesn't show, and why.
    pub left_out: Vec<String>,
}

/// Where each instruction of each of main.wat's own functions is in `text`, the functions in
/// their order, as [`crate::wasm::text::build`] reads them.
fn instructions(text: &str) -> Result<Vec<Vec<usize>>, String> {
    let mut buffer = ParseBuffer::new(text).map_err(|error| error.message())?;
    buffer.track_instr_spans(true);
    let parsed = parser::parse::<Wat>(&buffer).map_err(|error| error.message())?;
    let Wat::Module(module) = parsed else {
        return Err("main.wat is a component".to_owned());
    };
    let ModuleKind::Text(fields) = &module.kind else {
        return Err("main.wat's module is given as bytes".to_owned());
    };
    Ok(fields
        .iter()
        .filter_map(|field| match field {
            ModuleField::Func(func) => match &func.kind {
                FuncKind::Inline { expression, .. } => Some(
                    expression
                        .instr_spans
                        .as_deref()
                        .map(|spans| spans.iter().map(|span| span.offset()).collect())
                        .unwrap_or_default(),
                ),
                FuncKind::Import(..) => None,
            },
            _ => None,
        })
        .collect())
}

/// The line, from 1, of the byte at `offset` of `text`.
fn line_of(text: &str, offset: usize) -> usize {
    text.as_bytes()
        .iter()
        .take(offset)
        .filter(|byte| **byte == b'\n')
        .count()
        + 1
}

/// Whether `text` writes its module as `(module …)`, after its first comments.
fn wrapped(text: &str) -> bool {
    let mut rest = text.trim_start();
    loop {
        if let Some(after) = rest.strip_prefix(";;") {
            rest = after.split_once('\n').map_or("", |(_, rest)| rest);
        } else if let Some(after) = rest.strip_prefix("(;") {
            rest = after.split_once(";)").map_or("", |(_, rest)| rest);
        } else {
            return rest
                .strip_prefix('(')
                .is_some_and(|inside| inside.trim_start().starts_with("module"));
        }
        rest = rest.trim_start();
    }
}

/// Where the `)` is that ends the module of `text`: the last one that is in no comment.
fn module_end(text: &str) -> Option<usize> {
    let mut line_end = text.len();
    loop {
        let line_start = text
            .get(..line_end)?
            .rfind('\n')
            .map_or(0, |newline| newline + 1);
        let line = text.get(line_start..line_end)?;
        let code = line.find(";;").map_or(line, |comment| &line[..comment]);
        if let Some(at) = code.rfind(')')
            && !code[..at].ends_with(';')
        {
            return Some(line_start + at);
        }
        if line_start == 0 {
            return None;
        }
        line_end = line_start - 1;
    }
}

/// How the copy writes a global of type `ty`: the type, and the value it starts with. None for
/// a reference, which a stop doesn't show.
const fn written(ty: ValType) -> Option<(&'static str, &'static str)> {
    match ty {
        ValType::I32 => Some(("i32", "(i32.const 0)")),
        ValType::I64 => Some(("i64", "(i64.const 0)")),
        ValType::F32 => Some(("f32", "(f32.const 0)")),
        ValType::F64 => Some(("f64", "(f64.const 0)")),
        ValType::V128 => Some(("v128", "(v128.const i64x2 0 0)")),
        ValType::Ref(_) => None,
    }
}

/// A copy of main.wat's `text`, whose build is `built`, that stops at `place`: just before the
/// first instruction there, each local of the function goes to a global the copy adds and
/// exports, and the copy traps. The copy also exports each global of main.wat that the program
/// may change. The stop is written on the line it stops at and the rest after the module's last
/// field, so the copy has main.wat's lines, and no function, global or local of main.wat changes
/// its number.
///
/// # Errors
///
/// When main.wat has no such place, or lengua can't write the stop there.
pub fn plan(text: &str, built: &Built, place: &Place) -> Result<Plan, String> {
    let module = Module::read(text, built)?;
    let places = instructions(text)?;
    let own: Vec<_> = module.own().collect();
    if own.len() != places.len() {
        return Err("lengua couldn't match main.wat's functions with its build".to_owned());
    }
    // The first instruction of the place, by where it is written: one written before its
    // operands, as `(local.set $x (…))`, stops before they run.
    let mut best: Option<(usize, usize)> = None;
    let mut lines = BTreeSet::new();
    for (index, (function, offsets)) in own.iter().zip(&places).enumerate() {
        let named = matches!(
            place,
            Place::Function(name) if function.name.as_deref() == Some(name.as_str())
        );
        for (op, offset) in function.body.iter().zip(offsets) {
            // An `else` or an `end` is no place of its own: `(else …)` takes no instruction
            // before it.
            if matches!(op.operator, Operator::Else | Operator::End) {
                continue;
            }
            let line = line_of(text, *offset);
            lines.insert(line);
            let wanted = match place {
                Place::Function(_) => named,
                Place::Line(wanted) => line == *wanted,
            };
            if wanted && best.is_none_or(|(least, _)| *offset < least) {
                best = Some((*offset, index));
            }
        }
    }
    let Some((offset, index)) = best else {
        return Err(match place {
            Place::Function(name) => {
                let known = own
                    .iter()
                    .any(|function| function.name.as_deref() == Some(name.as_str()));
                if known {
                    format!("${name} has no instruction to stop at")
                } else {
                    let names: Vec<String> =
                        own.iter().take(32).map(|function| function.shown()).collect();
                    format!(
                        "main.wat defines no function ${name}; its functions are {}",
                        names.join(", ")
                    )
                }
            }
            Place::Line(wanted) => {
                let before = lines.range(..*wanted).next_back();
                let after = lines.range(*wanted..).next();
                let near: Vec<String> = before
                    .into_iter()
                    .chain(after)
                    .map(ToString::to_string)
                    .collect();
                format!(
                    "no instruction of main.wat's functions starts on line {wanted}; the \
                     nearest lines where one does: {}",
                    near.join(" and ")
                )
            }
        });
    };
    let function = own
        .get(index)
        .ok_or("lengua couldn't find the function again")?;
    // A folded instruction starts at its `(`.
    let bytes = text.as_bytes();
    let mut at = offset;
    if bytes.get(at) != Some(&b'(')
        && let Some(before) = bytes
            .get(..at)
            .and_then(|head| head.iter().rposition(|byte| !byte.is_ascii_whitespace()))
        && bytes.get(before) == Some(&b'(')
    {
        at = before;
    }
    let mut stop = String::new();
    let mut fields = format!(
        " (global ${STOP_PREFIX}reached (export \"{STOP_PREFIX}reached\") (mut i32) (i32.const 0))"
    );
    let mut names = BTreeMap::new();
    let mut left_out = Vec::new();
    let locals = u32::try_from(function.local_count()).unwrap_or(u32::MAX);
    if locals > SHOWN_VALUES {
        left_out.push(format!(
            "{} has {locals} locals, and a stop shows its first {SHOWN_VALUES}",
            function.shown()
        ));
    }
    for local in 0..locals.min(SHOWN_VALUES) {
        let shown = function.local_shown(local);
        let Some((ty, zero)) = function.local_type(local).and_then(written) else {
            left_out.push(format!("{shown} is a reference, which a stop doesn't show"));
            continue;
        };
        let name = format!("{STOP_PREFIX}local_{local}");
        let _ = write!(
            fields,
            " (global ${name} (export \"{name}\") (mut {ty}) {zero})"
        );
        let _ = write!(stop, "(global.set ${name} (local.get {local})) ");
        names.insert(name, shown);
    }
    for global in &module.globals {
        // The library's own names stay its own, and a constant's value is in main.wat.
        let library = global
            .name
            .as_deref()
            .is_some_and(|name| name.starts_with(text::RESERVED));
        if !global.mutable || library || global.index >= SHOWN_VALUES {
            continue;
        }
        if written(global.ty).is_none() {
            left_out.push(format!(
                "{} is a reference, which a stop doesn't show",
                global.shown()
            ));
            continue;
        }
        let name = format!("{STOP_PREFIX}global_{}", global.index);
        let which = global
            .name
            .as_ref()
            .map_or_else(|| global.index.to_string(), |name| format!("${name}"));
        let _ = write!(fields, " (export \"{name}\" (global {which}))");
        names.insert(name, global.shown());
    }
    let _ = write!(
        stop,
        "(global.set ${STOP_PREFIX}reached (i32.const 1)) (unreachable) "
    );
    let end = if wrapped(text) {
        module_end(text)
    } else {
        Some(text.len())
    }
    .filter(|end| *end >= at)
    .ok_or("lengua couldn't find where main.wat's module ends")?;
    let (head, rest) = text.split_at(at);
    let (body, tail) = rest.split_at(end - at);
    let mut copy = String::with_capacity(text.len() + stop.len() + fields.len());
    copy.push_str(head);
    copy.push_str(&stop);
    copy.push_str(body);
    copy.push_str(&fields);
    copy.push_str(tail);
    Ok(Plan {
        text: copy,
        at: format!(
            "line {} of main.wat, in {}",
            line_of(text, offset),
            function.shown()
        ),
        names,
        left_out,
    })
}

/// main.wat's lines by operator offset, and where its functions' bodies are.
pub type Lines = (BTreeMap<u64, usize>, Vec<(u64, u64)>);

/// What turns what a run's tools found into a report: the names and lines of the module that
/// ran.
#[derive(Debug, Clone, Default)]
pub struct Names {
    /// Each function's name as main.wat writes it, and the line of its `func`, by its index.
    pub functions: BTreeMap<u32, (String, Option<usize>)>,
    /// main.wat's lines, for the module that ran.
    pub lines: Option<Lines>,
    /// The copy with a stop that ran, when one did.
    pub stop: Option<Plan>,
    /// Whether the program splits its work into pieces, whose calls and time the tools don't
    /// see.
    pub splits: bool,
}

impl Names {
    /// The names and lines of the module `built`, which main.wat's `text` builds. A module
    /// lengua can't read back has no names, and its functions are shown by number.
    #[must_use]
    pub fn of(text: &str, built: &Built) -> Self {
        let functions = Module::read(text, built)
            .map(|module| {
                module
                    .functions
                    .iter()
                    .map(|function| (function.index, (function.shown(), function.line)))
                    .collect()
            })
            .unwrap_or_default();
        Self {
            functions,
            lines: Some((built.lines.clone(), built.bodies.clone())),
            stop: None,
            splits: false,
        }
    }

    /// How a report names the function `index`, whose name in the module that ran is `name`,
    /// and its line in main.wat.
    fn function(&self, index: u32, name: Option<&str>) -> (String, Option<usize>) {
        if let Some((shown, line)) = self.functions.get(&index) {
            return (shown.clone(), *line);
        }
        let shown = name.map_or_else(|| format!("function {index}"), |name| format!("${name}"));
        (shown, None)
    }
}

/// A run's calls to the host, as `run` reports them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct TraceReport {
    /// Each call the program made to the host, in order, the first 512: the function's name,
    /// the numbers the program passed in brackets, then ` = ` and the error number the host
    /// answered, 0 for none, with WASI's name for another. A call that ended the run says so.
    pub calls: Vec<String>,
    /// How many calls came after these, which the trace doesn't keep.
    pub left_out: u64,
    /// How to read the trace, and what it leaves out.
    pub note: String,
}

/// One function's share of a profile.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct FunctionShare {
    /// The function, as main.wat names it.
    pub function: String,
    /// The line of main.wat where it is defined.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<usize>,
    /// The samples that fell in the function itself.
    pub own: u64,
    /// The samples that fell in it or in a function it called.
    pub with_calls: u64,
}

/// One line's share of a profile.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct LineShare {
    /// The line of main.wat.
    pub line: usize,
    /// The samples that fell on it.
    pub samples: u64,
}

/// Where a run's time went, as `run` reports it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct ProfileReport {
    /// How many samples the run took.
    pub samples: u64,
    /// About how many microseconds lie between two samples.
    pub every_us: u64,
    /// The functions with the most samples of their own, the most first, at most 32.
    pub functions: Vec<FunctionShare>,
    /// The lines of main.wat with the most samples, the most first, at most 16.
    pub lines: Vec<LineShare>,
    /// How to read the profile, and what it leaves out.
    pub note: String,
}

/// The values at a stop, as `run` reports them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct StopReport {
    /// Whether the run reached the place.
    pub reached: bool,
    /// The place, in words.
    pub at: String,
    /// Each parameter and local of the function, as main.wat names it, with its type and its
    /// value.
    pub locals: Vec<String>,
    /// Each global the program may change, with its type and its value.
    pub globals: Vec<String>,
    /// The bytes of memory asked for, in hex, or why lengua can't show them.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memory: Option<String>,
    /// What ran, and what the stop doesn't show.
    pub note: String,
}

/// What a run's tools report.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Reports {
    /// The trace.
    pub host_calls: Option<TraceReport>,
    /// The profile.
    pub profile: Option<ProfileReport>,
    /// The stop.
    pub stopped: Option<StopReport>,
}

/// The trace's report.
fn trace_report(traced: Traced, detail: Detail, names: &Names) -> TraceReport {
    let mut note = match detail {
        Detail::Arguments => "Each line is one call the program made to the host: the \
                              function, the numbers the program passed (descriptors, and \
                              addresses and lengths in its memory, never the bytes there), and \
                              the error number the host answered, 0 for none."
            .to_owned(),
        Detail::Names => "The idea names a secret, so each line is the function the program \
                          called and the error number the host answered, 0 for none, with \
                          none of the numbers the program passed: nothing says such a number \
                          isn't the secret."
            .to_owned(),
    };
    if traced.left_out > 0 {
        let _ = write!(
            note,
            " The trace keeps the first {TRACE_CALLS} calls; {} more came after them.",
            traced.left_out
        );
    }
    if names.splits {
        note.push_str(" The calls the pieces of its split work made are not in the trace.");
    }
    TraceReport {
        calls: traced.calls,
        left_out: traced.left_out,
        note,
    }
}

/// The profile's report.
fn profile_report(sampled: &Sampled, names: &Names) -> ProfileReport {
    let mut shares: Vec<&(u32, Share)> = sampled.functions.iter().collect();
    shares.sort_by(|(first, a), (second, b)| {
        b.own
            .cmp(&a.own)
            .then(b.with_calls.cmp(&a.with_calls))
            .then(first.cmp(second))
    });
    let functions = shares
        .into_iter()
        .take(PROFILE_FUNCTIONS)
        .map(|(index, share)| {
            let (function, line) = names.function(*index, share.name.as_deref());
            FunctionShare {
                function,
                line,
                own: share.own,
                with_calls: share.with_calls,
            }
        })
        .collect();
    let mut by_line: BTreeMap<usize, u64> = BTreeMap::new();
    if let Some((lines, bodies)) = &names.lines {
        for (place, count) in &sampled.places {
            if let Some(line) = text::line_at(lines, bodies, *place) {
                let total = by_line.entry(line).or_default();
                *total = total.saturating_add(*count);
            }
        }
    }
    let mut lines: Vec<LineShare> = by_line
        .into_iter()
        .map(|(line, samples)| LineShare { line, samples })
        .collect();
    lines.sort_by(|a, b| b.samples.cmp(&a.samples).then(a.line.cmp(&b.line)));
    lines.truncate(PROFILE_LINES);
    let every_us = u64::try_from(PROFILE_TICK.as_micros()).unwrap_or(u64::MAX);
    let mut note = if sampled.taken == 0 {
        format!(
            "The run took no sample: its own code ran for less than the {every_us} \
             microseconds between two samples, or the run never started."
        )
    } else {
        "A sample is taken about every `every_us` microseconds while the program's own code \
         runs. `own` counts the samples that fell in the function itself, and `with_calls` \
         those that fell in it or in a function it called, among the 20 innermost calls. A \
         line is where the engine looks at its clock: the start of a function or the head of \
         a loop. The time inside a call of the host is not sampled, and the counts change \
         from run to run."
            .to_owned()
    };
    if names.splits {
        note.push_str(" The pieces of its split work are not sampled.");
    }
    ProfileReport {
        samples: sampled.taken,
        every_us,
        functions,
        lines,
        note,
    }
}

/// The stop's report, for the copy `plan` that ran.
fn stop_report(stopped: Stopped, plan: &Plan) -> StopReport {
    let mut locals = Vec::new();
    let mut globals = Vec::new();
    for (export, value) in &stopped.values {
        let Some(name) = plan.names.get(export) else {
            continue;
        };
        let line = format!("{name}: {value}");
        if export.starts_with(&format!("{STOP_PREFIX}local_")) {
            locals.push(line);
        } else {
            globals.push(line);
        }
    }
    let mut note = if stopped.reached {
        format!(
            "The run stopped at {}, before the instruction there ran. What ran is a copy of \
             main.wat with the stop written in, under the idea's grants: check never tested or \
             stamped the copy, and the run's output is the copy's.",
            plan.at
        )
    } else {
        format!(
            "The run ended before it reached {}: `ended` says how. What ran is a copy of \
             main.wat with the stop written in, under the idea's grants.",
            plan.at
        )
    };
    for left in &plan.left_out {
        let _ = write!(note, " {left}.");
    }
    StopReport {
        reached: stopped.reached,
        at: plan.at.clone(),
        locals,
        globals,
        memory: stopped
            .memory
            .map(|memory| memory.unwrap_or_else(|problem| problem)),
        note,
    }
}

/// What the tools `tools` report of a run that found `found`, with `names` for the module that
/// ran: a report for each tool asked for, also of a run that never started.
#[must_use]
pub fn reports(found: Option<Found>, tools: Tools, names: &Names) -> Reports {
    let found = found.unwrap_or_default();
    Reports {
        host_calls: tools
            .trace
            .map(|detail| trace_report(found.trace.unwrap_or_default(), detail, names)),
        profile: tools
            .profile
            .then(|| profile_report(&found.profile.unwrap_or_default(), names)),
        stopped: names
            .stop
            .as_ref()
            .filter(|_| tools.stop.is_some())
            .map(|plan| stop_report(found.stopped.unwrap_or_default(), plan)),
    }
}

impl Reports {
    /// The reports as a run from a shell says them on standard error, one line each, after
    /// `lengua`, the program's name.
    #[must_use]
    pub fn lines(&self, lengua: &str) -> Vec<String> {
        let mut lines = Vec::new();
        if let Some(trace) = &self.host_calls {
            for call in &trace.calls {
                lines.push(format!("{lengua}: trace: {call}"));
            }
            lines.push(format!("{lengua}: trace: {}", trace.note));
        }
        if let Some(profile) = &self.profile {
            lines.push(format!(
                "{lengua}: profile: {} samples, one about every {} microseconds",
                profile.samples, profile.every_us
            ));
            for share in &profile.functions {
                let line = share
                    .line
                    .map(|line| format!(" (main.wat line {line})"))
                    .unwrap_or_default();
                lines.push(format!(
                    "{lengua}: profile: function {}{line}: {} own, {} with calls",
                    share.function, share.own, share.with_calls
                ));
            }
            for share in &profile.lines {
                lines.push(format!(
                    "{lengua}: profile: main.wat line {}: {} samples",
                    share.line, share.samples
                ));
            }
            lines.push(format!("{lengua}: profile: {}", profile.note));
        }
        if let Some(stop) = &self.stopped {
            if stop.reached {
                lines.push(format!("{lengua}: stop: stopped at {}", stop.at));
            }
            for local in &stop.locals {
                lines.push(format!("{lengua}: stop: local {local}"));
            }
            for global in &stop.globals {
                lines.push(format!("{lengua}: stop: global {global}"));
            }
            if let Some(memory) = &stop.memory {
                lines.push(format!("{lengua}: stop: memory: {memory}"));
            }
            lines.push(format!("{lengua}: stop: {}", stop.note));
        }
        lines
    }
}

/// What a run from a shell is given for its tools.
#[derive(Debug, Clone)]
pub struct Equipment {
    /// The tools.
    pub tools: Tools,
    /// The engine's configuration the module was compiled for: a profile takes one whose clock
    /// moves.
    pub setup: Setup,
    /// The names and lines of the module that runs.
    pub names: Names,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A main.wat with two functions, a global the program changes and locals of every number
    /// type.
    const TEXT: &str = "(module\n  (memory (export \"memory\") 1)\n  (global $count (mut i32) \
                        (i32.const 7))\n  (global $LIMIT i32 (i32.const 9))\n  (func $twice \
                        (param $n i32) (result i32)\n    (local $wide i64)\n    (local.set \
                        $wide (i64.const 5))\n    (i32.mul\n      (local.get $n)\n      \
                        (i32.const 2)))\n  (func (export \"_start\")\n    (global.set $count \
                        (call $twice (i32.const 21)))))\n";

    /// A trace keeps each call's name, numbers and answer, in order; past its size it counts.
    #[test]
    fn a_trace_keeps_its_first_calls_and_counts_the_rest() {
        let mut trace = Trace::new(Detail::Arguments);
        let first = trace.begin("fd_write", &[1, 1024, 1, 1040]);
        trace.end(first, Gave::Code(0));
        let second = trace.begin("fd_close", &[9]);
        trace.end(second, Gave::Code(i32::from(errno::BADF)));
        let last = trace.begin("proc_exit", &[3]);
        trace.end(last, Gave::Exit(3));
        assert_eq!(
            trace.found().calls,
            [
                "fd_write(1, 1024, 1, 1040) = 0",
                "fd_close(9) = 8 (BADF)",
                "proc_exit(3) ended the run with exit code 3"
            ]
        );
        for _ in 0..TRACE_CALLS {
            let at = trace.begin("sched_yield", &[]);
            trace.end(at, Gave::Code(0));
        }
        let found = trace.found();
        assert_eq!(found.calls.len(), TRACE_CALLS);
        assert_eq!(found.left_out, 3);
    }

    /// For an idea that names a secret, a trace keeps no number the program passed, the exit
    /// code included.
    #[test]
    fn a_trace_of_names_keeps_no_number_the_program_passed() {
        let mut trace = Trace::new(Detail::Names);
        let first = trace.begin("fd_read", &[0, 4242, 1, 77]);
        trace.end(first, Gave::Code(0));
        let last = trace.begin("proc_exit", &[41]);
        trace.end(last, Gave::Exit(41));
        let calls = trace.found().calls;
        assert_eq!(calls, ["fd_read = 0", "proc_exit ended the run"]);
        let names = Names::default();
        let report = trace_report(trace.found(), Detail::Names, &names);
        assert!(report.note.contains("secret"), "{}", report.note);
    }

    /// A stop at a function is written before its first instruction, and one at a line before
    /// the first instruction of the line; the copy builds, has main.wat's lines, and keeps the
    /// function's locals and the globals the program changes.
    #[test]
    fn a_copy_with_a_stop_builds_and_keeps_main_wat_s_lines() {
        let built = text::build(TEXT).expect("it builds");
        for place in [Place::Function("twice".to_owned()), Place::Line(8)] {
            let plan = plan(TEXT, &built, &place).unwrap_or_else(|why| panic!("{why}"));
            assert_eq!(plan.text.lines().count(), TEXT.lines().count(), "{place:?}");
            let copy = text::build(&plan.text).unwrap_or_else(|why| panic!("{why}"));
            assert_ne!(copy.wasm, built.wasm);
            assert!(plan.at.contains("$twice"), "{}", plan.at);
            let shown: Vec<&str> = plan.names.values().map(String::as_str).collect();
            assert_eq!(shown, ["$count", "$n", "$wide"], "{place:?}");
        }
        let entry = plan(TEXT, &built, &Place::Function("twice".to_owned())).expect("a plan");
        assert!(entry.at.starts_with("line 7 "), "{}", entry.at);
        let line = plan(TEXT, &built, &Place::Line(8)).expect("a plan");
        assert!(line.at.starts_with("line 8 "), "{}", line.at);
        assert!(
            line.text.contains("(unreachable) (i32.mul"),
            "{}",
            line.text
        );
    }

    /// A place main.wat doesn't have is refused, with the places it has.
    #[test]
    fn a_stop_names_a_place_main_wat_has() {
        let built = text::build(TEXT).expect("it builds");
        let no_function = plan(TEXT, &built, &Place::Function("thrice".to_owned()));
        let why = no_function.expect_err("no such function");
        assert!(why.contains("$twice"), "{why}");
        let no_line = plan(TEXT, &built, &Place::Line(4)).expect_err("no instruction there");
        assert!(no_line.contains("line 4") && no_line.contains('7'), "{no_line}");
        assert_eq!(Place::parse("$twice"), Ok(Place::Function("twice".into())));
        assert_eq!(Place::parse(" 12 "), Ok(Place::Line(12)));
        assert!(Place::parse("0").is_err());
        assert!(Place::parse("two words").is_err());
        assert_eq!(memory_range("1024:64"), Ok((1024, 64)));
        assert!(memory_range("1024").is_err());
        assert!(memory_range("0:4096").is_err());
    }

    /// The module's end is its last `)` outside a comment.
    #[test]
    fn the_modules_end_is_outside_every_comment() {
        assert_eq!(module_end("(module)\n"), Some(7));
        assert_eq!(module_end("(module) ;; the end)\n"), Some(7));
        assert_eq!(module_end("(module)\n(; a note ;)\n"), Some(7));
        assert!(wrapped(";; a note\n(; another ;) (module)"));
        assert!(!wrapped("(func)"));
    }

    /// A profile's report names the functions with the most samples first, by main.wat's
    /// names, and says when a run took none.
    #[test]
    fn a_profile_names_the_functions_with_the_most_samples_first() {
        let built = text::build(TEXT).expect("it builds");
        let names = Names::of(TEXT, &built);
        let share = |own, with_calls| Share {
            name: None,
            own,
            with_calls,
        };
        let sampled = Sampled {
            taken: 10,
            functions: vec![(1, share(2, 10)), (0, share(8, 8))],
            places: Vec::new(),
        };
        let report = profile_report(&sampled, &names);
        assert_eq!(report.samples, 10);
        assert_eq!(report.functions[0].function, "$twice");
        assert_eq!(report.functions[0].line, Some(5));
        assert_eq!(report.functions[0].own, 8);
        assert_eq!(report.functions[1].with_calls, 10);
        let none = profile_report(&Sampled::default(), &names);
        assert!(none.note.contains("no sample"), "{}", none.note);
        let lines = Reports {
            profile: Some(report),
            ..Reports::default()
        }
        .lines("lengua-mcp");
        assert!(
            lines.contains(
                &"lengua-mcp: profile: function $twice (main.wat line 5): 8 own, 8 with calls"
                    .to_owned()
            ),
            "{lines:#?}"
        );
    }
}
