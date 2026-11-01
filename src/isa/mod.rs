//! The CPUs lengua checks code for, as the facts its rules read. Each CPU's decoder turns every
//! machine instruction into an [`Insn`]: the registers it reads and writes, how control leaves
//! it, its memory operand, what it does to the stack, and which of the dialect's idioms it is.
//! The rules are written once, over these facts, with each CPU's calling convention in its
//! [`Abi`].

pub mod aarch64;
pub mod x86_64;

use std::fmt;

/// A CPU lengua checks code for.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum Isa {
    /// x86-64, in the dialect's Intel syntax.
    #[default]
    X86_64,
    /// AArch64 (ARMv8-A), in GNU syntax.
    Aarch64,
}

impl Isa {
    /// What starts a comment in its assembly: `#` for x86-64, `//` for AArch64, where `#`
    /// starts a number.
    #[must_use]
    pub const fn comment(self) -> &'static str {
        match self {
            Self::X86_64 => "#",
            Self::Aarch64 => "//",
        }
    }

    /// Its calling convention and the registers its rules name.
    #[must_use]
    pub const fn abi(self) -> &'static Abi {
        match self {
            Self::X86_64 => &x86_64::ABI,
            Self::Aarch64 => &aarch64::ABI,
        }
    }
}

/// A register, at the width an instruction names it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Reg {
    /// An x86-64 register, as iced names it.
    X86(iced_x86::Register),
    /// An AArch64 general-purpose register.
    A64(aarch64::A64Reg),
}

impl Reg {
    /// The whole register this is part of: `rax` for `eax`.
    #[must_use]
    pub fn full(self) -> Self {
        match self {
            Self::X86(register) => Self::X86(register.full_register()),
            Self::A64(register) => Self::A64(register.full()),
        }
    }

    /// How many bits wide it is as named.
    #[must_use]
    pub fn bits(self) -> usize {
        match self {
            Self::X86(register) => register.size() * 8,
            Self::A64(register) => {
                if register.wide {
                    64
                } else {
                    32
                }
            }
        }
    }

    /// Its place in a [`RegSet`], the same for every width of it: general-purpose and vector
    /// registers, and x86's mask registers. Other registers, such as flags, have none, nor does
    /// AArch64's zero register, which holds nothing.
    #[must_use]
    pub fn slot(self) -> Option<u32> {
        match self {
            Self::X86(register) => x86_64::slot(register),
            Self::A64(register) => aarch64::slot(register),
        }
    }
}

impl fmt::Display for Reg {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::X86(register) => {
                let name = format!("{register:?}");
                formatter.write_str(&name.to_ascii_lowercase())
            }
            Self::A64(register) => write!(formatter, "{register}"),
        }
    }
}

/// A set of whole registers: a register is in it when any part of it is.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RegSet(u64);

impl RegSet {
    /// No register.
    pub const EMPTY: Self = Self(0);

    /// Whether any part of `register` is in the set.
    #[must_use]
    pub fn contains(self, register: Reg) -> bool {
        register
            .slot()
            .is_some_and(|slot| self.0 & (1 << slot) != 0)
    }

    /// Adds `register`, whole.
    pub fn insert(&mut self, register: Reg) {
        if let Some(slot) = register.slot() {
            self.0 |= 1 << slot;
        }
    }
}

/// How control leaves an instruction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Control {
    /// On to the next instruction.
    Next,
    /// A jump to this address.
    Jump(u64),
    /// A jump to this address when the condition holds, otherwise on.
    Branch {
        /// Where it jumps.
        target: u64,
        /// When.
        cond: Cond,
    },
    /// A call of the function at this address.
    Call(u64),
    /// A return to the caller.
    Return,
    /// A system call.
    Syscall,
    /// A jump to an address held in a register or in memory.
    IndirectJump,
    /// A call of an address held in a register or in memory.
    IndirectCall,
    /// Anything else: an interrupt, a trap.
    Other,
}

/// When a conditional jump jumps.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cond {
    /// Signed overflow.
    Overflow,
    /// No signed overflow.
    NotOverflow,
    /// Below, without sign (carry).
    Below,
    /// Above or equal, without sign (no carry).
    AboveEqual,
    /// Equal (zero).
    Equal,
    /// Not equal (not zero).
    NotEqual,
    /// Below or equal, without sign.
    BelowEqual,
    /// Above, without sign.
    Above,
    /// Negative.
    Sign,
    /// Not negative.
    NotSign,
    /// Even parity.
    Parity,
    /// Odd parity.
    NotParity,
    /// Less, with sign.
    Less,
    /// Greater or equal, with sign.
    GreaterEqual,
    /// Less or equal, with sign.
    LessEqual,
    /// Greater, with sign.
    Greater,
    /// The register tested is zero: AArch64's `cbz`.
    Zero,
    /// The register tested isn't zero: `cbnz`.
    NotZero,
    /// This bit of the register tested is clear: `tbz`.
    BitClear(u8),
    /// This bit of the register tested is set: `tbnz`.
    BitSet(u8),
    /// Always (AArch64's `b.al` and `b.nv`).
    Always,
    /// Any other condition, such as x86's `jrcxz` and `loop`.
    Other,
}

/// An instruction's memory operand, as written in it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MemRef {
    /// The base register; none for an address relative to the instruction, or a constant one.
    pub base: Option<Reg>,
    /// The index register.
    pub index: Option<Reg>,
    /// What the index is multiplied by.
    pub scale: u32,
    /// The constant added; for an operand relative to the instruction, the address it names.
    pub displacement: i64,
    /// Whether the address is relative to the instruction, as `[rip + name]` is.
    pub pc_relative: bool,
    /// The bytes it reads or writes; 0 where it's only an address, as `lea`'s is.
    pub width: u64,
    /// Whether the instruction reads this operand.
    pub reads: bool,
    /// Whether the instruction writes this operand.
    pub writes: bool,
}

/// One of an instruction's operands, as an idiom needs it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Operand {
    /// A register.
    Reg(Reg),
    /// A number in the instruction.
    Imm(i64),
    /// Memory.
    Mem,
    /// Where a jump or call goes.
    Target(u64),
}

/// The dialect's idioms an instruction can be, for the rules that look for them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op {
    /// A number into a register: `mov eax, 60`.
    MovImm {
        /// The register, as written.
        dst: Reg,
        /// The number.
        value: i64,
    },
    /// Any other move.
    Move,
    /// An address into a register, with no access to memory: `lea`.
    AddressOf {
        /// The register, as written.
        dst: Reg,
    },
    /// A comparison that sets the flags as a subtraction does: `cmp`.
    Compare {
        /// What is compared.
        left: Operand,
        /// What it's compared with.
        right: Operand,
    },
    /// A test of bits: `test`.
    Test,
    /// One off a register, as a counted loop's counter goes: `dec r`, `sub r, 1`, AArch64's
    /// `subs xR, xR, #1` and `sub xR, xR, #1`.
    Decrement {
        /// The register, whole.
        counter: Reg,
        /// Whether it sets the flags, which a jump on them then reads: AArch64's `sub`
        /// doesn't.
        flags: bool,
    },
    /// A push onto the stack, of these registers (whole) when it pushes registers.
    Push([Option<Reg>; 2]),
    /// A pop from the stack, into these registers (whole) when it pops into registers.
    Pop([Option<Reg>; 2]),
    /// Two values swapped: `xchg`.
    Exchange,
    /// A hint that touches no memory, whatever its operand says: `nop [m]`, `prefetcht0 [m]`.
    Hint,
    /// Anything else.
    Other,
}

/// What an instruction puts in its first operand, as the memory rules follow owners.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Value {
    /// The address its memory operand computes: `lea`.
    Address,
    /// What this operand holds: `mov r, r`, `mov r, imm`, `xchg`.
    Copy(Operand),
    /// What it loads from memory: `mov r, [m]`.
    Load,
    /// A number, never an address: a widening move.
    Number,
    /// The sum of the two: `add` (with the destination as the first), `inc`.
    Add(Operand, Operand),
    /// The first less the second: `sub`, `dec`.
    Sub(Operand, Operand),
    /// The bits of the first the second keeps: `and`.
    And(Operand, Operand),
    /// What the stack held: `pop`.
    Pop,
    /// What it held, or this: `cmov`.
    Select(Operand),
    /// Anything else, a number unless it reads an address or something unknown.
    Other,
}

/// What an instruction does to the stack pointer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Stack {
    /// Leaves it where it is, as far as the function's own depth goes: calls and returns
    /// included.
    Same,
    /// Moves it this many bytes deeper (shallower when negative).
    Moves(i64),
    /// Changes it in a way the dialect doesn't allow, and why.
    Refused(String),
}

/// One machine instruction, as the rules read it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Insn {
    /// Its address.
    pub address: u64,
    /// The decoder's name for it, in lowercase, for messages.
    pub mnemonic: String,
    /// How control leaves it.
    pub control: Control,
    /// The registers it reads, as its decoder says, whole.
    pub reads: RegSet,
    /// The registers it writes, as its decoder says, whole: not the ones a call or system call
    /// changes, which are [`Abi`]'s.
    pub writes: RegSet,
    /// Its first operand, when that's a register, as written.
    pub dst: Option<Reg>,
    /// Its operands, as written.
    pub operands: Vec<Operand>,
    /// Its memory operand.
    pub memory: Option<MemRef>,
    /// Whether it reads memory at all, the stack included.
    pub reads_memory: bool,
    /// Whether it writes memory at all, the stack included.
    pub writes_memory: bool,
    /// The idiom it is.
    pub op: Op,
    /// What it puts in its first operand.
    pub value: Value,
    /// What it does to the stack pointer.
    pub stack: Stack,
}

/// A CPU's calling convention, as lengua's programs follow it.
#[derive(Debug)]
pub struct Abi {
    /// The general-purpose registers, whole, in the order of their [`Reg::slot`]s.
    pub registers: &'static [Reg],
    /// The stack pointer.
    pub stack_pointer: Reg,
    /// The bytes a call puts on the stack: the return address, where the CPU pushes it.
    pub return_address: i64,
    /// The register that chooses a system call.
    pub syscall_number: Reg,
    /// The registers a system call takes its arguments in, in order.
    pub syscall_arguments: &'static [Reg],
    /// The registers a function takes its arguments in, in order.
    pub arguments: &'static [Reg],
    /// The register a result comes back in.
    pub result: Reg,
    /// The registers a function returns in: what an address in its frame mustn't leave by.
    pub returns: [Reg; 2],
    /// The register an exit status goes in.
    pub first_argument: Reg,
    /// The register an assertion's number goes in, as the template writes it.
    pub assertion_number: Reg,
    /// The register a worker thread keeps its slot in.
    pub worker_slot: Reg,
    /// The register a call leaves its return address in, which a function that calls must
    /// save first: AArch64's x30. None where the call pushes it.
    pub link: Option<Reg>,
    /// The registers a call may change.
    pub caller_saved: &'static [Reg],
    /// The registers a system call changes.
    pub syscall_clobbers: &'static [Reg],
    /// The registers a function must give back unchanged.
    pub callee_saved: &'static [Reg],
}

impl Insn {
    /// Whether it changes any part of `register`, counting what a call or system call changes.
    #[must_use]
    pub fn changes(&self, register: Reg, abi: &Abi) -> bool {
        let full = register.full();
        let clobbered = match self.control {
            Control::Call(_) | Control::IndirectCall => abi.caller_saved,
            Control::Syscall => abi.syscall_clobbers,
            _ => &[],
        };
        clobbered.contains(&full) || self.writes.contains(register)
    }

    /// Whether it reads any part of `register`.
    #[must_use]
    pub fn uses(&self, register: Reg) -> bool {
        self.reads.contains(register)
    }
}
