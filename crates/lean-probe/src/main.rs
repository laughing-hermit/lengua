//! Temporary, for W19's measurements: runs a precompiled module whose only import is
//! `proc_exit`, in Wasmtime's runtime alone. `lean-probe <module.cwasm> [file|bytes|none]`.

use std::process::ExitCode;

use wasmparser::WasmFeatures;
use wasmtime::{Config, Engine, Linker, Store};

/// lengua's features, as `src/wasm/engine.rs` has them.
const FEATURES: WasmFeatures = WasmFeatures::FLOATS
    .union(WasmFeatures::MUTABLE_GLOBAL)
    .union(WasmFeatures::SATURATING_FLOAT_TO_INT)
    .union(WasmFeatures::SIGN_EXTENSION)
    .union(WasmFeatures::REFERENCE_TYPES)
    .union(WasmFeatures::MULTI_VALUE)
    .union(WasmFeatures::BULK_MEMORY)
    .union(WasmFeatures::SIMD);

/// The program's exit, as an error that ends its call.
#[derive(Debug)]
struct Exit(i32);

impl std::fmt::Display for Exit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "exit {}", self.0)
    }
}

impl std::error::Error for Exit {}

/// Runs the module, giving its exit code.
fn run(path: &str, how: &str) -> Result<i32, String> {
    let mut config = Config::new();
    config.wasm_features(WasmFeatures::all(), false);
    config.wasm_features(FEATURES, true);
    config.consume_fuel(false);
    config.epoch_interruption(false);
    config.max_wasm_stack(512 * 1024);
    config.generate_address_map(true);
    let engine = Engine::new(&config).map_err(|error| format!("{error:#}"))?;
    if how == "none" {
        return Ok(0);
    }
    let module = if how == "file" {
        lengua_os::deserialize_file(&engine, std::path::Path::new(path))?
    } else {
        let bytes = std::fs::read(path).map_err(|error| error.to_string())?;
        lengua_os::deserialize(&engine, &bytes)?
    };
    let mut linker: Linker<()> = Linker::new(&engine);
    linker
        .func_wrap(
            "wasi_snapshot_preview1",
            "proc_exit",
            |code: i32| -> wasmtime::Result<()> { Err(wasmtime::Error::new(Exit(code))) },
        )
        .map_err(|error| format!("{error:#}"))?;
    let mut store = Store::new(&engine, ());
    let instance = linker
        .instantiate(&mut store, &module)
        .map_err(|error| format!("{error:#}"))?;
    let start = instance
        .get_typed_func::<(), ()>(&mut store, "_start")
        .map_err(|error| format!("{error:#}"))?;
    match start.call(&mut store, ()) {
        Ok(()) => Ok(0),
        Err(error) => error
            .downcast_ref::<Exit>()
            .map(|exit| exit.0)
            .ok_or_else(|| format!("{error:#}")),
    }
}

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let path = args.next().unwrap_or_default();
    let how = args.next().unwrap_or_else(|| "bytes".to_owned());
    match run(&path, &how) {
        Ok(code) => ExitCode::from(u8::try_from(code).unwrap_or(125)),
        Err(problem) => {
            eprintln!("lean-probe: {problem}");
            ExitCode::from(125)
        }
    }
}
