//! The compile-time preparation of a float `step` lattice (typl §4.3), shared
//! by the backends that emit a run-time check for it.
//!
//! The declared origin and step are exact decimals. A binary64 value cannot
//! hold them, so the preparation reduces the origin modulo the step and splits
//! the phase and the step each into a high and a low binary64 part. The
//! generated check recovers the nearest lattice point from those parts with
//! primitive operations only. Every backend that emits the check calls
//! [`prepare_float_step`], so two backends cannot prepare different constants.

use num_bigint::BigInt;
use num_rational::BigRational;
use num_traits::{Signed, ToPrimitive};

/// What a generated check needs to decide whether a float is off its lattice.
#[derive(Debug, Clone, PartialEq)]
pub enum FloatStepPlan {
    /// The declared lattice cannot be read, or has no representable point:
    /// every value is off the lattice.
    Never,
    /// The lattice is finer than every rounding cell of the wire precision:
    /// every finite value is on the lattice.
    Finite,
    /// The step overflows binary64. These are the finite lattice points
    /// nearest the origin, rounded to the wire precision. A value is on the
    /// lattice when it is within a relative tolerance of one of them.
    Points(Vec<f64>),
    /// The general case.
    Lattice(FloatLattice),
}

/// The constants of the general case. `factor` is a power of two that scales a
/// subnormal-sized step into the normal range, or 1.
#[derive(Debug, Clone, PartialEq)]
pub struct FloatLattice {
    pub factor: f64,
    /// The declared step rounded to binary64, before scaling.
    pub original_step: f64,
    pub step_high: f64,
    pub step_low: f64,
    pub phase_high: f64,
    pub phase_low: f64,
}

/// Reads exact decimal text, including exponent spellings in hand-built IR.
fn decimal(text: &str) -> Option<BigRational> {
    let (mantissa, exponent) = text.split_once(['e', 'E']).unwrap_or((text, "0"));
    let exponent: i32 = exponent.parse().ok()?;
    let (whole, fraction) = mantissa.split_once('.').unwrap_or((mantissa, ""));
    let coefficient: BigInt = format!("{whole}{fraction}").parse().ok()?;
    let scale = i32::try_from(fraction.len()).ok()?.checked_sub(exponent)?;
    // Canonical source decimals are bounded by the source itself. Guard
    // exponent spellings supplied through IR against an excessive allocation.
    if exponent.unsigned_abs() > 10000 {
        return None;
    }
    let power = BigInt::from(10).pow(scale.unsigned_abs());
    Some(if scale >= 0 {
        BigRational::new(coefficient, power)
    } else {
        BigRational::from_integer(coefficient * power)
    })
}

fn split(value: &BigRational) -> Option<(f64, f64)> {
    let high = value.to_f64()?;
    if !high.is_finite() {
        return None;
    }
    let exact_high = BigRational::from_float(high)?;
    let low = (value - exact_high).to_f64()?;
    Some((high, low))
}

/// Prepares the lattice `origin + k * step` for a float of the given wire
/// precision. `origin` is the declared minimum, or `None` for origin 0.
/// Reducing the origin modulo the step preserves the lattice and avoids
/// cancellation against an unnecessarily large origin.
pub fn prepare_float_step(origin: Option<&str>, step: &str, wire_f32: bool) -> FloatStepPlan {
    let Some(step) = decimal(step).filter(|step| step.is_positive()) else {
        return FloatStepPlan::Never;
    };
    let Some(origin) = decimal(origin.unwrap_or("0")) else {
        return FloatStepPlan::Never;
    };
    let quotient = &origin / &step;
    let whole = quotient.numer() / quotient.denom();
    let mut phase = &origin - &step * BigRational::from_integer(whole);
    if phase.is_negative() {
        phase += &step;
    }
    let original_step = step.to_f64().unwrap_or(f64::INFINITY);
    if original_step == 0.0 {
        // The exact positive lattice is finer than every binary64 rounding cell.
        return FloatStepPlan::Finite;
    }
    // Subnormal coefficients lose their residual when split directly. Exact
    // power-of-two scaling preserves that residual in the normal range.
    let factor = if original_step < f64::MIN_POSITIVE {
        f64::MIN_POSITIVE.recip()
    } else {
        1.0
    };
    let exact_factor = BigRational::from_float(factor).expect("the scale is finite");
    let scaled_step = &step * &exact_factor;
    let scaled_phase = &phase * &exact_factor;
    let Some((step_high, step_low)) = split(&scaled_step) else {
        // An overflowing positive step has only finitely many lattice points
        // that round to finite floats. Enumerate them in the compiler instead
        // of emitting an infinite literal or making every constructor fail.
        let points: Vec<f64> = [-2i32, -1, 0, 1, 2]
            .into_iter()
            .filter_map(|index| {
                let point = &phase + &step * BigRational::from_integer(BigInt::from(index));
                let rounded = if wire_f32 {
                    point.to_f32().map(f64::from)
                } else {
                    point.to_f64()
                }?;
                rounded.is_finite().then_some(rounded)
            })
            .collect();
        return if points.is_empty() {
            FloatStepPlan::Never
        } else {
            FloatStepPlan::Points(points)
        };
    };
    let Some((phase_high, phase_low)) = split(&scaled_phase) else {
        return FloatStepPlan::Never;
    };
    FloatStepPlan::Lattice(FloatLattice {
        factor,
        original_step: original_step.min(f64::MAX),
        step_high,
        step_low,
        phase_high,
        phase_low,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decimal_exponents_preserve_exact_values() {
        assert_eq!(decimal("0.0125"), decimal("125e-4"));
        assert_eq!(decimal("-100"), decimal("-1e2"));
        assert!(decimal("1e1000000").is_none());
    }

    #[test]
    fn a_non_positive_or_unreadable_step_has_no_lattice() {
        for step in ["0", "-1", "x"] {
            assert_eq!(prepare_float_step(None, step, false), FloatStepPlan::Never);
        }
        assert_eq!(
            prepare_float_step(Some("y"), "1", false),
            FloatStepPlan::Never
        );
    }

    #[test]
    fn a_step_finer_than_binary64_accepts_every_finite_value() {
        assert_eq!(
            prepare_float_step(None, "1e-400", false),
            FloatStepPlan::Finite
        );
    }

    #[test]
    fn a_step_beyond_binary64_enumerates_its_finite_points() {
        let FloatStepPlan::Points(points) = prepare_float_step(Some("0"), "1e400", false) else {
            panic!("expected points");
        };
        assert_eq!(points, vec![0.0]);
    }

    #[test]
    fn the_phase_is_the_origin_reduced_modulo_the_step() {
        let FloatStepPlan::Lattice(lattice) = prepare_float_step(Some("-1.0"), "0.25", false)
        else {
            panic!("expected a lattice");
        };
        assert_eq!(lattice.phase_high, 0.0);
        assert_eq!(lattice.step_high, 0.25);
        assert_eq!(lattice.factor, 1.0);
        let FloatStepPlan::Lattice(lattice) = prepare_float_step(Some("0.1"), "0.25", false) else {
            panic!("expected a lattice");
        };
        assert_eq!(lattice.phase_high, 0.1);
        assert_ne!(lattice.phase_low, 0.0);
    }

    #[test]
    fn a_negative_origin_that_is_not_a_multiple_gets_a_positive_phase() {
        let FloatStepPlan::Lattice(lattice) = prepare_float_step(Some("-1.05"), "0.1", false)
        else {
            panic!("expected a lattice");
        };
        assert!((lattice.phase_high - 0.05).abs() < 1e-15, "{lattice:?}");
    }

    #[test]
    fn points_are_rounded_to_the_wire_precision() {
        let FloatStepPlan::Points(narrow) = prepare_float_step(Some("0.1"), "1e400", true) else {
            panic!("expected points");
        };
        let FloatStepPlan::Points(wide) = prepare_float_step(Some("0.1"), "1e400", false) else {
            panic!("expected points");
        };
        assert_eq!(narrow, vec![f64::from(0.1f32)]);
        assert_eq!(wide, vec![0.1]);
    }
}
