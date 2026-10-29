//! Runs a program's machine code on terms instead of numbers: each register, flag and byte holds
//! a term over the inputs. A branch the inputs decide splits the run in two, each side keeping
//! its condition, so the finished runs cover every input of a shape. With concrete inputs every
//! term is a number, and the same code is a plain emulator, which the prover checks against real
//! runs.

use std::collections::{BTreeMap, HashMap};
use std::rc::Rc;

use iced_x86::{ConditionCode, Instruction, Mnemonic, OpKind, Register};

use super::term::{Pool, Term};
use crate::checker::decode::Machine;

/// The address of `argc` when `_start` begins.
pub const STACK_TOP: u64 = 0x7ffd_0000_0000;

/// How far below that the stack may grow.
const STACK_SIZE: u64 = 8 * 1024 * 1024;

/// Room above the stack top for the argument strings and pointers.
const ARGUMENT_ROOM: u64 = 1024 * 1024;

/// The most instructions one run may take before the prover gives up on it.
pub const MAX_STEPS: u64 = 5_000_000;

/// The bytes of a vector register: a zmm register's 512 bits.
const VECTOR_BYTES: usize = 64;

/// A range of addresses the program may use.
#[derive(Debug, Clone, Copy)]
struct Region {
    start: u64,
    end: u64,
    writable: bool,
}

/// The program as loaded: its instructions by address, and its memory.
pub struct Image {
    instructions: HashMap<u64, Instruction>,
    regions: Vec<Region>,
    initial: HashMap<u64, u8>,
    entry: u64,
    /// Each function's name, by the address range of its code.
    functions: Vec<(u64, u64, String)>,
}

impl Image {
    /// Loads the functions and data the decoder read from the executable.
    #[must_use]
    pub fn new(machine: &Machine) -> Option<Self> {
        let entry = machine.functions.get("_start")?.address;
        let mut instructions = HashMap::new();
        let mut functions = Vec::new();
        for (name, function) in &machine.functions {
            for instruction in &function.instructions {
                instructions.insert(instruction.ip(), *instruction);
            }
            functions.push((
                function.address,
                function.address + function.size,
                name.clone(),
            ));
        }
        let mut regions = vec![Region {
            start: STACK_TOP - STACK_SIZE,
            end: STACK_TOP + ARGUMENT_ROOM,
            writable: true,
        }];
        let mut initial = HashMap::new();
        for data in machine.data.values() {
            regions.push(Region {
                start: data.address,
                end: data.address + data.size,
                writable: data.writable,
            });
            for (offset, byte) in data.bytes.iter().enumerate() {
                initial.insert(data.address + offset as u64, *byte);
            }
        }
        Some(Self {
            instructions,
            regions,
            initial,
            entry,
            functions,
        })
    }

    /// The name of the function whose code holds `address`.
    fn function_at(&self, address: u64) -> &str {
        self.functions
            .iter()
            .find(|(start, end, _)| (*start..*end).contains(&address))
            .map_or("its code", |(_, _, name)| name.as_str())
    }

    fn region(&self, address: u64) -> Option<Region> {
        self.regions
            .iter()
            .copied()
            .find(|region| (region.start..region.end).contains(&address))
    }
}

/// How a run ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum End {
    /// `exit` or `exit_group`, with the status's low byte.
    Exit(Term),
    /// The CPU or kernel would stop the program with this signal.
    Signal(&'static str),
    /// Something the prover doesn't model, so this run proves nothing.
    Unsupported(String),
    /// The paths through the code ran past the limit before this one was followed.
    TooManyPaths,
}

/// What last set the flags, when a branch can read it as a plain comparison.
#[derive(Debug, Clone, Copy)]
enum Last {
    /// `cmp a, b` or `sub a, b`.
    Compare(Term, Term),
    /// `test`, `and`, `or` or `xor`, with this result.
    Logic(Term),
    /// Anything else: read the flags themselves.
    Other,
}

/// Bytes of memory by address, in pages that the runs a fork makes share until one of them
/// writes to a page: a fork copies the page table, never the bytes.
#[derive(Debug, Clone, Default)]
pub struct Memory {
    pages: BTreeMap<u64, Rc<[Option<Term>; PAGE]>>,
}

/// The bytes in one page of [`Memory`].
const PAGE: usize = 256;

impl Memory {
    /// The byte at `address`, if anything was stored there.
    #[must_use]
    pub fn get(&self, address: u64) -> Option<Term> {
        let page = self.pages.get(&(address / PAGE as u64))?;
        page[(address % PAGE as u64) as usize]
    }

    /// Stores `byte` at `address`.
    pub fn insert(&mut self, address: u64, byte: Term) {
        let page = self
            .pages
            .entry(address / PAGE as u64)
            .or_insert_with(|| Rc::new([None; PAGE]));
        Rc::make_mut(page)[(address % PAGE as u64) as usize] = Some(byte);
    }
}

/// One run's state.
#[derive(Debug, Clone)]
pub struct State {
    registers: [Term; 16],
    last: Last,
    carry: Term,
    zero: Term,
    sign: Term,
    overflow: Term,
    parity: Term,
    /// The vector registers xmm, ymm and zmm 0 to 31, each its 64 bytes from the lowest,
    /// shared with the runs a fork made until one writes it.
    vectors: Vec<Rc<[Term; VECTOR_BYTES]>>,
    /// The mask registers k0 to k7, 64 bits each.
    masks: [Term; 8],
    /// The bytes written or read so far.
    pub memory: Memory,
    /// The value last stored whole at an address, while none of its bytes has changed since:
    /// loading those bytes gives it back as it was, not rebuilt from bytes the simplifier may
    /// have rewritten.
    stored: BTreeMap<u64, Term>,
    rip: u64,
    /// The conditions the inputs meet on this run.
    pub path: Vec<Term>,
    stdin: Rc<Vec<Term>>,
    stdin_read: usize,
    /// The files in the working folder before the run, by name.
    files: Rc<BTreeMap<String, Vec<Term>>>,
    /// The files the run created or replaced, by name, with everything written to them.
    pub written: BTreeMap<String, Vec<Term>>,
    /// The files open, by descriptor.
    open: BTreeMap<u64, Open>,
    /// Bytes written to standard output, shared with the runs a fork made until one writes.
    pub stdout: Rc<Vec<Term>>,
    /// Bytes written to standard error, shared the same way.
    pub stderr: Rc<Vec<Term>>,
    steps: u64,
    /// How the run ended, once it has.
    pub end: Option<End>,
    /// The bytes a test supplies for `getrandom`, in order, when it supplies them: the kernel's
    /// are any value.
    random: Option<Rc<Vec<Term>>>,
    random_used: usize,
    /// The readings a test supplies for `clock_gettime`, seconds and nanoseconds, 64 bits each;
    /// the last one repeats. Empty, the kernel's are any time.
    clock: Rc<Vec<(Term, Term)>>,
    readings_used: usize,
}

/// A file the program has open.
#[derive(Debug, Clone)]
struct Open {
    name: String,
    /// The next byte a read gets.
    position: usize,
    writing: bool,
}

/// `openat`'s directory meaning the working folder, as the kernel reads it: the low 32 bits,
/// an `int`. The flags lengua models.
const AT_FDCWD: u64 = 0xffff_ff9c;
const O_ACCMODE: u64 = 3;
const O_WRONLY: u64 = 1;
const O_CREAT: u64 = 0o100;
const O_TRUNC: u64 = 0o1000;
/// Flags that change nothing the model tracks: no controlling terminal, large files, closing on
/// exec.
const O_HARMLESS: u64 = 0o400 | 0o100_000 | 0o2_000_000;
const ENOENT: i64 = 2;
const EBADF: i64 = 9;
const EINVAL: i64 = 22;

/// The general-purpose register a register is part of, 0 for rax to 15 for r15.
fn slot(register: Register) -> Option<usize> {
    let full = register.full_register();
    let index = (full as usize).checked_sub(Register::RAX as usize)?;
    (index < 16).then_some(index)
}

/// Whether `register` is ah, bh, ch or dh.
fn high_byte(register: Register) -> bool {
    matches!(
        register,
        Register::AH | Register::BH | Register::CH | Register::DH
    )
}

impl State {
    /// The state `_start` begins in: `args` and `stdin` as byte terms, laid out as Linux does.
    pub fn start(image: &Image, pool: &mut Pool, args: &[Vec<Term>], stdin: Vec<Term>) -> Self {
        let zero = pool.bv(64, 0);
        let zero_byte = pool.bv(8, 0);
        let no = pool.boolean(false);
        let mut state = Self {
            registers: [zero; 16],
            last: Last::Other,
            carry: no,
            zero: no,
            sign: no,
            overflow: no,
            parity: no,
            // One page of zeros, shared by every register until one is written.
            vectors: {
                let zeros = Rc::new([zero_byte; VECTOR_BYTES]);
                (0..32).map(|_| Rc::clone(&zeros)).collect()
            },
            masks: [zero; 8],
            memory: Memory::default(),
            stored: BTreeMap::new(),
            rip: image.entry,
            path: Vec::new(),
            stdin: Rc::new(stdin),
            stdin_read: 0,
            files: Rc::new(BTreeMap::new()),
            written: BTreeMap::new(),
            open: BTreeMap::new(),
            stdout: Rc::new(Vec::new()),
            stderr: Rc::new(Vec::new()),
            steps: 0,
            end: None,
            random: None,
            random_used: 0,
            clock: Rc::new(Vec::new()),
            readings_used: 0,
        };
        // The strings above the pointers: the program's path, then each argument.
        let program_name: Vec<Term> = b"/lengua/main"
            .iter()
            .map(|byte| pool.bv(8, u128::from(*byte)))
            .collect();
        let mut strings = Vec::new();
        let mut cursor = STACK_TOP + 4096;
        for text in std::iter::once(&program_name).chain(args) {
            strings.push(cursor);
            for byte in text {
                state.memory.insert(cursor, *byte);
                cursor += 1;
            }
            let nul = pool.bv(8, 0);
            state.memory.insert(cursor, nul);
            cursor += 1;
        }
        // argc, then argv's pointers and a null, then an empty environment's null.
        let mut words = vec![strings.len() as u64];
        words.extend(&strings);
        words.extend([0, 0]);
        for (index, word) in words.iter().enumerate() {
            let value = pool.bv(64, u128::from(*word));
            state.store(pool, STACK_TOP + 8 * index as u64, value);
        }
        state.registers[4] = pool.bv(64, u128::from(STACK_TOP));
        state
    }

    /// The same state with `files` in its working folder.
    #[must_use]
    pub fn with_files(mut self, files: BTreeMap<String, Vec<Term>>) -> Self {
        self.files = Rc::new(files);
        self
    }

    /// The same state with a test's random bytes and clock readings in place of the kernel's.
    #[must_use]
    pub fn with_supply(mut self, random: Option<Vec<Term>>, clock: Vec<(Term, Term)>) -> Self {
        self.random = random.map(Rc::new);
        self.clock = Rc::new(clock);
        self
    }

    /// What the file `name` holds after the run, if it exists: what the run wrote, or else what
    /// it held before.
    #[must_use]
    pub fn file(&self, name: &str) -> Option<&[Term]> {
        self.written
            .get(name)
            .or_else(|| self.files.get(name))
            .map(Vec::as_slice)
    }

    /// The files the working folder held before the run.
    #[must_use]
    pub fn files_before(&self) -> &BTreeMap<String, Vec<Term>> {
        &self.files
    }

    /// Writes `value` little-endian at `address`, which must be known to be writable.
    fn store(&mut self, pool: &mut Pool, address: u64, value: Term) {
        let bytes = pool.width(value) / 8;
        // A whole value any of these bytes belonged to is gone; values are at most 8 bytes.
        let end = address + u64::from(bytes);
        let overlapping: Vec<u64> = self
            .stored
            .range(address.saturating_sub(7)..end)
            .filter(|(start, old)| **start + u64::from(pool.width(**old) / 8) > address)
            .map(|(start, _)| *start)
            .collect();
        for start in overlapping {
            self.stored.remove(&start);
        }
        if bytes > 1 {
            self.stored.insert(address, value);
        }
        for byte in 0..bytes {
            let part = pool.extract(value, byte * 8 + 7, byte * 8);
            self.memory.insert(address + u64::from(byte), part);
        }
    }

    fn register(&self, pool: &mut Pool, register: Register) -> Option<Term> {
        let full = self.registers[slot(register)?];
        Some(match (register.size(), high_byte(register)) {
            (8, _) => full,
            (4, _) => pool.extract(full, 31, 0),
            (2, _) => pool.extract(full, 15, 0),
            (1, true) => pool.extract(full, 15, 8),
            _ => pool.extract(full, 7, 0),
        })
    }

    fn set_register(&mut self, pool: &mut Pool, register: Register, value: Term) -> Option<()> {
        let index = slot(register)?;
        let old = self.registers[index];
        self.registers[index] = match (register.size(), high_byte(register)) {
            (8, _) => value,
            (4, _) => pool.zero_extend(value, 32),
            (2, _) => {
                let upper = pool.extract(old, 63, 16);
                pool.concat(upper, value)
            }
            (1, true) => {
                let upper = pool.extract(old, 63, 16);
                let lower = pool.extract(old, 7, 0);
                let middle = pool.concat(value, lower);
                pool.concat(upper, middle)
            }
            _ => {
                let upper = pool.extract(old, 63, 8);
                pool.concat(upper, value)
            }
        };
        Some(())
    }
}

/// Why a step stopped the run, or the condition it split on.
enum Step {
    /// Go on with the state as changed.
    Next,
    /// Split: the state continues where `condition` holds, and `other` where it doesn't.
    Split(Term, Box<State>),
}

type Stop = End;

fn unsupported(text: impl Into<String>) -> Stop {
    End::Unsupported(text.into())
}

/// Runs one step of `state`.
fn step(image: &Image, state: &mut State, pool: &mut Pool) -> Result<Step, Stop> {
    let Some(instruction) = image.instructions.get(&state.rip).copied() else {
        return Err(End::Signal("SIGSEGV"));
    };
    state.steps += 1;
    if state.steps > MAX_STEPS {
        return Err(unsupported("a run took too many instructions to follow"));
    }
    let next = instruction.next_ip();
    state.rip = next;
    let mut cpu = Cpu::new(image, state, pool, &instruction);
    cpu.execute()
}

/// One instruction being executed on a state.
struct Cpu<'a> {
    image: &'a Image,
    state: &'a mut State,
    pool: &'a mut Pool,
    instruction: &'a Instruction,
}

impl<'a> Cpu<'a> {
    fn new(
        image: &'a Image,
        state: &'a mut State,
        pool: &'a mut Pool,
        instruction: &'a Instruction,
    ) -> Self {
        Self {
            image,
            state,
            pool,
            instruction,
        }
    }

    fn constant(&self, term: Term) -> Result<u64, Stop> {
        self.pool
            .constant(term)
            .and_then(|value| u64::try_from(value).ok())
            .ok_or_else(|| unsupported("an address or count that depends on the input"))
    }

    /// The address of the instruction's memory operand.
    fn address(&mut self) -> Result<u64, Stop> {
        let instruction = self.instruction;
        if instruction.is_ip_rel_memory_operand() {
            return Ok(instruction.ip_rel_memory_address());
        }
        let mut address = instruction.memory_displacement64();
        if instruction.memory_base() != Register::None {
            let base = self
                .state
                .register(self.pool, instruction.memory_base())
                .ok_or_else(|| unsupported("an unusual base register"))?;
            address = address.wrapping_add(self.constant(base)?);
        }
        if instruction.memory_index() != Register::None {
            let index = self
                .state
                .register(self.pool, instruction.memory_index())
                .ok_or_else(|| unsupported("an unusual index register"))?;
            let scale = u64::from(instruction.memory_index_scale());
            address = address.wrapping_add(self.constant(index)?.wrapping_mul(scale));
        }
        Ok(address)
    }

    fn load(&mut self, address: u64, bytes: u32) -> Result<Term, Stop> {
        if let Some(whole) = self.state.stored.get(&address).copied()
            && self.pool.width(whole) == bytes * 8
            && self.image.region(address).is_some()
            && self.image.region(address + u64::from(bytes) - 1).is_some()
        {
            return Ok(whole);
        }
        let mut value: Option<Term> = None;
        for offset in 0..u64::from(bytes) {
            let at = address.wrapping_add(offset);
            if self.image.region(at).is_none() {
                return Err(End::Signal("SIGSEGV"));
            }
            let byte = match self.state.memory.get(at) {
                Some(byte) => byte,
                None => match self.image.initial.get(&at) {
                    Some(byte) => self.pool.bv(8, u128::from(*byte)),
                    None if (STACK_TOP - STACK_SIZE..STACK_TOP - 8).contains(&at) => {
                        // Stack the program never wrote: whatever was there.
                        let unknown = self.pool.var(8, &format!("stack{at:x}"));
                        self.state.memory.insert(at, unknown);
                        unknown
                    }
                    // Zero-filled data (.bss) and the arguments' padding, as the kernel
                    // leaves them.
                    None => self.pool.bv(8, 0),
                },
            };
            value = Some(match value {
                None => byte,
                Some(low) => self.pool.concat(byte, low),
            });
        }
        value.ok_or_else(|| unsupported("an empty memory access"))
    }

    fn save(&mut self, address: u64, value: Term) -> Result<(), Stop> {
        let bytes = self.pool.width(value) / 8;
        for offset in 0..u64::from(bytes) {
            match self.image.region(address.wrapping_add(offset)) {
                Some(region) if region.writable => {}
                _ => return Err(End::Signal("SIGSEGV")),
            }
        }
        self.state.store(self.pool, address, value);
        Ok(())
    }

    /// The width in bits of operand `index`.
    fn width(&self, index: u32) -> u32 {
        let instruction = self.instruction;
        match instruction.op_kind(index) {
            OpKind::Register => {
                u32::try_from(instruction.op_register(index).size() * 8).unwrap_or(64)
            }
            OpKind::Memory => u32::try_from(instruction.memory_size().size() * 8).unwrap_or(64),
            OpKind::Immediate8 => 8,
            OpKind::Immediate16 => 16,
            OpKind::Immediate32 => 32,
            _ => 64,
        }
    }

    /// Operand `index`, as a term `width` bits wide (immediates are sign-extended to it).
    fn read(&mut self, index: u32, width: u32) -> Result<Term, Stop> {
        let instruction = self.instruction;
        match instruction.op_kind(index) {
            OpKind::Register => self
                .state
                .register(self.pool, instruction.op_register(index))
                .ok_or_else(|| {
                    unsupported(format!("register {:?}", instruction.op_register(index)))
                }),
            OpKind::Memory => {
                let address = self.address()?;
                self.load(address, self.width(index) / 8)
            }
            OpKind::Immediate8
            | OpKind::Immediate16
            | OpKind::Immediate32
            | OpKind::Immediate64
            | OpKind::Immediate8to16
            | OpKind::Immediate8to32
            | OpKind::Immediate8to64
            | OpKind::Immediate32to64 => Ok(self
                .pool
                .bv(width, u128::from(instruction.immediate(index)))),
            other => Err(unsupported(format!("an operand of kind {other:?}"))),
        }
    }

    fn write(&mut self, index: u32, value: Term) -> Result<(), Stop> {
        let instruction = self.instruction;
        match instruction.op_kind(index) {
            OpKind::Register => self
                .state
                .set_register(self.pool, instruction.op_register(index), value)
                .ok_or_else(|| unsupported("writing an unusual register")),
            OpKind::Memory => {
                let address = self.address()?;
                self.save(address, value)
            }
            _ => Err(unsupported(
                "writing an operand that isn't a register or memory",
            )),
        }
    }

    fn bit(&mut self, value: Term, bit: u32) -> Term {
        let part = self.pool.extract(value, bit, bit);
        let one = self.pool.bv(1, 1);
        self.pool.eq(part, one)
    }

    fn unknown(&mut self) -> Term {
        let name = format!("undefined{}", self.pool.len());
        self.pool.bool_var(&name)
    }

    /// Sets ZF, SF and PF from a result.
    fn result_flags(&mut self, result: Term) {
        self.state.last = Last::Other;
        let width = self.pool.width(result);
        let zero = self.pool.bv(width, 0);
        self.state.zero = self.pool.eq(result, zero);
        self.state.sign = self.bit(result, width - 1);
        let low = self.pool.extract(result, 7, 0);
        let mut folded = low;
        for shift in [4_u128, 2, 1] {
            let amount = self.pool.bv(8, shift);
            let shifted = self.pool.lshr(folded, amount);
            folded = self.pool.xor(folded, shifted);
        }
        let odd = self.bit(folded, 0);
        self.state.parity = self.pool.bool_not(odd);
    }

    fn condition(&mut self, code: ConditionCode) -> Result<Term, Stop> {
        // After a comparison or a logic operation, a branch is a plain comparison of values,
        // which value ranges can often settle without the solver.
        let pool = &mut *self.pool;
        match self.state.last {
            Last::Compare(a, b) => {
                let found = match code {
                    ConditionCode::e => Some(pool.eq(a, b)),
                    ConditionCode::ne => {
                        let same = pool.eq(a, b);
                        Some(pool.bool_not(same))
                    }
                    ConditionCode::b => Some(pool.ult(a, b)),
                    ConditionCode::ae => {
                        let below = pool.ult(a, b);
                        Some(pool.bool_not(below))
                    }
                    ConditionCode::be => Some(pool.ule(a, b)),
                    ConditionCode::a => Some(pool.ult(b, a)),
                    ConditionCode::l => Some(pool.slt(a, b)),
                    ConditionCode::ge => {
                        let less = pool.slt(a, b);
                        Some(pool.bool_not(less))
                    }
                    ConditionCode::le => Some(pool.sle(a, b)),
                    ConditionCode::g => Some(pool.slt(b, a)),
                    _ => None,
                };
                if let Some(found) = found {
                    return Ok(found);
                }
            }
            Last::Logic(result) => {
                let width = pool.width(result);
                let zero = pool.bv(width, 0);
                let is_zero = pool.eq(result, zero);
                let negative = pool.slt(result, zero);
                let found = match code {
                    ConditionCode::e | ConditionCode::be => Some(is_zero),
                    ConditionCode::ne | ConditionCode::a => Some(pool.bool_not(is_zero)),
                    ConditionCode::s | ConditionCode::l => Some(negative),
                    ConditionCode::ns | ConditionCode::ge => Some(pool.bool_not(negative)),
                    ConditionCode::le => Some(pool.sle(result, zero)),
                    ConditionCode::g => Some(pool.slt(zero, result)),
                    ConditionCode::b | ConditionCode::o => Some(pool.boolean(false)),
                    ConditionCode::ae | ConditionCode::no => Some(pool.boolean(true)),
                    _ => None,
                };
                if let Some(found) = found {
                    return Ok(found);
                }
            }
            Last::Other => {}
        }
        let state = &*self.state;
        let (carry, zero, sign, overflow, parity) = (
            state.carry,
            state.zero,
            state.sign,
            state.overflow,
            state.parity,
        );
        let pool = &mut *self.pool;
        let less = {
            let same = pool.eq(sign, overflow);
            pool.bool_not(same)
        };
        Ok(match code {
            ConditionCode::o => overflow,
            ConditionCode::no => pool.bool_not(overflow),
            ConditionCode::b => carry,
            ConditionCode::ae => pool.bool_not(carry),
            ConditionCode::e => zero,
            ConditionCode::ne => pool.bool_not(zero),
            ConditionCode::be => pool.bool_or(carry, zero),
            ConditionCode::a => {
                let either = pool.bool_or(carry, zero);
                pool.bool_not(either)
            }
            ConditionCode::s => sign,
            ConditionCode::ns => pool.bool_not(sign),
            ConditionCode::p => parity,
            ConditionCode::np => pool.bool_not(parity),
            ConditionCode::l => less,
            ConditionCode::ge => pool.bool_not(less),
            ConditionCode::le => pool.bool_or(zero, less),
            ConditionCode::g => {
                let either = pool.bool_or(zero, less);
                pool.bool_not(either)
            }
            ConditionCode::None => return Err(unsupported("a branch without a condition")),
        })
    }

    fn push(&mut self, value: Term) -> Result<(), Stop> {
        let rsp = self.state.registers[4];
        let eight = self.pool.bv(64, 8);
        let lower = self.pool.sub(rsp, eight);
        let address = self.constant(lower)?;
        self.save(address, value)?;
        self.state.registers[4] = lower;
        Ok(())
    }

    fn pop(&mut self) -> Result<Term, Stop> {
        let rsp = self.state.registers[4];
        let address = self.constant(rsp)?;
        let value = self.load(address, 8)?;
        let eight = self.pool.bv(64, 8);
        self.state.registers[4] = self.pool.add(rsp, eight);
        Ok(value)
    }

    fn execute(&mut self) -> Result<Step, Stop> {
        let instruction = self.instruction;
        let mnemonic = instruction.mnemonic();
        match mnemonic {
            Mnemonic::Nop => Ok(Step::Next),
            Mnemonic::Mov => {
                let width = self.width(0);
                let value = self.read(1, width)?;
                self.write(0, value)?;
                Ok(Step::Next)
            }
            Mnemonic::Movzx | Mnemonic::Movsx | Mnemonic::Movsxd => {
                let (to, from) = (self.width(0), self.width(1));
                let value = self.read(1, from)?;
                let value = if mnemonic == Mnemonic::Movzx {
                    self.pool.zero_extend(value, to - from)
                } else {
                    self.pool.sign_extend(value, to - from)
                };
                self.write(0, value)?;
                Ok(Step::Next)
            }
            Mnemonic::Lea => {
                let value = self.effective_address()?;
                let width = self.width(0);
                let value = self.pool.extract(value, width - 1, 0);
                self.write(0, value)?;
                Ok(Step::Next)
            }
            Mnemonic::Push => {
                let value = self.read(0, 64)?;
                let value = if self.pool.width(value) < 64 {
                    let width = self.pool.width(value);
                    self.pool.sign_extend(value, 64 - width)
                } else {
                    value
                };
                self.push(value)?;
                Ok(Step::Next)
            }
            Mnemonic::Pop => {
                let value = self.pop()?;
                self.write(0, value)?;
                Ok(Step::Next)
            }
            Mnemonic::Xchg => {
                let width = self.width(0);
                let (a, b) = (self.read(0, width)?, self.read(1, width)?);
                self.write(0, b)?;
                self.write(1, a)?;
                Ok(Step::Next)
            }
            Mnemonic::Bswap | Mnemonic::Movbe => {
                let width = self.width(0);
                // movbe reads its second operand, bswap its only one.
                let from = u32::from(mnemonic == Mnemonic::Movbe);
                let value = self.read(from, width)?;
                let swapped = self.swap_bytes(value)?;
                self.write(0, swapped)?;
                Ok(Step::Next)
            }
            Mnemonic::Add | Mnemonic::Adc | Mnemonic::Sub | Mnemonic::Sbb | Mnemonic::Cmp => {
                self.arithmetic(mnemonic)
            }
            Mnemonic::Inc | Mnemonic::Dec => {
                let width = self.width(0);
                let a = self.read(0, width)?;
                let one = self.pool.bv(width, 1);
                let result = if mnemonic == Mnemonic::Inc {
                    self.pool.add(a, one)
                } else {
                    self.pool.sub(a, one)
                };
                let edge = if mnemonic == Mnemonic::Inc {
                    self.pool.bv(width, (1_u128 << (width - 1)) - 1)
                } else {
                    self.pool.bv(width, 1_u128 << (width - 1))
                };
                self.state.overflow = self.pool.eq(a, edge);
                self.result_flags(result);
                self.write(0, result)?;
                Ok(Step::Next)
            }
            Mnemonic::Neg => {
                let width = self.width(0);
                let a = self.read(0, width)?;
                let result = self.pool.neg(a);
                let zero = self.pool.bv(width, 0);
                let is_zero = self.pool.eq(a, zero);
                self.state.carry = self.pool.bool_not(is_zero);
                let minimum = self.pool.bv(width, 1_u128 << (width - 1));
                self.state.overflow = self.pool.eq(a, minimum);
                self.result_flags(result);
                self.write(0, result)?;
                Ok(Step::Next)
            }
            Mnemonic::And | Mnemonic::Or | Mnemonic::Xor | Mnemonic::Test => {
                let width = self.width(0);
                let (a, b) = (self.read(0, width)?, self.read(1, width)?);
                let result = match mnemonic {
                    Mnemonic::Or => self.pool.or(a, b),
                    Mnemonic::Xor => self.pool.xor(a, b),
                    _ => self.pool.and(a, b),
                };
                let no = self.pool.boolean(false);
                self.state.carry = no;
                self.state.overflow = no;
                self.result_flags(result);
                self.state.last = Last::Logic(result);
                if mnemonic != Mnemonic::Test {
                    self.write(0, result)?;
                }
                Ok(Step::Next)
            }
            Mnemonic::Not => {
                let width = self.width(0);
                let a = self.read(0, width)?;
                let result = self.pool.not(a);
                self.write(0, result)?;
                Ok(Step::Next)
            }
            Mnemonic::Shl | Mnemonic::Shr | Mnemonic::Sar | Mnemonic::Rol | Mnemonic::Ror => {
                self.shift(mnemonic)
            }
            Mnemonic::Imul if instruction.op_count() >= 2 => self.imul_truncated(),
            Mnemonic::Mul | Mnemonic::Imul => self.multiply(mnemonic == Mnemonic::Imul),
            Mnemonic::Div | Mnemonic::Idiv => self.divide(mnemonic == Mnemonic::Idiv),
            Mnemonic::Cbw | Mnemonic::Cwde | Mnemonic::Cdqe => {
                let (from, register) = match mnemonic {
                    Mnemonic::Cbw => (Register::AL, Register::AX),
                    Mnemonic::Cwde => (Register::AX, Register::EAX),
                    _ => (Register::EAX, Register::RAX),
                };
                let value = self
                    .state
                    .register(self.pool, from)
                    .ok_or_else(|| unsupported("cbw"))?;
                let width = self.pool.width(value);
                let wide = self.pool.sign_extend(value, width);
                self.state.set_register(self.pool, register, wide);
                Ok(Step::Next)
            }
            Mnemonic::Cwd | Mnemonic::Cdq | Mnemonic::Cqo => {
                let (from, to) = match mnemonic {
                    Mnemonic::Cwd => (Register::AX, Register::DX),
                    Mnemonic::Cdq => (Register::EAX, Register::EDX),
                    _ => (Register::RAX, Register::RDX),
                };
                let value = self
                    .state
                    .register(self.pool, from)
                    .ok_or_else(|| unsupported("cqo"))?;
                let width = self.pool.width(value);
                let amount = self.pool.bv(width, u128::from(width - 1));
                let fill = self.pool.ashr(value, amount);
                self.state.set_register(self.pool, to, fill);
                Ok(Step::Next)
            }
            Mnemonic::Cmovo
            | Mnemonic::Cmovno
            | Mnemonic::Cmovb
            | Mnemonic::Cmovae
            | Mnemonic::Cmove
            | Mnemonic::Cmovne
            | Mnemonic::Cmovbe
            | Mnemonic::Cmova
            | Mnemonic::Cmovs
            | Mnemonic::Cmovns
            | Mnemonic::Cmovp
            | Mnemonic::Cmovnp
            | Mnemonic::Cmovl
            | Mnemonic::Cmovge
            | Mnemonic::Cmovle
            | Mnemonic::Cmovg => {
                let width = self.width(0);
                let condition = self.condition(instruction.condition_code())?;
                // The source is read, and a 32-bit destination cleared above, either way.
                let source = self.read(1, width)?;
                let current = self.read(0, width)?;
                let value = self.pool.ite(condition, source, current);
                self.write(0, value)?;
                Ok(Step::Next)
            }
            Mnemonic::Seto
            | Mnemonic::Setno
            | Mnemonic::Setb
            | Mnemonic::Setae
            | Mnemonic::Sete
            | Mnemonic::Setne
            | Mnemonic::Setbe
            | Mnemonic::Seta
            | Mnemonic::Sets
            | Mnemonic::Setns
            | Mnemonic::Setp
            | Mnemonic::Setnp
            | Mnemonic::Setl
            | Mnemonic::Setge
            | Mnemonic::Setle
            | Mnemonic::Setg => {
                let condition = self.condition(instruction.condition_code())?;
                let (one, zero) = (self.pool.bv(8, 1), self.pool.bv(8, 0));
                let value = self.pool.ite(condition, one, zero);
                self.write(0, value)?;
                Ok(Step::Next)
            }
            Mnemonic::Jmp => {
                self.state.rip = instruction.near_branch_target();
                Ok(Step::Next)
            }
            Mnemonic::Jo
            | Mnemonic::Jno
            | Mnemonic::Jb
            | Mnemonic::Jae
            | Mnemonic::Je
            | Mnemonic::Jne
            | Mnemonic::Jbe
            | Mnemonic::Ja
            | Mnemonic::Js
            | Mnemonic::Jns
            | Mnemonic::Jp
            | Mnemonic::Jnp
            | Mnemonic::Jl
            | Mnemonic::Jge
            | Mnemonic::Jle
            | Mnemonic::Jg => {
                let condition = self.condition(instruction.condition_code())?;
                let target = instruction.near_branch_target();
                match self.pool.truth(condition) {
                    Some(true) => {
                        self.state.rip = target;
                        Ok(Step::Next)
                    }
                    Some(false) => Ok(Step::Next),
                    None => {
                        let other = self.state.clone();
                        self.state.rip = target;
                        Ok(Step::Split(condition, Box::new(other)))
                    }
                }
            }
            Mnemonic::Call => {
                let back = self.pool.bv(64, u128::from(instruction.next_ip()));
                self.push(back)?;
                self.state.rip = instruction.near_branch_target();
                Ok(Step::Next)
            }
            Mnemonic::Ret => {
                let target = self.pop()?;
                self.state.rip = self.constant(target)?;
                Ok(Step::Next)
            }
            Mnemonic::Syscall => self.syscall(),
            Mnemonic::Popcnt => self.popcnt(),
            Mnemonic::Tzcnt | Mnemonic::Lzcnt | Mnemonic::Bsf | Mnemonic::Bsr => {
                self.find_bit(mnemonic)
            }
            Mnemonic::Andn
            | Mnemonic::Blsr
            | Mnemonic::Blsi
            | Mnemonic::Blsmsk
            | Mnemonic::Bzhi
            | Mnemonic::Bextr => self.bmi(mnemonic),
            Mnemonic::Shlx | Mnemonic::Shrx | Mnemonic::Sarx | Mnemonic::Rorx => {
                self.shift_keeping_flags(mnemonic)
            }
            Mnemonic::Mulx => self.mulx(),
            Mnemonic::Adcx | Mnemonic::Adox => self.add_with_flag(mnemonic),
            Mnemonic::Bt | Mnemonic::Bts | Mnemonic::Btr | Mnemonic::Btc => self.bit_test(mnemonic),
            Mnemonic::Shld | Mnemonic::Shrd => self.double_shift(mnemonic),
            Mnemonic::Pdep | Mnemonic::Pext => self.deposit(mnemonic),
            Mnemonic::Stc | Mnemonic::Clc | Mnemonic::Cmc => {
                self.set_carry(mnemonic);
                Ok(Step::Next)
            }
            other => self.vector(other),
        }
    }

    /// The address `lea` computes, as a 64-bit term.
    fn effective_address(&mut self) -> Result<Term, Stop> {
        let instruction = self.instruction;
        if instruction.is_ip_rel_memory_operand() {
            return Ok(self
                .pool
                .bv(64, u128::from(instruction.ip_rel_memory_address())));
        }
        let mut value = self
            .pool
            .bv(64, u128::from(instruction.memory_displacement64()));
        if instruction.memory_base() != Register::None {
            let base = self
                .state
                .register(self.pool, instruction.memory_base())
                .ok_or_else(|| unsupported("lea with an unusual base"))?;
            let base = self.widen(base);
            value = self.pool.add(base, value);
        }
        if instruction.memory_index() != Register::None {
            let index = self
                .state
                .register(self.pool, instruction.memory_index())
                .ok_or_else(|| unsupported("lea with an unusual index"))?;
            let index = self.widen(index);
            let scale = self
                .pool
                .bv(64, u128::from(instruction.memory_index_scale()));
            let scaled = self.pool.mul(index, scale);
            value = self.pool.add(value, scaled);
        }
        Ok(value)
    }

    fn widen(&mut self, value: Term) -> Term {
        let width = self.pool.width(value);
        self.pool.zero_extend(value, 64 - width)
    }

    fn arithmetic(&mut self, mnemonic: Mnemonic) -> Result<Step, Stop> {
        let width = self.width(0);
        let a = self.read(0, width)?;
        let b = self.read(1, width)?;
        let with_carry = matches!(mnemonic, Mnemonic::Adc | Mnemonic::Sbb);
        let carry_in = if with_carry {
            let (one, zero) = (self.pool.bv(width + 1, 1), self.pool.bv(width + 1, 0));
            self.pool.ite(self.state.carry, one, zero)
        } else {
            self.pool.bv(width + 1, 0)
        };
        let adding = matches!(mnemonic, Mnemonic::Add | Mnemonic::Adc);
        let (wide_a, wide_b) = (self.pool.zero_extend(a, 1), self.pool.zero_extend(b, 1));
        let wide = if adding {
            let sum = self.pool.add(wide_a, wide_b);
            self.pool.add(sum, carry_in)
        } else {
            let difference = self.pool.sub(wide_a, wide_b);
            self.pool.sub(difference, carry_in)
        };
        let result = self.pool.extract(wide, width - 1, 0);
        self.state.carry = self.bit(wide, width);
        let (sign_a, sign_b, sign_result) = (
            self.bit(a, width - 1),
            self.bit(b, width - 1),
            self.bit(result, width - 1),
        );
        let operands_agree = self.pool.eq(sign_a, sign_b);
        let operands_differ = self.pool.bool_not(operands_agree);
        let result_kept = self.pool.eq(sign_result, sign_a);
        let result_flipped = self.pool.bool_not(result_kept);
        self.state.overflow = if adding {
            self.pool.bool_and(operands_agree, result_flipped)
        } else {
            self.pool.bool_and(operands_differ, result_flipped)
        };
        if mnemonic == Mnemonic::Add {
            // Carry out of a plain addition is a result below what was added to; with a carry
            // in, the result can equal it, so adc keeps the bit computed above.
            self.state.carry = self.pool.ult(result, a);
        } else if !with_carry {
            self.state.carry = self.pool.ult(a, b);
        }
        self.result_flags(result);
        self.state.last = if matches!(mnemonic, Mnemonic::Sub | Mnemonic::Cmp) {
            Last::Compare(a, b)
        } else {
            Last::Other
        };
        if mnemonic != Mnemonic::Cmp {
            self.write(0, result)?;
        }
        Ok(Step::Next)
    }

    fn shift(&mut self, mnemonic: Mnemonic) -> Result<Step, Stop> {
        let width = self.width(0);
        let a = self.read(0, width)?;
        let count = self.read(1, 8)?;
        let mask = if width == 64 { 63 } else { 31 };
        let count = u32::try_from(self.constant(count)? & mask).unwrap_or(0);
        if count == 0 {
            // The flags stay, but the destination is still written: a 32-bit register's upper
            // half clears.
            self.write(0, a)?;
            return Ok(Step::Next);
        }
        let amount = self.pool.bv(width, u128::from(count));
        let result = match mnemonic {
            Mnemonic::Shl => self.pool.shl(a, amount),
            Mnemonic::Shr => self.pool.lshr(a, amount),
            Mnemonic::Sar => self.pool.ashr(a, amount),
            _ => {
                let rotate = count % width;
                let back = self.pool.bv(width, u128::from((width - rotate) % width));
                let rotate = self.pool.bv(width, u128::from(rotate));
                let (left, right) = if mnemonic == Mnemonic::Rol {
                    (self.pool.shl(a, rotate), self.pool.lshr(a, back))
                } else {
                    (self.pool.lshr(a, rotate), self.pool.shl(a, back))
                };
                let result = self.pool.or(left, right);
                self.state.carry = if mnemonic == Mnemonic::Rol {
                    self.bit(result, 0)
                } else {
                    self.bit(result, width - 1)
                };
                self.state.overflow = self.unknown();
                self.state.last = Last::Other;
                self.write(0, result)?;
                return Ok(Step::Next);
            }
        };
        if count <= width {
            let out = match mnemonic {
                Mnemonic::Shl => width - count,
                _ => count - 1,
            };
            self.state.carry = self.bit(a, out);
        } else {
            self.state.carry = self.unknown();
        }
        self.state.overflow = if count == 1 {
            match mnemonic {
                Mnemonic::Shl => {
                    let top = self.bit(result, width - 1);
                    let same = self.pool.eq(top, self.state.carry);
                    self.pool.bool_not(same)
                }
                Mnemonic::Shr => self.bit(a, width - 1),
                _ => self.pool.boolean(false),
            }
        } else {
            self.unknown()
        };
        self.result_flags(result);
        self.write(0, result)?;
        Ok(Step::Next)
    }

    /// `imul` with two or three operands: the low half, with CF and OF set when it doesn't hold
    /// the whole signed product.
    fn imul_truncated(&mut self) -> Result<Step, Stop> {
        let instruction = self.instruction;
        let width = self.width(0);
        let (a, b) = if instruction.op_count() == 3 {
            (self.read(1, width)?, self.read(2, width)?)
        } else {
            (self.read(0, width)?, self.read(1, width)?)
        };
        let (wide_a, wide_b) = (
            self.pool.sign_extend(a, width),
            self.pool.sign_extend(b, width),
        );
        let product = self.pool.mul(wide_a, wide_b);
        let result = self.pool.extract(product, width - 1, 0);
        let refit = self.pool.sign_extend(result, width);
        let fits = self.pool.eq(refit, product);
        let overflow = self.pool.bool_not(fits);
        self.state.last = Last::Other;
        self.state.carry = overflow;
        self.state.overflow = overflow;
        self.state.zero = self.unknown();
        self.state.sign = self.unknown();
        self.state.parity = self.unknown();
        self.write(0, result)?;
        Ok(Step::Next)
    }

    fn halves(&self, width: u32) -> (Register, Register) {
        match width {
            8 => (Register::AL, Register::AH),
            16 => (Register::AX, Register::DX),
            32 => (Register::EAX, Register::EDX),
            _ => (Register::RAX, Register::RDX),
        }
    }

    /// One-operand `mul` and `imul`: the double-width product in rdx:rax (ah:al for bytes).
    fn multiply(&mut self, signed: bool) -> Result<Step, Stop> {
        let width = self.width(0);
        let source = self.read(0, width)?;
        let (low_register, high_register) = self.halves(width);
        let a = self
            .state
            .register(self.pool, low_register)
            .ok_or_else(|| unsupported("mul"))?;
        let (wide_a, wide_b) = if signed {
            (
                self.pool.sign_extend(a, width),
                self.pool.sign_extend(source, width),
            )
        } else {
            (
                self.pool.zero_extend(a, width),
                self.pool.zero_extend(source, width),
            )
        };
        let product = self.pool.mul(wide_a, wide_b);
        let low = self.pool.extract(product, width - 1, 0);
        let high = self.pool.extract(product, 2 * width - 1, width);
        let fits = if signed {
            let refit = self.pool.sign_extend(low, width);
            self.pool.eq(refit, product)
        } else {
            let zero = self.pool.bv(width, 0);
            self.pool.eq(high, zero)
        };
        let overflow = self.pool.bool_not(fits);
        self.state.last = Last::Other;
        self.state.carry = overflow;
        self.state.overflow = overflow;
        self.state.zero = self.unknown();
        self.state.sign = self.unknown();
        self.state.parity = self.unknown();
        if width == 8 {
            self.state.set_register(self.pool, Register::AX, product);
        } else {
            self.state.set_register(self.pool, low_register, low);
            self.state.set_register(self.pool, high_register, high);
        }
        Ok(Step::Next)
    }

    /// `div` and `idiv`: rdx:rax by the operand, faulting on zero or a quotient too big.
    fn divide(&mut self, signed: bool) -> Result<Step, Stop> {
        let width = self.width(0);
        let divisor = self.read(0, width)?;
        let (low_register, high_register) = self.halves(width);
        let dividend = if width == 8 {
            self.state
                .register(self.pool, Register::AX)
                .ok_or_else(|| unsupported("div"))?
        } else {
            let low = self
                .state
                .register(self.pool, low_register)
                .ok_or_else(|| unsupported("div"))?;
            let high = self
                .state
                .register(self.pool, high_register)
                .ok_or_else(|| unsupported("div"))?;
            self.pool.concat(high, low)
        };
        let wide_divisor = if signed {
            self.pool.sign_extend(divisor, width)
        } else {
            self.pool.zero_extend(divisor, width)
        };
        let (quotient, remainder) = if signed {
            (
                self.pool.sdiv(dividend, wide_divisor),
                self.pool.srem(dividend, wide_divisor),
            )
        } else {
            (
                self.pool.udiv(dividend, wide_divisor),
                self.pool.urem(dividend, wide_divisor),
            )
        };
        let low_quotient = self.pool.extract(quotient, width - 1, 0);
        let zero = self.pool.bv(width, 0);
        let by_zero = self.pool.eq(divisor, zero);
        let too_big = if signed {
            let refit = self.pool.sign_extend(low_quotient, width);
            let fits = self.pool.eq(refit, quotient);
            self.pool.bool_not(fits)
        } else {
            let high = self.pool.extract(quotient, 2 * width - 1, width);
            let fits = self.pool.eq(high, zero);
            self.pool.bool_not(fits)
        };
        let fault = self.pool.bool_or(by_zero, too_big);
        let low_remainder = self.pool.extract(remainder, width - 1, 0);
        self.state.last = Last::Other;
        for flag in 0..5 {
            let unknown = self.unknown();
            match flag {
                0 => self.state.carry = unknown,
                1 => self.state.zero = unknown,
                2 => self.state.sign = unknown,
                3 => self.state.overflow = unknown,
                _ => self.state.parity = unknown,
            }
        }
        let mut faulted = self.state.clone();
        faulted.end = Some(End::Signal("SIGFPE"));
        if width == 8 {
            let both = self.pool.concat(low_remainder, low_quotient);
            self.state.set_register(self.pool, Register::AX, both);
        } else {
            self.state
                .set_register(self.pool, low_register, low_quotient);
            self.state
                .set_register(self.pool, high_register, low_remainder);
        }
        match self.pool.truth(fault) {
            Some(false) => Ok(Step::Next),
            Some(true) => Err(End::Signal("SIGFPE")),
            None => {
                // The run splits: where it doesn't fault it goes on, where it does it ends.
                let no_fault = self.pool.bool_not(fault);
                Ok(Step::Split(no_fault, Box::new(faulted)))
            }
        }
    }

    /// `openat` on the test's files: a file the working folder holds opens for reading; any
    /// name opens for writing, created or emptied. A name the folder doesn't hold can't be read.
    fn open_at(&mut self) -> Result<u64, Stop> {
        let registers = self.state.registers;
        let (folder, path, flags) = (registers[7], registers[6], registers[2]);
        // The kernel reads the folder and the flags as ints, and file descriptors as unsigned ints.
        if self.constant(folder)? & 0xffff_ffff != AT_FDCWD {
            return Err(unsupported(
                "opening a file other than in the working folder",
            ));
        }
        let mut address = self.constant(path)?;
        let mut name = Vec::new();
        loop {
            let byte = self.load(address, 1)?;
            let byte = self
                .pool
                .constant(byte)
                .ok_or_else(|| unsupported("a file name chosen by the input"))?;
            if byte == 0 {
                break;
            }
            name.push(u8::try_from(byte).unwrap_or(0));
            if name.len() > 4096 {
                return Err(unsupported("a file name longer than 4096 bytes"));
            }
            address += 1;
        }
        let name =
            String::from_utf8(name).map_err(|_| unsupported("a file name that isn't UTF-8"))?;
        // Only the test's folder is modeled: no absolute paths and no way out of it.
        if name.starts_with('/') || name.split('/').any(|part| part == "..") || name.is_empty() {
            return Err(unsupported(format!(
                "opening {name:?}, outside the working folder"
            )));
        }
        let flags = self.constant(flags)? & 0xffff_ffff;
        let writing = match flags & O_ACCMODE {
            0 if flags & !O_ACCMODE & !O_HARMLESS == 0 => false,
            O_WRONLY if flags & !O_ACCMODE & !O_HARMLESS & !O_TRUNC == O_CREAT => true,
            _ => return Err(unsupported(format!("opening a file with flags {flags:#o}"))),
        };
        if writing {
            let exists =
                self.state.files.contains_key(&name) || self.state.written.contains_key(&name);
            if exists && flags & O_TRUNC == 0 {
                return Err(unsupported("writing into a file without emptying it first"));
            }
            self.state.written.insert(name.clone(), Vec::new());
        } else if self.state.written.contains_key(&name) {
            return Err(unsupported("reading a file the program wrote"));
        } else if !self.state.files.contains_key(&name) {
            return Ok((-ENOENT).cast_unsigned());
        }
        let fd = (3..)
            .find(|fd| !self.state.open.contains_key(fd))
            .unwrap_or(3);
        self.state.open.insert(
            fd,
            Open {
                name,
                position: 0,
                writing,
            },
        );
        Ok(fd)
    }

    fn syscall(&mut self) -> Result<Step, Stop> {
        let number = self.state.registers[0];
        let number = self
            .pool
            .constant(number)
            .ok_or_else(|| unsupported("a system call chosen by the input"))?;
        let registers = self.state.registers;
        let (rdi, rsi, rdx) = (registers[7], registers[6], registers[2]);
        let result = match number {
            0 => {
                let fd = self.constant(rdi)? & 0xffff_ffff;
                let buffer = self.constant(rsi)?;
                let count = self.constant(rdx)?;
                if fd == 0 {
                    let available = (self.state.stdin.len() - self.state.stdin_read) as u64;
                    let taken = count.min(available);
                    for offset in 0..taken {
                        let byte = self.state.stdin[self.state.stdin_read + offset as usize];
                        self.save(buffer + offset, byte)?;
                    }
                    self.state.stdin_read += taken as usize;
                    taken
                } else if let Some(open) = self.state.open.get(&fd).cloned()
                    && !open.writing
                {
                    // A regular file gives everything asked for, up to its end.
                    let files = Rc::clone(&self.state.files);
                    let contents = files.get(&open.name).map_or(&[][..], Vec::as_slice);
                    let available = (contents.len() - open.position.min(contents.len())) as u64;
                    let taken = count.min(available);
                    for offset in 0..taken {
                        let byte = contents[open.position + offset as usize];
                        self.save(buffer + offset, byte)?;
                    }
                    if let Some(open) = self.state.open.get_mut(&fd) {
                        open.position += taken as usize;
                    }
                    taken
                } else {
                    (-EBADF).cast_unsigned()
                }
            }
            1 => {
                let fd = self.constant(rdi)? & 0xffff_ffff;
                let buffer = self.constant(rsi)?;
                let count = self.constant(rdx)?;
                // Standard output or error, a file open for writing, or nothing that can be.
                let target = match fd {
                    1 | 2 => Some(None),
                    _ => match self.state.open.get(&fd) {
                        Some(open) if open.writing => Some(Some(open.name.clone())),
                        _ => None,
                    },
                };
                match target {
                    None => (-EBADF).cast_unsigned(),
                    Some(file) => {
                        for offset in 0..count {
                            let byte = self.load(buffer + offset, 1)?;
                            match (&file, fd) {
                                (Some(name), _) => {
                                    self.state
                                        .written
                                        .entry(name.clone())
                                        .or_default()
                                        .push(byte);
                                }
                                (None, 1) => Rc::make_mut(&mut self.state.stdout).push(byte),
                                (None, _) => Rc::make_mut(&mut self.state.stderr).push(byte),
                            }
                        }
                        count
                    }
                }
            }
            3 => {
                let fd = self.constant(rdi)? & 0xffff_ffff;
                if self.state.open.remove(&fd).is_some() {
                    0
                } else {
                    (-EBADF).cast_unsigned()
                }
            }
            257 => self.open_at()?,
            // The test's readings, as the supervisor gives them, the last one repeating.
            228 if !self.state.clock.is_empty() => {
                let buffer = self.constant(rsi)?;
                let readings = Rc::clone(&self.state.clock);
                let index = self.state.readings_used.min(readings.len() - 1);
                let (seconds, nanoseconds) = readings[index];
                self.save(buffer, seconds)?;
                self.save(buffer + 8, nanoseconds)?;
                self.state.readings_used += 1;
                0
            }
            // The test's bytes, as the supervisor gives them: zeros past their end.
            318 if self.state.random.is_some() => {
                let buffer = self.constant(rdi)?;
                let count = self.constant(rsi)?;
                let flags = self.constant(rdx)?;
                if flags & !7 != 0 || flags & 6 == 6 {
                    (-EINVAL).cast_unsigned()
                } else {
                    if count > 4096 {
                        return Err(unsupported("getrandom for more than 4096 bytes at once"));
                    }
                    let bytes = self.state.random.clone().unwrap_or_default();
                    let used = self.state.random_used;
                    for offset in 0..count {
                        let index = used + usize::try_from(offset).unwrap_or(usize::MAX);
                        let byte = match bytes.get(index) {
                            Some(byte) => *byte,
                            None => self.pool.bv(8, 0),
                        };
                        self.save(buffer + offset, byte)?;
                    }
                    self.state.random_used += usize::try_from(count).unwrap_or(usize::MAX);
                    count
                }
            }
            // Values the kernel chooses: any at all, a fresh variable for each.
            228 => {
                let clock = self.constant(rdi)? & 0xffff_ffff;
                if !matches!(clock, 0 | 1) {
                    return Err(unsupported(format!("reading clock {clock}")));
                }
                let buffer = self.constant(rsi)?;
                let fresh = self.pool.len();
                let seconds =
                    self.pool
                        .var_in(64, &format!("clock{fresh}.seconds"), 0, (1 << 62) - 1);
                let nanoseconds =
                    self.pool
                        .var_in(64, &format!("clock{fresh}.nanoseconds"), 0, 999_999_999);
                self.save(buffer, seconds)?;
                self.save(buffer + 8, nanoseconds)?;
                0
            }
            318 => {
                let buffer = self.constant(rdi)?;
                let count = self.constant(rsi)?;
                if count > 256 {
                    return Err(unsupported("getrandom for more than 256 bytes at once"));
                }
                let fresh = self.pool.len();
                for offset in 0..count {
                    let byte = self.pool.var(8, &format!("random{fresh}.{offset}"));
                    self.save(buffer + offset, byte)?;
                }
                count
            }
            60 | 231 => {
                let status = self.pool.extract(rdi, 7, 0);
                return Err(End::Exit(status));
            }
            other => return Err(unsupported(format!("system call {other}"))),
        };
        self.state.registers[0] = self.pool.bv(64, u128::from(result));
        self.state.registers[1] = self.pool.bv(64, u128::from(self.instruction.next_ip()));
        self.state.registers[11] = self.pool.bv(64, 0x202);
        Ok(Step::Next)
    }
}

/// Decides whether a condition can hold for some input on a run, given the run's conditions.
pub trait Oracle {
    /// Whether `condition` and every one of `path` can hold together; true when unsure.
    ///
    /// # Errors
    ///
    /// Fails when the solver stops responding.
    fn possible(&mut self, pool: &Pool, path: &[Term], condition: Term) -> Result<bool, String>;
}

/// Every run from `start`, splitting at branches the inputs decide, at most `max_runs`.
///
/// # Errors
///
/// Fails when the oracle does.
pub fn explore(
    image: &Image,
    start: State,
    pool: &mut Pool,
    oracle: &mut dyn Oracle,
    max_runs: usize,
) -> Result<Vec<State>, String> {
    let mut pending = vec![start];
    let mut finished = Vec::new();
    while let Some(mut state) = pending.pop() {
        if finished.len() + pending.len() > max_runs {
            state.end = Some(End::TooManyPaths);
            finished.push(state);
            break;
        }
        loop {
            if state.end.is_some() {
                finished.push(state);
                break;
            }
            match step(image, &mut state, pool) {
                Ok(Step::Next) => {}
                Ok(Step::Split(condition, other)) => {
                    let mut other = *other;
                    let opposite = pool.bool_not(condition);
                    // A run's conditions can always be met together, so when one side of a
                    // branch is impossible the other is certain, and needs no question.
                    let yes = oracle.possible(pool, &state.path, condition)?;
                    let no = !yes || oracle.possible(pool, &other.path, opposite)?;
                    if no {
                        other.path.push(opposite);
                        pending.push(other);
                    }
                    if yes {
                        state.path.push(condition);
                    } else {
                        break;
                    }
                }
                Err(end) => {
                    state.end = Some(end);
                    finished.push(state);
                    break;
                }
            }
        }
    }
    Ok(finished)
}

/// An oracle for concrete runs, where no branch depends on anything unknown.
pub struct Concrete;

impl Oracle for Concrete {
    fn possible(&mut self, _: &Pool, _: &[Term], _: Term) -> Result<bool, String> {
        Err("a concrete run reached a branch that depends on something unknown".into())
    }
}

/// Bytes as constant terms.
#[must_use]
pub fn bytes(pool: &mut Pool, text: &[u8]) -> Vec<Term> {
    text.iter()
        .map(|byte| pool.bv(8, u128::from(*byte)))
        .collect()
}

mod bits;
mod vector;

#[cfg(test)]
mod against_cpu;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::Profile;
    use crate::toolchain::Toolchain;

    /// Builds `source` and loads it.
    async fn image(source: &str) -> Image {
        let toolchain = Toolchain::discover().await.expect("llvm-mc and ld.lld");
        let folder = tempfile::tempdir().expect("temporary folder");
        let (assembly, object, executable) = (
            folder.path().join("main.s"),
            folder.path().join("main.o"),
            folder.path().join("main"),
        );
        std::fs::write(&assembly, source).expect("write");
        toolchain
            .assemble(Profile::LinuxX86_64, &assembly, &object)
            .await
            .expect("assemble");
        toolchain.link(&object, &executable).await.expect("link");
        let bytes = std::fs::read(&executable).expect("read");
        let machine = crate::checker::decode::read(&bytes).expect("decode");
        Image::new(&machine).expect("an entry point")
    }

    /// Runs `image` on concrete arguments and stdin: its stdout, stderr and exit status.
    fn run(image: &Image, args: &[&str], stdin: &str) -> (String, String, Option<u128>) {
        let mut pool = Pool::default();
        let args: Vec<Vec<Term>> = args
            .iter()
            .map(|arg| bytes(&mut pool, arg.as_bytes()))
            .collect();
        let stdin = bytes(&mut pool, stdin.as_bytes());
        let start = State::start(image, &mut pool, &args, stdin);
        let mut runs = explore(image, start, &mut pool, &mut Concrete, 4).expect("concrete");
        assert_eq!(runs.len(), 1);
        let state = runs.remove(0);
        let text = |terms: &[Term]| {
            terms
                .iter()
                .map(|term| {
                    char::from(u8::try_from(pool.constant(*term).expect("a byte")).expect("a byte"))
                })
                .collect::<String>()
        };
        let status = match state.end {
            Some(End::Exit(status)) => pool.constant(status),
            other => panic!("ended with {other:?}"),
        };
        (text(&state.stdout), text(&state.stderr), status)
    }

    #[tokio::test]
    async fn adds_with_carry_like_the_cpu() {
        // 0xff..ff + 0xff..ff with a carry in: the result equals the first operand, yet it
        // carries out, so the exit status is 1.
        let source = ".intel_syntax noprefix\n.globl _start\n.text\n.type _start, @function\n\
                      _start:\n    mov rax, -1\n    mov rbx, -1\n    stc\n    adc rax, rbx\n    \
                      setb dil\n    movzx edi, dil\n    mov eax, 60\n    syscall\n\
                      .size _start, . - _start\n";
        let image = image(source).await;
        let mut pool = Pool::default();
        let start = State::start(&image, &mut pool, &[], Vec::new());
        let runs = explore(&image, start, &mut pool, &mut Concrete, 2).expect("runs");
        let status = match &runs[0].end {
            Some(End::Exit(status)) => pool.constant(*status),
            other => panic!("ended with {other:?}"),
        };
        assert_eq!(status, Some(1));
    }

    #[tokio::test]
    async fn runs_the_template_like_the_cpu() {
        let image = image(crate::checker::TEMPLATE).await;
        assert_eq!(
            run(&image, &["7"], ""),
            ("-7\n".into(), String::new(), Some(0))
        );
        assert_eq!(
            run(&image, &["-9223372036854775807"], ""),
            ("9223372036854775807\n".into(), String::new(), Some(0))
        );
        assert_eq!(
            run(&image, &[], ""),
            (String::new(), "usage: negate <n>\n".into(), Some(1))
        );
        assert_eq!(
            run(&image, &["12x"], ""),
            (String::new(), "error: not a number\n".into(), Some(2))
        );
    }
}
