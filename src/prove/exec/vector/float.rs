//! The floating-point instructions, scalar and packed, on singles and doubles, as SSE, AVX and
//! AVX-512 compute them under the MXCSR every program starts with: rounding to nearest, ties to
//! even, and every exception masked. An element is one float term of its 32 or 64 bits. A scalar
//! operation writes the low element: legacy SSE keeps the rest of the register, and VEX and EVEX
//! take the rest of the low 16 bytes from a source and clear the bytes above.

use iced_x86::{EncodingKind, Mnemonic, OpKind, RoundingControl};

use super::super::{Cpu, Last, Stop, unsupported};
use crate::prove::term::{FloatOp, FloatTest, Rounding, Term};

/// Where a scalar result's register gets the rest of its low 16 bytes.
#[derive(Clone, Copy)]
enum Rest {
    /// Kept, as legacy SSE keeps everything it doesn't write.
    Kept,
    /// From this operand, a vector register.
    From(u32),
    /// Zeros, as a scalar load gives.
    Zero,
}

/// The bytes of an element by a mnemonic's last letter: 8 for a double's `d`, 4 for a single's.
fn element_of(name: &str) -> usize {
    if name.ends_with('d') { 8 } else { 4 }
}

/// How `roundsd`'s immediate rounds: its low two bits, unless bit 2 picks the MXCSR's, which is
/// to nearest.
const fn rounding_of(immediate: u8) -> Rounding {
    if immediate & 4 != 0 {
        return Rounding::Nearest;
    }
    match immediate & 3 {
        0 => Rounding::Nearest,
        1 => Rounding::Down,
        2 => Rounding::Up,
        _ => Rounding::Zero,
    }
}

/// Which of less, equal, greater and unordered make a `cmpsd` predicate true, by its number:
/// the 16 above 15 repeat them, differing only in the exceptions they signal.
const PREDICATES: [[bool; 4]; 16] = [
    [false, true, false, false],
    [true, false, false, false],
    [true, true, false, false],
    [false, false, false, true],
    [true, false, true, true],
    [false, true, true, true],
    [false, false, true, true],
    [true, true, true, false],
    [false, true, false, true],
    [true, false, false, true],
    [true, true, false, true],
    [false, false, false, false],
    [true, false, true, false],
    [false, true, true, false],
    [false, false, true, false],
    [true, true, true, true],
];

impl Cpu<'_> {
    /// The low element of operand `index`, as one term.
    fn low_element(&mut self, index: u32, element: usize) -> Result<Term, Stop> {
        let bytes = self.bytes_in(index, element)?;
        Ok(self.join(&bytes))
    }

    /// Writes a scalar result into operand 0's low element, with the rest of its low 16 bytes
    /// as `rest` says. A write mask's bit 0 picks the result or, when clear, the destination's
    /// own element, or zero with `{z}`.
    fn scalar_out(&mut self, result: Term, element: usize, rest: Rest) -> Result<(), Stop> {
        let mut bytes = self.split(result, element);
        if let Some(mask) = self.write_mask() {
            let on = self.pool.bit_is_set(mask, 0);
            let other = if self.instruction.zeroing_masking() {
                vec![self.pool.bv(8, 0); element]
            } else {
                self.bytes_in(0, element)?
            };
            for (byte, other) in bytes.iter_mut().zip(other) {
                *byte = self.pool.ite(on, *byte, other);
            }
        }
        match rest {
            Rest::Kept => {}
            Rest::From(index) => {
                let source = self.bytes_in(index, 16)?;
                bytes.extend_from_slice(&source[element..]);
            }
            Rest::Zero => bytes.resize(16, self.pool.bv(8, 0)),
        }
        self.bytes_out(0, &bytes)
    }

    /// Where a unary scalar operation reads and what keeps the rest: legacy SSE reads operand 1
    /// and keeps the destination's; VEX and EVEX read operand 2 and take the rest from
    /// operand 1.
    fn unary_scalar(&self) -> (u32, Rest) {
        if self.instruction.encoding() == EncodingKind::Legacy {
            (1, Rest::Kept)
        } else {
            (2, Rest::From(1))
        }
    }

    /// The two sources of a binary operation that may carry an immediate: the destination and
    /// operand 1 for legacy SSE, operands 1 and 2 otherwise.
    fn float_sources(&self) -> (u32, u32) {
        if self.instruction.encoding() == EncodingKind::Legacy {
            (0, 1)
        } else {
            (1, 2)
        }
    }

    /// `name`'s arithmetic on two floats: the four operations, or `min` and `max`, which give
    /// the second operand when either is a NaN or both are zeros.
    fn float_binary(&mut self, name: &str, a: Term, b: Term) -> Term {
        let base = name.trim_start_matches('v');
        if base.starts_with("min") {
            let less = self.pool.float_test(FloatTest::Less, a, b);
            return self.pool.ite(less, a, b);
        }
        if base.starts_with("max") {
            let greater = self.pool.float_test(FloatTest::Less, b, a);
            return self.pool.ite(greater, a, b);
        }
        let op = if base.starts_with("add") {
            FloatOp::Add
        } else if base.starts_with("sub") {
            FloatOp::Sub
        } else if base.starts_with("mul") {
            FloatOp::Mul
        } else {
            FloatOp::Div
        };
        self.pool.float(op, a, b)
    }

    /// Whether `cmpsd`'s predicate `immediate` holds for two floats.
    fn float_predicate(&mut self, immediate: u8, a: Term, b: Term) -> Term {
        let wanted = PREDICATES[usize::from(immediate & 15)];
        let tests = [
            self.pool.float_test(FloatTest::Less, a, b),
            self.pool.float_test(FloatTest::Equal, a, b),
            self.pool.float_test(FloatTest::Less, b, a),
            self.pool.float_test(FloatTest::Unordered, a, b),
        ];
        let holding: Vec<Term> = tests
            .iter()
            .zip(wanted)
            .filter(|(_, wanted)| *wanted)
            .map(|(test, _)| *test)
            .collect();
        self.pool.any(&holding)
    }

    /// The floating-point instructions, or `None` for any other mnemonic.
    #[expect(clippy::too_many_lines, reason = "one arm per kind of instruction")]
    pub(super) fn floating(&mut self, mnemonic: Mnemonic) -> Option<Result<(), Stop>> {
        let name = format!("{mnemonic:?}").to_lowercase();
        let result = match mnemonic {
            Mnemonic::Addsd
            | Mnemonic::Subsd
            | Mnemonic::Mulsd
            | Mnemonic::Divsd
            | Mnemonic::Minsd
            | Mnemonic::Maxsd
            | Mnemonic::Addss
            | Mnemonic::Subss
            | Mnemonic::Mulss
            | Mnemonic::Divss
            | Mnemonic::Minss
            | Mnemonic::Maxss
            | Mnemonic::Vaddsd
            | Mnemonic::Vsubsd
            | Mnemonic::Vmulsd
            | Mnemonic::Vdivsd
            | Mnemonic::Vminsd
            | Mnemonic::Vmaxsd
            | Mnemonic::Vaddss
            | Mnemonic::Vsubss
            | Mnemonic::Vmulss
            | Mnemonic::Vdivss
            | Mnemonic::Vminss
            | Mnemonic::Vmaxss => self.scalar_binary(&name),
            Mnemonic::Addpd
            | Mnemonic::Subpd
            | Mnemonic::Mulpd
            | Mnemonic::Divpd
            | Mnemonic::Minpd
            | Mnemonic::Maxpd
            | Mnemonic::Addps
            | Mnemonic::Subps
            | Mnemonic::Mulps
            | Mnemonic::Divps
            | Mnemonic::Minps
            | Mnemonic::Maxps
            | Mnemonic::Vaddpd
            | Mnemonic::Vsubpd
            | Mnemonic::Vmulpd
            | Mnemonic::Vdivpd
            | Mnemonic::Vminpd
            | Mnemonic::Vmaxpd
            | Mnemonic::Vaddps
            | Mnemonic::Vsubps
            | Mnemonic::Vmulps
            | Mnemonic::Vdivps
            | Mnemonic::Vminps
            | Mnemonic::Vmaxps => self.packed_binary(&name),
            Mnemonic::Sqrtsd
            | Mnemonic::Sqrtss
            | Mnemonic::Vsqrtsd
            | Mnemonic::Vsqrtss
            | Mnemonic::Roundsd
            | Mnemonic::Roundss
            | Mnemonic::Vroundsd
            | Mnemonic::Vroundss => self.scalar_unary(&name),
            Mnemonic::Sqrtpd
            | Mnemonic::Sqrtps
            | Mnemonic::Vsqrtpd
            | Mnemonic::Vsqrtps
            | Mnemonic::Roundpd
            | Mnemonic::Roundps
            | Mnemonic::Vroundpd
            | Mnemonic::Vroundps => self.packed_unary(&name),
            Mnemonic::Ucomisd
            | Mnemonic::Comisd
            | Mnemonic::Ucomiss
            | Mnemonic::Comiss
            | Mnemonic::Vucomisd
            | Mnemonic::Vcomisd
            | Mnemonic::Vucomiss
            | Mnemonic::Vcomiss => self.compare_flags(&name),
            Mnemonic::Cmpsd | Mnemonic::Cmpss | Mnemonic::Vcmpsd | Mnemonic::Vcmpss
                if self.instruction.op_kind(0) == OpKind::Register =>
            {
                self.compare_scalar(&name)
            }
            Mnemonic::Cmppd | Mnemonic::Cmpps | Mnemonic::Vcmppd | Mnemonic::Vcmpps => {
                self.compare_packed(&name)
            }
            Mnemonic::Cvtsi2sd | Mnemonic::Cvtsi2ss | Mnemonic::Vcvtsi2sd | Mnemonic::Vcvtsi2ss => {
                self.whole_to_float(&name)
            }
            Mnemonic::Cvtsd2si
            | Mnemonic::Cvttsd2si
            | Mnemonic::Cvtss2si
            | Mnemonic::Cvttss2si
            | Mnemonic::Vcvtsd2si
            | Mnemonic::Vcvttsd2si
            | Mnemonic::Vcvtss2si
            | Mnemonic::Vcvttss2si => self.float_to_whole(&name),
            Mnemonic::Cvtsd2ss | Mnemonic::Cvtss2sd | Mnemonic::Vcvtsd2ss | Mnemonic::Vcvtss2sd => {
                self.scalar_precision(&name)
            }
            Mnemonic::Cvtdq2pd
            | Mnemonic::Cvtpd2dq
            | Mnemonic::Cvttpd2dq
            | Mnemonic::Cvtdq2ps
            | Mnemonic::Cvtps2dq
            | Mnemonic::Cvttps2dq
            | Mnemonic::Cvtps2pd
            | Mnemonic::Cvtpd2ps
            | Mnemonic::Vcvtdq2pd
            | Mnemonic::Vcvtpd2dq
            | Mnemonic::Vcvttpd2dq
            | Mnemonic::Vcvtdq2ps
            | Mnemonic::Vcvtps2dq
            | Mnemonic::Vcvttps2dq
            | Mnemonic::Vcvtps2pd
            | Mnemonic::Vcvtpd2ps => self.packed_conversion(mnemonic),
            Mnemonic::Vfmadd132sd
            | Mnemonic::Vfmadd213sd
            | Mnemonic::Vfmadd231sd
            | Mnemonic::Vfmadd132ss
            | Mnemonic::Vfmadd213ss
            | Mnemonic::Vfmadd231ss
            | Mnemonic::Vfmadd132pd
            | Mnemonic::Vfmadd213pd
            | Mnemonic::Vfmadd231pd
            | Mnemonic::Vfmadd132ps
            | Mnemonic::Vfmadd213ps
            | Mnemonic::Vfmadd231ps => self.fused(&name),
            Mnemonic::Movsd | Mnemonic::Movss | Mnemonic::Vmovsd | Mnemonic::Vmovss => {
                self.scalar_move(&name)
            }
            Mnemonic::Vbroadcastsd | Mnemonic::Vbroadcastss => {
                let element = element_of(&name);
                let size = self.operand_bytes(0);
                self.bytes_in(1, element).and_then(|value| {
                    let copies = value.repeat(size / element);
                    self.masked_out(&copies, element)
                })
            }
            Mnemonic::Movmskpd | Mnemonic::Movmskps | Mnemonic::Vmovmskpd | Mnemonic::Vmovmskps => {
                self.sign_mask(&name)
            }
            Mnemonic::Unpcklpd
            | Mnemonic::Unpckhpd
            | Mnemonic::Unpcklps
            | Mnemonic::Unpckhps
            | Mnemonic::Vunpcklpd
            | Mnemonic::Vunpckhpd
            | Mnemonic::Vunpcklps
            | Mnemonic::Vunpckhps
            | Mnemonic::Shufpd
            | Mnemonic::Shufps
            | Mnemonic::Vshufpd
            | Mnemonic::Vshufps
            | Mnemonic::Haddpd
            | Mnemonic::Haddps
            | Mnemonic::Hsubpd
            | Mnemonic::Hsubps
            | Mnemonic::Vhaddpd
            | Mnemonic::Vhaddps
            | Mnemonic::Vhsubpd
            | Mnemonic::Vhsubps => self.lanes(&name),
            Mnemonic::Movhlps | Mnemonic::Movlhps | Mnemonic::Vmovhlps | Mnemonic::Vmovlhps => {
                self.halves_move(&name)
            }
            _ => return None,
        };
        Some(result)
    }

    /// Stops at a rounding that an EVEX instruction embeds, which the proofs leave to tests.
    fn default_rounding(&self) -> Result<(), Stop> {
        if self.instruction.rounding_control() == RoundingControl::None {
            Ok(())
        } else {
            Err(unsupported("an embedded rounding, such as {rn-sae}"))
        }
    }

    fn scalar_binary(&mut self, name: &str) -> Result<(), Stop> {
        self.default_rounding()?;
        let element = element_of(name);
        let (first, second) = self.float_sources();
        let (a, b) = (
            self.low_element(first, element)?,
            self.low_element(second, element)?,
        );
        let result = self.float_binary(name, a, b);
        let rest = if first == 0 {
            Rest::Kept
        } else {
            Rest::From(1)
        };
        self.scalar_out(result, element, rest)
    }

    fn packed_binary(&mut self, name: &str) -> Result<(), Stop> {
        self.default_rounding()?;
        let element = element_of(name);
        let size = self.operand_bytes(0);
        let result = self.each(size, element, &mut |cpu, x, y| cpu.float_binary(name, x, y))?;
        self.masked_out(&result, element)
    }

    /// One float's square root or rounding to a whole float.
    fn unary(&mut self, name: &str, a: Term) -> Term {
        if name.contains("sqrt") {
            self.pool.float_sqrt(a)
        } else {
            let rounding = rounding_of(self.instruction.immediate8());
            self.pool.float_round(a, rounding)
        }
    }

    fn scalar_unary(&mut self, name: &str) -> Result<(), Stop> {
        self.default_rounding()?;
        let element = element_of(name);
        let (source, rest) = self.unary_scalar();
        let a = self.low_element(source, element)?;
        let result = self.unary(name, a);
        self.scalar_out(result, element, rest)
    }

    fn packed_unary(&mut self, name: &str) -> Result<(), Stop> {
        self.default_rounding()?;
        let element = element_of(name);
        let size = self.operand_bytes(0);
        let source = self.masked_in(1, size, element)?;
        let elements = self.elements(&source, element);
        let results: Vec<Term> = elements
            .into_iter()
            .map(|value| self.unary(name, value))
            .collect();
        let bytes = self.unelements(&results);
        self.masked_out(&bytes, element)
    }

    /// `ucomisd` and `comisd`, which differ only in the exceptions they signal: ZF, PF and CF
    /// all set when either is a NaN, else ZF for equal and CF for less; OF and SF clear.
    fn compare_flags(&mut self, name: &str) -> Result<(), Stop> {
        let element = element_of(name);
        let (a, b) = (self.low_element(0, element)?, self.low_element(1, element)?);
        let less = self.pool.float_test(FloatTest::Less, a, b);
        let equal = self.pool.float_test(FloatTest::Equal, a, b);
        let unordered = self.pool.float_test(FloatTest::Unordered, a, b);
        let no = self.pool.boolean(false);
        self.state.last = Last::Other;
        self.state.zero = self.pool.bool_or(equal, unordered);
        self.state.parity = unordered;
        self.state.carry = self.pool.bool_or(less, unordered);
        self.state.sign = no;
        self.state.overflow = no;
        Ok(())
    }

    /// A comparison's predicate: legacy SSE reads the immediate's low three bits, VEX and EVEX
    /// its low five.
    fn predicate(&self) -> u8 {
        let immediate = self.instruction.immediate8();
        if self.instruction.encoding() == EncodingKind::Legacy {
            immediate & 7
        } else {
            immediate & 31
        }
    }

    /// All ones in an element of `element` bytes where `condition` holds, else zeros.
    fn element_mask(&mut self, condition: Term, element: usize) -> Term {
        let width = u32::try_from(element * 8).unwrap_or(64);
        self.all_ones(condition, width)
    }

    fn compare_scalar(&mut self, name: &str) -> Result<(), Stop> {
        let element = element_of(name);
        let (first, second) = self.float_sources();
        let (a, b) = (
            self.low_element(first, element)?,
            self.low_element(second, element)?,
        );
        let holds = self.float_predicate(self.predicate(), a, b);
        if self.instruction.op_register(0).is_k() {
            return self.mask_out(&[holds]);
        }
        let result = self.element_mask(holds, element);
        let rest = if first == 0 {
            Rest::Kept
        } else {
            Rest::From(1)
        };
        self.scalar_out(result, element, rest)
    }

    fn compare_packed(&mut self, name: &str) -> Result<(), Stop> {
        let element = element_of(name);
        let into_mask = self.instruction.op_register(0).is_k();
        let size = if into_mask {
            self.operand_bytes(1)
        } else {
            self.operand_bytes(0)
        };
        let (first, second) = self.float_sources();
        let a = self.bytes_in(first, size)?;
        let b = self.masked_in(second, size, element)?;
        let (a, b) = (self.elements(&a, element), self.elements(&b, element));
        let predicate = self.predicate();
        let holds: Vec<Term> = a
            .iter()
            .zip(&b)
            .map(|(x, y)| self.float_predicate(predicate, *x, *y))
            .collect();
        if into_mask {
            return self.mask_out(&holds);
        }
        let masks: Vec<Term> = holds
            .iter()
            .map(|condition| self.element_mask(*condition, element))
            .collect();
        let bytes = self.unelements(&masks);
        self.masked_out(&bytes, element)
    }

    /// `cvtsi2sd`: a signed 32- or 64-bit whole number, from a register or memory, as the
    /// nearest float.
    fn whole_to_float(&mut self, name: &str) -> Result<(), Stop> {
        self.default_rounding()?;
        let element = element_of(name);
        let (source, rest) = self.unary_scalar();
        let width = self.width(source);
        let number = self.read(source, width)?;
        let result = self
            .pool
            .float_from_int(number, u32::try_from(element * 8).unwrap_or(64));
        self.scalar_out(result, element, rest)
    }

    /// `cvtsd2si`, rounding to nearest, and `cvttsd2si`, truncating: a float as a signed whole
    /// number of the destination's width, or the integer indefinite.
    fn float_to_whole(&mut self, name: &str) -> Result<(), Stop> {
        self.default_rounding()?;
        let element = if name.contains("sd2") { 8 } else { 4 };
        let truncate = name.contains("cvtt");
        let a = self.low_element(1, element)?;
        let to = self.width(0);
        let value = self.pool.float_to_int(a, to, truncate);
        self.write(0, value)
    }

    /// `cvtsd2ss` and `cvtss2sd`: one float as the nearest float of the other precision.
    fn scalar_precision(&mut self, name: &str) -> Result<(), Stop> {
        self.default_rounding()?;
        let (from, to) = if name.ends_with("sd2ss") {
            (8, 4)
        } else {
            (4, 8)
        };
        let (source, rest) = self.unary_scalar();
        let a = self.low_element(source, from)?;
        let result = self
            .pool
            .float_convert(a, u32::try_from(to * 8).unwrap_or(64));
        self.scalar_out(result, to, rest)
    }

    /// The packed conversions between whole numbers, singles and doubles, element for element:
    /// one that narrows its elements fills the low half of its destination and clears the rest.
    fn packed_conversion(&mut self, mnemonic: Mnemonic) -> Result<(), Stop> {
        self.default_rounding()?;
        let base = format!("{mnemonic:?}").to_lowercase();
        let base = base.trim_start_matches('v');
        let (from, to) = match base {
            "cvtdq2pd" | "cvtps2pd" => (4, 8),
            "cvtpd2dq" | "cvttpd2dq" | "cvtpd2ps" => (8, 4),
            _ => (4, 4),
        };
        let count = if to >= from {
            self.operand_bytes(0) / to
        } else {
            self.operand_bytes(1) / from
        };
        let source = self.masked_in(1, count * from, from)?;
        let elements = self.elements(&source, from);
        let width = u32::try_from(to * 8).unwrap_or(64);
        let truncate = base.starts_with("cvtt");
        let results: Vec<Term> = elements
            .into_iter()
            .map(|value| match base {
                "cvtdq2pd" | "cvtdq2ps" => self.pool.float_from_int(value, width),
                "cvtps2pd" | "cvtpd2ps" => self.pool.float_convert(value, width),
                _ => self.pool.float_to_int(value, width, truncate),
            })
            .collect();
        let mut bytes = self.unelements(&results);
        if self.instruction.encoding() == EncodingKind::Legacy {
            bytes.resize(16, self.pool.bv(8, 0));
            return self.bytes_out(0, &bytes);
        }
        self.masked_out(&bytes, to)
    }

    /// `vfmadd`: `a × b + c` rounded once, where the digits name the operands in that order: 132
    /// is operand 0 × operand 2 + operand 1. A NaN among them comes through quieted, the first
    /// in that order first, as the CPU picks.
    fn fused(&mut self, name: &str) -> Result<(), Stop> {
        self.default_rounding()?;
        let element = element_of(name);
        let order = &name[name.len() - 5..name.len() - 2];
        let (a, b, c) = match order {
            "132" => (0, 2, 1),
            "213" => (1, 0, 2),
            _ => (1, 2, 0),
        };
        if name.ends_with("sd") || name.ends_with("ss") {
            let x = self.low_element(a, element)?;
            let y = self.low_element(b, element)?;
            let z = self.low_element(c, element)?;
            let result = self.pool.float_fma(x, y, z);
            // The rest of the low 16 bytes is the destination's own.
            return self.scalar_out(result, element, Rest::From(0));
        }
        let size = self.operand_bytes(0);
        let mut operands = Vec::with_capacity(3);
        for index in [a, b, c] {
            let bytes = if index == 2 {
                self.masked_in(2, size, element)?
            } else {
                self.bytes_in(index, size)?
            };
            operands.push(self.elements(&bytes, element));
        }
        let results: Vec<Term> = (0..operands[0].len())
            .map(|at| {
                self.pool
                    .float_fma(operands[0][at], operands[1][at], operands[2][at])
            })
            .collect();
        let bytes = self.unelements(&results);
        self.masked_out(&bytes, element)
    }

    /// `movsd` and `movss` on floats: a store writes the low element; a load clears the rest
    /// of the low 16 bytes; between registers, legacy SSE keeps the destination's rest and VEX
    /// takes it from its middle operand.
    fn scalar_move(&mut self, name: &str) -> Result<(), Stop> {
        let element = element_of(name);
        let instruction = self.instruction;
        let vector = |index: u32| {
            instruction.op_kind(index) == OpKind::Register
                && instruction.op_register(index).is_xmm()
        };
        match (instruction.op_kind(0), instruction.op_count()) {
            (OpKind::Memory, 2) if vector(1) => {
                let value = self.bytes_in(1, element)?;
                self.masked_out(&value, element)
            }
            (OpKind::Register, 2) if vector(0) && instruction.op_kind(1) == OpKind::Memory => {
                let value = self.masked_in(1, element, element)?;
                let value = self.join(&value);
                self.scalar_out(value, element, Rest::Zero)
            }
            (OpKind::Register, 2) if vector(0) && vector(1) => {
                let value = self.low_element(1, element)?;
                self.scalar_out(value, element, Rest::Kept)
            }
            (OpKind::Register, 3) if vector(0) && vector(1) && vector(2) => {
                let value = self.low_element(2, element)?;
                self.scalar_out(value, element, Rest::From(1))
            }
            _ => Err(unsupported(format!("{name} that doesn't move a float"))),
        }
    }

    /// `movmskpd`: each element's sign bit, lowest element first, into a general register.
    fn sign_mask(&mut self, name: &str) -> Result<(), Stop> {
        let element = element_of(name);
        let size = self.operand_bytes(1);
        let bytes = self.bytes_in(1, size)?;
        let bits: Vec<Term> = bytes
            .chunks(element)
            .map(|chunk| self.pool.extract(chunk[element - 1], 7, 7))
            .collect();
        let joined = self.join(&bits);
        let width = self.pool.width(joined);
        let target = self.width(0);
        let value = self.pool.zero_extend(joined, target - width);
        self.write(0, value)
    }

    /// The instructions that work on each 16-byte lane of two sources: unpacking, shuffling by
    /// an immediate, and adding or subtracting neighboring elements.
    fn lanes(&mut self, name: &str) -> Result<(), Stop> {
        self.default_rounding()?;
        let element = element_of(name);
        let size = self.operand_bytes(0);
        let (first, second) = self.float_sources();
        // Elements move between places, so a mask doesn't spare any of a source's: it's read
        // whole.
        let a = self.bytes_in(first, size)?;
        let b = self.bytes_in(second, size)?;
        let (a, b) = (self.elements(&a, element), self.elements(&b, element));
        let per_lane = 16 / element;
        let base = name.trim_start_matches('v');
        let immediate = if base.starts_with("shuf") {
            self.instruction.immediate8()
        } else {
            0
        };
        let mut result = Vec::with_capacity(a.len());
        for lane in 0..size / 16 {
            let (x, y) = (
                &a[lane * per_lane..(lane + 1) * per_lane],
                &b[lane * per_lane..(lane + 1) * per_lane],
            );
            match (base, per_lane) {
                ("unpcklpd", _) => result.extend([x[0], y[0]]),
                ("unpckhpd", _) => result.extend([x[1], y[1]]),
                ("unpcklps", _) => result.extend([x[0], y[0], x[1], y[1]]),
                ("unpckhps", _) => result.extend([x[2], y[2], x[3], y[3]]),
                ("shufpd", _) => {
                    let bit = |at: usize| usize::from(immediate >> (2 * lane + at) & 1);
                    result.extend([x[bit(0)], y[bit(1)]]);
                }
                ("shufps", _) => {
                    let pick = |at: u32| usize::from(immediate >> (2 * at) & 3);
                    result.extend([x[pick(0)], x[pick(1)], y[pick(2)], y[pick(3)]]);
                }
                (_, 2) => {
                    let op = if base.starts_with("hadd") {
                        FloatOp::Add
                    } else {
                        FloatOp::Sub
                    };
                    let low = self.pool.float(op, x[0], x[1]);
                    let high = self.pool.float(op, y[0], y[1]);
                    result.extend([low, high]);
                }
                _ => {
                    let op = if base.starts_with("hadd") {
                        FloatOp::Add
                    } else {
                        FloatOp::Sub
                    };
                    for pair in [[x[0], x[1]], [x[2], x[3]], [y[0], y[1]], [y[2], y[3]]] {
                        result.push(self.pool.float(op, pair[0], pair[1]));
                    }
                }
            }
        }
        let bytes = self.unelements(&result);
        self.masked_out(&bytes, element)
    }

    /// `movhlps` moves a source's high 8 bytes into the low ones, and `movlhps` its low 8
    /// into the high ones; the VEX forms take the other half from their middle operand.
    fn halves_move(&mut self, name: &str) -> Result<(), Stop> {
        let high_to_low = name.trim_start_matches('v').starts_with("movhl");
        let legacy = self.instruction.encoding() == EncodingKind::Legacy;
        let (keep, source) = if legacy { (0, 1) } else { (1, 2) };
        let kept = self.bytes_in(keep, 16)?;
        let moved = self.bytes_in(source, 16)?;
        let bytes = if high_to_low {
            [&moved[8..], &kept[8..]].concat()
        } else {
            [&kept[..8], &moved[..8]].concat()
        };
        self.bytes_out(0, &bytes)
    }
}
