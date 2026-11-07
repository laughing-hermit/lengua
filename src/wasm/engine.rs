//! The engine that runs lengua's WebAssembly: Wasmtime with Cranelift, pinned, with exactly the
//! WebAssembly features [`FEATURES`] names, so that a module the validator takes is a module the
//! engine runs, and floats' NaNs canonical, so a program computes the same bits on every CPU.
//!
//! Three configurations ([`Setup`]): a test's counts every operation it executes as Wasmtime's
//! fuel, which is deterministic, and stops at a deadline through the engine's epochs; a run's
//! stops at its deadline the same way and counts nothing; a run from a shell, or a benchmark's,
//! does neither, so it pays for nothing it doesn't use. Compiled modules are kept in Wasmtime's
//! own cache, in the `cache` folder of lengua's data folder.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use wasmparser::WasmFeatures;
use wasmtime::{Cache, CacheConfig, Config, Engine, Strategy};

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

/// How often the engine's epoch moves on, which is how often a running program's deadline and
/// stop are looked at.
pub const TICK: Duration = Duration::from_millis(10);

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

/// The engine's configuration for `setup`, keeping compiled modules in `cache` when given.
///
/// # Errors
///
/// When the cache can't be set up in that folder.
pub fn config(setup: Setup, cache: Option<&Path>) -> Result<Config, String> {
    let mut config = Config::new();
    config.strategy(Strategy::Cranelift);
    // Every feature off, then lengua's on: whatever Wasmtime's defaults are, its set is this one.
    config.wasm_features(WasmFeatures::all(), false);
    config.wasm_features(FEATURES, true);
    config.cranelift_nan_canonicalization(true);
    config.parallel_compilation(true);
    config.consume_fuel(setup.counts());
    config.epoch_interruption(setup.interrupts());
    if let Some(folder) = cache {
        let mut settings = CacheConfig::new();
        settings.with_directory(folder);
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

/// Moves `engine`'s epoch on every [`TICK`], for as long as the process lives.
fn tick(engine: &Engine) {
    loop {
        std::thread::sleep(TICK);
        engine.increment_epoch();
    }
}

/// Where compiled modules are kept: `$PLUGIN_DATA/cache` when the agent's harness gives the
/// plugin a data folder, else the `cache` folder of lengua's own. The lock's signing key stays
/// in lengua's own folder either way.
#[must_use]
pub fn cache_folder(data_dir: &crate::data_dir::DataDir) -> PathBuf {
    match std::env::var_os("PLUGIN_DATA")
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
        if std::env::var_os("PLUGIN_DATA").is_none() {
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
