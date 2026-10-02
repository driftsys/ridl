//! Compiler-side decimal lattice preparation and dependency-free validation.

use num_bigint::BigInt;
use num_rational::BigRational;
use num_traits::{Signed, ToPrimitive};
use proc_macro2::{Literal, TokenStream};
use quote::quote;

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

fn split(value: &BigRational) -> Option<(Literal, Literal)> {
    let high = value.to_f64()?;
    if !high.is_finite() {
        return None;
    }
    let exact_high = BigRational::from_float(high)?;
    let low = (value - exact_high).to_f64()?;
    Some((Literal::f64_suffixed(high), Literal::f64_suffixed(low)))
}

/// Emits the invalidity predicate for a float's declared decimal lattice.
/// All exact arithmetic happens in the compiler; generated code uses only
/// primitive operations. Reducing the origin modulo the step preserves the
/// lattice and avoids cancellation against an unnecessarily large origin.
pub(crate) fn float_invalid(
    origin: Option<&str>,
    step: &str,
    value: &TokenStream,
    wire_f32: bool,
) -> TokenStream {
    let checked = if wire_f32 {
        quote! { #value as ::core::primitive::f32 as ::core::primitive::f64 }
    } else {
        quote! { #value }
    };
    let invalid = float_invalid_at_precision(origin, step, &quote! { __checked }, wire_f32);
    quote! {{
        let __checked: ::core::primitive::f64 = #checked;
        #invalid
    }}
}

fn float_invalid_at_precision(
    origin: Option<&str>,
    step: &str,
    value: &TokenStream,
    wire_f32: bool,
) -> TokenStream {
    let Some(step) = decimal(step).filter(|step| step.is_positive()) else {
        return quote! { true };
    };
    let Some(origin) = decimal(origin.unwrap_or("0")) else {
        return quote! { true };
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
        return quote! { !#value.is_finite() };
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
    let factor = Literal::f64_suffixed(factor);
    let original_step = Literal::f64_suffixed(original_step.min(f64::MAX));
    let Some((step_high, step_low)) = split(&scaled_step) else {
        // An overflowing positive step has only finitely many lattice points
        // that round to finite floats. Enumerate them in the compiler instead
        // of emitting an infinite literal or making every constructor fail.
        let candidates: Vec<_> = [-2i32, -1, 0, 1, 2]
            .into_iter()
            .filter_map(|index| {
                let point = &phase + &step * BigRational::from_integer(BigInt::from(index));
                let rounded = if wire_f32 {
                    point.to_f32().map(f64::from)
                } else {
                    point.to_f64()
                }?;
                rounded.is_finite().then(|| Literal::f64_suffixed(rounded))
            })
            .collect();
        if candidates.is_empty() {
            return quote! { true };
        }
        let epsilon = if wire_f32 {
            quote! { (::core::primitive::f32::EPSILON as ::core::primitive::f64) }
        } else {
            quote! { ::core::primitive::f64::EPSILON }
        };
        return quote! {
            !#value.is_finite() || ![#(#candidates),*].iter().any(|__point| {
                (#value - *__point).abs()
                    <= 4.0 * #epsilon * #value.abs().max(__point.abs())
            })
        };
    };
    let Some((phase_high, phase_low)) = split(&scaled_phase) else {
        return quote! { true };
    };
    let epsilon = if wire_f32 {
        quote! { (::core::primitive::f32::EPSILON as ::core::primitive::f64) }
    } else {
        quote! { ::core::primitive::f64::EPSILON }
    };
    let represented = if wire_f32 {
        quote! { #value as ::core::primitive::f32 }
    } else {
        quote! { #value }
    };
    let float = if wire_f32 {
        quote! { ::core::primitive::f32 }
    } else {
        quote! { ::core::primitive::f64 }
    };
    let nearest_at_precision = if wire_f32 {
        quote! { (__nearest / __factor / __rescale) as ::core::primitive::f32 as ::core::primitive::f64 }
    } else {
        quote! { __nearest / __factor / __rescale }
    };
    quote! {
        {
            let __factor: ::core::primitive::f64 = #factor;
            let __original_step: ::core::primitive::f64 = #original_step;
            let __step: ::core::primitive::f64 = #step_high;
            let __step_low: ::core::primitive::f64 = #step_low;
            let __phase: ::core::primitive::f64 = #phase_high;
            let __phase_low: ::core::primitive::f64 = #phase_low;
            if !#value.is_finite() {
                true
            } else {
                // Adjacent values define the actual rounding cell. Signed
                // zero has the same two adjacent magnitudes as positive zero.
                let __represented = (#represented).abs();
                let __bits = __represented.to_bits();
                let __below = if __bits == 0 {
                    -#float::from_bits(1)
                } else {
                    #float::from_bits(__bits - 1)
                };
                let __above = #float::from_bits(__bits + 1);
                let __lower_gap = (__represented - __below) as ::core::primitive::f64;
                let __upper_gap = if __above.is_finite() {
                    (__above - __represented) as ::core::primitive::f64
                } else {
                    __lower_gap
                };
                let __cell = (__lower_gap + __upper_gap) / 2.0;
                if __original_step < __cell {
                    false
                } else {
                    let __scaled_value = #value * __factor;
                    let __delta = __scaled_value - __phase;
                    let __quotient = if __delta.is_finite() {
                        __delta / __step
                    } else {
                        __scaled_value / __step - __phase / __step
                    };
                    // Remainder gives truncation without a std-only rounding
                    // method. Ties round away from zero, as for f64::round.
                    let __fraction = __quotient % 1.0;
                    let __index = __quotient - __fraction + if __fraction >= 0.5 {
                        1.0
                    } else if __fraction <= -0.5 {
                        -1.0
                    } else {
                        0.0
                    };
                    let __scale = #value.abs().max((__phase / __factor).abs()).max(__original_step.abs());
                    let __tolerance = (4.0 * #epsilon * __scale).min(__original_step / 4.0);
                    let mut __distance = ::core::primitive::f64::INFINITY;
                    // Scale large terms before reconstructing. A rounded high
                    // sum can otherwise overflow before its negative decimal
                    // residual brings the exact point back into finite range.
                    let __rescale = if __scaled_value.abs().max(__phase.abs()).max(__step.abs())
                        > ::core::primitive::f64::MAX / 4.0 {
                        0.25
                    } else {
                        1.0
                    };
                    // Dekker's two-product with mantissa splitting uses core
                    // operations only. Clearing low bits avoids the overflow
                    // of the traditional multiplication-based splitter.
                    let __product_with_error = |__left: ::core::primitive::f64, __right: ::core::primitive::f64| {
                        let __product = __left * __right;
                        let __left_high = ::core::primitive::f64::from_bits(__left.to_bits() & !((1u64 << 27) - 1));
                        let __right_high = ::core::primitive::f64::from_bits(__right.to_bits() & !((1u64 << 27) - 1));
                        let __left_low = __left - __left_high;
                        let __right_low = __right - __right_high;
                        let __error = ((__left_high * __right_high - __product)
                            + __left_high * __right_low + __left_low * __right_high)
                            + __left_low * __right_low;
                        (__product, __error)
                    };
                    // The estimate can select a neighboring lattice index
                    // through division rounding. Check both neighbors too.
                    for __candidate in [__index - 1.0, __index, __index + 1.0] {
                        let (__product, __product_error) = __product_with_error(__candidate, __step * __rescale);
                        let __scaled_phase = __phase * __rescale;
                        let __sum = __product + __scaled_phase;
                        let __part = __sum - __product;
                        let __sum_error = (__product - (__sum - __part)) + (__scaled_phase - __part);
                        let __low = __candidate * (__step_low * __rescale) + __phase_low * __rescale
                            + __product_error + __sum_error;
                        let __nearest = __sum + __low;
                        let __nearest = #nearest_at_precision;
                        if __nearest.is_finite() {
                            __distance = __distance.min((#value - __nearest).abs());
                        }
                    }
                    !__quotient.is_finite() || __distance > __tolerance
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::decimal;

    #[test]
    fn decimal_exponents_preserve_exact_values() {
        assert_eq!(decimal("0.0125"), decimal("125e-4"));
        assert_eq!(decimal("-100"), decimal("-1e2"));
        assert!(decimal("1e1000000").is_none());
    }
}
