//! Property-test value generators derived from a checked range — the shipped
//! "typl ranges are generators" feature (ADR-0004 §9, docs/ROADMAP.md epic
//! E1.18).
//!
//! A typl range is a specification of a value domain, so it is also a
//! specification of how to sample that domain. This module turns a checked
//! [`IntRange`] or [`FloatRange`] into `proptest` strategies and into the small
//! boundary and violation corpora a conformance test drives:
//!
//! - [`int_values`] / [`float_values`] — `proptest` strategies that draw only
//!   in-range values. A float range with a `step` draws only grid points
//!   `min + n·step`, so a generated float always satisfies both the range and
//!   the quantization (typl reference §4.3).
//! - [`boundary_values`] — the four edge samples `min`, `min+1`, `max-1`,
//!   `max`, which straddle the §4.2 width thresholds (the `[0..255]` versus
//!   `[0..256]` width flip).
//! - [`violations`] — the two just-outside samples `min-1` and `max+1`, the
//!   values a range must reject.
//!
//! This is the seed the E2.11 conformance-corpus generator grows from. The
//! strategies here draw values; running them against generated code is E2.11's
//! work.

use num_bigint::{BigInt, Sign};
use num_rational::BigRational;
use proptest::strategy::Strategy;

use crate::scalar::{ExactValue, FloatRange, IntRange};

/// A `proptest` strategy that draws `i64` values uniformly from the closed
/// range `[min..max]` (typl reference §4.2). Every drawn value satisfies the
/// range. The bounds are read from the checked [`IntRange`]; a bound outside
/// the `i64` domain (which the checker rejects as TYPL-111 before a value is
/// ever generated) saturates to the `i64` limit.
pub fn int_values(r: &IntRange) -> impl Strategy<Value = i64> {
    let lo = to_i64_saturating(&r.min);
    let hi = to_i64_saturating(&r.max);
    // A checked range has min <= max; order defensively so the strategy never
    // panics on an inverted range.
    let (lo, hi) = if lo <= hi { (lo, hi) } else { (hi, lo) };
    lo..=hi
}

/// A `proptest` strategy that draws `f64` values from the closed range
/// `[min..max]` (typl reference §4.3).
///
/// When the range carries a `step`, every drawn value is a grid point
/// `min + n·step` for an integer `n` with `0 <= n` and `min + n·step <= max`,
/// so a generated value satisfies both the range and the quantization. Without
/// a `step` the strategy draws uniformly across `[min..max]`.
pub fn float_values(r: &FloatRange) -> impl Strategy<Value = f64> {
    let min_f = to_f64(&r.min);
    let max_f = to_f64(&r.max);
    match &r.step {
        Some(step) => {
            let min = r.min.0.clone();
            let step = step.0.clone();
            // n_max = floor((max - min) / step): the highest grid index whose
            // value stays at or below `max`, so every drawn value is in range.
            let n_max = max_grid_index(&r.min.0, &r.max.0, &step);
            (0u64..=n_max)
                .prop_map(move |n| grid_value(&min, &step, n))
                .boxed()
        }
        // No step: uniform across the closed interval.
        None => {
            let (lo, hi) = if min_f <= max_f {
                (min_f, max_f)
            } else {
                (max_f, min_f)
            };
            (lo..=hi).boxed()
        }
    }
}

/// The four boundary samples of an integer range in table order — `min`,
/// `min+1`, `max-1`, `max` — filtered to the values that actually lie inside
/// `[min..max]` and deduplicated while preserving order. These straddle the
/// §4.2 width thresholds: `boundary_values` of `[0..255]` yields `255`
/// (`derive_int_width` → `U8`) whose successor `256` is the [`violations`]
/// sample that flips the width to `U16`.
pub fn boundary_values(r: &IntRange) -> Vec<i64> {
    let lo = to_i64_saturating(&r.min);
    let hi = to_i64_saturating(&r.max);
    let mut out = Vec::new();
    for candidate in [Some(lo), lo.checked_add(1), hi.checked_sub(1), Some(hi)]
        .into_iter()
        .flatten()
    {
        if (lo..=hi).contains(&candidate) && !out.contains(&candidate) {
            out.push(candidate);
        }
    }
    out
}

/// The two just-outside samples of an integer range — `min-1` and `max+1` —
/// each emitted only when it is representable in `i64` (so a range at the
/// `i64` floor or ceiling omits the sample that would overflow). These are the
/// values a range must reject, and they sit one step across a §4.2 width
/// threshold: `violations` of `[0..255]` yields `256`, whose range `[0..256]`
/// derives `U16`.
pub fn violations(r: &IntRange) -> Vec<i64> {
    let lo = to_i64_saturating(&r.min);
    let hi = to_i64_saturating(&r.max);
    let mut out = Vec::new();
    if let Some(below) = lo.checked_sub(1) {
        out.push(below);
    }
    if let Some(above) = hi.checked_add(1) {
        out.push(above);
    }
    out
}

/// `floor((max - min) / step)` as a saturating `u64` — the highest grid index
/// `n` whose value `min + n·step` stays at or below `max`. `step` is positive
/// (the checker rejects a non-positive step as TYPL-105 before a value is
/// generated); a non-positive step here yields `0` so the strategy still draws
/// the single value `min`.
fn max_grid_index(min: &BigRational, max: &BigRational, step: &BigRational) -> u64 {
    if step.numer().sign() != Sign::Plus {
        return 0;
    }
    let span = max - min;
    if span.numer().sign() == Sign::Minus {
        return 0;
    }
    // Integer (truncating, hence floor for a non-negative quotient) division of
    // the span by the step.
    let quotient = (span.numer() * step.denom()) / (span.denom() * step.numer());
    u64::try_from(&quotient).unwrap_or(u64::MAX)
}

/// The grid point `min + n·step`, computed exactly in rationals and converted
/// to `f64` only at the end.
fn grid_value(min: &BigRational, step: &BigRational, n: u64) -> f64 {
    let point = min + step * BigRational::from_integer(BigInt::from(n));
    rational_to_f64(&point)
}

/// Converts an [`ExactValue`] to the nearest `f64`.
fn to_f64(v: &ExactValue) -> f64 {
    rational_to_f64(&v.0)
}

/// Converts a rational to the nearest `f64` through its canonical decimal
/// string. A range or step literal always has a power-of-ten denominator, so
/// the decimal form is finite and Rust's correctly-rounded float parser yields
/// the nearest `f64`. The `numer/denom` fallback covers any non-terminating
/// rational a caller might construct directly.
fn rational_to_f64(value: &BigRational) -> f64 {
    let text = ExactValue(value.clone()).to_decimal_string();
    text.parse::<f64>().unwrap_or_else(|_| {
        let numer = value.numer().to_string().parse::<f64>().unwrap_or(f64::NAN);
        let denom = value.denom().to_string().parse::<f64>().unwrap_or(f64::NAN);
        numer / denom
    })
}

/// Converts an integer-valued [`ExactValue`] to `i64`, saturating to the `i64`
/// limit on either side for a value outside the domain.
fn to_i64_saturating(v: &ExactValue) -> i64 {
    let integer = v.0.to_integer();
    integer.to_string().parse::<i64>().unwrap_or_else(|_| {
        if integer.sign() == Sign::Minus {
            i64::MIN
        } else {
            i64::MAX
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scalar::{IntWidth, derive_int_width};

    fn exact(text: &str) -> ExactValue {
        ExactValue::parse(text).unwrap_or_else(|| panic!("`{text}` must parse"))
    }

    fn int_range(min: &str, max: &str) -> IntRange {
        IntRange {
            min: exact(min),
            max: exact(max),
        }
    }

    #[test]
    fn boundary_values_of_uint8_ceiling() {
        assert_eq!(
            boundary_values(&int_range("0", "255")),
            vec![0, 1, 254, 255]
        );
    }

    #[test]
    fn boundary_values_dedup_on_narrow_ranges() {
        // A single-value range collapses to just that value.
        assert_eq!(boundary_values(&int_range("5", "5")), vec![5]);
        // A two-value range yields both, with min+1 == max and max-1 == min
        // filtered by the dedup.
        assert_eq!(boundary_values(&int_range("5", "6")), vec![5, 6]);
    }

    #[test]
    fn violations_straddle_the_uint8_width_flip() {
        // [0..255] derives uint8; its violations are -1 and 256.
        assert_eq!(derive_int_width(&int_range("0", "255")), Ok(IntWidth::U8));
        assert_eq!(violations(&int_range("0", "255")), vec![-1, 256]);
        // 256 pushes the max out of uint8: [0..256] widens to uint16.
        assert_eq!(derive_int_width(&int_range("0", "256")), Ok(IntWidth::U16));
        // -1 pushes the min below zero: [-1..255] can no longer be unsigned,
        // so it derives the next signed width that holds 255, int16.
        assert_eq!(derive_int_width(&int_range("-1", "255")), Ok(IntWidth::I16));
    }

    #[test]
    fn violations_omit_unrepresentable_samples_at_the_i64_edges() {
        // At the i64 floor there is no representable min-1; only max+1 remains.
        let at_floor = int_range("-9223372036854775808", "0");
        assert_eq!(violations(&at_floor), vec![1]);
        // At the i64 ceiling there is no representable max+1; only min-1
        // remains.
        let at_ceiling = int_range("0", "9223372036854775807");
        assert_eq!(violations(&at_ceiling), vec![-1]);
    }

    #[test]
    fn grid_value_lands_on_exact_step_points() {
        // min + n*step at 0.1 granularity, checked at f64 precision.
        let min = exact("0.0").0;
        let step = exact("0.5").0;
        assert_eq!(grid_value(&min, &step, 0), 0.0);
        assert_eq!(grid_value(&min, &step, 1), 0.5);
        assert_eq!(grid_value(&min, &step, 500), 250.0);
    }

    #[test]
    fn max_grid_index_counts_whole_steps() {
        // [0.0..250.0 step 0.5]: 500 whole steps, index 0..=500 (501 values).
        assert_eq!(
            max_grid_index(&exact("0.0").0, &exact("250.0").0, &exact("0.5").0),
            500
        );
        // A step larger than the span leaves only the single value min.
        assert_eq!(
            max_grid_index(&exact("0.0").0, &exact("1.0").0, &exact("5.0").0),
            0
        );
    }
}

#[cfg(test)]
mod properties {
    use super::*;
    use crate::scalar::{ExactValue, FloatRange, IntRange};
    use num_bigint::BigInt;
    use num_rational::BigRational;
    use proptest::prelude::*;
    use proptest::strategy::ValueTree;
    use proptest::test_runner::TestRunner;

    fn int_range_from(a: i64, b: i64) -> IntRange {
        let (lo, hi) = if a <= b { (a, b) } else { (b, a) };
        IntRange {
            min: ExactValue(BigRational::from_integer(BigInt::from(lo))),
            max: ExactValue(BigRational::from_integer(BigInt::from(hi))),
        }
    }

    /// Draws one value from `strategy` using `runner`.
    fn draw<T>(strategy: impl Strategy<Value = T>, runner: &mut TestRunner) -> T {
        strategy
            .new_tree(runner)
            .expect("a range strategy always produces a value tree")
            .current()
    }

    proptest! {
        /// Every value drawn from an integer range's strategy lies inside the
        /// closed range — the generator never produces an out-of-range value.
        #[test]
        fn int_values_stay_in_range(a in i64::MIN..=i64::MAX, b in i64::MIN..=i64::MAX) {
            let range = int_range_from(a, b);
            let lo = super::to_i64_saturating(&range.min);
            let hi = super::to_i64_saturating(&range.max);

            let mut runner = TestRunner::default();
            for _ in 0..64 {
                let value = draw(int_values(&range), &mut runner);
                prop_assert!((lo..=hi).contains(&value), "{value} out of [{lo}..{hi}]");
            }
        }

        /// Every value drawn from a stepped float range is a grid point
        /// `min + n*step` within `[min..max]`: it satisfies both the range and
        /// the quantization (typl reference §4.3).
        #[test]
        fn float_values_are_in_range_grid_points(
            min_num in -1000i64..=1000,
            step_den in 1i64..=1000,
            count in 1u64..=100_000,
        ) {
            // min = min_num, step = 1/step_den, max = min + (count-1)*step.
            let min_rat = BigRational::from_integer(BigInt::from(min_num));
            let step_rat = BigRational::new(BigInt::from(1), BigInt::from(step_den));
            let max_rat = &min_rat + &step_rat * BigRational::from_integer(BigInt::from(count - 1));
            let range = FloatRange {
                min: ExactValue(min_rat.clone()),
                max: ExactValue(max_rat.clone()),
                step: Some(ExactValue(step_rat.clone())),
            };

            let min_f = super::to_f64(&range.min);
            let max_f = super::to_f64(&range.max);
            let step_f = super::to_f64(range.step.as_ref().unwrap());

            let mut runner = TestRunner::default();
            for _ in 0..64 {
                let value = draw(float_values(&range), &mut runner);

                // Range: the value is within the closed interval.
                prop_assert!(value >= min_f && value <= max_f, "{value} out of [{min_f}..{max_f}]");

                // Quantization: recover the grid index and confirm the value is
                // exactly the grid point at that index (self-consistent at f64
                // precision), and that the index is a whole, non-negative step
                // count within the range.
                let n_approx = ((value - min_f) / step_f).round();
                prop_assert!(n_approx >= 0.0, "grid index {n_approx} is negative");
                let n = n_approx as u64;
                prop_assert!(n < count, "grid index {n} past the last value {}", count - 1);
                prop_assert_eq!(value, super::grid_value(&min_rat, &step_rat, n));
            }
        }

        /// Boundary samples are always inside the range; violation samples are
        /// always outside it. This is the property the E2.11 conformance corpus
        /// relies on.
        #[test]
        fn boundary_inside_and_violations_outside(a in -100_000i64..=100_000, b in -100_000i64..=100_000) {
            let range = int_range_from(a, b);
            let lo = super::to_i64_saturating(&range.min);
            let hi = super::to_i64_saturating(&range.max);

            for value in boundary_values(&range) {
                prop_assert!((lo..=hi).contains(&value), "boundary {value} not in [{lo}..{hi}]");
            }
            for value in violations(&range) {
                prop_assert!(!(lo..=hi).contains(&value), "violation {value} inside [{lo}..{hi}]");
            }
        }
    }
}
