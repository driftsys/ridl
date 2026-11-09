//! Compiler-side decimal lattice preparation and dependency-free validation.

use proc_macro2::{Literal, TokenStream};
use quote::quote;
use ridl_ir::step::{FloatLattice, FloatStepPlan, prepare_float_step};

/// Clears the IEEE-754 sign bit using operations available in core on Rust 1.83.
fn absolute(value: &TokenStream, wire_f32: bool) -> TokenStream {
    if wire_f32 {
        quote! { ::core::primitive::f32::from_bits((#value).to_bits() & 0x7fff_ffffu32) }
    } else {
        quote! { ::core::primitive::f64::from_bits((#value).to_bits() & 0x7fff_ffff_ffff_ffffu64) }
    }
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
    let lattice = match prepare_float_step(origin, step, wire_f32) {
        FloatStepPlan::Never => return quote! { true },
        FloatStepPlan::Finite => return quote! { !#value.is_finite() },
        FloatStepPlan::Points(points) => {
            let candidates: Vec<_> = points.into_iter().map(Literal::f64_suffixed).collect();
            let epsilon = if wire_f32 {
                quote! { (::core::primitive::f32::EPSILON as ::core::primitive::f64) }
            } else {
                quote! { ::core::primitive::f64::EPSILON }
            };
            let distance = absolute(&quote! { #value - *__point }, false);
            let magnitude = absolute(value, false);
            let point = absolute(&quote! { *__point }, false);
            return quote! {
                !#value.is_finite() || ![#(#candidates),*].iter().any(|__point| {
                    #distance <= 4.0 * #epsilon * #magnitude.max(#point)
                })
            };
        }
        FloatStepPlan::Lattice(lattice) => lattice,
    };
    let FloatLattice {
        factor,
        original_step,
        step_high,
        step_low,
        phase_high,
        phase_low,
    } = lattice;
    let factor = Literal::f64_suffixed(factor);
    let original_step = Literal::f64_suffixed(original_step);
    let step_high = Literal::f64_suffixed(step_high);
    let step_low = Literal::f64_suffixed(step_low);
    let phase_high = Literal::f64_suffixed(phase_high);
    let phase_low = Literal::f64_suffixed(phase_low);
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
    // The gaps are widened to `f64` only from `f32`: a cast from `f64` to
    // itself draws `clippy::unnecessary_cast`.
    let (lower_gap, upper_gap) = if wire_f32 {
        (
            quote! { (__represented - __below) as ::core::primitive::f64 },
            quote! { (__above - __represented) as ::core::primitive::f64 },
        )
    } else {
        (
            quote! { __represented - __below },
            quote! { __above - __represented },
        )
    };
    let represented = absolute(&represented, wire_f32);
    let magnitude = absolute(value, false);
    let phase_magnitude = absolute(&quote! { __phase / __factor }, false);
    let step_magnitude = absolute(&quote! { __original_step }, false);
    let scaled_magnitude = absolute(&quote! { __scaled_value }, false);
    let scaled_phase_magnitude = absolute(&quote! { __phase }, false);
    let scaled_step_magnitude = absolute(&quote! { __step }, false);
    let distance = absolute(&quote! { #value - __nearest }, false);
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
                let __represented = #represented;
                let __bits = __represented.to_bits();
                let __below = if __bits == 0 {
                    -#float::from_bits(1)
                } else {
                    #float::from_bits(__bits - 1)
                };
                let __above = #float::from_bits(__bits + 1);
                let __lower_gap = #lower_gap;
                let __upper_gap = if __above.is_finite() {
                    #upper_gap
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
                    let __scale = #magnitude.max(#phase_magnitude).max(#step_magnitude);
                    let __tolerance = (4.0 * #epsilon * __scale).min(__original_step / 4.0);
                    let mut __distance = ::core::primitive::f64::INFINITY;
                    // Scale large terms before reconstructing. A rounded high
                    // sum can otherwise overflow before its negative decimal
                    // residual brings the exact point back into finite range.
                    let __rescale = if #scaled_magnitude.max(#scaled_phase_magnitude).max(#scaled_step_magnitude)
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
                            __distance = __distance.min(#distance);
                        }
                    }
                    !__quotient.is_finite() || __distance > __tolerance
                }
            }
        }
    }
}
