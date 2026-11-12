//! The tools of a run, as the `run` tool and a run from a shell ask for them
//! ([`crate::wasm::tools`]): a trace of the program's calls to the host, a profile of its time,
//! and a stop at a line or a function of main.wat. Every check of a run comes first, and the
//! tools change none of them. What each tool does with a program that names a secret, splits its
//! work or is an outside module is decided here, in one place:
//!
//! - a trace of an idea that names a secret keeps the calls' names and the host's answers, and
//!   none of the numbers the program passed;
//! - a stop shows values, so an idea that names a secret gets none; it reads one program's
//!   values, so a program that splits its work gets none; and it is written into main.wat, so an
//!   outside module, which has no main.wat, gets none;
//! - a trace and a profile of a program that splits its work are of the program itself, not of
//!   its pieces.

use super::super::RunReport;
use super::{Code, NotRun, Runnable, built_as_tested};
use crate::state::{CODE_FILE, OUTSIDE_FILE, ProgramFolder};
use crate::wasm::engine::{self, Setup};
use crate::wasm::text::{self, Built};
use crate::wasm::tools::{self, Asked, Detail, Equipment, Found, Names, Place, Show, Tools};

/// Why an idea that names a secret gets no stop.
pub(super) const NO_STOP_SECRET: &str = "idea.md names a secret, and a stop shows the program's \
                                         values, which may be the secret: lengua doesn't stop \
                                         such a program. A trace of it keeps the calls' names \
                                         only, and a profile shows no value";

/// Why a program that splits its work gets no stop.
pub(super) const NO_STOP_SPLITS: &str = "idea.md has a `- Splits:` line, and a stop reads one \
                                         program's values, not those of the pieces that run at \
                                         once: lengua doesn't stop such a program. A trace and \
                                         a profile of it are of the program itself, not of its \
                                         pieces";

/// Why a server gets no tool.
pub(super) const NO_TOOLS_SERVING: &str = "this program serves in the background, and trace, \
                                           profile and stop_at take a run that ends: run it \
                                           without them";

/// Why an outside module gets no stop.
fn no_stop_outside() -> String {
    format!(
        "the program is {OUTSIDE_FILE}, an outside module, which has no {CODE_FILE} to stop in: \
         lengua doesn't stop it. A trace and a profile work for it"
    )
}

/// What a run with tools runs and reports with.
pub(super) struct Equipped {
    /// The tools, as the run's host takes them.
    pub tools: Tools,
    /// The names and lines of the module that runs.
    pub names: Names,
    /// The copy of main.wat with a stop written in, built, which runs in place of the module
    /// check tested.
    pub instead: Option<Built>,
}

/// What the run `asked` asks for comes to, for `ready`, whose module check tested is `built`:
/// the tools, and for a stop the copy that runs.
///
/// # Errors
///
/// When a tool isn't for this program, or main.wat has no such place to stop at.
pub(super) fn equipped(asked: &Asked, ready: &Runnable, built: &Built) -> Result<Equipped, String> {
    if !asked.any() {
        return Ok(Equipped {
            tools: Tools::default(),
            names: Names::default(),
            instead: None,
        });
    }
    let secret = !ready.secrets.is_empty();
    let splits = ready.network.reach.split.is_some();
    let text = match &ready.code {
        Code::Text(text) => Some(text.as_str()),
        Code::Outside { .. } => None,
    };
    let Some(place) = &asked.stop_at else {
        if asked.show_memory.is_some() {
            return Err("show_memory shows memory at a stop: give stop_at too".to_owned());
        }
        let mut names = text.map_or_else(Names::default, |text| Names::of(text, built));
        names.splits = splits;
        return Ok(Equipped {
            tools: tools_of(asked, secret, None),
            names,
            instead: None,
        });
    };
    let Some(text) = text else {
        return Err(no_stop_outside());
    };
    if secret {
        return Err(NO_STOP_SECRET.to_owned());
    }
    if splits {
        return Err(NO_STOP_SPLITS.to_owned());
    }
    let place = Place::parse(place)?;
    let memory = asked
        .show_memory
        .as_deref()
        .map(tools::memory_range)
        .transpose()?;
    let plan = tools::plan(text, built, &place)
        .map_err(|why| format!("lengua can't stop there: {why}"))?;
    let copy = text::build(&plan.text).map_err(|problem| {
        format!(
            "lengua can't stop at {}: the copy of {CODE_FILE} with the stop written in doesn't \
             build ({problem}). Name another line, or a function.",
            plan.at
        )
    })?;
    let mut names = Names::of(&plan.text, &copy);
    names.stop = Some(plan);
    Ok(Equipped {
        tools: tools_of(asked, secret, Some(Show { memory })),
        names,
        instead: Some(copy),
    })
}

/// The tools `asked` asks for, for an idea that names a secret or doesn't, with `stop`.
fn tools_of(asked: &Asked, secret: bool, stop: Option<Show>) -> Tools {
    let detail = if secret {
        Detail::Names
    } else {
        Detail::Arguments
    };
    Tools {
        trace: asked.trace.then_some(detail),
        profile: asked.profile,
        stop,
    }
}

/// Puts what a run's tools found into its `report`: each tool's own part, and, for a run that
/// reached its stop, that it ended there.
pub(super) fn into_report(
    report: &mut RunReport,
    found: Option<Box<Found>>,
    tools: Tools,
    names: &Names,
) {
    let reports = tools::reports(found.map(|found| *found), tools, names);
    if let Some(stop) = &reports.stopped
        && stop.reached
    {
        report.ended = format!("stopped at {}, as stop_at asked", stop.at);
        report.exit_code = None;
    }
    report.host_calls = reports.host_calls;
    report.profile = reports.profile;
    report.stopped = reports.stopped;
}

/// What a run from a shell that `asked` asks tools for is given, for `ready` in `folder`: the
/// tools, and, for a profile or a stop, the module that runs, compiled here. A profile takes the
/// engine whose clock moves, and a stop takes the copy with the stop written in, so neither is
/// the compiled form a run loads. A trace alone changes nothing of what runs.
///
/// # Errors
///
/// When a tool isn't for this program; or, in a process that only runs what was compiled
/// before, that a profile or a stop is lengua-mcp's to do ([`NotRun::Tool`]).
pub(super) fn for_shell(
    asked: &Asked,
    folder: &ProgramFolder,
    ready: &Runnable,
) -> Result<Option<(Equipment, Option<wasmtime::Module>)>, NotRun> {
    if !asked.any() {
        return Ok(None);
    }
    if !asked.profile && asked.stop_at.is_none() {
        if asked.show_memory.is_some() {
            return Err(NotRun::Refused(
                "--show-memory shows memory at a stop: give --stop-at too".to_owned(),
            ));
        }
        let names = Names {
            splits: ready.network.reach.split.is_some(),
            ..Names::default()
        };
        let equipment = Equipment {
            tools: tools_of(asked, !ready.secrets.is_empty(), None),
            setup: Setup::Shell,
            names,
        };
        return Ok(Some((equipment, None)));
    }
    if !engine::compiles() {
        return Err(NotRun::Tool(
            "the run asks for a profile or a stop".to_owned(),
        ));
    }
    let built = built_as_tested(folder, ready)?;
    let equipped = equipped(asked, ready, &built)?;
    let setup = if asked.profile {
        Setup::Run
    } else {
        Setup::Shell
    };
    let wasm = equipped
        .instead
        .as_ref()
        .map_or(&built.wasm, |copy| &copy.wasm);
    let compiler = engine::engine(setup)?;
    let module = engine::compile(&compiler, wasm)
        .map_err(|error| format!("the engine couldn't compile the module: {error:#}"))?;
    let equipment = Equipment {
        tools: equipped.tools,
        setup,
        names: equipped.names,
    };
    Ok(Some((equipment, Some(module))))
}
