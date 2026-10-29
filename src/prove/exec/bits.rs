//! The bit instructions: bit counts (popcnt, lzcnt, tzcnt, bsf, bsr), the BMI1 and BMI2
//! operations, bit tests, double shifts, byte-swapping moves and the carry flag's own
//! instructions. Each is a term built from the operands' bits, so a count over input bytes stays
//! a sum the prover can compare with a formula's.

use iced_x86::{Mnemonic, OpKind, Register};

use super::{Cpu, Last, Step, Stop, slot, unsupported};
use crate::prove::term::Term;

/// The runs of set bits in the low `width` bits of `mask`, lowest first, as (lowest bit, length).
fn runs(mask: u128, width: u32) -> Vec<(u32, u32)> {
    let mut runs = Vec::new();
    let mut bit = 0;
    while bit < width {
        if mask >> bit & 1 == 0 {
            bit += 1;
            continue;
        }
        let start = bit;
        while bit < width && mask >> bit & 1 == 1 {
            bit += 1;
        }
        runs.push((start, bit - start));
    }
    runs
}

impl Cpu<'_> {
    /// A value of `width` bits the CPU doesn't define, which a proof must hold for whatever it is.
    fn undefined(&mut self, width: u32) -> Term {
        let name = format!("undefined{}", self.pool.len());
        self.pool.var(width, &name)
    }

    /// `value` with its bytes in the opposite order.
    pub(super) fn swap_bytes(&mut self, value: Term) -> Result<Term, Stop> {
        let width = self.pool.width(value);
        let mut swapped: Option<Term> = None;
        for byte in 0..width / 8 {
            let part = self.pool.extract(value, byte * 8 + 7, byte * 8);
            swapped = Some(match swapped {
                None => part,
                Some(high) => self.pool.concat(high, part),
            });
        }
        swapped.ok_or_else(|| unsupported("swapping the bytes of nothing"))
    }

    /// ZF and SF from `result`, CF and OF cleared and PF undefined: what most BMI operations do.
    fn bmi_flags(&mut self, result: Term) {
        let width = self.pool.width(result);
        let zero = self.pool.bv(width, 0);
        let no = self.pool.boolean(false);
        self.state.last = Last::Other;
        self.state.zero = self.pool.eq(result, zero);
        self.state.sign = self.bit(result, width - 1);
        self.state.carry = no;
        self.state.overflow = no;
        self.state.parity = self.unknown();
    }

    /// `popcnt`: the number of set bits, as their sum.
    pub(super) fn popcnt(&mut self) -> Result<Step, Stop> {
        let width = self.width(0);
        let source = self.read(1, width)?;
        let mut bits = Vec::with_capacity(width as usize);
        for bit in 0..width {
            let part = self.pool.extract(source, bit, bit);
            bits.push(self.pool.zero_extend(part, width - 1));
        }
        let count = self.pool.sum(width, &bits);
        let zero = self.pool.bv(width, 0);
        let no = self.pool.boolean(false);
        self.state.last = Last::Other;
        self.state.zero = self.pool.eq(source, zero);
        self.state.carry = no;
        self.state.sign = no;
        self.state.overflow = no;
        self.state.parity = no;
        self.write(0, count)?;
        Ok(Step::Next)
    }

    /// `tzcnt`, `lzcnt`, `bsf` and `bsr`: where the lowest or highest set bit is, as a choice
    /// per bit with the bit the instruction looks at first outermost.
    pub(super) fn find_bit(&mut self, mnemonic: Mnemonic) -> Result<Step, Stop> {
        let width = self.width(0);
        let source = self.read(1, width)?;
        let zero = self.pool.bv(width, 0);
        let empty = self.pool.eq(source, zero);
        let lowest = matches!(mnemonic, Mnemonic::Tzcnt | Mnemonic::Bsf);
        let order: Vec<u32> = if lowest {
            (0..width).rev().collect()
        } else {
            (0..width).collect()
        };
        // No bit set: tzcnt and lzcnt give the width; bsf and bsr leave the register undefined.
        let mut result = self.pool.bv(width, u128::from(width));
        for bit in order {
            let set = self.bit(source, bit);
            let at = if mnemonic == Mnemonic::Lzcnt {
                width - 1 - bit
            } else {
                bit
            };
            let at = self.pool.bv(width, u128::from(at));
            result = self.pool.ite(set, at, result);
        }
        let scans = matches!(mnemonic, Mnemonic::Bsf | Mnemonic::Bsr);
        self.state.last = Last::Other;
        self.state.overflow = self.unknown();
        self.state.sign = self.unknown();
        self.state.parity = self.unknown();
        if scans {
            self.state.zero = empty;
            self.state.carry = self.unknown();
        } else {
            // The count is 0 exactly when the first bit looked at is set.
            self.state.carry = empty;
            let first = if lowest { 0 } else { width - 1 };
            self.state.zero = self.bit(source, first);
        }
        self.write(0, result)?;
        if scans {
            // AMD documents the destination as kept, Intel as undefined: a proof holds for any.
            let index = slot(self.instruction.op_register(0))
                .ok_or_else(|| unsupported("a bit scan into an unusual register"))?;
            let written = self.state.registers[index];
            let anything = self.undefined(64);
            self.state.registers[index] = self.pool.ite(empty, anything, written);
        }
        Ok(Step::Next)
    }

    /// `andn`, `blsr`, `blsi`, `blsmsk`, `bzhi` and `bextr`.
    pub(super) fn bmi(&mut self, mnemonic: Mnemonic) -> Result<Step, Stop> {
        let width = self.width(0);
        let source = self.read(1, width)?;
        let zero = self.pool.bv(width, 0);
        let one = self.pool.bv(width, 1);
        let empty = self.pool.eq(source, zero);
        let result = match mnemonic {
            Mnemonic::Andn => {
                let other = self.read(2, width)?;
                let inverted = self.pool.not(source);
                let result = self.pool.and(inverted, other);
                self.bmi_flags(result);
                // Flags as `and` leaves them, so a branch reads them as a comparison.
                self.state.last = Last::Logic(result);
                result
            }
            Mnemonic::Blsr | Mnemonic::Blsi | Mnemonic::Blsmsk => {
                let below = self.pool.sub(source, one);
                let result = match mnemonic {
                    Mnemonic::Blsr => self.pool.and(source, below),
                    Mnemonic::Blsi => {
                        let negated = self.pool.neg(source);
                        self.pool.and(source, negated)
                    }
                    _ => self.pool.xor(source, below),
                };
                self.bmi_flags(result);
                self.state.carry = if mnemonic == Mnemonic::Blsi {
                    self.pool.bool_not(empty)
                } else {
                    empty
                };
                if mnemonic == Mnemonic::Blsmsk {
                    self.state.zero = self.pool.boolean(false);
                }
                result
            }
            Mnemonic::Bzhi => {
                // The bits from the index in the low byte up are cleared; an index past the
                // width keeps them all and sets CF. Shifting 1 that far gives 0, and 0 - 1 is
                // every bit, so one mask covers both.
                let index = self.read(2, width)?;
                let low = self.pool.extract(index, 7, 0);
                let index = self.pool.zero_extend(low, width - 8);
                let bit = self.pool.shl(one, index);
                let mask = self.pool.sub(bit, one);
                let result = self.pool.and(source, mask);
                let limit = self.pool.bv(width, u128::from(width));
                let inside = self.pool.ult(index, limit);
                self.bmi_flags(result);
                self.state.carry = self.pool.bool_not(inside);
                result
            }
            _ => {
                // bextr: the length in the second byte of bits from the start in the first,
                // zeros past the source's top.
                let control = self.read(2, width)?;
                let start = self.pool.extract(control, 7, 0);
                let start = self.pool.zero_extend(start, width - 8);
                let length = self.pool.extract(control, 15, 8);
                let length = self.pool.zero_extend(length, width - 8);
                let shifted = self.pool.lshr(source, start);
                let bit = self.pool.shl(one, length);
                let mask = self.pool.sub(bit, one);
                let result = self.pool.and(shifted, mask);
                self.bmi_flags(result);
                self.state.sign = self.unknown();
                result
            }
        };
        self.write(0, result)?;
        Ok(Step::Next)
    }

    /// `shlx`, `shrx`, `sarx` and `rorx`: shifts and a rotation that leave the flags alone, by
    /// a count that may depend on the input for the shifts.
    pub(super) fn shift_keeping_flags(&mut self, mnemonic: Mnemonic) -> Result<Step, Stop> {
        let width = self.width(0);
        let source = self.read(1, width)?;
        let result = if mnemonic == Mnemonic::Rorx {
            let count = self.instruction.immediate(2) & u64::from(width - 1);
            let count = u32::try_from(count).unwrap_or(0);
            if count == 0 {
                source
            } else {
                let low = self.pool.extract(source, count - 1, 0);
                let high = self.pool.extract(source, width - 1, count);
                self.pool.concat(low, high)
            }
        } else {
            let count = self.read(2, width)?;
            let mask = self.pool.bv(width, u128::from(width - 1));
            let count = self.pool.and(count, mask);
            match mnemonic {
                Mnemonic::Shlx => self.pool.shl(source, count),
                Mnemonic::Shrx => self.pool.lshr(source, count),
                _ => self.pool.ashr(source, count),
            }
        };
        self.write(0, result)?;
        Ok(Step::Next)
    }

    /// `mulx`: rdx (or edx) times the source, unsigned, the high half to the first operand and
    /// the low half to the second, with the flags left alone. One register for both gets the
    /// high half.
    pub(super) fn mulx(&mut self) -> Result<Step, Stop> {
        let width = self.width(0);
        let source = self.read(2, width)?;
        let implicit = if width == 64 {
            Register::RDX
        } else {
            Register::EDX
        };
        let factor = self
            .state
            .register(self.pool, implicit)
            .ok_or_else(|| unsupported("mulx"))?;
        let (wide_a, wide_b) = (
            self.pool.zero_extend(factor, width),
            self.pool.zero_extend(source, width),
        );
        let product = self.pool.mul(wide_a, wide_b);
        let low = self.pool.extract(product, width - 1, 0);
        let high = self.pool.extract(product, 2 * width - 1, width);
        self.write(1, low)?;
        self.write(0, high)?;
        Ok(Step::Next)
    }

    /// `adcx` and `adox`: an unsigned add with CF, or with OF, carrying into that flag alone.
    pub(super) fn add_with_flag(&mut self, mnemonic: Mnemonic) -> Result<Step, Stop> {
        let width = self.width(0);
        let (a, b) = (self.read(0, width)?, self.read(1, width)?);
        let flag = if mnemonic == Mnemonic::Adcx {
            self.state.carry
        } else {
            self.state.overflow
        };
        let (one, zero) = (self.pool.bv(width + 1, 1), self.pool.bv(width + 1, 0));
        let carry_in = self.pool.ite(flag, one, zero);
        let (wide_a, wide_b) = (self.pool.zero_extend(a, 1), self.pool.zero_extend(b, 1));
        let sum = self.pool.add(wide_a, wide_b);
        let sum = self.pool.add(sum, carry_in);
        let result = self.pool.extract(sum, width - 1, 0);
        let out = self.bit(sum, width);
        self.state.last = Last::Other;
        if mnemonic == Mnemonic::Adcx {
            self.state.carry = out;
        } else {
            self.state.overflow = out;
        }
        self.write(0, result)?;
        Ok(Step::Next)
    }

    /// `bt`, `bts`, `btr` and `btc`: CF gets the bit, which the last three set, clear or flip.
    pub(super) fn bit_test(&mut self, mnemonic: Mnemonic) -> Result<Step, Stop> {
        let instruction = self.instruction;
        let width = self.width(0);
        let offset = self.read(1, width)?;
        let (address, value, at) = if instruction.op_kind(0) == OpKind::Memory
            && instruction.op_kind(1) == OpKind::Register
        {
            // A register's offset reaches past the operand, either way, in whole operands.
            let offset = self
                .pool
                .constant(offset)
                .ok_or_else(|| unsupported("a bit test in memory at a place the input chooses"))?;
            let signed = if offset >> (width - 1) & 1 == 1 {
                i128::try_from(offset).unwrap_or(0) - (1_i128 << width)
            } else {
                i128::try_from(offset).unwrap_or(0)
            };
            let operand = i128::from(width);
            let (word, bit) = (signed.div_euclid(operand), signed.rem_euclid(operand));
            let step = u64::try_from(word.unsigned_abs() * operand.unsigned_abs() / 8)
                .map_err(|_| unsupported("a bit test too far from its operand"))?;
            let base = self.address()?;
            let address = if word < 0 {
                base.wrapping_sub(step)
            } else {
                base.wrapping_add(step)
            };
            let value = self.load(address, width / 8)?;
            let at = self.pool.bv(width, bit.unsigned_abs());
            (Some(address), value, at)
        } else {
            let value = self.read(0, width)?;
            let mask = self.pool.bv(width, u128::from(width - 1));
            (None, value, self.pool.and(offset, mask))
        };
        let shifted = self.pool.lshr(value, at);
        let carry = self.bit(shifted, 0);
        let changed = if mnemonic == Mnemonic::Bt {
            None
        } else {
            let one = self.pool.bv(width, 1);
            let single = self.pool.shl(one, at);
            Some(match mnemonic {
                Mnemonic::Bts => self.pool.or(value, single),
                Mnemonic::Btr => {
                    let others = self.pool.not(single);
                    self.pool.and(value, others)
                }
                _ => self.pool.xor(value, single),
            })
        };
        // ZF stays as it was.
        self.state.last = Last::Other;
        self.state.carry = carry;
        self.state.overflow = self.unknown();
        self.state.sign = self.unknown();
        self.state.parity = self.unknown();
        match (changed, address) {
            (Some(changed), Some(address)) => self.save(address, changed)?,
            (Some(changed), None) => self.write(0, changed)?,
            (None, _) => {}
        }
        Ok(Step::Next)
    }

    /// `shld` and `shrd`: the destination shifted with the source's bits coming in, by a known
    /// count.
    pub(super) fn double_shift(&mut self, mnemonic: Mnemonic) -> Result<Step, Stop> {
        let width = self.width(0);
        let destination = self.read(0, width)?;
        let source = self.read(1, width)?;
        let count = self.read(2, 8)?;
        let mask = if width == 64 { 63 } else { 31 };
        let count = u32::try_from(self.constant(count)? & mask).unwrap_or(0);
        if count == 0 {
            // The flags stay; a 32-bit register's upper half still clears.
            self.write(0, destination)?;
            return Ok(Step::Next);
        }
        self.state.last = Last::Other;
        if count > width {
            // Only a 16-bit operand can be shifted past its width, which leaves it undefined.
            let anything = self.undefined(width);
            self.state.carry = self.unknown();
            self.state.zero = self.unknown();
            self.state.sign = self.unknown();
            self.state.overflow = self.unknown();
            self.state.parity = self.unknown();
            self.write(0, anything)?;
            return Ok(Step::Next);
        }
        let (result, out) = if mnemonic == Mnemonic::Shld {
            let both = self.pool.concat(destination, source);
            let result = self
                .pool
                .extract(both, 2 * width - 1 - count, width - count);
            (result, self.bit(destination, width - count))
        } else {
            let both = self.pool.concat(source, destination);
            let result = self.pool.extract(both, count + width - 1, count);
            (result, self.bit(destination, count - 1))
        };
        self.state.carry = out;
        self.state.overflow = if count == 1 {
            let before = self.bit(destination, width - 1);
            let after = self.bit(result, width - 1);
            let same = self.pool.eq(before, after);
            self.pool.bool_not(same)
        } else {
            self.unknown()
        };
        self.result_flags(result);
        self.write(0, result)?;
        Ok(Step::Next)
    }

    /// `pdep` and `pext` with a mask the program fixes: each run of the mask's set bits takes
    /// the next of the source's low bits, or gives its own bits to them.
    pub(super) fn deposit(&mut self, mnemonic: Mnemonic) -> Result<Step, Stop> {
        let width = self.width(0);
        let source = self.read(1, width)?;
        let mask = self.read(2, width)?;
        let mask = self
            .pool
            .constant(mask)
            .ok_or_else(|| unsupported("pdep or pext with a mask that depends on the input"))?;
        // The result so far, from bit 0 up, and how many bits it has.
        let mut built: Option<Term> = None;
        let mut filled = 0;
        let mut taken = 0;
        for (low, length) in runs(mask, width) {
            let part = if mnemonic == Mnemonic::Pext {
                self.pool.extract(source, low + length - 1, low)
            } else {
                if low > filled {
                    let gap = self.pool.bv(low - filled, 0);
                    built = Some(match built {
                        None => gap,
                        Some(below) => self.pool.concat(gap, below),
                    });
                    filled = low;
                }
                self.pool.extract(source, taken + length - 1, taken)
            };
            built = Some(match built {
                None => part,
                Some(below) => self.pool.concat(part, below),
            });
            filled += length;
            taken += length;
        }
        let result = match built {
            None => self.pool.bv(width, 0),
            Some(built) if filled < width => self.pool.zero_extend(built, width - filled),
            Some(built) => built,
        };
        self.write(0, result)?;
        Ok(Step::Next)
    }

    /// `stc`, `clc` and `cmc`.
    pub(super) fn set_carry(&mut self, mnemonic: Mnemonic) {
        self.state.last = Last::Other;
        self.state.carry = match mnemonic {
            Mnemonic::Stc => self.pool.boolean(true),
            Mnemonic::Clc => self.pool.boolean(false),
            _ => self.pool.bool_not(self.state.carry),
        };
    }
}

#[cfg(test)]
mod tests {
    use super::runs;

    #[test]
    fn finds_the_runs_of_a_mask() {
        assert_eq!(runs(0, 32), Vec::<(u32, u32)>::new());
        assert_eq!(runs(0b1011_0110, 8), vec![(1, 2), (4, 2), (7, 1)]);
        assert_eq!(runs(u128::from(u64::MAX), 64), vec![(0, 64)]);
        assert_eq!(runs(1 << 40, 32), Vec::<(u32, u32)>::new());
    }
}
