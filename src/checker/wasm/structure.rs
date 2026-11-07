//! The module's structure: the features the engine has; what main.wat imports (only the WASI
//! preview 1 functions its idea allows, with their types, and only what lengua's library needs
//! that the idea allows too) and exports (only `_start` and `memory`); no start function, tables
//! or element segments; one memory sized once; its regions by the convention (`$x`, an immutable
//! i32 global, with `$x.size` or `$x.rosize`), from 1024 up, apart and inside the memory; data
//! only inside read-only regions; no `memory.grow`; and none of the names lengua keeps.

use std::collections::{BTreeMap, BTreeSet};

use wasmparser::{ExternalKind, Operator, ValType};

use super::model::{FIRST_ADDRESS, Function, Kind, Module, Region};
use crate::checker::{Rule, Violation};
use crate::edge::Allowance;
use crate::wasm::Policy;
use crate::wasm::text::{Built, LIBRARY_FUNCTIONS, RESERVED};
use crate::wasm::wasi;

/// What an idea needs to say for a program to call `host`, in words.
#[must_use]
pub fn needs(host: &wasi::Function) -> String {
    host.allowances
        .iter()
        .filter_map(|allowance| match allowance {
            Allowance::Always => None,
            Allowance::Use(found) => Some(format!("`{}` in its `- Uses:` line", found.name())),
            Allowance::Network => Some("a `- Network:` line".to_owned()),
            Allowance::Listens => Some("`- Network: listens on TCP port N`".to_owned()),
            Allowance::AtOnce => {
                Some("`up to N clients at once` in its `- Network:` line".to_owned())
            }
            Allowance::Threads => Some("a `- Threads:` line".to_owned()),
        })
        .collect::<Vec<_>>()
        .join(" or ")
}

/// A WASI value's type as WebAssembly's.
fn value_type(value: wasi::Value) -> ValType {
    match value {
        wasi::Value::I32 => ValType::I32,
        wasi::Value::I64 => ValType::I64,
    }
}

/// How a report shows a list of types.
fn types(list: &[ValType]) -> String {
    let names: Vec<&str> = list
        .iter()
        .map(|ty| match ty {
            ValType::I32 => "i32",
            ValType::I64 => "i64",
            ValType::F32 => "f32",
            ValType::F64 => "f64",
            ValType::V128 => "v128",
            ValType::Ref(_) => "a reference",
        })
        .collect();
    format!("({})", names.join(" "))
}

/// The checks of this module, with what they report to.
struct Checker<'m, 'a, 'v> {
    /// The module.
    module: &'m Module<'a>,
    /// Where violations go.
    violations: &'v mut Vec<Violation>,
}

impl Checker<'_, '_, '_> {
    /// Reports a violation of `rule` on `line`.
    fn violation(
        &mut self,
        rule: Rule,
        line: Option<usize>,
        problem: impl Into<String>,
        fix: impl Into<String>,
    ) {
        self.violations.push(Violation {
            rule,
            line,
            problem: problem.into(),
            fix: fix.into(),
        });
    }

    /// Rule 10: the module is valid with exactly the engine's features.
    fn validated(&mut self, built: &Built) {
        if let Err(error) = crate::wasm::engine::validate(&built.wasm) {
            self.violation(
                Rule::Tools,
                built.line_at(error.offset()),
                format!("the validator refuses the module: {}", error.message()),
                "use only the dialect's WebAssembly: 2.0 with 128-bit SIMD, without threads, \
                 relaxed SIMD, wide arithmetic, 64-bit or several memories, tail calls, \
                 exceptions or GC",
            );
        }
    }

    /// The dialect: no annotations, and no custom sections but the names lengua writes.
    fn annotations(&mut self) {
        let module = self.module;
        for line in &module.notes.annotations {
            self.violation(
                Rule::Dialect,
                Some(*line),
                "this annotation, `(@…`, changes what the module's names or sections say",
                "remove it: a module's names are its `$` names, and lengua writes its sections",
            );
        }
        for name in module.customs.iter().filter(|name| *name != "name") {
            self.violation(
                Rule::Dialect,
                None,
                format!("the module has a custom section `{name}`"),
                "remove the annotation that makes it",
            );
        }
    }

    /// Rule 3: main.wat imports only WASI preview 1's functions, each with its type, and only
    /// those the idea allows.
    fn imports(&mut self, policy: &Policy) {
        let module = self.module;
        for (from, field, what, line) in &module.other_imports {
            self.violation(
                Rule::Resources,
                *line,
                format!(
                    "main.wat imports {what}, `{from}` `{field}`; a program imports only functions"
                ),
                "declare the memory and the globals in main.wat, and import only the WASI \
                 functions the idea allows",
            );
        }
        for function in module
            .functions
            .iter()
            .filter(|function| function.kind == Kind::Import)
        {
            let Some((from, field)) = &function.import else {
                continue;
            };
            let line = function.line;
            if from != wasi::MODULE {
                self.violation(
                    Rule::Resources,
                    line,
                    format!(
                        "{} is imported from `{from}`; a program imports only from `{}`",
                        function.shown(),
                        wasi::MODULE
                    ),
                    "import only the WASI preview 1 functions the idea allows",
                );
                continue;
            }
            let Some(host) = wasi::function(field) else {
                self.violation(
                    Rule::Resources,
                    line,
                    format!("`{}` has no function `{field}`", wasi::MODULE),
                    "import only the functions lengua's host gives: references/host.md lists \
                     them",
                );
                continue;
            };
            if !host.given {
                self.violation(
                    Rule::Resources,
                    line,
                    format!("lengua's host doesn't give `{field}`: it answers NOSYS"),
                    "use only the functions references/host.md lists",
                );
                continue;
            }
            if !policy.allows(host) {
                let needs = needs(host);
                self.violation(
                    Rule::Resources,
                    line,
                    format!(
                        "`{field}` isn't one the idea allows: it needs {needs}, which idea.md \
                         doesn't name"
                    ),
                    format!(
                        "use only what idea.md's Constraints allow, or, when the program needs \
                         it, add {needs} to idea.md and tell the user"
                    ),
                );
            }
            let params: Vec<ValType> = host.params.iter().copied().map(value_type).collect();
            let results: Vec<ValType> = host.results.iter().copied().map(value_type).collect();
            if function.params != params || function.results != results {
                self.violation(
                    Rule::Resources,
                    line,
                    format!(
                        "`{field}` takes {} and gives {}, but main.wat imports it as taking {} and \
                         giving {}",
                        types(&params),
                        types(&results),
                        types(&function.params),
                        types(&function.results)
                    ),
                    format!(
                        "import it with its own type: `(param {}) (result {})`",
                        types(&params).trim_matches(|c| c == '(' || c == ')'),
                        types(&results).trim_matches(|c| c == '(' || c == ')')
                    ),
                );
            }
        }
    }

    /// Rule 3: what lengua's library calls for a program, the idea allows: a call to `$fill`
    /// reads, as `fd_read` does.
    fn library(&mut self, policy: &Policy) {
        let module = self.module;
        let calls_of = |function: &Function<'_>| -> BTreeSet<u32> {
            function
                .body
                .iter()
                .filter_map(|op| match op.operator {
                    Operator::Call { function_index } => Some(function_index),
                    _ => None,
                })
                .collect()
        };
        for import in module
            .functions
            .iter()
            .filter(|function| function.kind == Kind::LibraryImport)
        {
            let Some(host) = import.host() else {
                continue;
            };
            if policy.allows(host) {
                continue;
            }
            // The library's functions that reach it, themselves or through another.
            let mut reaching: BTreeSet<u32> = BTreeSet::new();
            loop {
                let before = reaching.len();
                for function in module
                    .functions
                    .iter()
                    .filter(|function| function.kind == Kind::Library)
                {
                    let calls = calls_of(function);
                    if calls.contains(&import.index)
                        || calls.iter().any(|callee| reaching.contains(callee))
                    {
                        reaching.insert(function.index);
                    }
                }
                if reaching.len() == before {
                    break;
                }
            }
            let needs = needs(host);
            for function in module.own() {
                for op in &function.body {
                    let Operator::Call { function_index } = op.operator else {
                        continue;
                    };
                    if !reaching.contains(&function_index) {
                        continue;
                    }
                    let callee = module
                        .function(function_index)
                        .map_or_else(String::new, Function::shown);
                    self.violation(
                        Rule::Resources,
                        op.line,
                        format!(
                            "{callee} calls `{}`, which the idea doesn't allow: it needs {needs}",
                            host.name
                        ),
                        format!(
                            "use only what idea.md's Constraints allow, or, when the program \
                             needs it, add {needs} to idea.md and tell the user"
                        ),
                    );
                }
            }
        }
    }

    /// Rule 6: main.wat exports exactly the function `_start`, which takes and gives nothing, and
    /// the memory `memory`.
    fn exports(&mut self) {
        let module = self.module;
        let mut start = false;
        let mut memory = false;
        for export in &module.exports {
            match (export.name.as_str(), export.kind) {
                ("_start", ExternalKind::Func) => {
                    start = true;
                    let Some(function) = module.function(export.index) else {
                        continue;
                    };
                    if function.kind != Kind::Own {
                        self.violation(
                            Rule::Scope,
                            export.line,
                            "`_start` isn't one of main.wat's own functions",
                            "export the function the program starts in, defined in main.wat",
                        );
                    } else if !function.params.is_empty() || !function.results.is_empty() {
                        self.violation(
                            Rule::Scope,
                            function.line,
                            "`_start` takes or gives values, which nothing passes or reads",
                            "make `_start` take nothing and give nothing",
                        );
                    }
                }
                ("memory", ExternalKind::Memory) => memory = true,
                (name, _) => self.violation(
                    Rule::Scope,
                    export.line,
                    format!(
                        "main.wat exports `{name}`; a program exports only the function \
                         `_start` and the memory `memory`"
                    ),
                    "remove the export",
                ),
            }
        }
        if !start {
            self.violation(
                Rule::Scope,
                None,
                "main.wat exports no function `_start`, where the program starts",
                "export the function the program starts in: `(func $main (export \"_start\") …)`",
            );
        }
        if !memory {
            self.violation(
                Rule::Scope,
                None,
                "main.wat exports no memory `memory`, which the host reads and writes",
                "declare and export it: `(memory (export \"memory\") 1 1)`",
            );
        }
    }

    /// Rule 9: no start function, no tables and no element segments, so no indirect call.
    fn indirection(&mut self) {
        let module = self.module;
        if let Some((_, line)) = module.start {
            self.violation(
                Rule::Indirection,
                line,
                "main.wat has a start function, which runs before `_start`",
                "start the program in `_start` alone",
            );
        }
        for line in &module.tables {
            self.violation(
                Rule::Indirection,
                *line,
                "main.wat has a table, through which calls go indirectly",
                "call functions directly; a program has no tables",
            );
        }
        for line in &module.elements {
            self.violation(
                Rule::Indirection,
                *line,
                "main.wat has an element segment, which fills a table",
                "remove it: a program has no tables",
            );
        }
    }

    /// Rule 3: one memory, declared once with its minimum equal to its maximum; no `memory.grow`.
    fn memory(&mut self) {
        let module = self.module;
        let defined: Vec<_> = module
            .memories
            .iter()
            .filter(|memory| !memory.imported)
            .collect();
        if module.memories.is_empty() {
            self.violation(
                Rule::Resources,
                None,
                "main.wat declares no memory",
                "declare one, sized from the idea's limits: `(memory (export \"memory\") 1 1)`",
            );
        }
        for memory in &defined {
            if memory.ty.maximum != Some(memory.ty.initial) {
                let maximum = memory
                    .ty
                    .maximum
                    .map_or_else(|| "none".to_owned(), |maximum| maximum.to_string());
                self.violation(
                    Rule::Resources,
                    memory.line,
                    format!(
                        "the memory's minimum is {} pages and its maximum {maximum}: they must be \
                         equal, so its size is fixed",
                        memory.ty.initial
                    ),
                    format!(
                        "declare it with both sizes equal, such as `(memory (export \"memory\") \
                         {0} {0})`",
                        memory.ty.initial.max(1)
                    ),
                );
            }
        }
        for memory in defined.iter().skip(1) {
            self.violation(
                Rule::Resources,
                memory.line,
                "main.wat declares a second memory",
                "keep everything in one memory, cut into regions",
            );
        }
        for function in module.own() {
            for op in &function.body {
                match op.operator {
                    Operator::MemoryGrow { .. } => self.violation(
                        Rule::Resources,
                        op.line,
                        "`memory.grow` changes the memory's size while the program runs",
                        "size the memory once, from the idea's limits",
                    ),
                    Operator::MemoryInit { .. } | Operator::DataDrop { .. } => self.violation(
                        Rule::Dialect,
                        op.line,
                        "`memory.init` and `data.drop` use data segments, which only lengua's \
                         library does",
                        "place the program's data with `data` segments in read-only regions",
                    ),
                    _ => {}
                }
            }
        }
    }

    /// M1: the regions follow the convention, start at 1024 or above, fit in the memory and
    /// don't overlap.
    fn regions(&mut self) {
        let module = self.module;
        let by_name: BTreeMap<&str, _> = module
            .globals
            .iter()
            .filter_map(|global| Some((global.name.as_deref()?, global)))
            .collect();
        for global in &module.globals {
            let Some(name) = global.name.as_deref() else {
                if !global.imported {
                    self.violation(
                        Rule::Constants,
                        global.line,
                        "this global has no name",
                        "name it, such as `(global $MAX_LINE i32 (i32.const 80))`",
                    );
                }
                continue;
            };
            let (base, other) = match (name.strip_suffix(".size"), name.strip_suffix(".rosize")) {
                (Some(base), _) => (base, format!("{base}.rosize")),
                (None, Some(base)) => (base, format!("{base}.size")),
                (None, None) => continue,
            };
            let constant =
                !global.mutable && global.ty == ValType::I32 && global.unsigned().is_some();
            if !constant {
                self.violation(
                    Rule::Owners,
                    global.line,
                    format!("${name} is a region's size, so it must be an immutable i32 constant"),
                    format!("declare it as `(global ${name} i32 (i32.const <bytes>))`"),
                );
            } else if global.unsigned() == Some(0) {
                self.violation(
                    Rule::Owners,
                    global.line,
                    format!("${name} is 0: a region holds at least one byte"),
                    "give the region the bytes the idea's limits need",
                );
            }
            match by_name.get(base) {
                None => self.violation(
                    Rule::Owners,
                    global.line,
                    format!("${name} sizes ${base}, which isn't a global"),
                    format!(
                        "declare the region's address as `(global ${base} i32 (i32.const \
                         <address>))`, or rename the size"
                    ),
                ),
                Some(address)
                    if address.mutable
                        || address.ty != ValType::I32
                        || address.unsigned().is_none() =>
                {
                    self.violation(
                        Rule::Owners,
                        address.line,
                        format!(
                            "${base} is a region's address, so it must be an immutable i32 \
                             constant"
                        ),
                        format!("declare it as `(global ${base} i32 (i32.const <address>))`"),
                    );
                }
                Some(_) => {}
            }
            if name.ends_with(".size") && by_name.contains_key(other.as_str()) {
                self.violation(
                    Rule::Owners,
                    global.line,
                    format!("${base} has both a size, `${name}`, and a read-only size, `${other}`"),
                    "keep one: `.size` for a writable region, `.rosize` for a read-only one",
                );
            }
        }
        let bytes = module
            .memories
            .iter()
            .find(|memory| !memory.imported)
            .map(super::model::Memory::bytes);
        for region in &module.regions {
            let name = &region.name;
            if region.start < FIRST_ADDRESS {
                self.violation(
                    Rule::Owners,
                    region.line,
                    format!(
                        "${name} starts at {}, inside lengua's addresses 0 to {}",
                        region.start,
                        FIRST_ADDRESS - 1
                    ),
                    format!("start every region at {FIRST_ADDRESS} or above"),
                );
            }
            if let Some(bytes) = bytes
                && region.end() > bytes
            {
                self.violation(
                    Rule::Owners,
                    region.line,
                    format!(
                        "${name} ends at {}, past the memory's {bytes} bytes",
                        region.end()
                    ),
                    "make the memory bigger, or the region smaller",
                );
            }
        }
        let mut sorted: Vec<&Region> = module.regions.iter().collect();
        sorted.sort_by_key(|region| (region.start, region.end()));
        let mut furthest: Option<&Region> = None;
        for region in sorted {
            if let Some(before) = furthest
                && region.start < before.end()
            {
                self.violation(
                    Rule::Owners,
                    region.line,
                    format!(
                        "${} starts at {}, inside ${}, which runs to {}",
                        region.name,
                        region.start,
                        before.name,
                        before.end()
                    ),
                    "place the regions apart: each starts at or after the end of the one before",
                );
            }
            if furthest.is_none_or(|before| region.end() > before.end()) {
                furthest = Some(region);
            }
        }
    }

    /// M6: main.wat's data is written only into read-only regions, when the program starts.
    fn data(&mut self) {
        let module = self.module;
        for data in module.data.iter().filter(|data| data.own) {
            let problem = match data.at {
                None => Some(
                    "this data segment is passive, and only lengua's library copies data while \
                     the program runs"
                        .to_owned(),
                ),
                Some((_, None)) => Some("this data segment's address isn't a constant".to_owned()),
                Some((_, Some(start))) => {
                    let end = start.saturating_add(data.len);
                    let inside = module.regions.iter().any(|region| {
                        region.read_only && start >= region.start && end <= region.end()
                    });
                    (!inside).then(|| {
                        format!(
                            "this data segment, bytes {start} to {}, isn't inside a read-only \
                             region",
                            end.saturating_sub(1).max(start)
                        )
                    })
                }
            };
            if let Some(problem) = problem {
                self.violation(
                    Rule::Immutability,
                    data.line,
                    problem,
                    "place data with `(data (i32.const <address>) …)` at the address of a \
                     region whose size is `$x.rosize`",
                );
            }
        }
    }

    /// The names lengua keeps: its library's functions, which main.wat calls and never defines,
    /// and every name that starts with `$lengua.`.
    fn reserved(&mut self) {
        let module = self.module;
        for function in &module.functions {
            if !matches!(function.kind, Kind::Own | Kind::Import) {
                continue;
            }
            let Some(name) = function.name.as_deref() else {
                continue;
            };
            if LIBRARY_FUNCTIONS.contains(&name) || name.starts_with(RESERVED) {
                self.violation(
                    Rule::Dialect,
                    function.line,
                    format!("main.wat defines ${name}, a name lengua keeps for its library"),
                    format!(
                        "call lengua's own ${name} rather than defining one, and give your \
                         functions other names"
                    ),
                );
            }
        }
    }
}

/// Checks the module's structure.
pub fn check(module: &Module<'_>, built: &Built, policy: &Policy, violations: &mut Vec<Violation>) {
    let mut checker = Checker { module, violations };
    checker.validated(built);
    checker.annotations();
    checker.imports(policy);
    checker.library(policy);
    checker.exports();
    checker.indirection();
    checker.memory();
    checker.regions();
    checker.data();
    checker.reserved();
}
