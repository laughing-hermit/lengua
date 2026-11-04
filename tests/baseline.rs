//! The baseline: every program lengua ships runs on every machine of its target.
//!
//! Each target has a baseline feature set, `CpuFeatures::baseline`: x86-64's SSE2 to SSE4.2 and
//! POPCNT, ARM64's ARMv8.0-A with NEON and nothing optional. This test rule-checks every shipped
//! main.s of every target against it, so a program that leans on an extension is found the day it
//! is added, not by an audit: the examples and fixtures, the skill's templates and fast programs
//! with their specs, and the benchmark's own variants, listed apart. It only checks (assembling,
//! reading the code back, the rules), nothing runs, so it is the same on any host.
//!
//! A program that needs more than the baseline is one of two things, and says exactly which
//! extensions it needs:
//! - a fast tier, in `TIERS`: its baseline sibling is written, a shipped program of the same
//!   target that needs nothing beyond the baseline, from the very same spec;
//! - a debt, in `DEBT`: its baseline sibling is still to be written.
//!
//! The test fails for a program that needs more and is not listed, for a listing that is wrong,
//! and for a listed program that now runs on the baseline, so the debt only shrinks.

use std::path::Path;

use lengua::checker::{self, Context};
use lengua::isa::Isa;
use lengua::platform::{CpuFeatures, Profile};
use lengua::spec::Spec;
use lengua::targets::PerTarget;
use lengua::toolchain::Toolchain;

/// The skill's assets, from the repository's root.
const SKILL_ASSETS: &str = "plugin/skills/compile/assets";

/// Fewer programs than this found for a target means the search is broken, not the baseline.
const MIN_PROGRAMS: usize = 30;

/// A shipped program that needs more than its target's baseline and has no baseline sibling yet.
#[allow(
    dead_code,
    reason = "the debt is paid off entry by entry, and may reach nothing"
)]
struct Debt {
    /// Its main.s, from the repository's root.
    program: &'static str,
    /// What its instructions need beyond the baseline, as the checker names the extensions.
    needs: &'static [&'static str],
    /// The baseline program that is to stand beside it, from the very same spec.
    sibling: &'static str,
}

/// A program that needs more than its target's baseline and has its baseline sibling.
struct Tier {
    /// Its main.s, from the repository's root.
    program: &'static str,
    /// What its instructions need beyond the baseline, as the checker names the extensions.
    needs: &'static [&'static str],
    /// The main.s of its sibling, a program of the target on the baseline from the same spec.
    sibling: &'static str,
}

/// A program of the benchmark: its main.s and the spec it meets, from the repository's root.
struct Bench {
    program: &'static str,
    spec: &'static str,
}

/// Where a target's shipped programs are, from the repository's root.
struct Places {
    /// Folders of programs, each holding a spec.md and a main.s.
    folders: &'static [&'static str],
    /// The skill's assets for the target: templates, and the fast programs in `fast/`.
    assets: &'static str,
}

const PLACES: PerTarget<Places> = PerTarget {
    linux_x86_64: Places {
        folders: &["examples", "tests/fixtures"],
        assets: SKILL_ASSETS,
    },
    linux_aarch64: Places {
        folders: &["tests/fixtures/linux-aarch64"],
        assets: "plugin/skills/compile/assets/linux-aarch64",
    },
};

/// The shipped programs that need more than the baseline and have no baseline sibling yet, with
/// the sibling each is to get. A debt is paid by writing the sibling, from the very same spec, and
/// removing the entry (or moving it to `TIERS`, when the extension's form stays as a fast tier).
const DEBT: PerTarget<&[Debt]> = PerTarget {
    linux_x86_64: &[Debt {
        program: "tests/fixtures/lines/main.s",
        needs: &["BMI2"],
        sibling: "lines with base shifts and masks in place of bzhi",
    }],
    linux_aarch64: &[],
};

/// The programs that need more than the baseline and have their baseline sibling: fast tiers,
/// named for their extension beside the program they speed up. The benchmark's variants are fast
/// tiers of the program they are measured against.
const TIERS: PerTarget<&[Tier]> = PerTarget {
    linux_x86_64: &[
        Tier {
            program: "benches/programs/count/avx512.s",
            needs: &["AVX-512BW", "BMI1", "BMI2"],
            sibling: "examples/count/main.s",
        },
        Tier {
            program: "benches/programs/shout/avx512-one.s",
            needs: &["AVX-512BW"],
            sibling: "plugin/skills/compile/assets/parallel-template.s",
        },
        Tier {
            program: "benches/programs/shout/avx512.s",
            needs: &["AVX-512BW"],
            sibling: "plugin/skills/compile/assets/parallel-template.s",
        },
        Tier {
            program: "benches/programs/upcase/avx512.s",
            needs: &["AVX-512BW"],
            sibling: "examples/upcase/main.s",
        },
        Tier {
            program: "plugin/skills/compile/assets/fast/aes-ni/main.s",
            needs: &["AES-NI"],
            sibling: "examples/aes/main.s",
        },
        Tier {
            program: "plugin/skills/compile/assets/fast/crc32c-pclmul/main.s",
            needs: &["PCLMULQDQ"],
            sibling: "plugin/skills/compile/assets/fast/crc32c/main.s",
        },
        Tier {
            program: "plugin/skills/compile/assets/fast/sha256-ni/main.s",
            needs: &["SHA"],
            sibling: "examples/sha256/main.s",
        },
        Tier {
            program: "plugin/skills/compile/assets/fast/wc-avx512/main.s",
            needs: &["AVX-512BW", "BMI2"],
            sibling: "plugin/skills/compile/assets/fast/wc/main.s",
        },
    ],
    linux_aarch64: &[
        Tier {
            program: "plugin/skills/compile/assets/linux-aarch64/fast/aes-aes/main.s",
            needs: &["aes"],
            sibling: "tests/fixtures/linux-aarch64/aes/main.s",
        },
        Tier {
            program: "plugin/skills/compile/assets/linux-aarch64/fast/crc32c-crc/main.s",
            needs: &["crc", "pmull"],
            sibling: "plugin/skills/compile/assets/linux-aarch64/fast/crc32c/main.s",
        },
        Tier {
            program: "plugin/skills/compile/assets/linux-aarch64/fast/sha256-sha2/main.s",
            needs: &["sha"],
            sibling: "tests/fixtures/linux-aarch64/sha256/main.s",
        },
        Tier {
            program: "plugin/skills/compile/assets/linux-aarch64/fast/wc-sve/main.s",
            needs: &["sve"],
            sibling: "plugin/skills/compile/assets/linux-aarch64/fast/wc/main.s",
        },
    ],
};

/// The benchmark's own variants, which are not shipped: they are what lengua's speed is measured
/// with, against C, C++ and Rust, so an AVX-512 one is a fast tier by its nature. Listed apart,
/// and held to the baseline like the rest: one that needs more is in `TIERS`.
const BENCH_ONLY: PerTarget<&[Bench]> = PerTarget {
    linux_x86_64: &[
        Bench {
            program: "benches/programs/count/avx512.s",
            spec: "examples/count/spec.md",
        },
        Bench {
            program: "benches/programs/shout/avx512-one.s",
            spec: "plugin/skills/compile/assets/parallel-template-spec.md",
        },
        Bench {
            program: "benches/programs/shout/avx512.s",
            spec: "plugin/skills/compile/assets/parallel-template-spec.md",
        },
        Bench {
            program: "benches/programs/upcase/avx512.s",
            spec: "examples/upcase/spec.md",
        },
    ],
    linux_aarch64: &[
        Bench {
            program: "benches/programs/shout/neon-one.s",
            spec: "plugin/skills/compile/assets/parallel-template-spec.md",
        },
        Bench {
            program: "benches/programs/upcase/neon.s",
            spec: "examples/upcase/spec.md",
        },
    ],
};

/// A program under test: its main.s and its spec, from the repository's root.
struct Program {
    code: String,
    spec: String,
    /// Whether it is one of the benchmark's, not shipped.
    bench: bool,
}

impl Program {
    /// How a message names it.
    fn label(&self) -> String {
        if self.bench {
            format!("{} (benchmark only)", self.code)
        } else {
            self.code.clone()
        }
    }
}

/// What the lists say of a program that needs more than the baseline.
struct Listed {
    /// Whether it is in `TIERS`, rather than in `DEBT`.
    tier: bool,
    needs: &'static [&'static str],
    sibling: &'static str,
}

fn root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

/// A file from the repository's root, as text.
fn read(path: &str) -> Result<String, String> {
    std::fs::read_to_string(root().join(path)).map_err(|error| format!("{path}: {error}"))
}

fn exists(path: &str) -> bool {
    root().join(path).is_file()
}

/// The names of what `folder` holds, sorted.
fn names(folder: &str) -> Vec<String> {
    let entries =
        std::fs::read_dir(root().join(folder)).unwrap_or_else(|error| panic!("{folder}: {error}"));
    let mut found = Vec::new();
    for entry in entries {
        let entry = entry.expect("an entry");
        found.push(entry.file_name().to_string_lossy().into_owned());
    }
    found.sort();
    found
}

/// The spec of the fast program `name`, a folder of `own`, the target's `fast` folder: its own
/// spec.md or, when it is named for the extension it uses after the program it speeds up
/// (`wc-sve`), that program's. Each is looked for in the target's folder first, then in the
/// shared one (x86-64's), where the programs of both targets that meet one spec keep it.
fn fast_spec(own: &str, name: &str) -> String {
    let shared = format!("{SKILL_ASSETS}/fast");
    let mut program = name;
    loop {
        for folder in [own, shared.as_str()] {
            let spec = format!("{folder}/{program}/spec.md");
            if exists(&spec) {
                return spec;
            }
        }
        match program.rsplit_once('-') {
            Some((speeds_up, _)) => program = speeds_up,
            None => return format!("{shared}/{program}/spec.md"),
        }
    }
}

/// Every program of `target` lengua ships, and the benchmark's variants for it, as the files
/// say: nothing is listed by hand but the benchmark's.
fn programs_of(target: Profile) -> Vec<Program> {
    let folders: &[&str] = PLACES.get(target).folders;
    let assets: &str = PLACES.get(target).assets;
    let mut found = Vec::new();
    for folder in folders {
        for name in names(folder) {
            if exists(&format!("{folder}/{name}/main.s")) {
                found.push(Program {
                    code: format!("{folder}/{name}/main.s"),
                    spec: format!("{folder}/{name}/spec.md"),
                    bench: false,
                });
            }
        }
    }
    // A template is made from the spec of its name: main-template.s from template-spec.md.
    for name in names(assets) {
        if let Some(stem) = name.strip_suffix("-template.s") {
            let spec = if stem == "main" {
                "template-spec.md".to_owned()
            } else {
                format!("{stem}-template-spec.md")
            };
            found.push(Program {
                code: format!("{assets}/{name}"),
                spec: format!("{SKILL_ASSETS}/{spec}"),
                bench: false,
            });
        }
    }
    // A fast program has its own spec.md, or, named for the extension it uses after the program
    // it speeds up (`wc-sve`), that program's.
    let fast = format!("{assets}/fast");
    let fast_names = if root().join(&fast).is_dir() {
        names(&fast)
    } else {
        Vec::new()
    };
    for name in fast_names {
        let code = format!("{fast}/{name}/main.s");
        if !exists(&code) {
            continue;
        }
        found.push(Program {
            code,
            spec: fast_spec(&fast, &name),
            bench: false,
        });
    }
    for bench in BENCH_ONLY.get(target).iter() {
        found.push(Program {
            code: bench.program.to_owned(),
            spec: bench.spec.to_owned(),
            bench: true,
        });
    }
    found
}

/// The assets of `target` that are neither a template nor a fast program: nothing may ship
/// without the baseline looking at it, so each of these is a problem, but for `fill.s`, the
/// function programs copy word for word, which is not a program of its own.
fn unrecognized(target: Profile) -> Vec<String> {
    let assets: &str = PLACES.get(target).assets;
    names(assets)
        .into_iter()
        .filter(|name| name.ends_with(".s") && !name.ends_with("-template.s") && name != "fill.s")
        .map(|name| format!("{assets}/{name}"))
        .collect()
}

/// The extension as a program's needs name it: SVE without the vector length its proof ran at.
fn extension(name: &str) -> String {
    if name.starts_with("sve") {
        "sve".to_owned()
    } else {
        name.to_owned()
    }
}

/// The first violations of a report, for a message.
fn lines(report: &checker::Report) -> String {
    let shown: Vec<String> = report
        .violations
        .iter()
        .take(3)
        .map(ToString::to_string)
        .collect();
    format!(
        "{} ({} in all)",
        shown.join("; "),
        report.violations.len() + report.hidden
    )
}

/// What `text`'s instructions need beyond `target`'s baseline, as the checker names the
/// extensions, sorted, or why the program couldn't be held to the rules.
async fn needs_beyond_baseline(
    toolchain: &Toolchain,
    target: Profile,
    text: &str,
    spec: &Spec,
) -> Result<Vec<String>, String> {
    let (source, layout) = checker::check_layout(text);
    if !layout.passed() {
        return Err(format!("its layout breaks the dialect: {}", lines(&layout)));
    }
    if source.isa != target.isa() {
        return Err(format!(
            "it is {:?} code, and the target is {}",
            source.isa,
            target.name()
        ));
    }
    let folder = tempfile::tempdir().map_err(|error| format!("a build folder: {error}"))?;
    let main = folder.path().join("main.s");
    let object = folder.path().join("main.o");
    let executable = folder.path().join("main");
    std::fs::write(&main, text).map_err(|error| format!("main.s: {error}"))?;
    toolchain
        .assemble(target, &main, &object)
        .await
        .map_err(|error| error.to_string())?;
    toolchain
        .link(target, &object, &executable)
        .await
        .map_err(|error| error.to_string())?;
    let bytes = std::fs::read(&executable).map_err(|error| format!("the executable: {error}"))?;
    let check = |features: CpuFeatures| {
        checker::check_program(
            &source,
            &bytes,
            Context {
                syscalls: &spec.syscalls,
                threads: spec.threads,
                features,
                profile: target,
            },
        )
    };
    // With every extension the rules are the same on any machine, and the facts name the
    // extensions the instructions use.
    let everything = check(CpuFeatures::all());
    if !everything.passed() {
        return Err(format!(
            "it breaks a rule even with every extension: {}",
            lines(&everything)
        ));
    }
    let facts = everything
        .facts
        .ok_or_else(|| "the rules didn't run on it".to_owned())?;
    let baseline = CpuFeatures::baseline(target);
    let has = |name: &str| match target.isa() {
        Isa::X86_64 => checker::x86_64::cpu_has(name, &baseline),
        Isa::Aarch64 => checker::aarch64::cpu_has(name, &baseline),
    };
    let mut beyond: Vec<String> = facts
        .features
        .iter()
        .copied()
        .filter(|&name| has(name) != Some(true))
        .map(extension)
        .collect();
    beyond.sort();
    beyond.dedup();
    // The rules' own verdict on a machine with the baseline and nothing more must agree.
    let on_baseline = check(baseline);
    if on_baseline.passed() != beyond.is_empty() {
        return Err(format!(
            "the facts say it needs {beyond:?} beyond the baseline, and with the baseline alone \
             the rules say: {}",
            lines(&on_baseline)
        ));
    }
    Ok(beyond)
}

/// What `program` needs beyond `target`'s baseline, or why it couldn't be held to the rules.
async fn measure(
    toolchain: &Toolchain,
    target: Profile,
    program: &Program,
) -> Result<Vec<String>, String> {
    let text = read(&program.code)?;
    let spec = lengua::spec::parse(&read(&program.spec)?)
        .map_err(|problems| format!("{}: {problems:?}", program.spec))?;
    needs_beyond_baseline(toolchain, target, &text, &spec).await
}

/// What the lists say of the program `code`.
fn listing(debts: &[Debt], tiers: &[Tier], code: &str) -> Option<Listed> {
    let debt = debts
        .iter()
        .find(|debt| debt.program == code)
        .map(|debt| Listed {
            tier: false,
            needs: debt.needs,
            sibling: debt.sibling,
        });
    debt.or_else(|| {
        tiers
            .iter()
            .find(|tier| tier.program == code)
            .map(|tier| Listed {
                tier: true,
                needs: tier.needs,
                sibling: tier.sibling,
            })
    })
}

/// What is wrong with the lists `debts` and `tiers`, given what each of `programs` needs beyond
/// the baseline (nothing known where it couldn't be measured): a program that needs more and is
/// not listed, an entry that names other extensions, one that is not needed any more, a tier
/// whose sibling is not a program on the baseline from the same spec, and an entry for no
/// program at all.
fn judge(
    name: &str,
    programs: &[Program],
    needs: &[Option<Vec<String>>],
    debts: &[Debt],
    tiers: &[Tier],
) -> Vec<String> {
    let mut problems = Vec::new();
    let on_the_baseline = |code: &str| {
        programs.iter().zip(needs).any(|(program, beyond)| {
            program.code == code && beyond.as_ref().is_some_and(Vec::is_empty)
        })
    };
    let spec_of = |code: &str| {
        programs
            .iter()
            .find(|program| program.code == code)
            .and_then(|program| std::fs::read(root().join(&program.spec)).ok())
    };
    for (program, beyond) in programs.iter().zip(needs) {
        let Some(beyond) = beyond else {
            continue;
        };
        let label = program.label();
        let Some(listed) = listing(debts, tiers, &program.code) else {
            if !beyond.is_empty() {
                let (code, needed) = (&program.code, beyond.join(", "));
                problems.push(format!(
                    "{name}: {label} needs {needed} beyond the baseline and isn't listed. With \
                     its baseline sibling written, from the same spec, list it in TIERS: Tier {{ \
                     program: {code:?}, needs: &{beyond:?}, sibling: \"…\" }}; until then, in \
                     DEBT: Debt {{ program: {code:?}, needs: &{beyond:?}, sibling: \"…\" }}"
                ));
            }
            continue;
        };
        if beyond.is_empty() {
            problems.push(format!(
                "{name}: {label} is listed, but it needs nothing beyond the baseline now: \
                 remove its entry"
            ));
            continue;
        }
        let mut said = listed.needs.to_vec();
        said.sort_unstable();
        if said != *beyond {
            problems.push(format!(
                "{name}: {label} needs {} beyond the baseline, and its entry says {}: correct \
                 the entry",
                beyond.join(", "),
                listed.needs.join(", ")
            ));
        }
        if !listed.tier {
            if listed.sibling.is_empty() {
                problems.push(format!(
                    "{name}: {label} is in DEBT with no sibling planned"
                ));
            }
            continue;
        }
        if listed.sibling == program.code || !on_the_baseline(listed.sibling) {
            problems.push(format!(
                "{name}: {label} is a fast tier, but its sibling {} isn't a program of the target \
                 that needs nothing beyond the baseline",
                listed.sibling
            ));
        } else if spec_of(listed.sibling) != spec_of(&program.code) {
            problems.push(format!(
                "{name}: {label} and its sibling {} are not from the very same spec",
                listed.sibling
            ));
        }
    }
    let listed: Vec<&str> = debts
        .iter()
        .map(|debt| debt.program)
        .chain(tiers.iter().map(|tier| tier.program))
        .collect();
    for (index, code) in listed.iter().enumerate() {
        if !programs.iter().any(|program| program.code == *code) {
            problems.push(format!("{name}: {code} is listed, but isn't a program"));
        }
        if listed[..index].contains(code) {
            problems.push(format!("{name}: {code} is listed twice"));
        }
    }
    problems
}

/// Holds every program of `target` to its baseline and the lists to the programs: what is wrong.
async fn hold(toolchain: &Toolchain, target: Profile) -> Vec<String> {
    let name = target.name();
    let programs = programs_of(target);
    let mut problems: Vec<String> = unrecognized(target)
        .into_iter()
        .map(|asset| format!("{name}: {asset} is neither a template nor a fast program"))
        .collect();
    if programs.len() < MIN_PROGRAMS {
        problems.push(format!("{name}: only {} programs found", programs.len()));
    }
    // What each program needs beyond the baseline; none where it couldn't be held to the rules.
    let mut needs: Vec<Option<Vec<String>>> = Vec::new();
    for program in &programs {
        match measure(toolchain, target, program).await {
            Ok(beyond) => needs.push(Some(beyond)),
            Err(why) => {
                problems.push(format!("{name}: {}: {why}", program.label()));
                needs.push(None);
            }
        }
    }
    problems.extend(judge(
        name,
        &programs,
        &needs,
        DEBT.get(target),
        TIERS.get(target),
    ));
    problems
}

/// Every shipped program of every target follows every rule on its target's baseline, or is
/// listed as a fast tier beside its baseline sibling, or as a debt.
#[tokio::test]
async fn every_shipped_program_runs_on_the_baseline() {
    let toolchain = Toolchain::discover()
        .await
        .expect("llvm-mc and ld.lld are installed");
    let mut problems = Vec::new();
    for target in Profile::ALL {
        problems.extend(hold(&toolchain, target).await);
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

/// The test is no use if it can't see an extension: a template with one instruction of BMI2
/// (x86-64) or of CRC-32 (ARM64) added needs exactly that beyond the baseline, and the template
/// alone needs nothing.
#[tokio::test]
async fn a_program_beyond_the_baseline_is_found() {
    let toolchain = Toolchain::discover()
        .await
        .expect("llvm-mc and ld.lld are installed");
    let spec_text = read(&format!("{SKILL_ASSETS}/template-spec.md")).expect("the template's spec");
    let spec = lengua::spec::parse(&spec_text).expect("a valid spec");
    for (target, plain, leaning, needed) in [
        (
            Profile::LinuxX86_64,
            checker::TEMPLATE.to_owned(),
            checker::TEMPLATE.replacen("    xor r9d, r9d\n", "    shlx r9, r9, rcx\n", 1),
            "BMI2",
        ),
        (
            Profile::LinuxAarch64,
            checker::AARCH64_TEMPLATE.to_owned(),
            checker::AARCH64_TEMPLATE
                .replacen(".arch armv8-a\n", ".arch armv8-a\n.arch_extension crc\n", 1)
                .replacen(
                    "    ldr x0, [sp]\n",
                    "    ldr x0, [sp]\n    crc32cb w9, w0, w0\n",
                    1,
                ),
            "crc",
        ),
    ] {
        assert_ne!(plain, leaning, "{}", target.name());
        let found = needs_beyond_baseline(&toolchain, target, &plain, &spec).await;
        assert_eq!(found, Ok(Vec::new()), "{}", target.name());
        let found = needs_beyond_baseline(&toolchain, target, &leaning, &spec).await;
        assert_eq!(found, Ok(vec![needed.to_owned()]), "{}", target.name());
    }
}

/// The lists are held to the programs: each way a listing can be wrong is a problem, and a right
/// one is not. `fast` and `base` are programs of one spec, `other` of another.
#[test]
fn the_baseline_lists_are_held_to_the_programs() {
    let program = |code: &str, spec: &str| Program {
        code: code.to_owned(),
        spec: spec.to_owned(),
        bench: false,
    };
    let programs = [
        program("base/main.s", "examples/add/spec.md"),
        program("fast/main.s", "examples/add/spec.md"),
        program("other/main.s", "examples/sort/spec.md"),
    ];
    let beyond = |names: &[&str]| -> Option<Vec<String>> {
        Some(names.iter().map(|name| (*name).to_owned()).collect())
    };
    let judged = |needs: [Option<Vec<String>>; 3], debts: &[Debt], tiers: &[Tier]| {
        judge("target", &programs, &needs, debts, tiers).join("\n")
    };
    let nothing = || [beyond(&[]), beyond(&[]), beyond(&[])];
    let fast_needs = |names: &[&str]| [beyond(&[]), beyond(names), beyond(&[])];
    let debt = |needs: &'static [&'static str]| Debt {
        program: "fast/main.s",
        needs,
        sibling: "base/main.s in SSE2",
    };
    let tier = |sibling: &'static str| Tier {
        program: "fast/main.s",
        needs: &["AVX2"],
        sibling,
    };

    // Nothing needs more and nothing is listed: nothing is wrong.
    assert_eq!(judged(nothing(), &[], &[]), "");
    // A program that needs more and isn't listed.
    let problems = judged(fast_needs(&["AVX2"]), &[], &[]);
    assert!(
        problems.contains("fast/main.s needs AVX2 beyond the baseline and isn't listed"),
        "{problems}"
    );
    // A debt that names what it needs, and a tier beside its baseline sibling.
    assert_eq!(judged(fast_needs(&["AVX2"]), &[debt(&["AVX2"])], &[]), "");
    assert_eq!(
        judged(fast_needs(&["AVX2"]), &[], &[tier("base/main.s")]),
        ""
    );
    // A debt that names other extensions than the program needs.
    let problems = judged(fast_needs(&["AVX2", "BMI2"]), &[debt(&["AVX2"])], &[]);
    assert!(problems.contains("correct the entry"), "{problems}");
    // A debt that is paid: the program needs nothing, and its entry has to go.
    let problems = judged(nothing(), &[debt(&["AVX2"])], &[]);
    assert!(problems.contains("remove its entry"), "{problems}");
    // An entry for no program, and a program listed twice.
    let gone = Debt {
        program: "gone/main.s",
        needs: &["AVX2"],
        sibling: "base/main.s in SSE2",
    };
    let problems = judged(nothing(), &[gone], &[]);
    assert!(
        problems.contains("gone/main.s is listed, but isn't a program"),
        "{problems}"
    );
    let problems = judged(
        fast_needs(&["AVX2"]),
        &[debt(&["AVX2"])],
        &[tier("base/main.s")],
    );
    assert!(
        problems.contains("fast/main.s is listed twice"),
        "{problems}"
    );
    // A tier whose sibling is itself, is no program, needs more itself, or has another spec.
    for (sibling, wanted) in [
        ("fast/main.s", "its sibling fast/main.s isn't a program"),
        (
            "nowhere/main.s",
            "its sibling nowhere/main.s isn't a program",
        ),
        ("other/main.s", "are not from the very same spec"),
    ] {
        let problems = judged(fast_needs(&["AVX2"]), &[], &[tier(sibling)]);
        assert!(problems.contains(wanted), "{sibling}: {problems}");
    }
    let needs = [beyond(&["BMI2"]), beyond(&["AVX2"]), beyond(&[])];
    let problems = judged(needs, &[], &[tier("base/main.s")]);
    assert!(
        problems.contains("its sibling base/main.s isn't a program"),
        "{problems}"
    );
}

/// Every variant under benches/programs is listed apart, and every listed one is there: a variant
/// can't be added to the benchmark and escape the baseline.
#[test]
fn every_benchmark_variant_is_listed_for_the_baseline() {
    let mut listed: Vec<String> = Vec::new();
    for target in Profile::ALL {
        listed.extend(
            BENCH_ONLY
                .get(target)
                .iter()
                .map(|bench| bench.program.to_owned()),
        );
    }
    listed.sort();
    let mut found = Vec::new();
    for folder in names("benches/programs") {
        for file in names(&format!("benches/programs/{folder}")) {
            if file.ends_with(".s") {
                found.push(format!("benches/programs/{folder}/{file}"));
            }
        }
    }
    found.sort();
    assert_eq!(found, listed);
}
