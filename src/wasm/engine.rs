//! The engine that runs lengua's WebAssembly: Wasmtime with Cranelift, pinned, with exactly the
//! WebAssembly features [`FEATURES`] names, so that a module the validator takes is a module the
//! engine runs, and floats' NaNs canonical, so a program computes the same bits on every CPU.
//!
//! Three configurations run programs ([`Setup`]): a test's counts every operation it executes as
//! Wasmtime's fuel, which is deterministic, and stops at a deadline through the engine's epochs; a
//! run's stops at its deadline the same way and counts nothing; a run from a shell, or a
//! benchmark's, does neither, so it pays for nothing it doesn't use. A fourth runs a WebAssembly
//! solver for the prover ([`crate::wasm::solver`]): [`SOLVER_FEATURES`], the programs' features
//! and exceptions, which a C++ solver throws and catches; neither a count nor a deadline, since
//! the prover stops a solver by ending its process; and the stack a native solver has. Compiled
//! modules are kept in Wasmtime's own cache, in the `cache` folder of lengua's data folder.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use wasmparser::WasmFeatures;
use wasmtime::{Cache, CacheConfig, Collector, Config, Engine, Strategy};

/// The WebAssembly features lengua's programs may use, the engine's and the validator's alike:
/// floats, mutable globals, saturating float-to-int conversions, sign extension, reference types
/// (the `funcref` tables of a C, C++ or Rust toolchain's module, which the store holds to
/// [`crate::wasm::host::limits::TABLE_ELEMENTS`] entries), multiple values, bulk memory and
/// 128-bit SIMD. Everything else is off: the component model, GC, threads, 64-bit and multiple
/// memories, tail calls, exceptions, relaxed SIMD and wide arithmetic.
pub const FEATURES: WasmFeatures = WasmFeatures::FLOATS
    .union(WasmFeatures::MUTABLE_GLOBAL)
    .union(WasmFeatures::SATURATING_FLOAT_TO_INT)
    .union(WasmFeatures::SIGN_EXTENSION)
    .union(WasmFeatures::REFERENCE_TYPES)
    .union(WasmFeatures::MULTI_VALUE)
    .union(WasmFeatures::BULK_MEMORY)
    .union(WasmFeatures::SIMD);

/// The WebAssembly features a solver's module may use, which the solver's engine alone turns on:
/// [`FEATURES`]; exceptions in their standard form (`try_table`, `throw` and `throw_ref`, which a
/// C++ toolchain's `throw` and `catch` compile to, and the `exnref` they pass); and GC's types,
/// without which Wasmtime has no collector's heap to keep a thrown exception in (its
/// `gc_support`). Not GC's own instructions (no `struct`, `array` or `i31`), typed function
/// references, or the legacy form of exceptions, which Wasmtime doesn't run.
pub const SOLVER_FEATURES: WasmFeatures = FEATURES
    .union(WasmFeatures::EXCEPTIONS)
    .union(WasmFeatures::GC_TYPES);

/// The engine's version, as Cargo.toml pins it.
pub const ENGINE_VERSION: &str = "50.0.1";

/// The engine and its version, as lengua.lock records what runs a module.
pub const ENGINE: &str = "wasmtime 50.0.1";

/// What WebAssembly 1.0 has of [`FEATURES`]: floats, and mutable globals, which it may import
/// and export.
const WASM1: WasmFeatures = WasmFeatures::FLOATS.union(WasmFeatures::MUTABLE_GLOBAL);

/// The features [`FEATURES`] turns on beyond WebAssembly 1.0, each by the name its proposal has,
/// in the order WebAssembly 2.0 lists them.
pub const OPTIONAL_FEATURES: [&str; 6] = [
    "sign-extension",
    "saturating-float-to-int",
    "multi-value",
    "reference-types",
    "bulk-memory",
    "simd",
];

/// The feature [`OPTIONAL_FEATURES`] names at `at`.
fn optional_feature(at: usize) -> WasmFeatures {
    match at {
        0 => WasmFeatures::SIGN_EXTENSION,
        1 => WasmFeatures::SATURATING_FLOAT_TO_INT,
        2 => WasmFeatures::MULTI_VALUE,
        3 => WasmFeatures::REFERENCE_TYPES,
        4 => WasmFeatures::BULK_MEMORY,
        _ => WasmFeatures::SIMD,
    }
}

/// The names of the features of [`OPTIONAL_FEATURES`] that the module `wasm` uses: each one the
/// validator refuses the module without, the others on.
#[must_use]
pub fn features_used(wasm: &[u8]) -> Vec<String> {
    (0..OPTIONAL_FEATURES.len())
        .filter(|&left_out| {
            let without = (0..OPTIONAL_FEATURES.len())
                .filter(|&at| at != left_out)
                .fold(WASM1, |features, at| features.union(optional_feature(at)));
            wasmparser::Validator::new_with_features(without)
                .validate_all(wasm)
                .is_err()
        })
        .map(|used| OPTIONAL_FEATURES[used].to_owned())
        .collect()
}

/// How often the engine's epoch moves on, which is how often a running program's deadline and
/// stop are looked at.
pub const TICK: Duration = Duration::from_millis(10);

/// The most of its thread's stack a program's WebAssembly calls may take, in bytes: past it, a
/// call traps. It is Wasmtime's own default, set here so that it holds whatever the default
/// becomes, as the prover's model holds a program's deepest calls to it.
pub const MAX_WASM_STACK: usize = 512 * 1024;

/// The most of its thread's stack a solver's WebAssembly calls may take, in bytes: the 8 MiB a
/// native solver has on each of the five systems, where Linux and macOS give a program's first
/// thread 8 MiB and z3's build gives its Windows program as much (`/STACK:8388608`, in its
/// cmake/msvc_legacy_quirks.cmake). Past it, a call traps.
pub const MAX_SOLVER_WASM_STACK: usize = 8 * 1024 * 1024;

/// The stack of the thread a solver runs on: [`MAX_SOLVER_WASM_STACK`] for its WebAssembly calls,
/// and beside it the room a program's thread keeps for the engine and the host
/// ([`crate::wasm::runner::STACK`]).
pub const SOLVER_STACK: usize = MAX_SOLVER_WASM_STACK + crate::wasm::runner::STACK;

/// What a configuration of the engine is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Setup {
    /// A test: every operation counted as fuel, and a deadline.
    Test,
    /// A run for someone, such as the `run` tool's: a deadline, and no count.
    Run,
    /// A run from a shell, or a benchmark: neither.
    Shell,
    /// A WebAssembly solver's, which the prover starts in a process of its own
    /// ([`crate::wasm::solver`]): exceptions on, and neither a count nor a deadline, since the
    /// prover stops a solver by ending its process.
    Solver,
}

impl Setup {
    /// Every configuration.
    pub const ALL: [Self; 4] = [Self::Test, Self::Run, Self::Shell, Self::Solver];

    /// The configurations that run lengua's programs, each with exactly [`FEATURES`].
    pub const PROGRAMS: [Self; 3] = [Self::Test, Self::Run, Self::Shell];

    /// Whether it counts the operations a program executes.
    #[must_use]
    pub const fn counts(self) -> bool {
        matches!(self, Self::Test)
    }

    /// Whether a deadline, or lengua, can stop a program in the middle of its work.
    #[must_use]
    pub const fn interrupts(self) -> bool {
        matches!(self, Self::Test | Self::Run)
    }

    /// The WebAssembly features it turns on: [`FEATURES`] for a program's, [`SOLVER_FEATURES`]
    /// for a solver's.
    #[must_use]
    pub const fn features(self) -> WasmFeatures {
        match self {
            Self::Test | Self::Run | Self::Shell => FEATURES,
            Self::Solver => SOLVER_FEATURES,
        }
    }

    /// The most of its thread's stack WebAssembly calls may take: [`MAX_WASM_STACK`] for a
    /// program's, [`MAX_SOLVER_WASM_STACK`] for a solver's.
    #[must_use]
    pub const fn max_wasm_stack(self) -> usize {
        match self {
            Self::Test | Self::Run | Self::Shell => MAX_WASM_STACK,
            Self::Solver => MAX_SOLVER_WASM_STACK,
        }
    }
}

/// The engine's configuration for `setup`, keeping compiled modules in `cache` when given.
///
/// # Errors
///
/// When the cache can't be set up in that folder.
pub fn config(setup: Setup, cache: Option<&Path>) -> Result<Config, String> {
    let mut config = Config::new();
    config.strategy(Strategy::Cranelift);
    // Every feature off, then the setup's on: whatever Wasmtime's defaults are, its set is this
    // one.
    config.wasm_features(WasmFeatures::all(), false);
    config.wasm_features(setup.features(), true);
    config.cranelift_nan_canonicalization(true);
    config.parallel_compilation(true);
    config.consume_fuel(setup.counts());
    config.epoch_interruption(setup.interrupts());
    config.max_wasm_stack(setup.max_wasm_stack());
    if setup == Setup::Solver {
        // A thrown exception is kept in the heap of Wasmtime's collector: the one of deferred
        // reference counting, the only one lengua builds in (Cargo.toml's `gc-drc`).
        config.collector(Collector::DeferredReferenceCounting);
        // Wasmtime takes no WebAssembly stack larger than an async call's, though lengua makes
        // no async call: one would have the solver's whole thread.
        config.async_stack_size(SOLVER_STACK);
    }
    if let Some(folder) = cache {
        let mut settings = CacheConfig::new();
        settings.with_directory(folder);
        // A module is never compressed again at zstd's higher level, which the cache's worker
        // does at a module's 256th use by default, during whichever run that is: a run's exit
        // can cut that short and leave its lock behind, and while the lock lasts every later use
        // of the module lists the cache's folder. Modules stay at the baseline level, so the
        // folder holds more.
        settings.with_optimized_compression_usage_counter_threshold(u64::MAX);
        let cache = Cache::new(settings).map_err(|error| {
            format!(
                "lengua can't keep compiled modules in {}: {error}",
                folder.display()
            )
        })?;
        config.cache(Some(cache));
    }
    Ok(config)
}

/// This process's engine for `setup`, made at its first use with compiled modules kept in
/// `cache`, which later calls don't change: a process has one data folder. An engine that stops
/// programs has a thread that moves its epoch on every [`TICK`].
///
/// # Errors
///
/// When the engine can't be made here.
pub fn engine(setup: Setup, cache: Option<&Path>) -> Result<Engine, String> {
    static TEST: OnceLock<Result<Engine, String>> = OnceLock::new();
    static RUN: OnceLock<Result<Engine, String>> = OnceLock::new();
    static SHELL: OnceLock<Result<Engine, String>> = OnceLock::new();
    static SOLVER: OnceLock<Result<Engine, String>> = OnceLock::new();
    let slot = match setup {
        Setup::Test => &TEST,
        Setup::Run => &RUN,
        Setup::Shell => &SHELL,
        Setup::Solver => &SOLVER,
    };
    slot.get_or_init(|| -> Result<Engine, String> {
        let engine = Engine::new(&config(setup, cache)?)
            .map_err(|error| format!("the WebAssembly engine couldn't start: {error}"))?;
        if setup.interrupts() {
            let ticking = engine.clone();
            std::thread::Builder::new()
                .name("lengua epochs".to_owned())
                .spawn(move || tick(&ticking))
                .map_err(|error| format!("the engine's clock couldn't start: {error}"))?;
        }
        Ok(engine)
    })
    .clone()
}

/// Moves `engine`'s epoch on every [`TICK`], for as long as the process lives.
fn tick(engine: &Engine) {
    loop {
        std::thread::sleep(TICK);
        engine.increment_epoch();
    }
}

/// The data folder the agent's harness gives the plugin, when it gives one: `PLUGIN_DATA`, as
/// Agent Plugins names it, or else `CLAUDE_PLUGIN_DATA`, as Claude Code sets it.
#[must_use]
pub fn plugin_data() -> Option<std::ffi::OsString> {
    std::env::var_os("PLUGIN_DATA").or_else(|| std::env::var_os("CLAUDE_PLUGIN_DATA"))
}

/// Where compiled modules are kept: the `cache` folder of the plugin's data folder when the
/// agent's harness gives one ([`plugin_data`]), else the `cache` folder of lengua's own. The
/// lock's signing key stays in lengua's own folder either way.
#[must_use]
pub fn cache_folder(data_dir: &crate::data_dir::DataDir) -> PathBuf {
    match plugin_data()
        .map(PathBuf::from)
        .filter(|folder| folder.is_absolute())
    {
        Some(folder) => folder.join("cache"),
        None => data_dir.path().join("cache"),
    }
}

/// Validates `wasm` as the engine would take it: with exactly [`FEATURES`].
///
/// # Errors
///
/// What's wrong, and where in the module.
pub fn validate(wasm: &[u8]) -> Result<(), wasmparser::BinaryReaderError> {
    wasmparser::Validator::new_with_features(FEATURES)
        .validate_all(wasm)
        .map(|_| ())
}

/// Validates `wasm` as the solver's engine would take it: with exactly [`SOLVER_FEATURES`].
///
/// # Errors
///
/// What's wrong, and where in the module.
pub fn validate_solver(wasm: &[u8]) -> Result<(), wasmparser::BinaryReaderError> {
    wasmparser::Validator::new_with_features(SOLVER_FEATURES)
        .validate_all(wasm)
        .map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A module the engine compiles; `text` is WebAssembly text.
    fn compiles(setup: Setup, text: &str) -> Result<(), String> {
        let wasm = crate::wasm::text::build(text)
            .map_err(|problem| problem.to_string())?
            .wasm;
        let engine = engine(setup, None)?;
        wasmtime::Module::from_binary(&engine, &wasm)
            .map(|_| ())
            .map_err(|error| error.to_string())
    }

    /// A toolchain's module that calls through its table, the table's number written in five
    /// bytes, as LLVM writes it with reference types on. In order: the magic number and version
    /// 1; one type, `() -> ()`; one function of it; one `funcref` table of at least one entry;
    /// and the function's body, 11 bytes: no locals, `i32.const 0`, `call_indirect` of type 0 on
    /// table 0 written as `80 80 80 80 00`, and `end`.
    const OVERLONG: &[u8] = &[
        0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00, 0x01, 0x04, 0x01, 0x60, 0x00, 0x00, 0x03,
        0x02, 0x01, 0x00, 0x04, 0x04, 0x01, 0x70, 0x00, 0x01, 0x0a, 0x0d, 0x01, 0x0b, 0x00, 0x41,
        0x00, 0x11, 0x00, 0x80, 0x80, 0x80, 0x80, 0x00, 0x0b,
    ];

    /// What lengua turns on, the engine runs, in every configuration; what it turns off, the
    /// validator refuses.
    #[test]
    fn the_engine_and_the_validator_take_the_same_features() {
        let simd = "(module (func (result v128) (v128.const i32x4 1 2 3 4) (i32x4.add (v128.const \
                    i32x4 5 6 7 8))))";
        let bulk = "(module (memory 1) (func (memory.fill (i32.const 0) (i32.const 0) (i32.const \
                    8))))";
        let saturating = "(module (func (result i32) (i32.trunc_sat_f32_s (f32.const 3.5))))";
        let sign = "(module (func (result i32) (i32.extend8_s (i32.const 255))))";
        let values = "(module (func (result i32 i32) (i32.const 1) (i32.const 2)))";
        // A call through a module's table, and reference types' growing of it, which the store
        // holds to its limit.
        let table = "(module (type $t (func)) (table 1 funcref) (func (call_indirect (type $t) \
                     (i32.const 0))))";
        let grows = "(module (table 1 funcref) (func (drop (table.grow 0 (ref.null func) \
                     (i32.const 1)))))";
        for setup in Setup::ALL {
            for text in [simd, bulk, saturating, sign, values, table, grows] {
                assert_eq!(compiles(setup, text), Ok(()), "{setup:?}: {text}");
            }
            let engine = engine(setup, None).expect("the engine");
            let module = wasmtime::Module::from_binary(&engine, OVERLONG);
            assert!(module.is_ok(), "{setup:?}: {:?}", module.err());
        }
        assert!(validate(OVERLONG).is_ok(), "{:?}", validate(OVERLONG).err());
        for refused in [
            // Two memories.
            "(module (memory 1) (memory 1))",
            // A tail call.
            "(module (func $f (return_call $f)))",
            // Threads' shared memory.
            "(module (memory 1 1 shared))",
            // 64-bit memory.
            "(module (memory i64 1))",
            // Relaxed SIMD.
            "(module (func (result v128) (i32x4.relaxed_trunc_f32x4_s (v128.const i32x4 0 0 0 \
             0))))",
            // Wide arithmetic.
            "(module (func (result i64 i64) (i64.add128 (i64.const 1) (i64.const 2) (i64.const \
             3) (i64.const 4))))",
            // GC's types.
            "(module (func (param externref)))",
        ] {
            let built = crate::wasm::text::build(refused);
            assert!(built.is_err(), "{refused} was taken");
        }
    }

    /// lengua names the engine that Cargo.toml pins.
    #[test]
    fn the_engine_is_the_one_cargo_pins() {
        assert_eq!(ENGINE, format!("wasmtime {ENGINE_VERSION}"));
        let manifest = include_str!("../../Cargo.toml");
        assert!(
            manifest.contains(&format!("wasmtime = {{ version = \"={ENGINE_VERSION}\"")),
            "Cargo.toml pins another wasmtime than {ENGINE_VERSION}"
        );
    }

    /// A module uses the features it can't be validated without, and no others.
    #[test]
    fn a_module_uses_the_features_it_needs() {
        let used = |text: &str| {
            let built =
                crate::wasm::text::build(text).unwrap_or_else(|problem| panic!("{problem}"));
            features_used(&built.wasm)
        };
        assert!(used("(module (func))").is_empty());
        // A mutable global, exported too, is WebAssembly 1.0's.
        let global = "(module (global (export \"counter\") (mut i32) (i32.const 0)))";
        assert!(used(global).is_empty(), "{:?}", used(global));
        let simd = "(module (func (result v128) (v128.const i32x4 1 2 3 4)))";
        assert_eq!(used(simd), ["simd"]);
        let bulk = "(module (memory 1) (func (memory.fill (i32.const 0) (i32.const 0) (i32.const \
                    8))))";
        assert_eq!(used(bulk), ["bulk-memory"]);
        let values = "(module (func (result i32 i32) (i32.const 1) (i32.const 2)))";
        assert_eq!(used(values), ["multi-value"]);
        let sign = "(module (func (result i32) (i32.extend8_s (i32.const 255))))";
        assert_eq!(used(sign), ["sign-extension"]);
        let saturating = "(module (func (result i32) (i32.trunc_sat_f32_s (f32.const 3.5))))";
        assert_eq!(used(saturating), ["saturating-float-to-int"]);
        // lengua's $assert_fail copies its message with memory.init.
        let asserts = "(module (memory (export \"memory\") 1) (func (export \"_start\") (call \
                       $assert_fail (i32.const 1))))";
        assert!(
            used(asserts).iter().any(|name| name == "bulk-memory"),
            "{:?}",
            used(asserts)
        );
    }

    #[test]
    fn each_setup_counts_and_stops_as_it_says() {
        assert!(Setup::Test.counts() && Setup::Test.interrupts());
        assert!(!Setup::Run.counts() && Setup::Run.interrupts());
        assert!(!Setup::Shell.counts() && !Setup::Shell.interrupts());
    }

    /// A solver's setup counts nothing and stops nothing: the prover ends its process.
    #[test]
    fn a_solver_s_setup_neither_counts_nor_stops() {
        assert!(!Setup::Solver.counts() && !Setup::Solver.interrupts());
        assert!(!Setup::PROGRAMS.contains(&Setup::Solver));
        assert!(Setup::ALL.contains(&Setup::Solver));
    }

    /// WebAssembly text as wast encodes it, with none of lengua's own checks.
    fn encoded(text: &str) -> Vec<u8> {
        let buffer = wast::parser::ParseBuffer::new(text).expect("WebAssembly text");
        let wast::Wat::Module(mut module) =
            wast::parser::parse::<wast::Wat>(&buffer).expect("a module")
        else {
            panic!("the text isn't a module");
        };
        module.encode().expect("the module's bytes")
    }

    /// A main.wat that throws: exceptions are a solver's alone. lengua's build of main.wat, which
    /// every check of a program runs, refuses it, as each of the three engines that run programs
    /// does, and the solver's engine compiles it.
    #[test]
    fn exceptions_are_a_solver_s_alone() {
        let text = "(module (memory (export \"memory\") 1) (tag $boom (param i32)) (func (export \
                    \"_start\") (throw $boom (i32.const 1))))";
        let refused = crate::wasm::text::build(text)
            .map(|_| ())
            .map_err(|problem| problem.to_string());
        assert!(
            refused
                .as_ref()
                .is_err_and(|problem| problem.contains("exceptions")),
            "{refused:?}"
        );
        let wasm = encoded(text);
        assert!(validate(&wasm).is_err());
        for setup in Setup::PROGRAMS {
            let engine = engine(setup, None).expect("the engine");
            let module = wasmtime::Module::from_binary(&engine, &wasm);
            assert!(
                module.is_err(),
                "the {setup:?} engine compiled a module with exceptions"
            );
        }
        let checked = validate_solver(&wasm);
        assert!(checked.is_ok(), "{:?}", checked.err());
        let engine = engine(Setup::Solver, None).expect("the solver's engine");
        let module = wasmtime::Module::from_binary(&engine, &wasm);
        assert!(module.is_ok(), "{:?}", module.err());
    }

    /// A function that calls itself as many times as its argument says, and gives that number.
    const DEEP: &str = "(module (func $deep (export \"deep\") (param i32) (result i32) (if \
                        (result i32) (local.get 0) (then (i32.add (call $deep (i32.sub \
                        (local.get 0) (i32.const 1))) (i32.const 1))) (else (i32.const 0)))))";

    /// What [`DEEP`] gives for `calls` in `setup`'s engine, on a thread of `stack` bytes: the
    /// number, or the engine's trap.
    fn deep(setup: Setup, stack: usize, calls: i32) -> Result<i32, String> {
        let wasm = encoded(DEEP);
        std::thread::Builder::new()
            .stack_size(stack)
            .spawn(move || {
                let engine = engine(setup, None)?;
                let module = wasmtime::Module::from_binary(&engine, &wasm)
                    .map_err(|error| error.to_string())?;
                let mut store = wasmtime::Store::new(&engine, ());
                let instance = wasmtime::Instance::new(&mut store, &module, &[])
                    .map_err(|error| error.to_string())?;
                let function = instance
                    .get_typed_func::<i32, i32>(&mut store, "deep")
                    .map_err(|error| error.to_string())?;
                function.call(&mut store, calls).map_err(|error| {
                    error
                        .downcast_ref::<wasmtime::Trap>()
                        .map_or_else(|| error.to_string(), |trap| format!("{trap:?}"))
                })
            })
            .expect("a thread")
            .join()
            .expect("the thread ends")
    }

    /// A solver's calls go as deep as a native solver's stack lets them, past where a program's
    /// trap: 60,000 calls take more than 512 KiB of any CPU's stack, at 16 bytes a call or more,
    /// and less than 8 MiB at under 139 bytes a call, which such a small function's frame is.
    #[test]
    fn a_solver_s_calls_go_deeper_than_a_program_s() {
        let calls = 60_000;
        assert_eq!(
            deep(Setup::Shell, crate::wasm::runner::STACK, calls),
            Err("StackOverflow".to_owned())
        );
        assert_eq!(deep(Setup::Solver, SOLVER_STACK, calls), Ok(calls));
    }

    #[test]
    fn compiled_modules_are_kept_in_the_data_folders_cache() {
        let folder = tempfile::tempdir().expect("a folder");
        let data_dir = crate::data_dir::DataDir::new(folder.path().join("data"));
        if plugin_data().is_none() {
            assert_eq!(
                cache_folder(&data_dir),
                folder.path().join("data").join("cache")
            );
        }
        let config = config(Setup::Test, Some(&folder.path().join("cache")));
        assert!(config.is_ok(), "{:?}", config.err());
        assert!(folder.path().join("cache").is_dir());
    }
}
