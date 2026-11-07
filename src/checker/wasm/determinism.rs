//! Every computer computes the same bits: no relaxed SIMD and no atomics, whose results differ
//! between computers or need threads; no NaN's bits taken as a number, through `reinterpret` of
//! a float that may be NaN; only the saturating conversions to integers, never the trapping ones;
//! and every division guarded, its divisor checked not zero and, for a signed one, the most
//! negative number divided by -1 ruled out, or that last left to the proof.
//!
//! `reinterpret` is the one view of a NaN's bits the checks follow. A float stored to memory
//! and read back as an integer, or a float vector's lanes read as integers, aren't followed:
//! there the engine's canonical NaNs (`src/wasm/engine.rs`) keep the bits the same on every
//! computer, as the standard says.

use wasmparser::Operator;

use super::memory::{Analysis, Value};
use super::{Facts, Obligation};
use crate::checker::{Rule, Violation};

/// The prefix byte of the threads proposal's instructions, the atomics.
const ATOMIC_PREFIX: u8 = 0xFE;

/// The name of `operator` when it's relaxed SIMD.
fn relaxed(operator: &Operator<'_>) -> Option<&'static str> {
    use Operator as O;
    Some(match operator {
        O::I8x16RelaxedSwizzle => "i8x16.relaxed_swizzle",
        O::I32x4RelaxedTruncF32x4S => "i32x4.relaxed_trunc_f32x4_s",
        O::I32x4RelaxedTruncF32x4U => "i32x4.relaxed_trunc_f32x4_u",
        O::I32x4RelaxedTruncF64x2SZero => "i32x4.relaxed_trunc_f64x2_s_zero",
        O::I32x4RelaxedTruncF64x2UZero => "i32x4.relaxed_trunc_f64x2_u_zero",
        O::F32x4RelaxedMadd => "f32x4.relaxed_madd",
        O::F32x4RelaxedNmadd => "f32x4.relaxed_nmadd",
        O::F64x2RelaxedMadd => "f64x2.relaxed_madd",
        O::F64x2RelaxedNmadd => "f64x2.relaxed_nmadd",
        O::I8x16RelaxedLaneselect => "i8x16.relaxed_laneselect",
        O::I16x8RelaxedLaneselect => "i16x8.relaxed_laneselect",
        O::I32x4RelaxedLaneselect => "i32x4.relaxed_laneselect",
        O::I64x2RelaxedLaneselect => "i64x2.relaxed_laneselect",
        O::F32x4RelaxedMin => "f32x4.relaxed_min",
        O::F32x4RelaxedMax => "f32x4.relaxed_max",
        O::F64x2RelaxedMin => "f64x2.relaxed_min",
        O::F64x2RelaxedMax => "f64x2.relaxed_max",
        O::I16x8RelaxedQ15mulrS => "i16x8.relaxed_q15mulr_s",
        O::I16x8RelaxedDotI8x16I7x16S => "i16x8.relaxed_dot_i8x16_i7x16_s",
        O::I32x4RelaxedDotI8x16I7x16AddS => "i32x4.relaxed_dot_i8x16_i7x16_add_s",
        _ => return None,
    })
}

/// The name of `operator` when it's a conversion to an integer that traps, with the saturating
/// one to use instead.
fn trapping(operator: &Operator<'_>) -> Option<(&'static str, &'static str)> {
    use Operator as O;
    Some(match operator {
        O::I32TruncF32S => ("i32.trunc_f32_s", "i32.trunc_sat_f32_s"),
        O::I32TruncF32U => ("i32.trunc_f32_u", "i32.trunc_sat_f32_u"),
        O::I32TruncF64S => ("i32.trunc_f64_s", "i32.trunc_sat_f64_s"),
        O::I32TruncF64U => ("i32.trunc_f64_u", "i32.trunc_sat_f64_u"),
        O::I64TruncF32S => ("i64.trunc_f32_s", "i64.trunc_sat_f32_s"),
        O::I64TruncF32U => ("i64.trunc_f32_u", "i64.trunc_sat_f32_u"),
        O::I64TruncF64S => ("i64.trunc_f64_s", "i64.trunc_sat_f64_s"),
        O::I64TruncF64U => ("i64.trunc_f64_u", "i64.trunc_sat_f64_u"),
        _ => return None,
    })
}

/// A division or remainder: its name, its width in bits, and whether it divides with sign.
fn division(operator: &Operator<'_>) -> Option<(&'static str, u32, bool)> {
    use Operator as O;
    Some(match operator {
        O::I32DivS => ("i32.div_s", 32, true),
        O::I32DivU => ("i32.div_u", 32, false),
        O::I32RemS => ("i32.rem_s", 32, false),
        O::I32RemU => ("i32.rem_u", 32, false),
        O::I64DivS => ("i64.div_s", 64, true),
        O::I64DivU => ("i64.div_u", 64, false),
        O::I64RemS => ("i64.rem_s", 64, false),
        O::I64RemU => ("i64.rem_u", 64, false),
        _ => return None,
    })
}

/// The largest number of `bits` bits, unsigned: -1's bits.
fn all_ones(bits: u32) -> u64 {
    if bits >= 64 {
        u64::MAX
    } else {
        (1_u64 << bits) - 1
    }
}

/// Checks that every computer computes the same bits, and that no arithmetic traps.
pub fn check(
    module: &super::model::Module<'_>,
    analysis: &Analysis,
    violations: &mut Vec<Violation>,
    facts: &mut Facts,
) {
    let mut violation = |line: Option<usize>, problem: String, fix: String| {
        violations.push(Violation {
            rule: Rule::Dialect,
            line,
            problem,
            fix,
        });
    };
    for function in module.own() {
        let records = analysis
            .followed
            .get(&function.index)
            .map(|followed| followed.records.as_slice())
            .unwrap_or_default();
        for (at, op) in function.body.iter().enumerate() {
            let operator = &op.operator;
            let line = op.line;
            let start = usize::try_from(op.offset).ok();
            if start.and_then(|start| module.wasm.get(start)) == Some(&ATOMIC_PREFIX) {
                violation(
                    line,
                    "this is an atomic instruction, from WebAssembly's threads, which a program of \
                     one thread doesn't have"
                        .to_owned(),
                    "use plain loads and stores: the program is one thread".to_owned(),
                );
                continue;
            }
            if let Some(name) = relaxed(operator) {
                violation(
                    line,
                    format!("`{name}` is relaxed SIMD, whose results differ between computers"),
                    "use the deterministic SIMD128 instructions instead".to_owned(),
                );
                continue;
            }
            if let Some((name, saturating)) = trapping(operator) {
                violation(
                    line,
                    format!("`{name}` traps on NaN and on a number out of its range"),
                    format!("use `{saturating}`, which saturates instead"),
                );
                continue;
            }
            let operands = records
                .get(at)
                .and_then(Option::as_ref)
                .map(|record| record.operands.as_slice());
            if let Some(name) = match operator {
                Operator::I32ReinterpretF32 => Some("i32.reinterpret_f32"),
                Operator::I64ReinterpretF64 => Some("i64.reinterpret_f64"),
                _ => None,
            } {
                let not_nan = operands
                    .and_then(<[Value]>::first)
                    .is_some_and(|value| matches!(value, Value::Float { nan: false }));
                if operands.is_some() && !not_nan {
                    violation(
                        line,
                        format!("`{name}` looks at the bits of a float that may be NaN"),
                        "check that it isn't NaN first, `(br_if $nan (f64.ne (local.get $x) \
                         (local.get $x)))`, or keep floats as floats"
                            .to_owned(),
                    );
                }
                continue;
            }
            let Some((name, bits, signed)) = division(operator) else {
                continue;
            };
            // A division no path reaches can't trap.
            let Some([dividend, divisor]) = operands else {
                continue;
            };
            let divisor = divisor.range();
            if divisor.low == 0 {
                violation(
                    line,
                    format!("`{name}` divides by a value nothing checks isn't zero, which traps"),
                    "check the divisor first, `(br_if $bad (i32.eqz (local.get $d)))`, or divide \
                     by a named constant that isn't zero"
                        .to_owned(),
                );
                continue;
            }
            let minus_one = (divisor.low..=divisor.high).contains(&all_ones(bits));
            let most_negative = 1_u64 << (bits - 1);
            let dividend = dividend.range();
            let may_be_most_negative = (dividend.low..=dividend.high).contains(&most_negative);
            if signed && minus_one && may_be_most_negative {
                facts.obligations.push(Obligation {
                    line,
                    what: format!(
                        "`{name}` never divides the most negative number by -1, which traps"
                    ),
                    division: true,
                });
            }
        }
    }
}
