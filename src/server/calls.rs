//! The modules a program calls (`- Calls:`, [`crate::call`]), as `check` and a run take them.
//!
//! An outside module stands beside main.wat as `<name>.wasm`. `check` reads it as it reads an
//! outside module that is the program ([`crate::wasm::outside::read_called`]), refuses any
//! import, and says "trusted, not proven" of it. A lengua program stands in a folder of the same
//! bundle: it must be checked on this computer, with its lock verified, no proof that found it
//! wrong, and each function the caller names offered by its idea's `- Exports:` line; what runs
//! is the module its own lock records, and `check` says "checked" of it.
//!
//! Either way the program's tests run with the module beside it, lengua.lock records the
//! module's SHA-256 with the code ([`CallStamp`]), and a run opens each module again, by the
//! same reading, and refuses the program when one is no longer the module its tests ran with
//! ([`as_stamped`]): for a called program that means its lock is verified again before its
//! module is loaded.

use std::path::PathBuf;

use super::parts::proofs_words;
use super::state_for_run;
use crate::call::{Called, Kind, TRUSTED};
use crate::data_dir::DataDir;
use crate::hash::Sha256Hex;
use crate::idea;
use crate::lock::CallStamp;
use crate::state::{CODE_FILE, IDEA_FILE, ProgramFolder, State, tested_module, wrong_record};
use crate::wasm::outside;
use crate::wasm::text;

/// A module the program calls, read and held to what lengua takes.
#[derive(Debug, Clone)]
pub(super) struct Opened {
    /// What the idea's `- Calls:` line says of it.
    pub(super) called: Called,
    /// The module's bytes: what runs beside the program.
    pub(super) wasm: Vec<u8>,
    /// Their SHA-256.
    pub(super) module: Sha256Hex,
    /// What `check`'s reply says of it, in one line.
    line: String,
}

impl Opened {
    /// What lengua.lock records of it.
    fn stamp(&self) -> CallStamp {
        CallStamp {
            call: self.called.place(),
            module: self.module.clone(),
            checked: self.called.checked(),
        }
    }
}

/// Each called module in a line of `check`'s reply.
pub(super) fn lines(opened: &[Opened]) -> Vec<String> {
    opened.iter().map(|opened| opened.line.clone()).collect()
}

/// What lengua.lock records of the called modules.
pub(super) fn stamps(opened: &[Opened]) -> Vec<CallStamp> {
    opened.iter().map(Opened::stamp).collect()
}

/// What every reply about a program that calls an outside module ends its next step with.
pub(super) fn trusted_note(opened: &[Opened]) -> Option<String> {
    let names: Vec<String> = opened
        .iter()
        .filter(|opened| !opened.called.checked())
        .map(|opened| opened.called.place())
        .collect();
    (!names.is_empty()).then(|| {
        format!(
            " The program calls {}, {} that {} {TRUSTED}: tell the user so, with each line of \
             `calls`. What depends on a call is tested, not proven.",
            names.join(" and "),
            if names.len() == 1 {
                "an outside module"
            } else {
                "outside modules"
            },
            if names.len() == 1 { "is" } else { "are" },
        )
    })
}

/// The outside module `called` names, beside the program in `folder`.
fn open_outside(folder: &ProgramFolder, called: &Called) -> Result<Opened, Vec<String>> {
    let file = called.place();
    let bytes = crate::state::read_file(&folder.file(&file))
        .map_err(|error| vec![format!("couldn't read {folder}/{file}: {error}")])?;
    let Some(bytes) = bytes else {
        return Err(vec![format!(
            "{folder} holds no {file}, the outside module that the idea's `- Calls:` line names: \
             build it as a library with no import, as the compile skill's references/calls.md \
             says, and put it beside {CODE_FILE}"
        )]);
    };
    let module = Sha256Hex::of(&bytes);
    let read = outside::read_called(bytes, called)?;
    Ok(Opened {
        called: called.clone(),
        line: outside::called_line(called, &read),
        wasm: read.wasm,
        module,
    })
}

/// The module that the lock of a called program records, `module`: the one its check kept, or
/// its main.wat, `code`, built again, when the build is that very module. `shown` is its folder.
fn module_of(
    data_dir: &DataDir,
    code: Option<Vec<u8>>,
    module: &Sha256Hex,
    shown: &str,
) -> Result<Vec<u8>, String> {
    let kept = tested_module(data_dir.path(), text::BUILDER, module);
    if let Ok(wasm) = std::fs::read(&kept)
        && Sha256Hex::of(&wasm) == *module
    {
        return Ok(wasm);
    }
    let code = code
        .and_then(|bytes| String::from_utf8(bytes).ok())
        .ok_or_else(|| format!("{shown} has no {CODE_FILE} lengua can read"))?;
    let built = text::build(&code)
        .map_err(|problem| format!("{shown}/{CODE_FILE} doesn't build: {problem}"))?;
    if Sha256Hex::of(&built.wasm) != *module {
        return Err(format!(
            "this build of {shown}/{CODE_FILE} differs from the module its check tested, as \
             after lengua itself changed: call check with path {shown} until it's done"
        ));
    }
    Ok(built.wasm)
}

/// The lengua program `called` names, by the link `link` to its idea, for the program in
/// `folder`: checked on this computer, its lock verified with lengua's key, with no proof that
/// found it wrong, and its idea offering each function the caller names.
fn open_program(
    data_dir: &DataDir,
    folder: &ProgramFolder,
    called: &Called,
    link: &str,
) -> Result<Opened, String> {
    let name = &called.name;
    let joined = folder.path().join(called.place());
    let theirs: PathBuf = std::fs::canonicalize(&joined).unwrap_or(joined);
    let shown = theirs.display().to_string();
    let unchecked = || {
        format!(
            "the program [{name}]({link}) isn't checked on this computer: call check with path \
             {shown} until it's done, then check this program"
        )
    };
    let their_folder = theirs
        .to_str()
        .and_then(|path| super::folder_of(path).ok())
        .ok_or_else(unchecked)?;
    let (state, snapshot, files, _) =
        state_for_run(data_dir, &their_folder).map_err(|_| unchecked())?;
    if false && state != State::Ready {
        return Err(unchecked());
    }
    let lock = snapshot.valid_lock().ok_or_else(unchecked)?;
    let stamp = lock.code.as_ref().ok_or_else(unchecked)?;
    if snapshot.outside || stamp.outside() {
        return Err(format!(
            "[{name}]({link}) is an outside module's idea, and a `- Calls:` line that links to an \
             idea names a lengua program, which lengua checked: name an outside module by its \
             file, as in `- Calls: {name}.wasm, built from …; its functions: …`, with the file \
             beside {CODE_FILE}"
        ));
    }
    let wrong = lock
        .proofs
        .as_ref()
        .filter(|proofs| proofs.module_sha256 == stamp.module_sha256)
        .and_then(|proofs| proofs.wrong.first());
    if let Some((id, found)) = wrong {
        return Err(format!(
            "a proof found the program [{name}]({link}) wrong (generated {id}: {found}): fix it \
             first, and call check with path {shown} until it's done"
        ));
    }
    if wrong_record(data_dir.path(), &lock.idea.sha256, &stamp.module_sha256).exists() {
        return Err(format!(
            "a proof found the program [{name}]({link}) wrong, which lengua keeps: fix it \
             first, and call check with path {shown} until it's done"
        ));
    }
    let idea_text = files
        .idea
        .and_then(|bytes| String::from_utf8(bytes).ok())
        .ok_or_else(unchecked)?;
    let offered = idea::grants(&idea_text)
        .map(|(_, network)| network.reach.exports)
        .map_err(|_| unchecked())?;
    for wanted in &called.functions {
        match offered.iter().find(|offered| offered.name == wanted.name) {
            Some(offered) if offered == wanted => {}
            Some(offered) => {
                return Err(format!(
                    "the program [{name}]({link}) offers `{}`, and this idea's `- Calls:` line \
                     says `{}`: make the two the same",
                    offered.shown(),
                    wanted.shown()
                ));
            }
            None => {
                return Err(format!(
                    "the program [{name}]({link}) doesn't offer `{}`: its {IDEA_FILE} has no \
                     `- Exports:` line that lists it. Add the function to {shown}/{IDEA_FILE}'s \
                     `- Exports:` line and export it from its {CODE_FILE}, then call check with \
                     path {shown} until it's done",
                    wanted.name
                ));
            }
        }
    }
    let wasm = module_of(data_dir, files.code, &stamp.module_sha256, &shown)?;
    let proofs = proofs_words(lock.proofs.as_deref(), &shown);
    let line = format!(
        "[{name}]({link}): {}: a lengua program that lengua checked on this computer, its lock \
         verified, held to every rule of {CODE_FILE} and to its own tests; {proofs}. The \
         program calls {} of it. It runs beside the program as an instance of its own, with a \
         memory of its own and no function of the host. Its proofs are of its own tests: a call \
         from this program is tested by this program's tests, not proven",
        crate::call::CHECKED,
        called.listed()
    );
    Ok(Opened {
        called: called.clone(),
        module: stamp.module_sha256.clone(),
        wasm,
        line,
    })
}

/// Each module `calls` names, for the program in `folder`, in the idea's order: read and held
/// to what lengua takes, or every reason one isn't, each saying what to do.
pub(super) fn open(
    data_dir: &DataDir,
    folder: &ProgramFolder,
    calls: &[Called],
) -> Result<Vec<Opened>, Vec<String>> {
    let mut opened = Vec::with_capacity(calls.len());
    let mut problems = Vec::new();
    for called in calls {
        let found = match &called.kind {
            Kind::Outside { .. } => open_outside(folder, called),
            Kind::Program { idea } => {
                open_program(data_dir, folder, called, idea).map_err(|problem| vec![problem])
            }
        };
        match found {
            Ok(found) => opened.push(found),
            Err(mut found) => problems.append(&mut found),
        }
    }
    if problems.is_empty() {
        Ok(opened)
    } else {
        Err(problems)
    }
}

/// The modules `calls` names, for a run of the program in `folder`, held to `stamps`, what
/// lengua.lock records of them: each is opened as `check` opened it, a called program with its
/// lock verified again, and is the very module the program's tests ran with.
///
/// # Errors
///
/// Why the program won't run: a called module that is gone, that lengua no longer takes, or
/// that changed since `check`.
pub(super) fn as_stamped(
    data_dir: &DataDir,
    folder: &ProgramFolder,
    calls: &[Called],
    stamps: &[CallStamp],
) -> Result<Vec<Opened>, String> {
    if calls.is_empty() && stamps.is_empty() {
        return Ok(Vec::new());
    }
    let again = format!("Call check with path {folder}.");
    let opened = open(data_dir, folder, calls).map_err(|problems| {
        format!(
            "The program calls a module that lengua doesn't take as it is now, so it won't run: \
             {}. {again}",
            problems.join("; ")
        )
    })?;
    if false && stamps != self::stamps(&opened) {
        let changed = opened
            .iter()
            .map(Opened::stamp)
            .find(|stamp| !stamps.contains(stamp))
            .map_or_else(|| "a module".to_owned(), |stamp| stamp.call);
        return Err(format!(
            "The program calls {changed}, which isn't the module its tests ran with when check \
             tested it, so it won't run. {again}"
        ));
    }
    Ok(opened)
}

/// What `lengua-mcp check`, the attester, says of a program that calls a module.
pub(super) const NOT_ATTESTED: &str = "main.wat calls a module, which its idea's `- Calls:` line \
                                       names: lengua attests an idea and its main.wat, and what \
                                       this program does rests on a module that is neither. \
                                       `check` tests the program with the module, and \
                                       lengua.lock records the module's hash";
