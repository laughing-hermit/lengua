//! Random-value tests of the prover's model of SVE against an AArch64 CPU with SVE: this
//! computer's, or QEMU's. A harness of its own, since a Z register is as long as the CPU makes
//! it: each vector is the registers, NZCV, the memory cell's words and the probe's address as
//! the main harness has them, then z0 to z7 and p0 to p7, as long as the CPU's vectors make them.
//! The harness loads and stores them with SVE's `ldr` and `str`, which the dialect doesn't have;
//! only the probes, in the dialect, run in the model.

use std::io::Write as _;
use std::process::{Command, Stdio};

use rand::rngs::ChaCha8Rng;
use rand::{RngExt, SeedableRng};

use super::super::{Image, Last, State, Step, step};
use super::{CELL_WORDS, FLAGS, Probe, REGISTERS, build, memory, probe, register, value};
use crate::prove::term::Pool;

/// The Z and P registers a probe works with: z0 to z7 and p0 to p7.
const SVE_REGISTERS: usize = 8;

/// Where the Z registers start in a vector and a result: after the registers, NZCV, the cell's
/// words and the probe's address.
const Z_AT: usize = (REGISTERS + CELL_WORDS + 2) * 8;

/// A vector's bytes, and a result's, for vectors of `vl` bytes.
const fn vector_bytes(vl: usize) -> usize {
    Z_AT + SVE_REGISTERS * vl + SVE_REGISTERS * vl / 8
}

/// A predicate of bytes, p0 to p7.
fn p(rng: &mut ChaCha8Rng) -> usize {
    rng.random_range(0..SVE_REGISTERS)
}

/// A Z register of bytes, z0 to z7.
fn z(rng: &mut ChaCha8Rng) -> usize {
    rng.random_range(0..SVE_REGISTERS)
}

/// Every SVE probe, its registers and numbers drawn from `rng`.
#[allow(clippy::too_many_lines)]
fn sve_probes(rng: &mut ChaCha8Rng) -> Vec<Probe> {
    let mut list = Vec::new();
    for _ in 0..2 {
        list.push(probe(&[format!("ptrue p{}.b", p(rng))]));
    }
    // whilelo with the limit past, at and before the start, and its flags with each condition.
    for gap in [0_u64, 1, 5, 15, 16, 17, 33, 63, 64, 65, 200] {
        let (d, n, m) = (p(rng), register(rng, true), register(rng, true));
        let start = rng.random_range(0..1000_u64);
        list.push(probe(&[
            format!("mov {n}, #{start}"),
            format!("mov {m}, #{}", start + gap),
            format!("whilelo p{d}.b, {n}, {m}"),
        ]));
    }
    for _ in 0..6 {
        let (d, n, m) = (p(rng), register(rng, true), register(rng, true));
        list.push(probe(&[format!("whilelo p{d}.b, {n}, {m}")]));
    }
    let d = p(rng);
    list.push(probe(&[
        "mov x3, #7".to_owned(),
        format!("whilelo p{d}.b, xzr, x3"),
    ]));
    for op in ["and", "bic", "eor", "orr"] {
        for _ in 0..2 {
            let (d, g, n, m) = (p(rng), p(rng), p(rng), p(rng));
            list.push(probe(&[format!("{op} p{d}.b, p{g}/z, p{n}.b, p{m}.b")]));
        }
    }
    let (d, g, n) = (p(rng), p(rng), p(rng));
    list.push(probe(&[format!("not p{d}.b, p{g}/z, p{n}.b")]));
    list.push(probe(&[format!("mov p{d}.b, p{g}/z, p{n}.b")]));
    list.push(probe(&[format!("mov p{d}.b, p{n}.b")]));
    for _ in 0..3 {
        let (rd, g, n) = (register(rng, true), p(rng), p(rng));
        list.push(probe(&[format!("cntp {rd}, p{g}, p{n}.b")]));
    }
    // Loads and stores of the cell, which x15 holds; the index 0, as the cell holds the longest
    // vector, which the rules hold every access to.
    for _ in 0..3 {
        let (t, g) = (z(rng), p(rng));
        list.push(memory(&[
            "mov x14, #0".to_owned(),
            format!("ld1b {{ z{t}.b }}, p{g}/z, [x15, x14]"),
        ]));
        let (t, g) = (z(rng), p(rng));
        list.push(memory(&[
            "mov x14, #0".to_owned(),
            format!("st1b {{ z{t}.b }}, p{g}, [x15, x14]"),
        ]));
    }
    // Compares of vectors in every test, the swapped ones too, and with numbers.
    for cmp in ["eq", "ne", "hs", "hi", "lo", "ls", "ge", "gt", "lt", "le"] {
        let (d, g, n, m) = (p(rng), p(rng), z(rng), z(rng));
        list.push(probe(&[format!("cmp{cmp} p{d}.b, p{g}/z, z{n}.b, z{m}.b")]));
        let (d, g, n) = (p(rng), p(rng), z(rng));
        let number = if matches!(cmp, "hs" | "hi" | "lo" | "ls") {
            rng.random_range(0..=127)
        } else {
            rng.random_range(-16..=15)
        };
        list.push(probe(&[format!(
            "cmp{cmp} p{d}.b, p{g}/z, z{n}.b, #{number}"
        )]));
    }
    for op in ["add", "sub"] {
        let (d, n, m) = (z(rng), z(rng), z(rng));
        list.push(probe(&[format!("{op} z{d}.b, z{n}.b, z{m}.b")]));
    }
    for op in ["and", "orr", "eor", "bic"] {
        let (d, n, m) = (z(rng), z(rng), z(rng));
        list.push(probe(&[format!("{op} z{d}.d, z{n}.d, z{m}.d")]));
    }
    let (d, n) = (z(rng), z(rng));
    list.push(probe(&[format!("mov z{d}.d, z{n}.d")]));
    let (d, g, n) = (z(rng), p(rng), z(rng));
    list.push(probe(&[format!("cnt z{d}.b, p{g}/m, z{n}.b")]));
    let (d, g, n, m) = (z(rng), p(rng), z(rng), z(rng));
    list.push(probe(&[format!("sel z{d}.b, p{g}, z{n}.b, z{m}.b")]));
    list.push(probe(&[format!("mov z{d}.b, p{g}/m, z{n}.b")]));
    for number in [0, 1, -1, 10, -128, 127] {
        let d = z(rng);
        list.push(probe(&[format!("mov z{d}.b, #{number}")]));
        let (d, g) = (z(rng), p(rng));
        list.push(probe(&[format!("mov z{d}.b, p{g}/z, #{number}")]));
        let (d, g) = (z(rng), p(rng));
        list.push(probe(&[format!("mov z{d}.b, p{g}/m, #{number}")]));
    }
    for _ in 0..2 {
        let (d, n) = (z(rng), register(rng, false));
        list.push(probe(&[format!("mov z{d}.b, {n}")]));
    }
    for _ in 0..3 {
        let (d, g, n) = (rng.random_range(0..SVE_REGISTERS), p(rng), z(rng));
        list.push(probe(&[format!("uaddv d{d}, p{g}, z{n}.b")]));
    }
    for times in [1, 2, 4, 16] {
        let rd = register(rng, true);
        let suffix = if times == 1 {
            String::new()
        } else {
            format!(", all, mul #{times}")
        };
        list.push(probe(&[format!("incb {rd}{suffix}")]));
        let rd = register(rng, true);
        list.push(probe(&[format!("cntb {rd}{suffix}")]));
    }
    for _ in 0..2 {
        let (d, m) = (z(rng), register(rng, false));
        list.push(probe(&[format!("insr z{d}.b, {m}")]));
    }
    let d = z(rng);
    list.push(probe(&[format!("insr z{d}.b, wzr")]));
    for _ in 0..3 {
        let (rd, g, n) = (register(rng, false), p(rng), z(rng));
        list.push(probe(&[format!("lastb {rd}, p{g}, z{n}.b")]));
    }
    // A NEON write clears the Z register above its V register.
    let (d, n) = (z(rng), z(rng));
    list.push(probe(&[format!("add v{d}.16b, v{n}.16b, v{n}.16b")]));
    list
}

/// The SVE harness: as the main one, with z0 to z7 and p0 to p7 for the V registers.
fn sve_harness(probes: &[Probe], capacity: usize, vl: usize) -> String {
    let vector = vector_bytes(vl);
    let piece = |value: usize, shift: u32| (value >> shift) & 0xffff;
    let bytes_in = capacity * vector;
    let p_at = Z_AT + SVE_REGISTERS * vl;
    let mut source = format!(
        ".arch armv8-a
.arch_extension sve
.text
.globl _start
.type _start, @function
_start:
    mov x19, #0
.Lread:
    mov x0, #0
    adrp x1, vectors
    add x1, x1, :lo12:vectors
    add x1, x1, x19
    movz x2, #{}
    movk x2, #{}, lsl #16
    sub x2, x2, x19
    mov x8, #63
    svc #0
    cmp x0, #0
    b.le .Lread_done
    add x19, x19, x0
    b .Lread
.Lread_done:
    mov x20, #{vector}
    udiv x21, x19, x20
    mov x22, #0
.Lnext:
    cmp x22, x21
    b.hs .Lfinish
    adrp x23, vectors
    add x23, x23, :lo12:vectors
    mov x24, #{vector}
    madd x23, x22, x24, x23
    adrp x26, cell
    add x26, x26, :lo12:cell
{}    add x9, x23, #{Z_AT}
{}    add x9, x23, #{p_at}
{}    ldr x0, [x23, #{}]
    msr nzcv, x0
    ldr x25, [x23, #{}]
",
        piece(bytes_in, 0),
        piece(bytes_in, 16),
        (0..CELL_WORDS / 2)
            .map(|pair| format!(
                "    ldp x0, x1, [x23, #{}]\n    stp x0, x1, [x26, #{}]\n",
                (REGISTERS + 1 + 2 * pair) * 8,
                pair * 16
            ))
            .collect::<String>(),
        (0..SVE_REGISTERS)
            .map(|number| format!("    ldr z{number}, [x9, #{number}, mul vl]\n"))
            .collect::<String>(),
        (0..SVE_REGISTERS)
            .map(|number| format!("    ldr p{number}, [x9, #{number}, mul vl]\n"))
            .collect::<String>(),
        REGISTERS * 8,
        (REGISTERS + 1 + CELL_WORDS) * 8,
    );
    for pair in 0..REGISTERS / 2 {
        source.push_str(&format!(
            "    ldp x{}, x{}, [x23, #{}]\n",
            pair * 2,
            pair * 2 + 1,
            pair * 16
        ));
    }
    source.push_str("    br x25\n.Lafter:\n    adrp x23, result\n    add x23, x23, :lo12:result\n");
    for pair in 0..REGISTERS / 2 {
        source.push_str(&format!(
            "    stp x{}, x{}, [x23, #{}]\n",
            pair * 2,
            pair * 2 + 1,
            pair * 16
        ));
    }
    source.push_str(&format!("    add x9, x23, #{Z_AT}\n"));
    for number in 0..SVE_REGISTERS {
        source.push_str(&format!("    str z{number}, [x9, #{number}, mul vl]\n"));
    }
    source.push_str(&format!("    add x9, x23, #{p_at}\n"));
    for number in 0..SVE_REGISTERS {
        source.push_str(&format!("    str p{number}, [x9, #{number}, mul vl]\n"));
    }
    source.push_str(&format!(
        "    mrs x0, nzcv
    str x0, [x23, #{}]
    adrp x26, cell
    add x26, x26, :lo12:cell
{}    adrp x24, results
    add x24, x24, :lo12:results
    mov x26, #{vector}
    madd x24, x22, x26, x24
    mov x27, #0
.Lcopy:
    ldr x0, [x23, x27, lsl #3]
    str x0, [x24, x27, lsl #3]
    add x27, x27, #1
    cmp x27, #{}
    b.lo .Lcopy
    add x22, x22, #1
    b .Lnext
.Lfinish:
    mov x26, #{vector}
    mul x20, x21, x26
    mov x19, #0
.Lwrite:
    cmp x19, x20
    b.hs .Lexit
    mov x0, #1
    adrp x1, results
    add x1, x1, :lo12:results
    add x1, x1, x19
    sub x2, x20, x19
    mov x8, #64
    svc #0
    cmp x0, #0
    b.le .Lexit
    add x19, x19, x0
    b .Lwrite
.Lexit:
    mov x0, #0
    mov x8, #93
    svc #0
.size _start, .-_start
",
        REGISTERS * 8,
        (0..CELL_WORDS / 2)
            .map(|pair| format!(
                "    ldp x0, x1, [x26, #{}]\n    stp x0, x1, [x23, #{}]\n",
                pair * 16,
                (REGISTERS + 1 + 2 * pair) * 8
            ))
            .collect::<String>(),
        vector / 8,
    ));
    for (number, probe) in probes.iter().enumerate() {
        source.push_str(&format!(".type probe{number}, @function\nprobe{number}:\n"));
        for line in &probe.code {
            source.push_str(&format!("    {line}\n"));
        }
        source.push_str(&format!(
            "    b .Lafter\n.size probe{number}, .-probe{number}\n"
        ));
    }
    source.push_str(".bss\n.balign 16\n");
    for (name, size) in [
        ("vectors", capacity * vector),
        ("results", capacity * vector),
        ("result", vector),
        ("cell", CELL_WORDS * 8),
    ] {
        source.push_str(&format!(
            ".type {name}, @object\n{name}:\n    .zero {size}\n.size {name}, {size}\n.balign 16\n"
        ));
    }
    source
}

/// One SVE vector's values.
struct SveValues {
    registers: [u64; REGISTERS],
    nzcv: u64,
    cell: [u64; CELL_WORDS],
    /// z0 to z7, `vl` bytes each.
    z: Vec<Vec<u8>>,
    /// p0 to p7, a bit for each byte of a Z register.
    p: Vec<Vec<bool>>,
}

/// Where the model ends.
struct SveEnded {
    registers: Vec<u128>,
    flags: Vec<bool>,
    cell: Vec<u8>,
    z: Vec<Vec<u8>>,
    p: Vec<Vec<bool>>,
}

/// Runs a probe in the model, from `start` to `end`, on one vector's values.
fn sve_model(
    image: &Image,
    (start, end): (u64, u64),
    values: &SveValues,
    cell_address: u64,
    vl: usize,
) -> Result<SveEnded, String> {
    let mut pool = Pool::default();
    let mut state = State::start(image, &mut pool, &[], &[], Vec::new());
    for (slot, value) in values.registers.iter().enumerate() {
        state.registers[slot] = pool.bv(64, u128::from(*value));
    }
    for (number, bytes) in values.z.iter().enumerate() {
        let vector = std::rc::Rc::make_mut(&mut state.vectors[number]);
        for (at, byte) in bytes.iter().enumerate() {
            vector[at] = pool.bv(8, u128::from(*byte));
        }
    }
    for (number, lanes) in values.p.iter().enumerate() {
        let predicate = std::rc::Rc::make_mut(&mut state.predicates[number]);
        for (at, lane) in lanes.iter().enumerate() {
            predicate[at] = pool.boolean(*lane);
        }
    }
    let bit = |pool: &mut Pool, mask: u64| pool.boolean(values.nzcv & mask != 0);
    state.sign = bit(&mut pool, 1 << 31);
    state.zero = bit(&mut pool, 1 << 30);
    state.carry = bit(&mut pool, 1 << 29);
    state.overflow = bit(&mut pool, 1 << 28);
    state.last = Last::Other;
    for (word, value) in values.cell.iter().enumerate() {
        let value = pool.bv(64, u128::from(*value));
        state.store(&mut pool, cell_address + 8 * word as u64, value);
    }
    state.rip = start;
    let mut steps = 0;
    while state.rip != end {
        steps += 1;
        if steps > 16 {
            return Err("the model ran past the probe".into());
        }
        match step(image, &mut state, &mut pool) {
            Ok(Step::Next) => {}
            Ok(Step::Split(..) | Step::Choose(_)) => {
                return Err("the model split a run with known values".into());
            }
            Err(end) => return Err(format!("the model stopped: {end:?}")),
        }
    }
    let known = |pool: &Pool, term, what: &str| {
        pool.constant(term)
            .ok_or_else(|| format!("the model doesn't know {what}"))
    };
    let registers = state.registers[..REGISTERS]
        .iter()
        .enumerate()
        .map(|(slot, term)| known(&pool, *term, &format!("x{slot}")))
        .collect::<Result<_, _>>()?;
    let flags = [state.sign, state.zero, state.carry, state.overflow]
        .iter()
        .zip(FLAGS)
        .map(|(flag, (_, name))| {
            pool.truth(*flag)
                .ok_or_else(|| format!("the model doesn't know {name}"))
        })
        .collect::<Result<_, _>>()?;
    let mut cell = Vec::with_capacity(CELL_WORDS * 8);
    for at in 0..(CELL_WORDS * 8) as u64 {
        let term = state
            .memory
            .get(cell_address + at)
            .ok_or("the model lost a byte of the cell")?;
        cell.push(known(&pool, term, "a byte of the cell")? as u8);
    }
    let mut z = Vec::new();
    for number in 0..SVE_REGISTERS {
        let bytes = state.vectors[number][..vl]
            .iter()
            .map(|term| known(&pool, *term, &format!("z{number}")).map(|byte| byte as u8))
            .collect::<Result<Vec<u8>, _>>()?;
        z.push(bytes);
    }
    let mut p = Vec::new();
    for number in 0..SVE_REGISTERS {
        let lanes = state.predicates[number][..vl]
            .iter()
            .map(|term| {
                pool.truth(*term)
                    .ok_or_else(|| format!("the model doesn't know p{number}"))
            })
            .collect::<Result<Vec<bool>, _>>()?;
        p.push(lanes);
    }
    Ok(SveEnded {
        registers,
        flags,
        cell,
        z,
        p,
    })
}

/// The SVE vector length the probes run at here: this CPU's, or QEMU's; none where neither
/// runs SVE.
fn vector_length() -> Option<usize> {
    let machine = crate::platform::Machine::detect();
    let bytes = if cfg!(target_arch = "aarch64") {
        machine.features.sve
    } else {
        let target = crate::platform::Target {
            profile: crate::platform::Profile::LinuxAarch64,
            emulator: Some(crate::platform::Emulator {
                name: "qemu-aarch64".into(),
            }),
        };
        target.features(&machine).sve
    };
    (bytes > 0).then_some(usize::from(bytes))
}

#[tokio::test]
#[allow(clippy::too_many_lines)]
async fn every_sve_instruction_matches_the_cpu() {
    if !super::runs_aarch64() {
        crate::skiplog::note("QEMU doesn't run AArch64 programs here (install qemu-user-static)");
        return;
    }
    let Some(vl) = vector_length() else {
        eprintln!("skipped: this CPU, or QEMU's, has no SVE");
        return;
    };
    let seed = std::env::var("LENGUA_RANDOM_SEED")
        .ok()
        .and_then(|seed| seed.parse().ok())
        .unwrap_or_else(|| {
            let mut bytes = [0; 8];
            getrandom::fill(&mut bytes).expect("a seed");
            u64::from_le_bytes(bytes)
        });
    let mut rng = ChaCha8Rng::seed_from_u64(seed);
    let probes = sve_probes(&mut rng);
    let per_probe = 16;
    let capacity = probes.len() * per_probe;
    let folder = tempfile::tempdir().expect("a folder");
    let (executable, mut machine) = build(&sve_harness(&probes, capacity, vl), folder.path()).await;
    machine.sve_bytes = u16::try_from(vl).expect("a vector length");
    let image = Image::new(&machine).expect("the harness has _start");
    let cell_address = machine.data["cell"].address;
    let spans: Vec<(u64, u64)> = (0..probes.len())
        .map(|number| {
            let function = &machine.functions[&format!("probe{number}")];
            (function.address, function.address + function.size - 4)
        })
        .collect();
    let vector = vector_bytes(vl);
    let mut runs = Vec::with_capacity(capacity);
    let mut input = Vec::with_capacity(capacity * vector);
    for (number, probe) in probes.iter().enumerate() {
        for _ in 0..per_probe {
            let mut registers = [0; REGISTERS];
            for register in &mut registers {
                *register = value(&mut rng);
            }
            if probe.memory {
                registers[15] = cell_address;
            }
            let nzcv = u64::from(rng.random_range(0_u32..16)) << 28;
            let cell: [u64; CELL_WORDS] = std::array::from_fn(|_| value(&mut rng));
            // Bytes near the tests' edges as often as random ones, and predicates of every
            // density, all lanes and none among them.
            let z: Vec<Vec<u8>> = (0..SVE_REGISTERS)
                .map(|_| {
                    (0..vl)
                        .map(|_| match rng.random_range(0..4) {
                            0 => [0, 1, 9, 10, 13, 14, 32, 127, 128, 255][rng.random_range(0..10)],
                            _ => rng.random(),
                        })
                        .collect()
                })
                .collect();
            let p: Vec<Vec<bool>> = (0..SVE_REGISTERS)
                .map(|_| {
                    let density = [0.0, 0.25, 0.5, 0.9, 1.0][rng.random_range(0..5)];
                    (0..vl).map(|_| rng.random_bool(density)).collect()
                })
                .collect();
            for word in registers
                .iter()
                .chain(&[nzcv])
                .chain(&cell)
                .chain(&[spans[number].0])
            {
                input.extend_from_slice(&word.to_le_bytes());
            }
            for bytes in &z {
                input.extend_from_slice(bytes);
            }
            for lanes in &p {
                for chunk in lanes.chunks(8) {
                    let byte = chunk
                        .iter()
                        .enumerate()
                        .fold(0_u8, |byte, (bit, lane)| byte | u8::from(*lane) << bit);
                    input.push(byte);
                }
            }
            runs.push((
                number,
                SveValues {
                    registers,
                    nzcv,
                    cell,
                    z,
                    p,
                },
            ));
        }
    }
    let mut child = Command::new(&executable)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("run the harness");
    let mut stdin = child.stdin.take().expect("its input");
    let writer = std::thread::spawn(move || stdin.write_all(&input));
    let output = child.wait_with_output().expect("its results");
    writer
        .join()
        .expect("the writer")
        .expect("write the vectors");
    assert!(output.status.success(), "the harness failed: {output:?}");
    assert_eq!(output.stdout.len(), capacity * vector, "every result");
    let mut problems = Vec::new();
    for (position, (number, values)) in runs.iter().enumerate() {
        let real = &output.stdout[position * vector..(position + 1) * vector];
        let word =
            |at: usize| u64::from_le_bytes(real[at * 8..at * 8 + 8].try_into().expect("8 bytes"));
        let code = probes[*number].code.join("; ");
        let ended = match sve_model(&image, spans[*number], values, cell_address, vl) {
            Ok(ended) => ended,
            Err(problem) => {
                problems.push(format!("`{code}`: {problem}"));
                continue;
            }
        };
        for (slot, modelled) in ended.registers.iter().enumerate() {
            if *modelled != u128::from(word(slot)) {
                problems.push(format!(
                    "`{code}`: x{slot} is {modelled:#x}, the CPU says {:#x}",
                    word(slot)
                ));
            }
        }
        for ((mask, name), modelled) in FLAGS.iter().zip(&ended.flags) {
            let expected = word(REGISTERS) & mask != 0;
            if *modelled != expected {
                problems.push(format!(
                    "`{code}`: {name} is {modelled}, the CPU says {expected}"
                ));
            }
        }
        let cell_at = (REGISTERS + 1) * 8;
        if ended.cell[..] != real[cell_at..cell_at + CELL_WORDS * 8] {
            problems.push(format!("`{code}`: the cell differs"));
        }
        for (number, modelled) in ended.z.iter().enumerate() {
            let at = Z_AT + number * vl;
            if modelled[..] != real[at..at + vl] {
                problems.push(format!(
                    "`{code}`: z{number} is {modelled:02x?}, the CPU says {:02x?}",
                    &real[at..at + vl]
                ));
            }
        }
        for (number, modelled) in ended.p.iter().enumerate() {
            let at = Z_AT + SVE_REGISTERS * vl + number * vl / 8;
            let real_lanes: Vec<bool> = (0..vl)
                .map(|lane| real[at + lane / 8] >> (lane % 8) & 1 == 1)
                .collect();
            if *modelled != real_lanes {
                problems.push(format!("`{code}`: p{number} differs"));
            }
        }
    }
    let shown: Vec<&String> = problems.iter().take(30).collect();
    assert!(
        problems.is_empty(),
        "seed {seed}, vectors of {vl} bytes: {} of {} runs differ:\n{shown:#?}",
        problems.len(),
        runs.len()
    );
}
