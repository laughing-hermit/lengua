mod http;
mod network;

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::Duration;

use lengua_idea::{Build, Grants, Reach};
use wasmtime::component::types::ComponentItem;
use wasmtime::component::{Component, HasData, Linker, ResourceTable};
use wasmtime::{Config, Engine, ResourceLimiter, Store, Trap};
use wasmtime_wasi::cli::{WasiCli, WasiCliView};
use wasmtime_wasi::clocks::{WasiClocks, WasiClocksView};
use wasmtime_wasi::filesystem::{WasiFilesystem, WasiFilesystemView};
use wasmtime_wasi::p2::bindings::sync::Command;
use wasmtime_wasi::p2::bindings::{cli, clocks, filesystem, random, sockets, sync};
use wasmtime_wasi::p2::pipe::{MemoryInputPipe, MemoryOutputPipe};
use wasmtime_wasi::random::{WasiRandom, WasiRandomView};
use wasmtime_wasi::sockets::{WasiSockets, WasiSocketsView};
use wasmtime_wasi::{
    Deterministic, FsPerms, HostMonotonicClock, HostWallClock, I32Exit, WasiCtx, WasiCtxBuilder,
    WasiCtxView, WasiView,
};
use wasmtime_wasi_http::{WasiHttpCtx, WasiHttpCtxView, WasiHttpView};

pub use crate::http::{read_request, response_bytes};

pub const TICK_MS: u64 = 10;
pub const OUTPUT: usize = 16 << 20;
const TABLE: usize = 1 << 20;
const STEP_NS: u64 = 1_000_000;
const FIXED_SECONDS: u64 = 1_767_225_600;
const SEED: u64 = 0x6c65_6e67_7561_0003;
const RANDOM_BYTES: usize = 4096;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Input {
    pub arguments: Vec<String>,
    pub stdin: Vec<u8>,
    pub fixed: bool,
    pub root: Option<PathBuf>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Ending {
    Exited(i32),
    OutOfTime(String),
    OutOfMemory,
    Trapped(String),
    Refused(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Outcome {
    pub ending: Ending,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

impl Outcome {
    #[must_use]
    pub fn refused(message: String) -> Self {
        Self {
            ending: Ending::Refused(message),
            stdout: Vec::new(),
            stderr: Vec::new(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Streams {
    Memory,
    Inherit,
}

#[derive(Clone)]
pub struct Host {
    engine: Engine,
}

pub struct Program<'a> {
    pub folder: &'a Path,
    pub component: &'a Component,
    pub trusted: &'a [Component],
    pub build: &'a Build,
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
    http: WasiHttpCtx,
    hooks: http::Refuse,
}

impl WasiView for Guest {
    fn ctx(&mut self) -> WasiCtxView<'_> {
        WasiCtxView {
            ctx: &mut self.ctx,
            table: &mut self.table,
        }
    }
}

impl WasiHttpView for Guest {
    fn http(&mut self) -> WasiHttpCtxView<'_> {
        WasiHttpCtxView {
            hooks: &mut self.hooks,
            table: &mut self.table,
            ctx: &mut self.http,
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

fn seeded(seed: u64, length: usize) -> Vec<u8> {
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

fn link_sockets(l: &mut Linker<Guest>, interface: &str) -> wasmtime::Result<bool> {
    let view = <Guest as WasiSocketsView>::sockets;
    match interface {
        "wasi:sockets/tcp" => sync::sockets::tcp::add_to_linker::<Guest, WasiSockets>(l, view)?,
        "wasi:sockets/udp" => sync::sockets::udp::add_to_linker::<Guest, WasiSockets>(l, view)?,
        "wasi:sockets/udp-create-socket" => {
            sync::sockets::udp_create_socket::add_to_linker::<Guest, WasiSockets>(l, view)?;
        }
        "wasi:sockets/ip-name-lookup" => {
            sync::sockets::ip_name_lookup::add_to_linker::<Guest, WasiSockets>(l, view)?;
        }
        "wasi:sockets/tcp-create-socket" => {
            sockets::tcp_create_socket::add_to_linker::<Guest, WasiSockets>(l, view)?;
        }
        "wasi:sockets/instance-network" => {
            sockets::instance_network::add_to_linker::<Guest, WasiSockets>(l, view)?;
        }
        "wasi:sockets/network" => {
            let options = sync::LinkOptions::default();
            let network: sockets::network::LinkOptions = (&options).into();
            sockets::network::add_to_linker::<Guest, WasiSockets>(l, &network, view)?;
        }
        "wasi:http/types" => wasmtime_wasi_http::p2::add_only_http_to_linker_sync(l)?,
        _ => return Ok(false),
    }
    Ok(true)
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
        _ => return link_sockets(l, interface),
    }
    Ok(true)
}

fn streams_of(
    builder: &mut WasiCtxBuilder,
    grants: &Grants,
    input: &Input,
    streams: Streams,
    output: (&MemoryOutputPipe, &MemoryOutputPipe),
) {
    match streams {
        Streams::Memory => {
            builder
                .stdin(MemoryInputPipe::new(input.stdin.clone()))
                .stdout(output.0.clone())
                .stderr(output.1.clone());
        }
        Streams::Inherit => {
            if grants.reach(Reach::StandardInput) {
                builder.inherit_stdin();
            } else {
                builder.stdin(MemoryInputPipe::new(Vec::new()));
            }
            builder.inherit_stdout().inherit_stderr();
        }
    }
}

fn context(
    run: &Program,
    input: &Input,
    streams: Streams,
    output: (&MemoryOutputPipe, &MemoryOutputPipe),
) -> Result<WasiCtx, String> {
    let grants = &run.build.grants;
    let mut builder = WasiCtxBuilder::new();
    let server = grants.server.is_some();
    let stdin_input = if server {
        Input::default()
    } else {
        input.clone()
    };
    streams_of(&mut builder, grants, &stdin_input, streams, output);
    if grants.reach(Reach::Arguments) {
        let mut all = vec![run.build.package.clone()];
        all.extend(input.arguments.iter().cloned());
        builder.args(all.as_slice());
    }
    if !grants.reach(Reach::Clock) || input.fixed {
        builder.monotonic_clock(Steps::default());
        builder.wall_clock(Fixed);
    }
    if input.fixed || !grants.reach(Reach::Random) {
        builder.insecure_random_seed(u128::from(SEED));
    }
    if input.fixed {
        builder
            .secure_random(Deterministic::new(seeded(SEED, RANDOM_BYTES)))
            .insecure_random(Deterministic::new(seeded(SEED ^ 1, RANDOM_BYTES)));
    }
    network::allow(&mut builder, grants);
    let root = input.root.as_deref().unwrap_or(run.folder);
    for granted in &grants.folders {
        let place = root.join(&granted.path);
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

fn ending(result: &wasmtime::Result<Result<(), ()>>, memory_hit: bool) -> Ending {
    match result {
        Ok(Ok(())) => Ending::Exited(0),
        Ok(Err(())) => Ending::Exited(1),
        Err(error) => trap_ending(error, memory_hit),
    }
}

fn trap_ending(error: &wasmtime::Error, memory_hit: bool) -> Ending {
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

#[must_use]
pub fn config() -> Config {
    let mut config = Config::new();
    config
        .epoch_interruption(true)
        .wasm_relaxed_simd(false)
        .wasm_shared_everything_threads(false);
    config
}

#[must_use]
pub fn exported_interfaces(engine: &Engine, component: &Component) -> Vec<String> {
    component
        .component_type()
        .exports(engine)
        .filter(|(_, item)| matches!(item.ty, ComponentItem::ComponentInstance(_)))
        .map(|(name, _)| name.to_string())
        .collect()
}

struct Prepared {
    store: Store<Guest>,
    linker: Linker<Guest>,
}

impl Host {
    pub fn new(config: &Config) -> Result<Self, String> {
        let engine = Engine::new(config).map_err(|e| format!("{e:#}"))?;
        let ticker = engine.clone();
        thread::spawn(move || {
            loop {
                thread::sleep(Duration::from_millis(TICK_MS));
                ticker.increment_epoch();
            }
        });
        Ok(Self { engine })
    }

    #[must_use]
    pub fn engine(&self) -> &Engine {
        &self.engine
    }

    #[must_use]
    pub fn run(&self, run: &Program, input: &Input, streams: Streams) -> Outcome {
        if run.build.grants.server.is_some() {
            return self.answer(run, input);
        }
        let stdout = MemoryOutputPipe::new(OUTPUT);
        let stderr = MemoryOutputPipe::new(OUTPUT);
        let ending = self.execute(run, input, streams, (&stdout, &stderr));
        Outcome {
            ending,
            stdout: stdout.contents().to_vec(),
            stderr: stderr.contents().to_vec(),
        }
    }

    fn prepare(
        &self,
        run: &Program,
        input: &Input,
        streams: Streams,
        output: (&MemoryOutputPipe, &MemoryOutputPipe),
    ) -> Result<Prepared, Ending> {
        let mut linker = Linker::new(&self.engine);
        for interface in &run.build.world {
            if let Err(error) = link(&mut linker, interface) {
                return Err(Ending::Refused(format!(
                    "the host could not link `{interface}`: {error:#}"
                )));
            }
        }
        let ctx = context(run, input, streams, output).map_err(Ending::Refused)?;
        let memory = usize::try_from(run.build.grants.memory).unwrap_or(usize::MAX);
        let guest = Guest {
            ctx,
            table: ResourceTable::new(),
            limits: Limits { memory, hit: false },
            http: WasiHttpCtx::new(),
            hooks: http::Refuse,
        };
        let mut store = Store::new(&self.engine, guest);
        store.limiter(limiter);
        store.set_epoch_deadline(run.build.grants.time_ms / TICK_MS + 1);
        self.link_trusted(&mut linker, &mut store, run.trusted)?;
        Ok(Prepared { store, linker })
    }

    fn link_trusted(
        &self,
        linker: &mut Linker<Guest>,
        store: &mut Store<Guest>,
        trusted: &[Component],
    ) -> Result<(), Ending> {
        let refuse = |error: wasmtime::Error| {
            Ending::Refused(format!(
                "the host could not link the trusted component: {error:#}"
            ))
        };
        for part in trusted {
            let instance = linker.instantiate(&mut *store, part).map_err(refuse)?;
            let ty = part.component_type();
            for (name, item) in ty.exports(&self.engine) {
                let ComponentItem::ComponentInstance(inner) = item.ty else {
                    continue;
                };
                let Some(outer) = instance.get_export_index(&mut *store, None, name) else {
                    continue;
                };
                let mut place = linker.instance(name).map_err(refuse)?;
                for (function, found) in inner.exports(&self.engine) {
                    if !matches!(found.ty, ComponentItem::ComponentFunc(_)) {
                        continue;
                    }
                    let index = instance.get_export_index(&mut *store, Some(&outer), function);
                    let Some(func) = index.and_then(|i| instance.get_func(&mut *store, &i)) else {
                        continue;
                    };
                    place
                        .func_new(function, move |mut s, _ty, params, results| {
                            func.call(&mut s, params, results)
                        })
                        .map_err(refuse)?;
                }
            }
        }
        Ok(())
    }

    fn execute(
        &self,
        run: &Program,
        input: &Input,
        streams: Streams,
        output: (&MemoryOutputPipe, &MemoryOutputPipe),
    ) -> Ending {
        let Prepared { mut store, linker } = match self.prepare(run, input, streams, output) {
            Ok(prepared) => prepared,
            Err(ending) => return ending,
        };
        let command = match Command::instantiate(&mut store, run.component, &linker) {
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
    use super::seeded;

    #[test]
    fn seeded_bytes_are_the_same_each_time_and_differ_by_seed() {
        assert_eq!(seeded(7, 100), seeded(7, 100));
        assert_eq!(seeded(7, 100).len(), 100);
        assert_ne!(seeded(7, 64), seeded(8, 64));
    }
}
