//! A traced run's host: each function of the host defined so that it writes its call into the
//! run's trace ([`crate::wasm::tools::Trace`]) and then answers as it always does. Only a run
//! that asks for a trace is given such a linker; every other run's functions are defined as they
//! were, and pay nothing.
//!
//! A call is traced by what crosses the host's edge: the function's name, the numbers the
//! program passed, and what the host gave back. Nothing is read from the program's memory. The
//! pieces of a program's split work run in the same linker, at once; their calls are not traced,
//! so a trace is one program's calls in the order it made them.

use std::sync::{Arc, Mutex};

use wasmtime::{Caller, Linker, WasmRet, WasmTy};

use super::{Host, Stop};
use crate::wasm::tools::{Gave, Trace};
use crate::wasm::wasi;

/// A run's trace, which its host's functions write.
pub type Shared = Arc<Mutex<Trace>>;

/// A number the program passes to the host.
pub trait Passed: Copy {
    /// The number, as a trace writes it: an i32 without a sign, as an address or a length is.
    fn passed(self) -> i64;
}

impl Passed for i32 {
    fn passed(self) -> i64 {
        i64::from(self.cast_unsigned())
    }
}

impl Passed for i64 {
    fn passed(self) -> i64 {
        self
    }
}

/// What a function of the host gives back.
pub trait Answered {
    /// What a function with this answer gives where it is defined for a module the program
    /// calls: a stop of the run, in a type the engine links as it links this one.
    type Refused: WasmRet;

    /// What it came to, as a trace says it.
    fn answered(&self) -> Gave;

    /// The stop of the run that a called module gets for a call of the host.
    fn refused() -> Self::Refused;
}

/// What a call that stopped the program with `error` came to.
fn stopped(error: &wasmtime::Error) -> Gave {
    match error.downcast_ref::<Stop>() {
        Some(Stop::Exit(code)) => Gave::Exit(*code),
        Some(stop) => Gave::Stopped(*stop),
        None => Gave::Failed,
    }
}

impl Answered for i32 {
    type Refused = wasmtime::Result<i32>;

    fn refused() -> Self::Refused {
        Err(wasmtime::Error::new(Stop::Reaches))
    }

    fn answered(&self) -> Gave {
        Gave::Code(*self)
    }
}

impl Answered for wasmtime::Result<i32> {
    type Refused = wasmtime::Result<i32>;

    fn refused() -> Self::Refused {
        Err(wasmtime::Error::new(Stop::Reaches))
    }

    fn answered(&self) -> Gave {
        match self {
            Ok(code) => Gave::Code(*code),
            Err(error) => stopped(error),
        }
    }
}

impl Answered for wasmtime::Result<()> {
    type Refused = wasmtime::Result<()>;

    fn refused() -> Self::Refused {
        Err(wasmtime::Error::new(Stop::Reaches))
    }

    fn answered(&self) -> Gave {
        match self {
            Ok(()) => Gave::Nothing,
            Err(error) => stopped(error),
        }
    }
}

/// How a trace names the function `name` of `module`: WASI's by their names alone.
fn call_name(module: &str, name: &str) -> String {
    if module == wasi::MODULE {
        name.to_owned()
    } else {
        format!("{module}.{name}")
    }
}

/// Notes in `trace` that the program calls `call` with `arguments`.
fn begin(trace: &Shared, call: &str, arguments: &[i64]) -> Option<usize> {
    trace
        .lock()
        .ok()
        .and_then(|mut trace| trace.begin(call, arguments))
}

/// Notes in `trace` what the call at `at` came to.
fn end(trace: &Shared, at: Option<usize>, gave: Gave) {
    if let Ok(mut trace) = trace.lock() {
        trace.end(at, gave);
    }
}

/// The marker of a host function that takes its caller first.
#[derive(Debug)]
pub struct Called;

/// The marker of a host function that takes only what the program passes.
#[derive(Debug)]
pub struct Alone;

/// A function of the host that a traced run can define: `Marker` only tells one shape of
/// function from another.
pub trait Traced<Marker>: Sized {
    /// Defines the function as `name` of `module` on `linker`, so that each call of it by the
    /// program itself goes into `trace` first and its answer after.
    ///
    /// # Errors
    ///
    /// When the linker doesn't take it.
    fn define(
        self,
        linker: &mut Linker<Host>,
        module: &str,
        name: &str,
        trace: &Shared,
    ) -> wasmtime::Result<()>;

    /// Defines the function as `name` of `module` on `linker` for a module the program calls:
    /// with its type, and so that a call of it stops the run. The function itself is never
    /// called.
    ///
    /// # Errors
    ///
    /// When the linker doesn't take it.
    fn refuse(self, linker: &mut Linker<Host>, module: &str, name: &str) -> wasmtime::Result<()>;
}

/// Implements [`Traced`] for the host functions of one count of parameters, with their caller
/// and without.
macro_rules! traced {
    ($($argument:ident),*) => {
        impl<F, R, $($argument),*> Traced<(Called, R, $($argument,)*)> for F
        where
            F: for<'a> Fn(Caller<'a, Host>, $($argument),*) -> R + Send + Sync + 'static,
            R: WasmRet + Answered,
            $($argument: WasmTy + Passed,)*
        {
            #[allow(non_snake_case)]
            fn define(
                self,
                linker: &mut Linker<Host>,
                module: &str,
                name: &str,
                trace: &Shared,
            ) -> wasmtime::Result<()> {
                let trace = Arc::clone(trace);
                let call = call_name(module, name);
                let function = self;
                linker.func_wrap(
                    module,
                    name,
                    move |caller: Caller<'_, Host>, $($argument: $argument),*| -> R {
                        if caller.data().policy.piece {
                            return function(caller, $($argument),*);
                        }
                        let arguments: &[i64] = &[$($argument.passed()),*];
                        let at = begin(&trace, &call, arguments);
                        let gave = function(caller, $($argument),*);
                        end(&trace, at, gave.answered());
                        gave
                    },
                )?;
                Ok(())
            }

            #[allow(non_snake_case)]
            fn refuse(
                self,
                linker: &mut Linker<Host>,
                module: &str,
                name: &str,
            ) -> wasmtime::Result<()> {
                linker.func_wrap(
                    module,
                    name,
                    move |_caller: Caller<'_, Host>, $($argument: $argument),*| -> R::Refused {
                        $(let _ = $argument;)*
                        R::refused()
                    },
                )?;
                Ok(())
            }
        }

        impl<F, R, $($argument),*> Traced<(Alone, R, $($argument,)*)> for F
        where
            F: Fn($($argument),*) -> R + Send + Sync + 'static,
            R: WasmRet + Answered,
            $($argument: WasmTy + Passed,)*
        {
            #[allow(non_snake_case)]
            fn define(
                self,
                linker: &mut Linker<Host>,
                module: &str,
                name: &str,
                trace: &Shared,
            ) -> wasmtime::Result<()> {
                let trace = Arc::clone(trace);
                let call = call_name(module, name);
                let function = self;
                linker.func_wrap(
                    module,
                    name,
                    move |caller: Caller<'_, Host>, $($argument: $argument),*| -> R {
                        if caller.data().policy.piece {
                            return function($($argument),*);
                        }
                        let arguments: &[i64] = &[$($argument.passed()),*];
                        let at = begin(&trace, &call, arguments);
                        let gave = function($($argument),*);
                        end(&trace, at, gave.answered());
                        gave
                    },
                )?;
                Ok(())
            }

            #[allow(non_snake_case)]
            fn refuse(
                self,
                linker: &mut Linker<Host>,
                module: &str,
                name: &str,
            ) -> wasmtime::Result<()> {
                linker.func_wrap(
                    module,
                    name,
                    move |_caller: Caller<'_, Host>, $($argument: $argument),*| -> R::Refused {
                        $(let _ = $argument;)*
                        R::refused()
                    },
                )?;
                Ok(())
            }
        }
    };
}

traced!();
traced!(A1);
traced!(A1, A2);
traced!(A1, A2, A3);
traced!(A1, A2, A3, A4);
traced!(A1, A2, A3, A4, A5);
traced!(A1, A2, A3, A4, A5, A6);
traced!(A1, A2, A3, A4, A5, A6, A7);
traced!(A1, A2, A3, A4, A5, A6, A7, A8);
traced!(A1, A2, A3, A4, A5, A6, A7, A8, A9);
traced!(A1, A2, A3, A4, A5, A6, A7, A8, A9, A10);
