use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread;
use std::time::Duration;

use lengua_idea::{Build, Grants};
use wasmtime::component::{Component, HasData, Linker, ResourceTable};
use wasmtime::{Config, Engine, ResourceLimiter, Store, Trap};
use wasmtime_wasi::cli::{WasiCli, WasiCliView};
use wasmtime_wasi::clocks::{WasiClocks, WasiClocksView};
use wasmtime_wasi::filesystem::{WasiFilesystem, WasiFilesystemView};
use wasmtime_wasi::p2::bindings::sync::Command;
use wasmtime_wasi::p2::bindings::{cli, clocks, filesystem, random, sync};
use wasmtime_wasi::p2::pipe::{MemoryInputPipe, MemoryOutputPipe};
use wasmtime_wasi::random::{WasiRandom, WasiRandomView};
use wasmtime_wasi::{
    Deterministic, FsPerms, HostMonotonicClock, HostWallClock, I32Exit, WasiCtx, WasiCtxBuilder,
    WasiCtxView, WasiView,
};

pub(crate) const TICK_MS: u64 = 10;
const OUTPUT: usize = 16 << 20;
const TABLE: usize = 1 << 20;
const STEP_NS: u64 = 1_000_000;
const FIXED_SECONDS: u64 = 1_767_225_600;
const SEED: u64 = 0x6c65_6e67_7561_0003;
const RANDOM_BYTES: usize = 4096;
const KEPT: usize = 32;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Input {
    pub arguments: Vec<String>,
    pub stdin: Vec<u8>,
    pub fixed: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Ending {
    Exited(i32),
    OutOfTime(String),
    OutOfMemory,
    Trapped(String),
    Refused(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Outcome {
    pub ending: Ending,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

#[derive(Clone)]
pub(crate) struct Host {
    engine: Engine,
    components: Arc<Mutex<HashMap<String, Component>>>,
}

struct Limits {
    memory: usize,
    hit: bool,
}

impl ResourceLimiter for Limits {
    fn memory_growing(
        &mut self,
        _current: usize,
        desired: usize,
        maximum: Option<usize>,
    ) -> wasmtime::Result<bool> {
        if desired > self.memory {
            self.hit = true;
            return Ok(false);
        }
        Ok(maximum.is_none_or(|m| desired <= m))
    }

    fn table_growing(
        &mut self,
        _current: usize,
        desired: usize,
        maximum: Option<usize>,
    ) -> wasmtime::Result<bool> {
        Ok(desired <= TABLE && maximum.is_none_or(|m| desired <= m))
    }
}

struct Guest {
    ctx: WasiCtx,
    table: ResourceTable,
    limits: Limits,
}

impl WasiView for Guest {
    fn ctx(&mut self) -> WasiCtxView<'_> {
        WasiCtxView {
            ctx: &mut self.ctx,
            table: &mut self.table,
        }
    }
}

fn limiter(guest: &mut Guest) -> &mut dyn ResourceLimiter {
    &mut guest.limits
}

struct Table;

impl HasData for Table {
    type Data<'a> = &'a mut ResourceTable;
}

fn table(guest: &mut Guest) -> &mut ResourceTable {
    &mut guest.table
}

#[derive(Default)]
struct Steps {
    now: AtomicU64,
}

impl HostMonotonicClock for Steps {
    fn resolution(&self) -> u64 {
        STEP_NS
    }

    fn now(&self) -> u64 {
        self.now.fetch_add(STEP_NS, Ordering::Relaxed) + STEP_NS
    }
}

struct Fixed;

impl HostWallClock for Fixed {
    fn resolution(&self) -> Duration {
        Duration::from_secs(1)
    }

    fn now(&self) -> Duration {
        Duration::from_secs(FIXED_SECONDS)
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

pub(crate) fn seeded(seed: u64, length: usize) -> Vec<u8> {
    let mut state = seed;
    let mut out = Vec::with_capacity(length + 8);
    while out.len() < length {
        state = state.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = state;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^= z >> 31;
        out.extend_from_slice(&z.to_le_bytes());
    }
    out.truncate(length);
    out
}

fn link(linker: &mut Linker<Guest>, interface: &str) -> wasmtime::Result<bool> {
    let l = linker;
    let cli_view = <Guest as WasiCliView>::cli;
    let clocks_view = <Guest as WasiClocksView>::clocks;
    let files_view = <Guest as WasiFilesystemView>::filesystem;
    let random_view = <Guest as WasiRandomView>::random;
    match interface {
        "wasi:cli/environment" => cli::environment::add_to_linker::<Guest, WasiCli>(l, cli_view)?,
        "wasi:cli/exit" => cli::exit::add_to_linker::<Guest, WasiCli>(l, cli_view)?,
        "wasi:cli/stderr" => cli::stderr::add_to_linker::<Guest, WasiCli>(l, cli_view)?,
        "wasi:cli/stdin" => cli::stdin::add_to_linker::<Guest, WasiCli>(l, cli_view)?,
        "wasi:cli/stdout" => cli::stdout::add_to_linker::<Guest, WasiCli>(l, cli_view)?,
        "wasi:cli/terminal-input" => {
            cli::terminal_input::add_to_linker::<Guest, WasiCli>(l, cli_view)?;
        }
        "wasi:cli/terminal-output" => {
            cli::terminal_output::add_to_linker::<Guest, WasiCli>(l, cli_view)?;
        }
        "wasi:cli/terminal-stderr" => {
            cli::terminal_stderr::add_to_linker::<Guest, WasiCli>(l, cli_view)?;
        }
        "wasi:cli/terminal-stdin" => {
            cli::terminal_stdin::add_to_linker::<Guest, WasiCli>(l, cli_view)?;
        }
        "wasi:cli/terminal-stdout" => {
            cli::terminal_stdout::add_to_linker::<Guest, WasiCli>(l, cli_view)?;
        }
        "wasi:clocks/monotonic-clock" => {
            clocks::monotonic_clock::add_to_linker::<Guest, WasiClocks>(l, clocks_view)?;
        }
        "wasi:clocks/wall-clock" => {
            clocks::wall_clock::add_to_linker::<Guest, WasiClocks>(l, clocks_view)?;
        }
        "wasi:filesystem/preopens" => {
            filesystem::preopens::add_to_linker::<Guest, WasiFilesystem>(l, files_view)?;
        }
        "wasi:filesystem/types" => {
            sync::filesystem::types::add_to_linker::<Guest, WasiFilesystem>(l, files_view)?;
        }
        "wasi:io/error" => {
            wasmtime_wasi_io::bindings::wasi::io::error::add_to_linker::<Guest, Table>(l, table)?;
        }
        "wasi:io/poll" => sync::io::poll::add_to_linker::<Guest, Table>(l, table)?,
        "wasi:io/streams" => sync::io::streams::add_to_linker::<Guest, Table>(l, table)?,
        "wasi:random/random" => random::random::add_to_linker::<Guest, WasiRandom>(l, random_view)?,
        "wasi:random/insecure" => {
            random::insecure::add_to_linker::<Guest, WasiRandom>(l, random_view)?;
        }
        "wasi:random/insecure-seed" => {
            random::insecure_seed::add_to_linker::<Guest, WasiRandom>(l, random_view)?;
        }
        _ => return Ok(false),
    }
    Ok(true)
}

fn context(
    folder: &Path,
    name: &str,
    grants: &Grants,
    input: &Input,
    output: (&MemoryOutputPipe, &MemoryOutputPipe),
) -> Result<WasiCtx, String> {
    let mut builder = WasiCtxBuilder::new();
    builder
        .stdin(MemoryInputPipe::new(input.stdin.clone()))
        .stdout(output.0.clone())
        .stderr(output.1.clone());
    if grants.arguments {
        let mut all = vec![name.to_string()];
        all.extend(input.arguments.iter().cloned());
        builder.args(all.as_slice());
    }
    if !grants.clock || input.fixed {
        builder.monotonic_clock(Steps::default());
        builder.wall_clock(Fixed);
    }
    if input.fixed {
        builder
            .secure_random(Deterministic::new(seeded(SEED, RANDOM_BYTES)))
            .insecure_random(Deterministic::new(seeded(SEED ^ 1, RANDOM_BYTES)))
            .insecure_random_seed(u128::from(SEED));
    }
    for granted in &grants.folders {
        let place = folder.join(&granted.path);
        if granted.write && !place.exists() {
            fs::create_dir_all(&place).map_err(|e| {
                format!(
                    "lengua could not make the folder `{}` that the idea grants: {e}",
                    granted.path
                )
            })?;
        }
        if !place.is_dir() {
            return Err(format!(
                "the folder `{}` that the idea grants is not there",
                granted.path
            ));
        }
        let perms = if granted.write {
            FsPerms::ReadWrite
        } else {
            FsPerms::ReadOnly
        };
        builder
            .preopened_dir(&place, &granted.path, perms)
            .map_err(|e| format!("lengua could not open the folder `{}`: {e:#}", granted.path))?;
    }
    Ok(builder.build())
}

pub(crate) fn outside(imports: &[String], world: &[String]) -> Vec<String> {
    imports
        .iter()
        .filter(|name| {
            let (interface, version) = name.split_once('@').unwrap_or((name.as_str(), ""));
            let known = world.iter().any(|w| w == interface);
            !(known && (version == "0.2" || version.starts_with("0.2.")))
        })
        .cloned()
        .collect()
}

pub(crate) fn grant_for(import: &str) -> Option<&'static str> {
    let interface = import.split('@').next().unwrap_or(import);
    if interface == "wasi:clocks/wall-clock" {
        Some("`- Reads: the clock`")
    } else if interface.starts_with("wasi:random/") {
        Some("`- Reads: random numbers`")
    } else if interface.starts_with("wasi:filesystem/") {
        Some("``- Reads: files in `<folder>` `` or ``- Writes: files in `<folder>` ``")
    } else if interface.starts_with("wasi:sockets/") {
        Some("``- Connects to: `<host>:<port>` ``")
    } else {
        None
    }
}

fn ending(result: &wasmtime::Result<Result<(), ()>>, memory_hit: bool) -> Ending {
    match result {
        Ok(Ok(())) => Ending::Exited(0),
        Ok(Err(())) => Ending::Exited(1),
        Err(error) => {
            if let Some(exit) = error.downcast_ref::<I32Exit>() {
                Ending::Exited(exit.0)
            } else if error.downcast_ref::<Trap>() == Some(&Trap::Interrupt) {
                Ending::OutOfTime("the program ran past the time of its idea".to_string())
            } else if memory_hit {
                Ending::OutOfMemory
            } else {
                Ending::Trapped(format!("{error:#}"))
            }
        }
    }
}

impl Host {
    pub(crate) fn new() -> Result<Self, String> {
        let mut config = Config::new();
        config.epoch_interruption(true);
        let engine = Engine::new(&config).map_err(|e| format!("{e:#}"))?;
        let ticker = engine.clone();
        thread::spawn(move || {
            loop {
                thread::sleep(Duration::from_millis(TICK_MS));
                ticker.increment_epoch();
            }
        });
        Ok(Self {
            engine,
            components: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    pub(crate) fn component(&self, bytes: &[u8], sha256: &str) -> Result<Component, String> {
        if let Some(found) = lock(&self.components).get(sha256) {
            return Ok(found.clone());
        }
        let component = Component::new(&self.engine, bytes).map_err(|e| format!("{e:#}"))?;
        let mut kept = lock(&self.components);
        if kept.len() >= KEPT {
            kept.clear();
        }
        kept.insert(sha256.to_string(), component.clone());
        Ok(component)
    }

    pub(crate) fn imports(&self, component: &Component) -> Vec<String> {
        component
            .component_type()
            .imports(&self.engine)
            .map(|(name, _)| name.to_string())
            .collect()
    }

    pub(crate) fn run(&self, folder: &Path, build: &Build, input: &Input) -> Outcome {
        let stdout = MemoryOutputPipe::new(OUTPUT);
        let stderr = MemoryOutputPipe::new(OUTPUT);
        let ending = self.execute(folder, build, input, (&stdout, &stderr));
        Outcome {
            ending,
            stdout: stdout.contents().to_vec(),
            stderr: stderr.contents().to_vec(),
        }
    }

    fn execute(
        &self,
        folder: &Path,
        build: &Build,
        input: &Input,
        output: (&MemoryOutputPipe, &MemoryOutputPipe),
    ) -> Ending {
        let bytes = match fs::read(folder.join(&build.artifact)) {
            Ok(bytes) => bytes,
            Err(error) => {
                return Ending::Refused(format!(
                    "lengua could not read the component `{}`: {error}; call check to build it",
                    build.artifact
                ));
            }
        };
        let component = match self.component(&bytes, &lengua_idea::sha256_hex(&bytes)) {
            Ok(component) => component,
            Err(error) => {
                return Ending::Refused(format!(
                    "wasmtime could not compile the component: {error}"
                ));
            }
        };
        let mut linker = Linker::new(&self.engine);
        for interface in &build.world {
            if let Err(error) = link(&mut linker, interface) {
                return Ending::Refused(format!(
                    "the host could not link `{interface}`: {error:#}"
                ));
            }
        }
        let ctx = match context(folder, &build.package, &build.grants, input, output) {
            Ok(ctx) => ctx,
            Err(message) => return Ending::Refused(message),
        };
        let memory = usize::try_from(build.grants.memory).unwrap_or(usize::MAX);
        let guest = Guest {
            ctx,
            table: ResourceTable::new(),
            limits: Limits { memory, hit: false },
        };
        let mut store = Store::new(&self.engine, guest);
        store.limiter(limiter);
        store.set_epoch_deadline(build.grants.time_ms / TICK_MS + 1);
        let command = match Command::instantiate(&mut store, &component, &linker) {
            Ok(command) => command,
            Err(error) => {
                if store.data().limits.hit {
                    return Ending::OutOfMemory;
                }
                return Ending::Refused(format!(
                    "the host refused the component, since it links only the world of the idea: {error:#}"
                ));
            }
        };
        let result = command.wasi_cli_run().call_run(&mut store);
        ending(&result, store.data().limits.hit)
    }
}

#[cfg(test)]
mod tests {
    use super::{grant_for, outside, seeded};

    #[test]
    fn imports_outside_the_world_are_named_and_any_0_2_version_is_taken() {
        let world = vec!["wasi:cli/stdout".to_string(), "wasi:io/streams".to_string()];
        let imports = vec![
            "wasi:cli/stdout@0.2.12".to_string(),
            "wasi:io/streams@0.2.0".to_string(),
            "wasi:random/random@0.2.12".to_string(),
            "wasi:cli/stdout@0.3.0".to_string(),
            "example:thing/other".to_string(),
        ];
        assert_eq!(
            outside(&imports, &world),
            [
                "wasi:random/random@0.2.12",
                "wasi:cli/stdout@0.3.0",
                "example:thing/other"
            ]
        );
        assert_eq!(
            grant_for("wasi:random/insecure@0.2.12"),
            Some("`- Reads: random numbers`")
        );
        assert!(grant_for("wasi:filesystem/types@0.2.12").is_some());
        assert_eq!(grant_for("example:thing/other"), None);
    }

    #[test]
    fn seeded_bytes_are_the_same_each_time_and_differ_by_seed() {
        assert_eq!(seeded(7, 100), seeded(7, 100));
        assert_eq!(seeded(7, 100).len(), 100);
        assert_ne!(seeded(7, 64), seeded(8, 64));
    }
}
