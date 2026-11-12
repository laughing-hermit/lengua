//! The modules a program calls, for an idea with a `- Calls:` line ([`crate::call`]): an outside
//! module, which a compiler made from another language, or another lengua program's module.
//!
//! A called module is an instance of its own in the program's store, started before the
//! program is, so with a memory, globals and a table of its own: the program's memory is not
//! its memory, and nothing links the two but the functions the idea's line names. Those are
//! linked to the program's imports as they are, WebAssembly to WebAssembly: numbers pass in a
//! call, and nothing else does. Bytes pass by copy, through `give` and `take` here, which the
//! program calls and the called module can't.
//!
//! A called module gets no function of lengua's host. Each function it imports, as a toolchain's
//! startup or a lengua program's own `_start` does, is defined as one that stops the run
//! ([`super::linker_refusing`]), so nothing the program's idea grants is the called module's:
//! it reads no file, opens no connection and prints nothing, whatever it was built to do.
//!
//! What bounds it is what bounds the program, in the same store: the run's deadline, the
//! engine's stack, which the calls of both share, and for its memory the maximum its module
//! declares, which `check` holds to 1 GiB ([`crate::wasm::outside::read_called`]).

use wasmtime::{AsContext, Caller, Extern, Func, Linker, Memory, Module, Store, ValType};

use super::{Host, Stop};
use crate::call::{Called, Signature, Type};
use crate::wasm::wasi::{Function, Unasked, errno};

/// The bytes one copy moves at a time: a copy goes through a buffer of lengua's, since no two
/// memories of a store are borrowed at once.
const CHUNK: usize = 64 * 1024;

/// A module the program calls, compiled in the run's engine, as a run is given it.
#[derive(Debug, Clone)]
pub struct Loaded {
    /// Its name, as the idea's `- Calls:` line and main.wat's imports name it.
    pub name: String,
    /// The module, compiled.
    pub module: Module,
}

/// A called module's instance in a run's store.
#[derive(Debug, Clone)]
pub struct Instance {
    /// Its name.
    pub name: String,
    /// Its memory, when it exports one as `memory`.
    pub memory: Option<Memory>,
    /// The functions the idea's line names, each with its name.
    pub functions: Vec<(String, Func)>,
}

/// The type of a number that crosses between modules, as the engine's.
const fn val_type(ty: Type) -> ValType {
    match ty {
        Type::I32 => ValType::I32,
        Type::I64 => ValType::I64,
        Type::F32 => ValType::F32,
        Type::F64 => ValType::F64,
    }
}

/// Whether `found`, a function a module exports, has the type `signature` gives it.
fn typed(found: &wasmtime::FuncType, signature: &Signature) -> bool {
    let same = |found: ValType, wanted: Type| {
        matches!(
            (found, val_type(wanted)),
            (ValType::I32, ValType::I32)
                | (ValType::I64, ValType::I64)
                | (ValType::F32, ValType::F32)
                | (ValType::F64, ValType::F64)
        )
    };
    let params: Vec<ValType> = found.params().collect();
    let results: Vec<ValType> = found.results().collect();
    params.len() == signature.params.len()
        && params
            .into_iter()
            .zip(&signature.params)
            .all(|(found, wanted)| same(found, *wanted))
        && match (results.as_slice(), signature.result) {
            ([], None) => true,
            ([found], Some(wanted)) => same(found.clone(), wanted),
            _ => false,
        }
}

/// Why a run can't start a module its idea calls.
fn unstarted(called: &Called, why: impl std::fmt::Display) -> wasmtime::Error {
    wasmtime::Error::msg(format!(
        "lengua couldn't start `{}`, a module the program calls: {why}",
        called.name
    ))
}

/// Starts each module the host's idea calls, of those the host was given ([`Host::calls`]), as
/// an instance of its own in `store`, before the program itself is started: with no function
/// of the host, and with its `_initialize` run when it exports one, as a module built as a
/// library does. Then gives `linker`, the program's, each function the idea's lines name, under
/// the called module's name, so that the program's imports link to them. The limits of the run
/// are set on the store before, so a called module that never ends is stopped as the program
/// is.
///
/// # Errors
///
/// When a module the idea calls wasn't given to the run, doesn't start, or doesn't export a
/// function the idea names with the type the idea gives it; and what stopped a called module's
/// own start.
pub fn start(store: &mut Store<Host>, linker: &mut Linker<Host>) -> wasmtime::Result<()> {
    let policy = store.data().policy;
    let declared: &'static [Called] = &policy.reach.get().calls;
    if declared.is_empty() || policy.piece {
        return Ok(());
    }
    let loaded = std::mem::take(&mut store.data_mut().calls);
    let mut instances = Vec::with_capacity(declared.len());
    for called in declared {
        let Some(found) = loaded.iter().find(|loaded| loaded.name == called.name) else {
            return Err(unstarted(called, "the run wasn't given it"));
        };
        let refusing = super::linker_refusing(&found.module).map_err(wasmtime::Error::msg)?;
        let instance = refusing.instantiate(&mut *store, &found.module)?;
        if let Some(initialize) = instance.get_func(&mut *store, "_initialize")
            && let Ok(initialize) = initialize.typed::<(), ()>(&*store)
        {
            initialize.call(&mut *store, ())?;
        }
        let mut functions = Vec::with_capacity(called.functions.len());
        for signature in &called.functions {
            let Some(function) = instance.get_func(&mut *store, &signature.name) else {
                return Err(unstarted(
                    called,
                    format!("it exports no function `{}`", signature.name),
                ));
            };
            if !typed(&function.ty(&*store), signature) {
                return Err(unstarted(
                    called,
                    format!(
                        "its function `{}` doesn't have the type the idea gives it, `{}`",
                        signature.name,
                        signature.shown()
                    ),
                ));
            }
            linker.define(&*store, &called.name, &signature.name, function)?;
            functions.push((signature.name.clone(), function));
        }
        instances.push(Instance {
            name: called.name.clone(),
            memory: instance.get_memory(&mut *store, "memory"),
            functions,
        });
    }
    store.data_mut().called = instances;
    Ok(())
}

/// Which way a copy goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Way {
    /// From the program's memory to the called module's.
    Give,
    /// From the called module's memory to the program's.
    Take,
}

/// Whether the `len` bytes at `at` lie inside a memory of `size` bytes.
fn inside(size: usize, at: u32, len: u32) -> Option<(usize, usize)> {
    let start = usize::try_from(at).ok()?;
    let len = usize::try_from(len).ok()?;
    let end = start.checked_add(len)?;
    (end <= size).then_some((start, len))
}

/// `give(module, at, from, len)` and `take(module, at, into, len)`: copies `len` bytes between
/// `own`, an address in the program's memory, and `at`, an address in the memory of the called
/// module `module`, which counts the idea's `- Calls:` lines from 0; `way` says which way.
/// Each side is checked against its own memory before a byte moves.
///
/// # Errors
///
/// A stop of the run for a buffer outside the program's own memory, as for every call of the
/// host, and past the run's deadline. For the program: `NOTCAPABLE` when its idea calls no
/// module, and `INVAL` when it calls no such module, the module has no memory, or the bytes at
/// `at` aren't inside it.
pub fn copy(
    caller: &mut Caller<'_, Host>,
    function: &'static Function,
    way: Way,
    module: u32,
    at: u32,
    own: u32,
    len: u32,
) -> wasmtime::Result<i32> {
    let Some(Extern::Memory(mine)) = caller.get_export("memory") else {
        return Err(wasmtime::Error::new(Stop::NoMemory));
    };
    let host = caller.data();
    if !host.policy.allows(function) && function.unasked == Unasked::Refused {
        return Ok(i32::from(errno::NOTCAPABLE));
    }
    if let Err(stop) = host.limits.check() {
        return Err(wasmtime::Error::new(stop));
    }
    let theirs = usize::try_from(module)
        .ok()
        .and_then(|index| host.called.get(index))
        .and_then(|called| called.memory);
    let Some((mine_at, length)) = inside(mine.data_size(caller.as_context()), own, len) else {
        return Err(wasmtime::Error::new(Stop::OutsideMemory));
    };
    let Some(theirs) = theirs else {
        return Ok(i32::from(errno::INVAL));
    };
    let Some((theirs_at, _)) = inside(theirs.data_size(caller.as_context()), at, len) else {
        return Ok(i32::from(errno::INVAL));
    };
    let (from, from_at, to, to_at) = match way {
        Way::Give => (mine, mine_at, theirs, theirs_at),
        Way::Take => (theirs, theirs_at, mine, mine_at),
    };
    let mut buffer = vec![0_u8; length.min(CHUNK)];
    let mut moved = 0;
    while moved < length {
        let step = (length - moved).min(CHUNK);
        let chunk = &mut buffer[..step];
        // Both ranges were checked against their memories above, and nothing runs in between.
        if from.read(&*caller, from_at + moved, chunk).is_err()
            || to.write(&mut *caller, to_at + moved, chunk).is_err()
        {
            return Err(wasmtime::Error::new(Stop::OutsideMemory));
        }
        moved += step;
    }
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A copy's bytes lie inside a memory by their start and their length, with no wrap.
    #[test]
    fn a_range_lies_inside_a_memory() {
        assert_eq!(inside(65_536, 0, 65_536), Some((0, 65_536)));
        assert_eq!(inside(65_536, 65_536, 0), Some((65_536, 0)));
        assert_eq!(inside(65_536, 1, 65_536), None);
        assert_eq!(inside(65_536, u32::MAX, 2), None);
        assert_eq!(inside(0, 0, 1), None);
    }
}
