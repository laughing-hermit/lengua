//! AArch64's SVE in the prover's model: each Z register its CPU's length of bytes, up to 64,
//! kept where the V registers are, its lowest 16 the V register; each predicate a condition for
//! each of those bytes. Every element is a byte, as the dialect has them. A load or store is held
//! to the longest vector's width, as the rules hold it, and reaches only its active lanes.

use std::rc::Rc;

use super::super::{Cpu, End, Last, Step, Stop};
use crate::isa::aarch64::sve::{Against, Cmp, PLogic, Sve, ZLogic};
use crate::isa::aarch64::{Address, Extend};
use crate::platform::SVE_MAX_BYTES;
use crate::prove::term::Term;

impl Cpu<'_> {
    /// The vector length, in bytes and so in lanes.
    fn sve_lanes(&self) -> usize {
        usize::from(self.image.sve_bytes)
    }

    /// Z`number`'s bytes, lowest first.
    fn z(&self, number: u8) -> Vec<Term> {
        self.core.vectors[usize::from(number)][..self.sve_lanes()].to_vec()
    }

    /// Sets Z`number` to `bytes` from its lowest, zeros above them.
    fn set_z(&mut self, number: u8, bytes: &[Term]) {
        let zero = self.pool.bv(8, 0);
        let vector = Rc::make_mut(&mut self.core.vectors[usize::from(number)]);
        for (at, byte) in vector.iter_mut().enumerate() {
            *byte = bytes.get(at).copied().unwrap_or(zero);
        }
    }

    /// P`number`'s lanes, lowest first.
    fn p(&self, number: u8) -> Vec<Term> {
        self.core.predicates[usize::from(number)][..self.sve_lanes()].to_vec()
    }

    /// Sets P`number` to `lanes`, inactive above them.
    fn set_p(&mut self, number: u8, lanes: &[Term]) {
        let no = self.pool.boolean(false);
        let predicate = Rc::make_mut(&mut self.core.predicates[usize::from(number)]);
        for (at, lane) in predicate.iter_mut().enumerate() {
            *lane = lanes.get(at).copied().unwrap_or(no);
        }
    }

    /// The flags a predicate's result sets over `mask`'s lanes, as the architecture's
    /// `PredTest` does: N whether the first active lane is set, Z whether none is, C whether the
    /// last active lane isn't, V clear. `result` is already zero outside `mask`.
    fn predicate_flags(&mut self, mask: &[Term], result: &[Term]) {
        let no = self.pool.boolean(false);
        let (mut first, mut seen) = (no, no);
        for (lane, set) in mask.iter().zip(result) {
            let before = self.pool.bool_not(seen);
            let fresh = self.pool.bool_and(*lane, before);
            let here = self.pool.bool_and(fresh, *set);
            first = self.pool.bool_or(first, here);
            seen = self.pool.bool_or(seen, *lane);
        }
        let (mut last, mut later) = (no, no);
        for (lane, set) in mask.iter().zip(result).rev() {
            let after = self.pool.bool_not(later);
            let fresh = self.pool.bool_and(*lane, after);
            let here = self.pool.bool_and(fresh, *set);
            last = self.pool.bool_or(last, here);
            later = self.pool.bool_or(later, *lane);
        }
        let mut any = no;
        for set in result {
            any = self.pool.bool_or(any, *set);
        }
        self.core.sign = first;
        self.core.zero = self.pool.bool_not(any);
        self.core.carry = self.pool.bool_not(last);
        self.core.overflow = no;
        self.core.last = Last::Other;
    }

    /// The test `cmp` of two bytes.
    fn sve_test(&mut self, cmp: Cmp, a: Term, b: Term) -> Term {
        match cmp {
            Cmp::Eq => self.pool.eq(a, b),
            Cmp::Ne => {
                let equal = self.pool.eq(a, b);
                self.pool.bool_not(equal)
            }
            Cmp::Hs => self.pool.ule(b, a),
            Cmp::Hi => self.pool.ult(b, a),
            Cmp::Lo => self.pool.ult(a, b),
            Cmp::Ls => self.pool.ule(a, b),
            Cmp::Ge => self.pool.sle(b, a),
            Cmp::Gt => self.pool.slt(b, a),
            Cmp::Lt => self.pool.slt(a, b),
            Cmp::Le => self.pool.sle(a, b),
        }
    }

    /// A general register's low byte.
    fn low_byte(&mut self, register: crate::isa::aarch64::A64Reg) -> Term {
        let value = self.x(register);
        self.pool.extract(value, 7, 0)
    }

    /// Runs the SVE instruction `sve`.
    #[allow(clippy::too_many_lines)]
    pub(super) fn execute_sve(&mut self, sve: Sve) -> Result<Step, Stop> {
        let lanes = self.sve_lanes();
        if lanes == 0 {
            return Err(Stop::from(End::Unsupported(
                "the program uses SVE, and the proof wasn't given the CPU's vector length".into(),
            )));
        }
        let zero = self.pool.bv(8, 0);
        match sve {
            Sve::Ptrue { pd } => {
                let yes = self.pool.boolean(true);
                self.set_p(pd, &vec![yes; lanes]);
            }
            Sve::Whilelo { pd, rn, rm } => {
                let (n, m) = (self.x(rn), self.x(rm));
                let below = self.pool.ult(n, m);
                let room = self.pool.sub(m, n);
                let mut result = Vec::with_capacity(lanes);
                for lane in 0..lanes {
                    let place = self.pool.bv(64, lane as u128);
                    let inside = self.pool.ult(place, room);
                    result.push(self.pool.bool_and(below, inside));
                }
                let all = vec![self.pool.boolean(true); lanes];
                self.predicate_flags(&all, &result);
                self.set_p(pd, &result);
            }
            Sve::PredLogic { op, pd, pg, pn, pm } => {
                let (governing, a, b) = (self.p(pg), self.p(pn), self.p(pm));
                let mut result = Vec::with_capacity(lanes);
                for lane in 0..lanes {
                    let value = match op {
                        PLogic::And => self.pool.bool_and(a[lane], b[lane]),
                        PLogic::Bic => {
                            let not = self.pool.bool_not(b[lane]);
                            self.pool.bool_and(a[lane], not)
                        }
                        PLogic::Eor => self.pool.bool_xor(a[lane], b[lane]),
                        PLogic::Orr => self.pool.bool_or(a[lane], b[lane]),
                    };
                    result.push(self.pool.bool_and(governing[lane], value));
                }
                self.set_p(pd, &result);
            }
            Sve::Cntp { rd, pg, pn } => {
                let (governing, counted) = (self.p(pg), self.p(pn));
                let both: Vec<Term> = governing
                    .iter()
                    .zip(&counted)
                    .map(|(a, b)| self.pool.bool_and(*a, *b))
                    .collect();
                let count = self.pool.count(64, &both);
                self.set_x(rd, count);
            }
            Sve::Access {
                store,
                zt,
                pg,
                base,
                index,
            } => {
                let address = Address::Register {
                    base,
                    index,
                    extend: Extend::Lsl,
                    shift: 0,
                };
                let held = self.held_a64(address);
                let (at, _) = self.address_of(address);
                let active = self.p(pg);
                // The longest vector's width, 16 bytes at a time, as memory holds values.
                let pieces = usize::from(SVE_MAX_BYTES) / 16;
                let mut places = Vec::with_capacity(pieces);
                let mut old = Vec::with_capacity(pieces * 16);
                for piece in 0..pieces {
                    let offset = self.pool.bv(64, (piece * 16) as u128);
                    let place = self.pool.add(at, offset);
                    let value = self.load_from(place, held, 16)?;
                    old.extend(self.split(value, 16));
                    places.push(place);
                }
                if store {
                    let bytes = self.z(zt);
                    let mut new = old.clone();
                    for lane in 0..lanes {
                        new[lane] = self.pool.ite(active[lane], bytes[lane], old[lane]);
                    }
                    for (piece, place) in places.into_iter().enumerate() {
                        let value = self.join(&new[piece * 16..piece * 16 + 16]);
                        self.store_to(place, held, value, None)?;
                    }
                } else {
                    let bytes: Vec<Term> = (0..lanes)
                        .map(|lane| self.pool.ite(active[lane], old[lane], zero))
                        .collect();
                    self.set_z(zt, &bytes);
                }
            }
            Sve::Compare {
                cmp,
                pd,
                pg,
                zn,
                against,
            } => {
                let (governing, a) = (self.p(pg), self.z(zn));
                let b = match against {
                    Against::Vector(zm) => self.z(zm),
                    Against::Number(number) => {
                        vec![self.pool.bv_signed(8, i128::from(number)); lanes]
                    }
                };
                let mut result = Vec::with_capacity(lanes);
                for lane in 0..lanes {
                    let test = self.sve_test(cmp, a[lane], b[lane]);
                    result.push(self.pool.bool_and(governing[lane], test));
                }
                self.predicate_flags(&governing, &result);
                self.set_p(pd, &result);
            }
            Sve::Arith { sub, zd, zn, zm } => {
                let (a, b) = (self.z(zn), self.z(zm));
                let bytes: Vec<Term> = (0..lanes)
                    .map(|lane| {
                        if sub {
                            self.pool.sub(a[lane], b[lane])
                        } else {
                            self.pool.add(a[lane], b[lane])
                        }
                    })
                    .collect();
                self.set_z(zd, &bytes);
            }
            Sve::Logic { op, zd, zn, zm } => {
                let (a, b) = (self.z(zn), self.z(zm));
                let bytes: Vec<Term> = (0..lanes)
                    .map(|lane| match op {
                        ZLogic::And => self.pool.and(a[lane], b[lane]),
                        ZLogic::Orr => self.pool.or(a[lane], b[lane]),
                        ZLogic::Eor => self.pool.xor(a[lane], b[lane]),
                        ZLogic::Bic => {
                            let not = self.pool.not(b[lane]);
                            self.pool.and(a[lane], not)
                        }
                    })
                    .collect();
                self.set_z(zd, &bytes);
            }
            Sve::Cnt { zd, pg, zn } => {
                let (active, a, old) = (self.p(pg), self.z(zn), self.z(zd));
                let mut bytes = Vec::with_capacity(lanes);
                for lane in 0..lanes {
                    let bits: Vec<Term> = (0..8)
                        .map(|bit| self.pool.bit_is_set(a[lane], bit))
                        .collect();
                    let count = self.pool.count(8, &bits);
                    bytes.push(self.pool.ite(active[lane], count, old[lane]));
                }
                self.set_z(zd, &bytes);
            }
            Sve::Sel { zd, pg, zn, zm } => {
                let (active, a, b) = (self.p(pg), self.z(zn), self.z(zm));
                let bytes: Vec<Term> = (0..lanes)
                    .map(|lane| self.pool.ite(active[lane], a[lane], b[lane]))
                    .collect();
                self.set_z(zd, &bytes);
            }
            Sve::DupImm { zd, imm } => {
                let byte = self.pool.bv(8, u128::from(imm.cast_unsigned()));
                self.set_z(zd, &vec![byte; lanes]);
            }
            Sve::DupReg { zd, rn } => {
                let byte = self.low_byte(rn);
                self.set_z(zd, &vec![byte; lanes]);
            }
            Sve::Cpy { zd, pg, merge, imm } => {
                let (active, old) = (self.p(pg), self.z(zd));
                let byte = self.pool.bv(8, u128::from(imm.cast_unsigned()));
                let bytes: Vec<Term> = (0..lanes)
                    .map(|lane| {
                        let other = if merge { old[lane] } else { zero };
                        self.pool.ite(active[lane], byte, other)
                    })
                    .collect();
                self.set_z(zd, &bytes);
            }
            Sve::Uaddv { vd, pg, zn } => {
                let (active, a) = (self.p(pg), self.z(zn));
                let mut sum = self.pool.bv(64, 0);
                let none = self.pool.bv(64, 0);
                for lane in 0..lanes {
                    let wide = self.pool.zero_extend(a[lane], 56);
                    let added = self.pool.ite(active[lane], wide, none);
                    sum = self.pool.add(sum, added);
                }
                let bytes = self.split(sum, 8);
                self.set_z(vd, &bytes);
            }
            Sve::Count { rd, add, times } => {
                let length = self.pool.bv(64, (lanes * usize::from(times)) as u128);
                let value = if add {
                    let old = self.x(rd);
                    self.pool.add(old, length)
                } else {
                    length
                };
                self.set_x(rd, value);
            }
            Sve::Insr { zdn, rm } => {
                let old = self.z(zdn);
                let mut bytes = vec![self.low_byte(rm)];
                bytes.extend_from_slice(&old[..lanes - 1]);
                self.set_z(zdn, &bytes);
            }
            Sve::Lastb { rd, pg, zn } => {
                let (active, a) = (self.p(pg), self.z(zn));
                // The last active lane's byte; with none active, the last lane's.
                let mut value = a[lanes - 1];
                for lane in 0..lanes {
                    value = self.pool.ite(active[lane], a[lane], value);
                }
                let wide = self.pool.zero_extend(value, 24);
                self.set_x(rd, wide);
            }
        }
        Ok(Step::Next)
    }
}
