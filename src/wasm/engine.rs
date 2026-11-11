//! The engine that runs lengua's WebAssembly: Wasmtime with Cranelift, pinned, with exactly the
//! WebAssembly features [`FEATURES`] names, so that a module the validator takes is a module the
//! engine runs, and floats' NaNs canonical, so a program computes the same bits on every CPU.
//!
//! Three configurations ([`Setup`]): a test's counts every operation it executes as Wasmtime's
//! fuel, which is deterministic, and stops at a deadline through the engine's epochs; a run's
//! stops at its deadline the same way and counts nothing; a run from a shell, or a benchmark's,
//! does neither, so it pays for nothing it doesn't use.
//!
//! Two engines for each: one that compiles ([`engine`]), with Wasmtime's own cache of compiled
//! modules in the `cache` folder of lengua's data folder, which the tests' workers and the `run`
//! tool use; and one that only runs ([`runner`]), with no compiler in it, which a run from a
//! shell loads a module into that was compiled ahead of the run and sealed with the user's key
//! ([`crate::lock::Key::compile_sealed`]). The two take the same settings wherever a compiled
//! module depends on them ([`shared`]), so the second takes what the first compiled. lengua-run
//! is built without the compiler (the `compiler` feature off) and holds only the second.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use wasmparser::WasmFeatures;
#[cfg(feature = "compiler")]
use wasmtime::{Cache, CacheConfig, Strategy};
use wasmtime::{Config, Engine, Module};

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

/// What a configuration of the engine is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Setup {
    /// A test: every operation counted as fuel, and a deadline.
    Test,
    /// A run for someone, such as the `run` tool's: a deadline, and no count.
    Run,
    /// A run from a shell, or a benchmark: neither.
    Shell,
}

impl Setup {
    /// Every configuration.
    pub const ALL: [Self; 3] = [Self::Test, Self::Run, Self::Shell];

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
}

/// Set once a process says it only runs what was compiled before ([`run_only`]).
static RUNS_ONLY: AtomicBool = AtomicBool::new(false);

/// What a process that can't compile says of a module it would have to.
const NO_COMPILER: &str = "this lengua only runs modules compiled before, and has no compiler";

/// Whether this build holds the engine's compiler (the `compiler` feature): lengua-mcp's does,
/// and lengua-run's doesn't when it is built by itself, as it ships.
pub const BUILT_WITH_COMPILER: bool = cfg!(feature = "compiler");

/// Says that this process only runs what was compiled before: lengua-run, which calls the
/// compiler nowhere, also in a build that has one, as the tests' build of the whole workspace
/// does.
pub fn run_only() {
    RUNS_ONLY.store(true, Ordering::Relaxed);
}

/// Whether this process can compile a module: its build has the compiler (the `compiler`
/// feature), and it isn't lengua-run ([`run_only`]).
#[must_use]
pub fn compiles() -> bool {
    cfg!(feature = "compiler") && !RUNS_ONLY.load(Ordering::Relaxed)
}

/// The program this process is, as what it says on standard error begins: lengua-run once it
/// said it only runs ([`run_only`]), else lengua-mcp.
#[must_use]
pub fn name() -> &'static str {
    if RUNS_ONLY.load(Ordering::Relaxed) {
        "lengua-run"
    } else {
        "lengua-mcp"
    }
}

/// The settings of `setup` that a compiled module depends on, which Wasmtime holds a module to
/// when it loads one: the engine that compiles and the engine that only runs set them here, and
/// nowhere else, so that the second takes what the first compiled.
fn shared(config: &mut Config, setup: Setup) {
    // Every feature off, then lengua's on: whatever Wasmtime's defaults are, its set is this one.
    config.wasm_features(WasmFeatures::all(), false);
    config.wasm_features(FEATURES, true);
    config.consume_fuel(setup.counts());
    config.epoch_interruption(setup.interrupts());
    config.max_wasm_stack(MAX_WASM_STACK);
    // The map from the engine's code to the module's operators, which the check of a secret
    // idea's machine code reads: Wasmtime's own default, set here so that it holds whatever the
    // default becomes.
    config.generate_address_map(true);
    // A run from a shell starts a module once, in a process of its own. Wasmtime's image of a
    // module's first memory, mapped copy-on-write, pays where a module starts many times: for
    // one start it costs a file in memory, made and taken away again (about 0.1 ms of a run
    // that takes 1 ms, as W19 measured). Without it the module's data is copied in once.
    if setup == Setup::Shell {
        config.memory_init_cow(false);
    }
}

/// The configuration of the engine that runs, for `setup`, what [`config`]'s engine compiled,
/// and compiles nothing: where the build has a compiler, it is left out of this engine.
#[must_use]
pub fn runner_config(setup: Setup) -> Config {
    let mut config = Config::new();
    #[cfg(feature = "compiler")]
    config.enable_compiler(false);
    shared(&mut config, setup);
    config
}

/// This process's engine that runs what was compiled for a run from a shell ([`Setup::Shell`])
/// and compiles nothing, made at its first use. It stops no program, so it has no clock.
///
/// # Errors
///
/// When the engine can't be made here.
pub fn runner() -> Result<Engine, String> {
    static RUNNER: OnceLock<Result<Engine, String>> = OnceLock::new();
    RUNNER
        .get_or_init(|| {
            Engine::new(&runner_config(Setup::Shell))
                .map_err(|error| format!("the WebAssembly engine couldn't start: {error}"))
        })
        .clone()
}

/// The configuration of the engine that compiles for `setup`, keeping compiled modules in
/// `cache` when given.
///
/// # Errors
///
/// When the cache can't be set up in that folder, or this process has no compiler.
#[cfg(feature = "compiler")]
pub fn config(setup: Setup, cache: Option<&Path>) -> Result<Config, String> {
    if !compiles() {
        return Err(NO_COMPILER.to_owned());
    }
    let mut config = Config::new();
    config.strategy(Strategy::Cranelift);
    shared(&mut config, setup);
    config.cranelift_nan_canonicalization(true);
    config.parallel_compilation(true);
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

/// The configuration of the engine that compiles: none, in a build without the compiler.
///
/// # Errors
///
/// Always: this build has no compiler.
#[cfg(not(feature = "compiler"))]
pub fn config(_setup: Setup, _cache: Option<&Path>) -> Result<Config, String> {
    Err(NO_COMPILER.to_owned())
}

/// This process's engine that compiles for `setup`, made at its first use with compiled modules
/// kept in `cache`, which later calls don't change: a process has one data folder. An engine
/// that stops programs has a thread that moves its epoch on every [`TICK`].
///
/// # Errors
///
/// When the engine can't be made here, or this process has no compiler.
pub fn engine(setup: Setup, cache: Option<&Path>) -> Result<Engine, String> {
    static TEST: OnceLock<Result<Engine, String>> = OnceLock::new();
    static RUN: OnceLock<Result<Engine, String>> = OnceLock::new();
    static SHELL: OnceLock<Result<Engine, String>> = OnceLock::new();
    let slot = match setup {
        Setup::Test => &TEST,
        Setup::Run => &RUN,
        Setup::Shell => &SHELL,
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

/// The module `wasm`, compiled by `engine`, one of [`engine`]'s, or loaded from its cache of
/// compiled modules.
///
/// # Errors
///
/// When the engine doesn't compile the module, or this build has no compiler.
pub fn compile(engine: &Engine, wasm: &[u8]) -> wasmtime::Result<Module> {
    #[cfg(feature = "compiler")]
    {
        Module::from_binary(engine, wasm)
    }
    #[cfg(not(feature = "compiler"))]
    {
        let _ = (engine, wasm);
        Err(wasmtime::Error::msg(NO_COMPILER))
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    /// A module the engine compiles; `text` is WebAssembly text.
    fn taken(setup: Setup, text: &str) -> Result<(), String> {
        let wasm = crate::wasm::text::build(text)
            .map_err(|problem| problem.to_string())?
            .wasm;
        let engine = engine(setup, None)?;
        compile(&engine, &wasm)
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
                assert_eq!(taken(setup, text), Ok(()), "{setup:?}: {text}");
            }
            let engine = engine(setup, None).expect("the engine");
            let module = compile(&engine, OVERLONG);
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

    /// The engine that only runs takes what the engine that compiles made, in every
    /// configuration: Wasmtime holds a compiled module to the settings it was compiled with,
    /// and the two engines share every one of them. It compiles nothing itself, and it refuses
    /// what was compiled for another configuration, so a module compiled to be counted or
    /// stopped never runs where nothing counts or stops it.
    #[test]
    fn the_engine_that_only_runs_takes_what_the_other_compiled() {
        let text = "(module (memory (export \"memory\") 1) (data (i32.const 8) \"lengua\") (func \
                    (export \"_start\") (i32.store8 (i32.const 0) (i32.load8_u (i32.const 8)))))";
        let wasm = crate::wasm::text::build(text).expect("it builds").wasm;
        let hash = crate::hash::Sha256Hex::of(&wasm);
        let key = crate::lock::Key::from_bytes([7; 32]);
        let sealed = |setup: Setup| {
            let compiler = engine(setup, None).expect("the engine that compiles");
            key.compile_sealed(&compiler, &hash, &wasm)
                .expect("it compiles")
        };
        for setup in Setup::ALL {
            let runner = Engine::new(&runner_config(setup)).expect("the engine that runs");
            let loaded = key.load_sealed(&runner, &hash, &sealed(setup));
            assert!(loaded.is_ok(), "{setup:?}: {:?}", loaded.err());
            assert!(
                Module::from_binary(&runner, &wasm).is_err(),
                "{setup:?}: the engine that only runs compiled a module"
            );
            for other in Setup::ALL {
                let refused = key.load_sealed(&runner, &hash, &sealed(other)).err();
                if other != setup {
                    assert!(
                        matches!(refused, Some(lengua_os::Unsealed::Engine(_))),
                        "{setup:?} took {other:?}'s module: {refused:?}"
                    );
                }
            }
        }
        // The process's own engine that runs is the one for a run from a shell.
        let runner = runner().expect("the engine that runs");
        assert!(
            key.load_sealed(&runner, &hash, &sealed(Setup::Shell))
                .is_ok()
        );
    }

    #[test]
    fn each_setup_counts_and_stops_as_it_says() {
        assert!(Setup::Test.counts() && Setup::Test.interrupts());
        assert!(!Setup::Run.counts() && Setup::Run.interrupts());
        assert!(!Setup::Shell.counts() && !Setup::Shell.interrupts());
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
