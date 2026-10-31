//! The vector instructions, from SSE2 to AVX-512: a vector register is its bytes, each a term,
//! and an instruction works element by element on them, so a compare of 64 input bytes is 64
//! tests of the bytes themselves. AVX-512's mask registers are 64-bit terms whose bits are
//! those tests. Legacy SSE writes keep a register's upper bytes; VEX and EVEX writes clear them.

use std::rc::Rc;

use iced_x86::{EncodingKind, Mnemonic, OpKind, Register};

use super::{Cpu, Last, Step, Stop, VECTOR_BYTES, unsupported};
use crate::prove::term::Term;

mod crypto;
mod float;

/// A vector register's number, 0 to 31.
fn vector_number(register: Register) -> Option<usize> {
    (register.is_xmm() || register.is_ymm() || register.is_zmm()).then(|| register.number())
}

/// A mask register's number, 0 to 7.
fn mask_number(register: Register) -> Option<usize> {
    register.is_k().then(|| register.number())
}

/// How two elements compare, as `vpcmp`'s predicate numbers them.
#[derive(Clone, Copy)]
enum Predicate {
    Equal,
    Less,
    LessOrEqual,
    Never,
    NotEqual,
    AtLeast,
    Greater,
    Always,
}

impl Predicate {
    const fn from_immediate(immediate: u8) -> Self {
        match immediate & 7 {
            0 => Self::Equal,
            1 => Self::Less,
            2 => Self::LessOrEqual,
            3 => Self::Never,
            4 => Self::NotEqual,
            5 => Self::AtLeast,
            6 => Self::Greater,
            _ => Self::Always,
        }
    }
}

/// The element size in bytes a mnemonic's last letter names: b, w, d or q.
fn element_of(name: &str) -> usize {
    match name.as_bytes().last() {
        Some(b'w') => 2,
        Some(b'd') => 4,
        Some(b'q') => 8,
        _ => 1,
    }
}

impl Cpu<'_> {
    /// The size in bytes of operand `index`.
    fn operand_bytes(&self, index: u32) -> usize {
        match self.instruction.op_kind(index) {
            OpKind::Register => self.instruction.op_register(index).size(),
            OpKind::Memory => self.instruction.memory_size().size(),
            _ => 0,
        }
    }

    /// `value`'s bytes, lowest first, padded with zeros to `bytes`.
    fn split(&mut self, value: Term, bytes: usize) -> Vec<Term> {
        let width = self.pool.width(value);
        let zero = self.pool.bv(8, 0);
        (0..bytes)
            .map(|at| {
                let low = u32::try_from(at * 8).unwrap_or(u32::MAX);
                if low + 8 <= width {
                    self.pool.extract(value, low + 7, low)
                } else {
                    zero
                }
            })
            .collect()
    }

    /// One term of `parts`, lowest first, joined as a balanced tree so a bit of it is reached in
    /// few steps.
    fn join(&mut self, parts: &[Term]) -> Term {
        match parts {
            [] => self.pool.bv(8, 0),
            [only] => *only,
            _ => {
                let half = parts.len() / 2;
                let low = self.join(&parts[..half]);
                let high = self.join(&parts[half..]);
                self.pool.concat(high, low)
            }
        }
    }

    /// The low `bytes` bytes of operand `index`: a vector register's, a mask's, a general
    /// register's or memory's.
    fn bytes_in(&mut self, index: u32, bytes: usize) -> Result<Vec<Term>, Stop> {
        let instruction = self.instruction;
        match instruction.op_kind(index) {
            OpKind::Register => {
                let register = instruction.op_register(index);
                if let Some(number) = vector_number(register) {
                    return Ok(self.state.vectors[number][..bytes.min(VECTOR_BYTES)].to_vec());
                }
                if let Some(number) = mask_number(register) {
                    let mask = self.state.masks[number];
                    return Ok(self.split(mask, bytes));
                }
                let width = u32::try_from(register.size() * 8).unwrap_or(64);
                let value = self.read(index, width)?;
                Ok(self.split(value, bytes))
            }
            OpKind::Memory => {
                if instruction.is_broadcast() {
                    return Err(unsupported("an embedded broadcast from memory"));
                }
                let address = self.address()?;
                self.reach_operand(address, bytes as u64, false)?;
                (0..bytes)
                    .map(|offset| self.load(address.wrapping_add(offset as u64), 1))
                    .collect()
            }
            other => Err(unsupported(format!("a vector operand of kind {other:?}"))),
        }
    }

    /// Writes `value` to operand `index`: a vector register's low bytes, keeping the rest for
    /// legacy SSE and clearing it for VEX and EVEX; a general register; or memory.
    fn bytes_out(&mut self, index: u32, value: &[Term]) -> Result<(), Stop> {
        let instruction = self.instruction;
        match instruction.op_kind(index) {
            OpKind::Register => {
                let register = instruction.op_register(index);
                let Some(number) = vector_number(register) else {
                    let bytes = register.size().min(value.len());
                    let joined = self.join(&value[..bytes]);
                    return self.write(index, joined);
                };
                let keeps = instruction.encoding() == EncodingKind::Legacy;
                let zero = self.pool.bv(8, 0);
                let vector = Rc::make_mut(&mut self.state.vectors[number]);
                for (at, byte) in vector.iter_mut().enumerate() {
                    if let Some(new) = value.get(at) {
                        *byte = *new;
                    } else if !keeps {
                        *byte = zero;
                    }
                }
                Ok(())
            }
            OpKind::Memory => {
                let address = self.address()?;
                self.reach_operand(address, value.len() as u64, true)?;
                for (offset, byte) in value.iter().enumerate() {
                    self.save(address.wrapping_add(offset as u64), *byte)?;
                }
                Ok(())
            }
            other => Err(unsupported(format!(
                "writing a vector operand of kind {other:?}"
            ))),
        }
    }

    /// The write mask, k1 to k7, when the instruction has one.
    fn write_mask(&self) -> Option<Term> {
        mask_number(self.instruction.op_mask())
            .filter(|number| *number != 0)
            .map(|number| self.state.masks[number])
    }

    /// Writes `result` to operand 0 under the write mask, element by element of `element`
    /// bytes: an element whose bit is clear is zeroed with `{z}`, kept otherwise, and in memory
    /// left unwritten.
    fn masked_out(&mut self, result: &[Term], element: usize) -> Result<(), Stop> {
        let Some(mask) = self.write_mask() else {
            return self.bytes_out(0, result);
        };
        if self.instruction.op_kind(0) == OpKind::Memory {
            let address = self.address()?;
            // A masked access counts its full width, as the rules hold it.
            self.reach_operand(address, result.len() as u64, true)?;
            for (offset, byte) in result.iter().enumerate() {
                let lane = u32::try_from(offset / element).unwrap_or(u32::MAX);
                let on = self.pool.bit_is_set(mask, lane);
                let at = address.wrapping_add(offset as u64);
                match self.pool.truth(on) {
                    Some(false) => {}
                    Some(true) => self.save(at, *byte)?,
                    None => {
                        let old = self.load(at, 1)?;
                        let value = self.pool.ite(on, *byte, old);
                        self.save(at, value)?;
                    }
                }
            }
            return Ok(());
        }
        let zeroing = self.instruction.zeroing_masking();
        let old = if zeroing {
            Vec::new()
        } else {
            self.bytes_in(0, result.len())?
        };
        let zero = self.pool.bv(8, 0);
        let mut out = Vec::with_capacity(result.len());
        for (at, byte) in result.iter().enumerate() {
            let lane = u32::try_from(at / element).unwrap_or(u32::MAX);
            let on = self.pool.bit_is_set(mask, lane);
            let other = if zeroing { zero } else { old[at] };
            out.push(self.pool.ite(on, *byte, other));
        }
        self.bytes_out(0, &out)
    }

    /// The bytes of a source under the write mask, for a masked load from memory: an element
    /// whose bit is known to be clear isn't read, as the CPU doesn't fault on it.
    fn masked_in(&mut self, index: u32, bytes: usize, element: usize) -> Result<Vec<Term>, Stop> {
        let Some(mask) = self
            .write_mask()
            .filter(|_| self.instruction.op_kind(index) == OpKind::Memory)
        else {
            return self.bytes_in(index, bytes);
        };
        let address = self.address()?;
        self.reach_operand(address, bytes as u64, false)?;
        let zero = self.pool.bv(8, 0);
        let mut out = Vec::with_capacity(bytes);
        for offset in 0..bytes {
            let lane = u32::try_from(offset / element).unwrap_or(u32::MAX);
            let on = self.pool.bit_is_set(mask, lane);
            out.push(if self.pool.truth(on) == Some(false) {
                zero
            } else {
                self.load(address.wrapping_add(offset as u64), 1)?
            });
        }
        Ok(out)
    }

    /// The two sources of an operation: the destination and operand 1 for a legacy two-operand
    /// instruction, operands 1 and 2 otherwise.
    fn sources(&mut self, bytes: usize, element: usize) -> Result<(Vec<Term>, Vec<Term>), Stop> {
        if self.instruction.op_count() == 2 {
            Ok((self.bytes_in(0, bytes)?, self.bytes_in(1, bytes)?))
        } else {
            Ok((self.bytes_in(1, bytes)?, self.masked_in(2, bytes, element)?))
        }
    }

    /// `bytes` as elements of `element` bytes each.
    fn elements(&mut self, bytes: &[Term], element: usize) -> Vec<Term> {
        bytes
            .chunks(element)
            .map(|chunk| self.join(chunk))
            .collect()
    }

    /// Elements back into bytes.
    fn unelements(&mut self, elements: &[Term]) -> Vec<Term> {
        let mut bytes = Vec::new();
        for element in elements {
            let width = self.pool.width(*element) as usize / 8;
            bytes.extend(self.split(*element, width));
        }
        bytes
    }

    /// Element by element: `operation` of the two sources' elements.
    fn each(
        &mut self,
        bytes: usize,
        element: usize,
        operation: &mut dyn FnMut(&mut Self, Term, Term) -> Term,
    ) -> Result<Vec<Term>, Stop> {
        let (a, b) = self.sources(bytes, element)?;
        let (a, b) = (self.elements(&a, element), self.elements(&b, element));
        let results: Vec<Term> = a
            .iter()
            .zip(&b)
            .map(|(x, y)| operation(self, *x, *y))
            .collect();
        Ok(self.unelements(&results))
    }

    /// All ones where `condition` holds, as an element of `width` bits.
    fn all_ones(&mut self, condition: Term, width: u32) -> Term {
        let ones = self.pool.bv(width, u128::MAX >> (128 - width));
        let zero = self.pool.bv(width, 0);
        self.pool.ite(condition, ones, zero)
    }

    /// The comparison `predicate` of `a` and `b`, signed or not.
    fn compare(&mut self, predicate: Predicate, signed: bool, a: Term, b: Term) -> Term {
        let less = |cpu: &mut Self, x: Term, y: Term| {
            if signed {
                cpu.pool.slt(x, y)
            } else {
                cpu.pool.ult(x, y)
            }
        };
        match predicate {
            Predicate::Equal => self.pool.eq(a, b),
            Predicate::NotEqual => {
                let same = self.pool.eq(a, b);
                self.pool.bool_not(same)
            }
            Predicate::Less => less(self, a, b),
            Predicate::AtLeast => {
                let below = less(self, a, b);
                self.pool.bool_not(below)
            }
            Predicate::Greater => less(self, b, a),
            Predicate::LessOrEqual => {
                let above = less(self, b, a);
                self.pool.bool_not(above)
            }
            Predicate::Never => self.pool.boolean(false),
            Predicate::Always => self.pool.boolean(true),
        }
    }

    /// A mask of `conditions`, bit i for condition i, under the write mask, into mask register
    /// operand 0, with the bits above them clear.
    fn mask_out(&mut self, conditions: &[Term]) -> Result<(), Stop> {
        let bits: Vec<Term> = conditions
            .iter()
            .map(|condition| {
                let (one, zero) = (self.pool.bv(1, 1), self.pool.bv(1, 0));
                self.pool.ite(*condition, one, zero)
            })
            .collect();
        let joined = self.join(&bits);
        let width = self.pool.width(joined);
        let mut mask = if width < 64 {
            self.pool.zero_extend(joined, 64 - width)
        } else {
            joined
        };
        if let Some(write) = self.write_mask() {
            mask = self.pool.and(mask, write);
        }
        let number = mask_number(self.instruction.op_register(0))
            .ok_or_else(|| unsupported("a mask written to something else"))?;
        self.state.masks[number] = mask;
        Ok(())
    }

    /// The width in bits a mask instruction's last letter names.
    fn mask_width(name: &str) -> u32 {
        match name.as_bytes().last() {
            Some(b'b') => 8,
            Some(b'w') => 16,
            Some(b'd') => 32,
            _ => 64,
        }
    }

    /// A mask operand's low `width` bits, zero-extended to 64: a mask register's, a general
    /// register's or memory's.
    fn mask_in(&mut self, index: u32, width: u32) -> Result<Term, Stop> {
        let instruction = self.instruction;
        let value = match instruction.op_kind(index) {
            OpKind::Register if instruction.op_register(index).is_k() => {
                self.state.masks[instruction.op_register(index).number()]
            }
            _ => {
                let bytes = self.bytes_in(index, (width / 8) as usize)?;
                self.join(&bytes)
            }
        };
        let low = self.pool.extract(value, width - 1, 0);
        Ok(if width < 64 {
            self.pool.zero_extend(low, 64 - width)
        } else {
            low
        })
    }

    /// The vector and mask instructions.
    #[expect(clippy::too_many_lines, reason = "one arm per kind of instruction")]
    pub(super) fn vector(&mut self, mnemonic: Mnemonic) -> Result<Step, Stop> {
        let instruction = self.instruction;
        let name = format!("{mnemonic:?}").to_lowercase();
        let evex = instruction.encoding() == EncodingKind::EVEX;
        // The operation's size: the destination's, or for a mask destination the sources'.
        let size = if instruction.op_register(0).is_k() || instruction.op_count() == 0 {
            self.operand_bytes(1)
        } else {
            self.operand_bytes(0)
        };
        match mnemonic {
            Mnemonic::Vzeroupper | Mnemonic::Vzeroall => {
                let zero = self.pool.bv(8, 0);
                let from = if mnemonic == Mnemonic::Vzeroupper {
                    16
                } else {
                    0
                };
                for number in 0..16 {
                    let vector = Rc::make_mut(&mut self.state.vectors[number]);
                    for byte in &mut vector[from..] {
                        *byte = zero;
                    }
                }
            }
            Mnemonic::Movdqu
            | Mnemonic::Movdqa
            | Mnemonic::Movups
            | Mnemonic::Movaps
            | Mnemonic::Movupd
            | Mnemonic::Movapd
            | Mnemonic::Lddqu
            | Mnemonic::Movntdq
            | Mnemonic::Vmovdqu
            | Mnemonic::Vmovdqa
            | Mnemonic::Vmovups
            | Mnemonic::Vmovaps
            | Mnemonic::Vmovupd
            | Mnemonic::Vmovapd
            | Mnemonic::Vlddqu
            | Mnemonic::Vmovntdq
            | Mnemonic::Vmovdqu8
            | Mnemonic::Vmovdqu16
            | Mnemonic::Vmovdqu32
            | Mnemonic::Vmovdqu64
            | Mnemonic::Vmovdqa32
            | Mnemonic::Vmovdqa64 => {
                let element = match mnemonic {
                    Mnemonic::Vmovdqu8 => 1,
                    Mnemonic::Vmovdqu16 => 2,
                    Mnemonic::Vmovdqu32 | Mnemonic::Vmovdqa32 => 4,
                    _ => 8,
                };
                let value = self.masked_in(1, size, element)?;
                self.masked_out(&value, element)?;
            }
            Mnemonic::Movd | Mnemonic::Movq | Mnemonic::Vmovd | Mnemonic::Vmovq => {
                let bytes = if matches!(mnemonic, Mnemonic::Movd | Mnemonic::Vmovd) {
                    4
                } else {
                    8
                };
                let value = self.bytes_in(1, bytes)?;
                let into_vector = instruction.op_kind(0) == OpKind::Register
                    && vector_number(instruction.op_register(0)).is_some();
                if into_vector {
                    // The rest of the low 128 bits clears, whatever the encoding.
                    let mut wide = value;
                    let zero = self.pool.bv(8, 0);
                    wide.resize(16, zero);
                    self.bytes_out(0, &wide)?;
                } else {
                    self.bytes_out(0, &value)?;
                }
            }
            Mnemonic::Pshufd | Mnemonic::Vpshufd => {
                let source = self.masked_in(1, size, 4)?;
                let order = instruction.immediate8();
                let mut result = Vec::with_capacity(size);
                for lane in 0..size / 16 {
                    for dword in 0..4 {
                        let from = lane * 16 + usize::from(order >> (2 * dword) & 3) * 4;
                        result.extend_from_slice(&source[from..from + 4]);
                    }
                }
                self.masked_out(&result, 4)?;
            }
            Mnemonic::Pshufb | Mnemonic::Vpshufb => {
                let (table, indices) = self.sources(size, 1)?;
                let zero = self.pool.bv(8, 0);
                let mut result = Vec::with_capacity(size);
                for (at, index) in indices.iter().enumerate() {
                    let lane = at / 16 * 16;
                    let cleared = self.pool.bit_is_set(*index, 7);
                    let low = self.pool.extract(*index, 3, 0);
                    let picked = match self.pool.constant(low) {
                        Some(known) => table[lane + known as usize],
                        None => {
                            let mut picked = table[lane];
                            for value in 1..16 {
                                let constant = self.pool.bv(4, value as u128);
                                let chosen = self.pool.eq(low, constant);
                                picked = self.pool.ite(chosen, table[lane + value], picked);
                            }
                            picked
                        }
                    };
                    result.push(self.pool.ite(cleared, zero, picked));
                }
                self.masked_out(&result, 1)?;
            }
            Mnemonic::Pcmpeqb
            | Mnemonic::Pcmpeqw
            | Mnemonic::Pcmpeqd
            | Mnemonic::Pcmpeqq
            | Mnemonic::Pcmpgtb
            | Mnemonic::Pcmpgtw
            | Mnemonic::Pcmpgtd
            | Mnemonic::Pcmpgtq
            | Mnemonic::Vpcmpeqb
            | Mnemonic::Vpcmpeqw
            | Mnemonic::Vpcmpeqd
            | Mnemonic::Vpcmpeqq
            | Mnemonic::Vpcmpgtb
            | Mnemonic::Vpcmpgtw
            | Mnemonic::Vpcmpgtd
            | Mnemonic::Vpcmpgtq => {
                let element = element_of(&name);
                let predicate = if name.contains("eq") {
                    Predicate::Equal
                } else {
                    Predicate::Greater
                };
                if instruction.op_register(0).is_k() {
                    let (a, b) = self.sources(size, element)?;
                    let (a, b) = (self.elements(&a, element), self.elements(&b, element));
                    let conditions: Vec<Term> = a
                        .iter()
                        .zip(&b)
                        .map(|(x, y)| self.compare(predicate, true, *x, *y))
                        .collect();
                    self.mask_out(&conditions)?;
                } else {
                    let width = u32::try_from(element * 8).unwrap_or(8);
                    let result = self.each(size, element, &mut |cpu, x, y| {
                        let holds = cpu.compare(predicate, true, x, y);
                        cpu.all_ones(holds, width)
                    })?;
                    self.bytes_out(0, &result)?;
                }
            }
            Mnemonic::Vpcmpb
            | Mnemonic::Vpcmpub
            | Mnemonic::Vpcmpw
            | Mnemonic::Vpcmpuw
            | Mnemonic::Vpcmpd
            | Mnemonic::Vpcmpud
            | Mnemonic::Vpcmpq
            | Mnemonic::Vpcmpuq => {
                let element = element_of(&name);
                let signed = !name.contains("pcmpu");
                let predicate = Predicate::from_immediate(instruction.immediate8());
                let (a, b) = self.sources(size, element)?;
                let (a, b) = (self.elements(&a, element), self.elements(&b, element));
                let conditions: Vec<Term> = a
                    .iter()
                    .zip(&b)
                    .map(|(x, y)| self.compare(predicate, signed, *x, *y))
                    .collect();
                self.mask_out(&conditions)?;
            }
            Mnemonic::Vptestmb
            | Mnemonic::Vptestmw
            | Mnemonic::Vptestmd
            | Mnemonic::Vptestmq
            | Mnemonic::Vptestnmb
            | Mnemonic::Vptestnmw
            | Mnemonic::Vptestnmd
            | Mnemonic::Vptestnmq => {
                let element = element_of(&name);
                let none = name.contains("testnm");
                let (a, b) = self.sources(size, element)?;
                let (a, b) = (self.elements(&a, element), self.elements(&b, element));
                let conditions: Vec<Term> = a
                    .iter()
                    .zip(&b)
                    .map(|(x, y)| {
                        let both = self.pool.and(*x, *y);
                        let width = self.pool.width(both);
                        let zero = self.pool.bv(width, 0);
                        let clear = self.pool.eq(both, zero);
                        if none {
                            clear
                        } else {
                            self.pool.bool_not(clear)
                        }
                    })
                    .collect();
                self.mask_out(&conditions)?;
            }
            Mnemonic::Pand
            | Mnemonic::Pandn
            | Mnemonic::Por
            | Mnemonic::Pxor
            | Mnemonic::Vpand
            | Mnemonic::Vpandn
            | Mnemonic::Vpor
            | Mnemonic::Vpxor
            | Mnemonic::Vpandd
            | Mnemonic::Vpandq
            | Mnemonic::Vpandnd
            | Mnemonic::Vpandnq
            | Mnemonic::Vpord
            | Mnemonic::Vporq
            | Mnemonic::Vpxord
            | Mnemonic::Vpxorq
            | Mnemonic::Andps
            | Mnemonic::Orps
            | Mnemonic::Xorps
            | Mnemonic::Andnps
            | Mnemonic::Vandps
            | Mnemonic::Vorps
            | Mnemonic::Vxorps
            | Mnemonic::Vandnps
            | Mnemonic::Andpd
            | Mnemonic::Orpd
            | Mnemonic::Xorpd
            | Mnemonic::Andnpd
            | Mnemonic::Vandpd
            | Mnemonic::Vorpd
            | Mnemonic::Vxorpd
            | Mnemonic::Vandnpd => {
                // A mask's elements: the integer forms' last letter, a single's 4 bytes or a
                // double's 8.
                let element = match (evex, name.ends_with("ps"), name.ends_with("pd")) {
                    (false, ..) => 1,
                    (true, true, _) => 4,
                    (true, _, true) => 8,
                    _ => element_of(&name),
                };
                let not_first = name.contains("andn");
                let operation = if name.contains("xor") {
                    2
                } else if name.contains("or") {
                    1
                } else {
                    0
                };
                let result = self.each(size, element, &mut |cpu, x, y| {
                    let x = if not_first { cpu.pool.not(x) } else { x };
                    match operation {
                        0 => cpu.pool.and(x, y),
                        1 => cpu.pool.or(x, y),
                        _ => cpu.pool.xor(x, y),
                    }
                })?;
                self.masked_out(&result, element)?;
            }
            Mnemonic::Paddb
            | Mnemonic::Paddw
            | Mnemonic::Paddd
            | Mnemonic::Paddq
            | Mnemonic::Psubb
            | Mnemonic::Psubw
            | Mnemonic::Psubd
            | Mnemonic::Psubq
            | Mnemonic::Vpaddb
            | Mnemonic::Vpaddw
            | Mnemonic::Vpaddd
            | Mnemonic::Vpaddq
            | Mnemonic::Vpsubb
            | Mnemonic::Vpsubw
            | Mnemonic::Vpsubd
            | Mnemonic::Vpsubq => {
                let element = element_of(&name);
                let adding = name.contains("add");
                let result = self.each(size, element, &mut |cpu, x, y| {
                    if adding {
                        cpu.pool.add(x, y)
                    } else {
                        cpu.pool.sub(x, y)
                    }
                })?;
                self.masked_out(&result, element)?;
            }
            Mnemonic::Paddusb | Mnemonic::Psubusb | Mnemonic::Vpaddusb | Mnemonic::Vpsubusb => {
                let adding = name.contains("add");
                let result = self.each(size, 1, &mut |cpu, x, y| {
                    if adding {
                        let (wide_x, wide_y) =
                            (cpu.pool.zero_extend(x, 1), cpu.pool.zero_extend(y, 1));
                        let sum = cpu.pool.add(wide_x, wide_y);
                        let over = cpu.pool.bit_is_set(sum, 8);
                        let low = cpu.pool.extract(sum, 7, 0);
                        let full = cpu.pool.bv(8, 255);
                        cpu.pool.ite(over, full, low)
                    } else {
                        let under = cpu.pool.ult(x, y);
                        let difference = cpu.pool.sub(x, y);
                        let zero = cpu.pool.bv(8, 0);
                        cpu.pool.ite(under, zero, difference)
                    }
                })?;
                self.masked_out(&result, 1)?;
            }
            Mnemonic::Pminub | Mnemonic::Pmaxub | Mnemonic::Vpminub | Mnemonic::Vpmaxub => {
                let smaller = name.contains("min");
                let result = self.each(size, 1, &mut |cpu, x, y| {
                    let below = cpu.pool.ult(x, y);
                    if smaller {
                        cpu.pool.ite(below, x, y)
                    } else {
                        cpu.pool.ite(below, y, x)
                    }
                })?;
                self.masked_out(&result, 1)?;
            }
            Mnemonic::Pmovmskb | Mnemonic::Vpmovmskb => {
                let bytes = self.bytes_in(1, self.operand_bytes(1))?;
                let bits: Vec<Term> = bytes
                    .iter()
                    .map(|byte| self.pool.extract(*byte, 7, 7))
                    .collect();
                let joined = self.join(&bits);
                let width = self.pool.width(joined);
                let target = u32::try_from(instruction.op_register(0).size() * 8).unwrap_or(32);
                let value = if width < target {
                    self.pool.zero_extend(joined, target - width)
                } else {
                    joined
                };
                self.write(0, value)?;
            }
            Mnemonic::Ptest | Mnemonic::Vptest => {
                let bytes = self.operand_bytes(0);
                let (a, b) = (self.bytes_in(0, bytes)?, self.bytes_in(1, bytes)?);
                let zero = self.pool.bv(8, 0);
                let (mut clear, mut clear_outside) = (Vec::new(), Vec::new());
                for (x, y) in a.iter().zip(&b) {
                    let both = self.pool.and(*x, *y);
                    clear.push(self.pool.eq(both, zero));
                    let inverted = self.pool.not(*x);
                    let outside = self.pool.and(inverted, *y);
                    clear_outside.push(self.pool.eq(outside, zero));
                }
                let no = self.pool.boolean(false);
                self.state.last = Last::Other;
                self.state.zero = self.pool.all(&clear);
                self.state.carry = self.pool.all(&clear_outside);
                self.state.sign = no;
                self.state.overflow = no;
                self.state.parity = no;
            }
            Mnemonic::Psllw
            | Mnemonic::Pslld
            | Mnemonic::Psllq
            | Mnemonic::Psrlw
            | Mnemonic::Psrld
            | Mnemonic::Psrlq
            | Mnemonic::Psraw
            | Mnemonic::Psrad
            | Mnemonic::Vpsllw
            | Mnemonic::Vpslld
            | Mnemonic::Vpsllq
            | Mnemonic::Vpsrlw
            | Mnemonic::Vpsrld
            | Mnemonic::Vpsrlq
            | Mnemonic::Vpsraw
            | Mnemonic::Vpsrad
                if matches!(
                    instruction.op_kind(instruction.op_count() - 1),
                    OpKind::Immediate8
                ) =>
            {
                let element = element_of(&name);
                let width = u32::try_from(element * 8).unwrap_or(8);
                let count = u32::from(instruction.immediate8());
                let from = if instruction.op_count() == 2 { 0 } else { 1 };
                let source = self.masked_in(from, size, element)?;
                let source = self.elements(&source, element);
                let shifted: Vec<Term> = source
                    .iter()
                    .map(|value| {
                        let arithmetic = name.contains("psra");
                        let by = if arithmetic {
                            count.min(width - 1)
                        } else {
                            count
                        };
                        if by >= width {
                            return self.pool.bv(width, 0);
                        }
                        let amount = self.pool.bv(width, u128::from(by));
                        if name.contains("psll") {
                            self.pool.shl(*value, amount)
                        } else if arithmetic {
                            self.pool.ashr(*value, amount)
                        } else {
                            self.pool.lshr(*value, amount)
                        }
                    })
                    .collect();
                let result = self.unelements(&shifted);
                self.masked_out(&result, element)?;
            }
            Mnemonic::Pslldq | Mnemonic::Psrldq | Mnemonic::Vpslldq | Mnemonic::Vpsrldq => {
                let from = if instruction.op_count() == 2 { 0 } else { 1 };
                let source = self.bytes_in(from, size)?;
                let count = usize::from(instruction.immediate8());
                let zero = self.pool.bv(8, 0);
                let left = name.contains("psll");
                let mut result = Vec::with_capacity(size);
                for at in 0..size {
                    let (lane, within) = (at / 16 * 16, at % 16);
                    let taken = if left {
                        within.checked_sub(count)
                    } else {
                        Some(within + count).filter(|place| *place < 16)
                    };
                    result.push(taken.map_or(zero, |place| source[lane + place]));
                }
                self.bytes_out(0, &result)?;
            }
            Mnemonic::Vpbroadcastb
            | Mnemonic::Vpbroadcastw
            | Mnemonic::Vpbroadcastd
            | Mnemonic::Vpbroadcastq => {
                let element = element_of(&name);
                let value = self.bytes_in(1, element)?;
                let result: Vec<Term> = (0..size).map(|at| value[at % element]).collect();
                self.masked_out(&result, element)?;
            }
            Mnemonic::Vpternlogd | Mnemonic::Vpternlogq => {
                let element = element_of(&name);
                let table = instruction.immediate8();
                let a = self.bytes_in(0, size)?;
                let (b, c) = (self.bytes_in(1, size)?, self.masked_in(2, size, element)?);
                let mut result = Vec::with_capacity(size);
                for at in 0..size {
                    let mut byte = self.pool.bv(8, 0);
                    for minterm in 0..8_u8 {
                        if table >> minterm & 1 == 0 {
                            continue;
                        }
                        let pick = |cpu: &mut Self, value: Term, set: bool| {
                            if set { value } else { cpu.pool.not(value) }
                        };
                        let x = pick(self, a[at], minterm & 4 != 0);
                        let y = pick(self, b[at], minterm & 2 != 0);
                        let z = pick(self, c[at], minterm & 1 != 0);
                        let xy = self.pool.and(x, y);
                        let term = self.pool.and(xy, z);
                        byte = self.pool.or(byte, term);
                    }
                    result.push(byte);
                }
                self.masked_out(&result, element)?;
            }
            Mnemonic::Vpmovm2b | Mnemonic::Vpmovm2w | Mnemonic::Vpmovm2d | Mnemonic::Vpmovm2q => {
                let element = element_of(&name);
                let mask = self.mask_in(1, 64)?;
                let width = u32::try_from(element * 8).unwrap_or(8);
                let elements: Vec<Term> = (0..size / element)
                    .map(|lane| {
                        let on = self.pool.bit_is_set(mask, u32::try_from(lane).unwrap_or(0));
                        self.all_ones(on, width)
                    })
                    .collect();
                let result = self.unelements(&elements);
                self.bytes_out(0, &result)?;
            }
            Mnemonic::Vpmovb2m | Mnemonic::Vpmovw2m | Mnemonic::Vpmovd2m | Mnemonic::Vpmovq2m => {
                let element = element_of(name.trim_end_matches("2m"));
                let bytes = self.bytes_in(1, size)?;
                let conditions: Vec<Term> = bytes
                    .chunks(element)
                    .map(|chunk| self.pool.bit_is_set(chunk[element - 1], 7))
                    .collect();
                self.mask_out(&conditions)?;
            }
            Mnemonic::Kmovb | Mnemonic::Kmovw | Mnemonic::Kmovd | Mnemonic::Kmovq => {
                let width = Self::mask_width(&name);
                let value = self.mask_in(1, width)?;
                let destination = instruction.op_register(0);
                if let Some(number) = mask_number(destination) {
                    self.state.masks[number] = value;
                } else {
                    let bytes = self.split(value, (width / 8) as usize);
                    if instruction.op_kind(0) == OpKind::Memory {
                        self.bytes_out(0, &bytes)?;
                    } else {
                        let target = u32::try_from(destination.size() * 8).unwrap_or(32);
                        let low = self.pool.extract(value, target - 1, 0);
                        self.write(0, low)?;
                    }
                }
            }
            Mnemonic::Korb
            | Mnemonic::Korw
            | Mnemonic::Kord
            | Mnemonic::Korq
            | Mnemonic::Kandb
            | Mnemonic::Kandw
            | Mnemonic::Kandd
            | Mnemonic::Kandq
            | Mnemonic::Kandnb
            | Mnemonic::Kandnw
            | Mnemonic::Kandnd
            | Mnemonic::Kandnq
            | Mnemonic::Kxorb
            | Mnemonic::Kxorw
            | Mnemonic::Kxord
            | Mnemonic::Kxorq
            | Mnemonic::Kxnorb
            | Mnemonic::Kxnorw
            | Mnemonic::Kxnord
            | Mnemonic::Kxnorq => {
                let width = Self::mask_width(&name);
                let (a, b) = (self.mask_in(1, width)?, self.mask_in(2, width)?);
                let value = if name.starts_with("kandn") {
                    let inverted = self.pool.not(a);
                    self.pool.and(inverted, b)
                } else if name.starts_with("kand") {
                    self.pool.and(a, b)
                } else if name.starts_with("kxnor") {
                    let different = self.pool.xor(a, b);
                    self.pool.not(different)
                } else if name.starts_with("kxor") {
                    self.pool.xor(a, b)
                } else {
                    self.pool.or(a, b)
                };
                let low = self.pool.extract(value, width - 1, 0);
                let value = if width < 64 {
                    self.pool.zero_extend(low, 64 - width)
                } else {
                    low
                };
                let number = mask_number(instruction.op_register(0))
                    .ok_or_else(|| unsupported("a mask operation into something else"))?;
                self.state.masks[number] = value;
            }
            Mnemonic::Knotb | Mnemonic::Knotw | Mnemonic::Knotd | Mnemonic::Knotq => {
                let width = Self::mask_width(&name);
                let value = self.mask_in(1, width)?;
                let inverted = self.pool.not(value);
                let low = self.pool.extract(inverted, width - 1, 0);
                let value = if width < 64 {
                    self.pool.zero_extend(low, 64 - width)
                } else {
                    low
                };
                let number = mask_number(instruction.op_register(0))
                    .ok_or_else(|| unsupported("a mask operation into something else"))?;
                self.state.masks[number] = value;
            }
            Mnemonic::Kshiftlb
            | Mnemonic::Kshiftlw
            | Mnemonic::Kshiftld
            | Mnemonic::Kshiftlq
            | Mnemonic::Kshiftrb
            | Mnemonic::Kshiftrw
            | Mnemonic::Kshiftrd
            | Mnemonic::Kshiftrq => {
                let width = Self::mask_width(&name);
                let value = self.mask_in(1, width)?;
                let count = u32::from(instruction.immediate8());
                let value = if count >= width {
                    self.pool.bv(64, 0)
                } else {
                    let amount = self.pool.bv(64, u128::from(count));
                    let shifted = if name.starts_with("kshiftl") {
                        self.pool.shl(value, amount)
                    } else {
                        self.pool.lshr(value, amount)
                    };
                    let low = self.pool.extract(shifted, width - 1, 0);
                    if width < 64 {
                        self.pool.zero_extend(low, 64 - width)
                    } else {
                        low
                    }
                };
                let number = mask_number(instruction.op_register(0))
                    .ok_or_else(|| unsupported("a mask shift into something else"))?;
                self.state.masks[number] = value;
            }
            Mnemonic::Kortestb
            | Mnemonic::Kortestw
            | Mnemonic::Kortestd
            | Mnemonic::Kortestq
            | Mnemonic::Ktestb
            | Mnemonic::Ktestw
            | Mnemonic::Ktestd
            | Mnemonic::Ktestq => {
                let width = Self::mask_width(&name);
                let (a, b) = (self.mask_in(0, width)?, self.mask_in(1, width)?);
                let zero = self.pool.bv(64, 0);
                let (zero_flag, carry) = if name.starts_with("kortest") {
                    let either = self.pool.or(a, b);
                    let full = self.pool.bv(64, u128::MAX >> (128 - width));
                    (self.pool.eq(either, zero), self.pool.eq(either, full))
                } else {
                    let both = self.pool.and(a, b);
                    let inverted = self.pool.not(a);
                    let outside = self.pool.and(inverted, b);
                    let outside = self.pool.extract(outside, width - 1, 0);
                    let narrow_zero = self.pool.bv(width, 0);
                    (self.pool.eq(both, zero), self.pool.eq(outside, narrow_zero))
                };
                let no = self.pool.boolean(false);
                self.state.last = Last::Other;
                self.state.zero = zero_flag;
                self.state.carry = carry;
                self.state.sign = no;
                self.state.overflow = no;
                self.state.parity = no;
            }
            // Across lanes, by indices in a vector: each element of the result is the table's
            // element the index's low bits name, or, for the two-table forms, the table the
            // next bit picks.
            Mnemonic::Vpermb
            | Mnemonic::Vpermw
            | Mnemonic::Vpermd
            | Mnemonic::Vpermps
            | Mnemonic::Vpermt2b
            | Mnemonic::Vpermt2w
            | Mnemonic::Vpermt2d
            | Mnemonic::Vpermt2q
            | Mnemonic::Vpermi2b
            | Mnemonic::Vpermi2w
            | Mnemonic::Vpermi2d
            | Mnemonic::Vpermi2q => {
                let element = match mnemonic {
                    Mnemonic::Vpermb | Mnemonic::Vpermt2b | Mnemonic::Vpermi2b => 1,
                    Mnemonic::Vpermw | Mnemonic::Vpermt2w | Mnemonic::Vpermi2w => 2,
                    Mnemonic::Vpermt2q | Mnemonic::Vpermi2q => 8,
                    _ => 4,
                };
                let two = matches!(
                    mnemonic,
                    Mnemonic::Vpermt2b
                        | Mnemonic::Vpermt2w
                        | Mnemonic::Vpermt2d
                        | Mnemonic::Vpermt2q
                        | Mnemonic::Vpermi2b
                        | Mnemonic::Vpermi2w
                        | Mnemonic::Vpermi2d
                        | Mnemonic::Vpermi2q
                );
                let first = self.bytes_in(0, size)?;
                let (one, other) = (self.bytes_in(1, size)?, self.bytes_in(2, size)?);
                // vpermt2: indices in operand 1, tables the destination and operand 2; vpermi2:
                // indices in the destination, tables operands 1 and 2; vperm: indices in
                // operand 1, the table operand 2.
                let (indices, low_table, high_table) = match mnemonic {
                    Mnemonic::Vpermt2b
                    | Mnemonic::Vpermt2w
                    | Mnemonic::Vpermt2d
                    | Mnemonic::Vpermt2q => (one, first, other),
                    Mnemonic::Vpermi2b
                    | Mnemonic::Vpermi2w
                    | Mnemonic::Vpermi2d
                    | Mnemonic::Vpermi2q => (first, one, other),
                    _ => (one, other.clone(), other),
                };
                let count = size / element;
                let indices = self.elements(&indices, element);
                let (low_table, high_table) = (
                    self.elements(&low_table, element),
                    self.elements(&high_table, element),
                );
                let bits = count.trailing_zeros();
                let mut result = Vec::with_capacity(count);
                for index in indices {
                    let low = self.pool.extract(index, bits - 1, 0);
                    let mut picked = self.pick(&low_table, low);
                    if two {
                        let from_high = self.pick(&high_table, low);
                        let high = self.pool.bit_is_set(index, bits);
                        picked = self.pool.ite(high, from_high, picked);
                    }
                    result.push(picked);
                }
                let bytes = self.unelements(&result);
                self.masked_out(&bytes, element)?;
            }
            // Quadwords by an immediate, within each 256 bits, or by indices across the vector.
            Mnemonic::Vpermq | Mnemonic::Vpermpd => {
                if instruction.op_count() == 3 && instruction.op_kind(2) == OpKind::Immediate8 {
                    let source = self.bytes_in(1, size)?;
                    let order = instruction.immediate8();
                    let mut result = Vec::with_capacity(size);
                    for half in 0..size / 32 {
                        for quad in 0..4 {
                            let from = half * 32 + usize::from(order >> (2 * quad) & 3) * 8;
                            result.extend_from_slice(&source[from..from + 8]);
                        }
                    }
                    self.masked_out(&result, 8)?;
                } else {
                    let (indices, table) = (self.bytes_in(1, size)?, self.bytes_in(2, size)?);
                    let count = size / 8;
                    let (indices, table) = (self.elements(&indices, 8), self.elements(&table, 8));
                    let bits = count.trailing_zeros();
                    let mut result = Vec::with_capacity(count);
                    for index in indices {
                        let low = self.pool.extract(index, bits - 1, 0);
                        result.push(self.pick(&table, low));
                    }
                    let bytes = self.unelements(&result);
                    self.masked_out(&bytes, 8)?;
                }
            }
            // Each 128-bit half from one of four, or zero.
            Mnemonic::Vperm2i128 | Mnemonic::Vperm2f128 => {
                let (a, b) = (self.bytes_in(1, 32)?, self.bytes_in(2, 32)?);
                let order = instruction.immediate8();
                let zero = self.pool.bv(8, 0);
                let mut result = Vec::with_capacity(32);
                for half in 0..2 {
                    let control = order >> (4 * half) & 0xf;
                    if control & 8 != 0 {
                        result.extend(std::iter::repeat_n(zero, 16));
                        continue;
                    }
                    let source = if control & 2 == 0 { &a } else { &b };
                    let from = usize::from(control & 1) * 16;
                    result.extend_from_slice(&source[from..from + 16]);
                }
                self.bytes_out(0, &result)?;
            }
            // The elements a mask picks, packed to the bottom, and the reverse.
            Mnemonic::Vpcompressb
            | Mnemonic::Vpcompressw
            | Mnemonic::Vpcompressd
            | Mnemonic::Vpcompressq
            | Mnemonic::Vpexpandb
            | Mnemonic::Vpexpandw
            | Mnemonic::Vpexpandd
            | Mnemonic::Vpexpandq => self.compress(mnemonic)?,
            other => {
                if let Some(done) = self.floating(other).or_else(|| self.crypto(other)) {
                    done?;
                    return Ok(Step::Next);
                }
                let function = self.image.function_at(instruction.ip());
                let name = format!("{other:?}").to_lowercase();
                return Err(unsupported(format!(
                    "main.s's {function} uses {name}, which the proof doesn't model yet"
                )));
            }
        }
        Ok(Step::Next)
    }

    /// The element of `table` that `index` names: a choice among them where the index is open.
    fn pick(&mut self, table: &[Term], index: Term) -> Term {
        if let Some(known) = self.pool.constant(index) {
            return table[known as usize % table.len()];
        }
        let width = self.pool.width(index);
        let mut picked = table[0];
        for (value, element) in table.iter().enumerate().skip(1) {
            let constant = self.pool.bv(width, value as u128);
            let chosen = self.pool.eq(index, constant);
            picked = self.pool.ite(chosen, *element, picked);
        }
        picked
    }

    /// `vpcompress` and `vpexpand`, element by element under the write mask: compress packs
    /// the elements the mask picks to the bottom of the destination, or of memory, where only
    /// they are stored; expand puts the bottom elements, in order, where the mask's bits are
    /// set. An element a mask leaves is zeroed with `{z}` and kept otherwise.
    fn compress(&mut self, mnemonic: Mnemonic) -> Result<(), Stop> {
        let instruction = self.instruction;
        let element = match mnemonic {
            Mnemonic::Vpcompressb | Mnemonic::Vpexpandb => 1,
            Mnemonic::Vpcompressw | Mnemonic::Vpexpandw => 2,
            Mnemonic::Vpcompressd | Mnemonic::Vpexpandd => 4,
            _ => 8,
        };
        let compressing = matches!(
            mnemonic,
            Mnemonic::Vpcompressb
                | Mnemonic::Vpcompressw
                | Mnemonic::Vpcompressd
                | Mnemonic::Vpcompressq
        );
        let size = self.operand_bytes(1).max(self.operand_bytes(0));
        let count = size / element;
        let all = self.pool.bv(64, u128::from(u64::MAX));
        let mask = self.write_mask().unwrap_or(all);
        let source = self.bytes_in(1, size)?;
        let source = self.elements(&source, element);
        let zero = self.pool.bv(u32::try_from(element * 8).unwrap_or(8), 0);
        // How many of the mask's bits below each place are set.
        let mut below = Vec::with_capacity(count + 1);
        let mut total = self.pool.bv(8, 0);
        let one = self.pool.bv(8, 1);
        for place in 0..count {
            below.push(total);
            let set = self
                .pool
                .bit_is_set(mask, u32::try_from(place).unwrap_or(0));
            let added = self.pool.add(total, one);
            total = self.pool.ite(set, added, total);
        }
        let mut result = Vec::with_capacity(count);
        if compressing {
            // Place j holds the element whose mask bit is set with j set below it.
            for place in 0..count {
                let mut value: Option<Term> = None;
                for (from, item) in source.iter().enumerate().skip(place) {
                    let set = self.pool.bit_is_set(mask, u32::try_from(from).unwrap_or(0));
                    let wanted = self.pool.bv(8, place as u128);
                    let at = self.pool.eq(below[from], wanted);
                    let here = self.pool.bool_and(set, at);
                    value = Some(match value {
                        None => self.pool.ite(here, *item, zero),
                        Some(other) => self.pool.ite(here, *item, other),
                    });
                }
                result.push(value.unwrap_or(zero));
            }
        } else {
            // Place j, where the mask's bit is set, holds the element numbered by the bits
            // set below it.
            for set_below in below.iter().take(count) {
                let picked = self.pick(&source, *set_below);
                result.push(picked);
            }
        }
        let bytes = self.unelements(&result);
        if compressing && instruction.op_kind(0) == OpKind::Memory {
            // Only the packed elements are stored: element j where j is below the count.
            let address = self.address()?;
            self.reach_operand(address, size as u64, true)?;
            for (place, chunk) in bytes.chunks(element).enumerate() {
                let wanted = self.pool.bv(8, place as u128);
                let stored = self.pool.ult(wanted, total);
                for (offset, byte) in chunk.iter().enumerate() {
                    let at = address.wrapping_add((place * element + offset) as u64);
                    match self.pool.truth(stored) {
                        Some(false) => {}
                        Some(true) => self.save(at, *byte)?,
                        None => {
                            let old = self.peek(at);
                            let value = self.pool.ite(stored, *byte, old);
                            self.save(at, value)?;
                        }
                    }
                }
            }
            return Ok(());
        }
        if compressing {
            // The packed elements, and above them the destination's own or zero: compress
            // writes every element below the count, and leaves or zeroes the rest.
            let zeroing = instruction.zeroing_masking();
            let kept = self.bytes_in(0, size)?;
            let byte_zero = self.pool.bv(8, 0);
            let mut out = Vec::with_capacity(size);
            for (at, byte) in bytes.iter().enumerate() {
                let place = self.pool.bv(8, (at / element) as u128);
                let packed = self.pool.ult(place, total);
                let other = if zeroing { byte_zero } else { kept[at] };
                out.push(self.pool.ite(packed, *byte, other));
            }
            return self.bytes_out(0, &out);
        }
        self.masked_out(&bytes, element)
    }
}
