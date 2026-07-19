//! Exact scalar arithmetic: literal values, integer and float wire-width
//! derivation, enum-set widths, and range/step validation (typl language
//! reference §4.2–§4.5, §5.5–§5.6, §9.3, §16.2).
//!
//! Every value is an exact rational (`num_rational::BigRational`). No `f32` or
//! `f64` appears anywhere in this module: floating-point arithmetic would
//! silently miscompute widths and boundary cases (ADR-0004 §9, ADR-0007
//! decision 9). Binary32 representability is decided by an exact
//! mantissa/exponent decomposition of the rational, never by an `as f32`
//! round-trip.
//!
//! The width enums here are sem-local. Each converts into its `ridl-ir` v1
//! counterpart through a `From` implementation, so the checker (task 13) lowers
//! a derived width into the IR with a single `.into()`.

use num_bigint::{BigInt, BigUint, Sign};
use num_rational::BigRational;

/// An exact literal value, parsed straight from source text — never through an
/// intermediate `f64`. Wraps a reduced rational, so two values that denote the
/// same number compare and hash equal regardless of source spelling
/// (`0.10` and `0.1` are one value).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ExactValue(pub BigRational);

impl ExactValue {
    /// Parses a typl integer or float literal into an exact value. Accepts an
    /// optional leading `-`, decimal digits, and an optional fractional part
    /// after a single `.` (typl §2.4–§2.5). Returns `None` for any other text,
    /// including scientific notation, a bare `.`, or an empty string. Leading
    /// zeros carry no value meaning and are accepted here; the parser flags
    /// them as FORM-005 at the lexical layer.
    pub fn parse(text: &str) -> Option<ExactValue> {
        let (negative, body) = match text.strip_prefix('-') {
            Some(rest) => (true, rest),
            None => (false, text),
        };
        if body.is_empty() {
            return None;
        }
        let (int_str, frac_str) = match body.split_once('.') {
            Some((int_str, frac_str)) => (int_str, frac_str),
            None => (body, ""),
        };
        // Require an integer part; if a `.` was present, require fractional
        // digits after it; forbid a second `.`.
        if int_str.is_empty() {
            return None;
        }
        if body.contains('.') && frac_str.is_empty() {
            return None;
        }
        if !int_str.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        if !frac_str.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }

        let digits = format!("{int_str}{frac_str}");
        let mut numer = BigInt::parse_bytes(digits.as_bytes(), 10)?;
        if negative {
            numer = -numer;
        }
        let denom = pow10(frac_str.len() as u32);
        Some(ExactValue(BigRational::new(numer, denom)))
    }

    /// Renders the value as a canonical, lossless decimal string. An integer
    /// value renders with no fractional part (`250`, `-128`,
    /// `9223372036854775807`); a fractional value renders with the fewest
    /// fractional digits that reproduce it exactly (`0.1`, `0.5`, `0.001`).
    /// The rendering is canonical: equal values always produce the same string.
    ///
    /// A rational whose denominator has a prime factor other than 2 or 5 has no
    /// finite decimal form. Such a value never originates from a typl source
    /// literal; to stay total and lossless the method falls back to an exact
    /// `numer/denom` fraction string for that case.
    pub fn to_decimal_string(&self) -> String {
        let value = &self.0;
        if value.is_integer() {
            return value.to_integer().to_string();
        }
        let numer = value.numer();
        let denom = value.denom();
        match terminating_decimal_places(denom) {
            Some(places) => {
                let scaled = numer * pow10(places) / denom;
                format_scaled_decimal(&scaled, places)
            }
            None => format!("{numer}/{denom}"),
        }
    }
}

/// Derived integer wire width — the eight rows of the typl §4.2 table. All
/// eight stay distinct — in particular `U64` versus `I64` — so a width flip is
/// visible to `ridl-diff` (ADR-0007 decision 9).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IntWidth {
    U8,
    I8,
    U16,
    I16,
    U32,
    I32,
    U64,
    I64,
}

/// Derived float wire width — count-based inference (typl §4.3).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FloatWidth {
    F32,
    F64,
}

/// A closed integer range `[min..max]`. Both bounds are concrete: the caller
/// resolves any omitted bound (typl §5.5) before deriving a width.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IntRange {
    pub min: ExactValue,
    pub max: ExactValue,
}

/// A closed float range `[min..max]` with an optional quantization `step`
/// (typl §4.3). Valid values are `min + n·step`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FloatRange {
    pub min: ExactValue,
    pub max: ExactValue,
    pub step: Option<ExactValue>,
}

/// A width-derivation failure. The caller (task 13) maps it to a coded
/// diagnostic at the declaring span.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WidthError {
    /// An integer bound lies outside the `int64` domain `[-2⁶³..2⁶³−1]`
    /// (typl §4.2, TYPL-111).
    OutsideInt64Domain,
}

impl WidthError {
    /// The typl §16 code this error maps to.
    pub fn code(self) -> &'static str {
        match self {
            WidthError::OutsideInt64Domain => "TYPL-111",
        }
    }
}

/// A range or step validation failure. The caller (task 13) maps each variant
/// to its coded diagnostic at the declaring span.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiagKind {
    /// Range `min` greater than `max` (TYPL-104).
    MinGreaterThanMax,
    /// `step` is zero or negative (TYPL-105).
    StepNonPositive,
    /// `step` is larger than the range span `max − min` (TYPL-105).
    StepLargerThanRange,
    /// `step` type does not match the field type — for example a step given on
    /// an `integer` type, or an integer-form step literal on a `float` type
    /// (TYPL-105). Detecting a type mismatch needs the declared type, so this
    /// variant is constructed by the caller (task 13), not by `validate_step`.
    StepTypeMismatch,
}

impl DiagKind {
    /// The typl §16 code this kind maps to.
    pub fn code(self) -> &'static str {
        match self {
            DiagKind::MinGreaterThanMax => "TYPL-104",
            DiagKind::StepNonPositive
            | DiagKind::StepLargerThanRange
            | DiagKind::StepTypeMismatch => "TYPL-105",
        }
    }
}

/// Derives the narrowest integer wire width whose domain contains the range
/// (typl §4.2). Returns `WidthError::OutsideInt64Domain` (TYPL-111) when a
/// bound falls outside the `int64` domain, which no width can hold.
pub fn derive_int_width(r: &IntRange) -> Result<IntWidth, WidthError> {
    for (width, lo, hi) in int_width_domains() {
        if r.min.0 >= lo && r.max.0 <= hi {
            return Ok(width);
        }
    }
    Err(WidthError::OutsideInt64Domain)
}

/// Derives the float wire width by the count-based rule (typl §4.3). Returns
/// `F32` only when a step is declared, the value count
/// `N = (max − min) / step + 1` is at most `2²⁴`, and both bounds are exactly
/// representable in binary32; otherwise `F64`. The step's own representability
/// is deliberately not required — the scaled-integer wire form carries the
/// quantization (typl §4.3).
pub fn derive_float_width(r: &FloatRange) -> FloatWidth {
    let Some(step) = &r.step else {
        return FloatWidth::F64;
    };
    // A non-positive step or an inverted range makes the count meaningless;
    // those are TYPL-104/105 errors surfaced elsewhere. Fall back to F64.
    if step.0.numer().sign() != Sign::Plus {
        return FloatWidth::F64;
    }
    let span = &r.max.0 - &r.min.0;
    if span.numer().sign() == Sign::Minus {
        return FloatWidth::F64;
    }
    let count = span / step.0.clone() + BigRational::from_integer(BigInt::from(1));
    let max_f32_count = BigRational::from_integer(BigInt::from(1u32 << 24));
    if count <= max_f32_count
        && is_binary32_representable(&r.min.0)
        && is_binary32_representable(&r.max.0)
    {
        FloatWidth::F32
    } else {
        FloatWidth::F64
    }
}

/// Derives the unsigned width of an enum set from its highest bit position
/// (typl §9.3). The language layer is always `int64`; positions past bit 63
/// exceed that domain and saturate to `U64` — the caller rejects them
/// separately.
pub fn enumset_width(highest_bit: u32) -> IntWidth {
    match highest_bit {
        0..=7 => IntWidth::U8,
        8..=15 => IntWidth::U16,
        16..=31 => IntWidth::U32,
        _ => IntWidth::U64,
    }
}

/// Validates a range's ordering: `Some(MinGreaterThanMax)` (TYPL-104) when
/// `min > max`, `None` otherwise.
pub fn validate_range(min: &ExactValue, max: &ExactValue) -> Option<DiagKind> {
    if min.0 > max.0 {
        Some(DiagKind::MinGreaterThanMax)
    } else {
        None
    }
}

/// Validates a float range's step numerically (TYPL-105). Detects a
/// non-positive step and a step larger than the range span. A step that does
/// not match the field type is a `DiagKind::StepTypeMismatch` the caller
/// constructs; it is not detectable from the numeric range alone. An inverted
/// range is left to `validate_range` (TYPL-104). Returns `None` when there is
/// no step or the step is numerically valid.
pub fn validate_step(r: &FloatRange) -> Option<DiagKind> {
    let step = r.step.as_ref()?;
    if step.0.numer().sign() != Sign::Plus {
        return Some(DiagKind::StepNonPositive);
    }
    let span = &r.max.0 - &r.min.0;
    if span.numer().sign() == Sign::Minus {
        return None;
    }
    if step.0 > span {
        return Some(DiagKind::StepLargerThanRange);
    }
    None
}

impl From<IntWidth> for ridl_ir::v1::IntWidth {
    fn from(width: IntWidth) -> Self {
        match width {
            IntWidth::U8 => ridl_ir::v1::IntWidth::U8,
            IntWidth::I8 => ridl_ir::v1::IntWidth::I8,
            IntWidth::U16 => ridl_ir::v1::IntWidth::U16,
            IntWidth::I16 => ridl_ir::v1::IntWidth::I16,
            IntWidth::U32 => ridl_ir::v1::IntWidth::U32,
            IntWidth::I32 => ridl_ir::v1::IntWidth::I32,
            IntWidth::U64 => ridl_ir::v1::IntWidth::U64,
            IntWidth::I64 => ridl_ir::v1::IntWidth::I64,
        }
    }
}

impl From<FloatWidth> for ridl_ir::v1::FloatWidth {
    fn from(width: FloatWidth) -> Self {
        match width {
            FloatWidth::F32 => ridl_ir::v1::FloatWidth::F32,
            FloatWidth::F64 => ridl_ir::v1::FloatWidth::F64,
        }
    }
}

/// The §4.2 width domains in table order (narrowest first). `U64` is capped at
/// the `int64` maximum, not the full unsigned range, per the `int64` domain
/// rule (typl §4.2). The first width whose `[lo..hi]` contains the range wins.
fn int_width_domains() -> [(IntWidth, BigRational, BigRational); 8] {
    let int = |v: i128| BigRational::from_integer(BigInt::from(v));
    [
        (IntWidth::U8, int(0), int(255)),
        (IntWidth::I8, int(-128), int(127)),
        (IntWidth::U16, int(0), int(65535)),
        (IntWidth::I16, int(-32768), int(32767)),
        (IntWidth::U32, int(0), int(4294967295)),
        (IntWidth::I32, int(-2147483648), int(2147483647)),
        (IntWidth::U64, int(0), int(i64::MAX as i128)),
        (IntWidth::I64, int(i64::MIN as i128), int(i64::MAX as i128)),
    ]
}

/// Returns `true` when `value` is exactly a finite IEEE-754 binary32. The check
/// is exact and uses no floating point: it writes `|value|` as an odd mantissa
/// `m` times a power of two `2^k`, then tests the mantissa width and the bit
/// exponents against binary32's limits — a 24-bit significand, a lowest
/// representable exponent of `2⁻¹⁴⁹`, and a highest set-bit exponent of `2¹²⁷`.
fn is_binary32_representable(value: &BigRational) -> bool {
    if value.numer().sign() == Sign::NoSign {
        return true; // zero
    }
    let numer = value.numer().magnitude();
    let denom = value.denom().magnitude();

    // The value is a finite binary fraction only when the denominator is a
    // power of two. A power of two 2^s has exactly s trailing zero bits and
    // s + 1 total bits.
    let s = denom.trailing_zeros().expect("denominator is nonzero");
    if denom.bits() != s + 1 {
        return false;
    }

    // |value| = numer / 2^s = m · 2^(t − s), with m the odd part of numer.
    let t = numer.trailing_zeros().expect("numerator is nonzero");
    let mantissa = numer.clone() >> (t as usize);
    let k = t as i64 - s as i64; // exponent of the lowest set bit
    let bit_len = mantissa.bits() as i64; // significant bits of the mantissa
    let high = k + bit_len - 1; // exponent of the highest set bit

    bit_len <= 24 && k >= -149 && high <= 127
}

/// Returns `10^exp` as a `BigInt`.
fn pow10(exp: u32) -> BigInt {
    let digits = format!("1{}", "0".repeat(exp as usize));
    BigInt::parse_bytes(digits.as_bytes(), 10).expect("a power of ten is a valid decimal")
}

/// If `denom` is `2^a · 5^b`, returns `max(a, b)` — the fractional digit count
/// of the exact decimal expansion. Returns `None` for a non-terminating
/// denominator (any other prime factor).
fn terminating_decimal_places(denom: &BigInt) -> Option<u32> {
    let mut remaining = denom.magnitude().clone();
    let zero = BigUint::from(0u32);
    let two = BigUint::from(2u32);
    let five = BigUint::from(5u32);
    let mut twos = 0u32;
    let mut fives = 0u32;
    while &remaining % &two == zero {
        remaining /= &two;
        twos += 1;
    }
    while &remaining % &five == zero {
        remaining /= &five;
        fives += 1;
    }
    if remaining == BigUint::from(1u32) {
        Some(twos.max(fives))
    } else {
        None
    }
}

/// Formats `scaled` (the value times `10^places`) as a decimal string with
/// `places` fractional digits. `places` is at least 1 here — the integer case
/// is handled before this is called.
fn format_scaled_decimal(scaled: &BigInt, places: u32) -> String {
    let negative = scaled.sign() == Sign::Minus;
    let mut digits = scaled.magnitude().to_string();
    let places = places as usize;
    if digits.len() <= places {
        let pad = places + 1 - digits.len();
        digits = format!("{}{}", "0".repeat(pad), digits);
    }
    let split = digits.len() - places;
    let (int_part, frac_part) = digits.split_at(split);
    let sign = if negative { "-" } else { "" };
    format!("{sign}{int_part}.{frac_part}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn exact(text: &str) -> ExactValue {
        ExactValue::parse(text).unwrap_or_else(|| panic!("`{text}` must parse"))
    }

    fn int_range(min: &str, max: &str) -> IntRange {
        IntRange {
            min: exact(min),
            max: exact(max),
        }
    }

    fn float_range(min: &str, max: &str, step: Option<&str>) -> FloatRange {
        FloatRange {
            min: exact(min),
            max: exact(max),
            step: step.map(exact),
        }
    }

    // --- §4.2 integer width table: every row. ---

    #[test]
    fn integer_width_table_rows() {
        let cases = [
            ("0", "255", IntWidth::U8),
            ("-128", "127", IntWidth::I8),
            ("0", "65535", IntWidth::U16),
            ("-32768", "32767", IntWidth::I16),
            ("0", "4294967295", IntWidth::U32),
            ("-2147483648", "2147483647", IntWidth::I32),
            ("0", "9223372036854775807", IntWidth::U64),
            ("-9223372036854775808", "9223372036854775807", IntWidth::I64),
        ];
        for (min, max, expected) in cases {
            assert_eq!(
                derive_int_width(&int_range(min, max)),
                Ok(expected),
                "[{min}..{max}] must derive {expected:?}"
            );
        }
    }

    #[test]
    fn integer_width_boundary_flip_255_256() {
        assert_eq!(derive_int_width(&int_range("0", "255")), Ok(IntWidth::U8));
        // One past the uint8 ceiling flips the width to uint16 (typl §5.6).
        assert_eq!(derive_int_width(&int_range("0", "256")), Ok(IntWidth::U16));
    }

    #[test]
    fn integer_width_signed_boundary_flips() {
        // One below the int8 floor cannot be uint16 (negative) — it is int16.
        assert_eq!(
            derive_int_width(&int_range("-129", "127")),
            Ok(IntWidth::I16)
        );
        // The §5.6 widening example: [0..250] is uint8, [0..300] is uint16.
        assert_eq!(derive_int_width(&int_range("0", "250")), Ok(IntWidth::U8));
        assert_eq!(derive_int_width(&int_range("0", "300")), Ok(IntWidth::U16));
    }

    #[test]
    fn integer_width_larger_unsigned_and_signed() {
        // Above uint32 but non-negative and within the int64 ceiling: uint64.
        assert_eq!(
            derive_int_width(&int_range("0", "4294967296")),
            Ok(IntWidth::U64)
        );
        // Negative with a max beyond int32: no unsigned width fits, so int64.
        assert_eq!(
            derive_int_width(&int_range("-1", "4294967295")),
            Ok(IntWidth::I64)
        );
    }

    #[test]
    fn integer_range_outside_int64_domain_is_typl_111() {
        // 2^63 = 9223372036854775808 is one past the int64 maximum.
        let over = int_range("0", "9223372036854775808");
        assert_eq!(derive_int_width(&over), Err(WidthError::OutsideInt64Domain));
        assert_eq!(WidthError::OutsideInt64Domain.code(), "TYPL-111");
        // One below the int64 floor is also outside the domain.
        let under = int_range("-9223372036854775809", "0");
        assert_eq!(
            derive_int_width(&under),
            Err(WidthError::OutsideInt64Domain)
        );
    }

    // --- §4.3 float width: both conditions, incl. the errata example. ---

    #[test]
    fn float_width_range_and_step_is_f32() {
        assert_eq!(
            derive_float_width(&float_range("0.0", "250.0", Some("0.5"))),
            FloatWidth::F32,
            "[0.0..250.0 step 0.5]: N = 501, bounds representable"
        );
    }

    #[test]
    fn float_width_errata_example_is_f64() {
        // The §4.3 errata example: N = 10^9 + 1 far exceeds 2^24, so the
        // step-only rule (step >= 0.001 -> float32) is unsound.
        assert_eq!(
            derive_float_width(&float_range("0.0", "1000000.0", Some("0.001"))),
            FloatWidth::F64
        );
    }

    #[test]
    fn float_width_no_step_is_f64() {
        assert_eq!(
            derive_float_width(&float_range("0.0", "250.0", None)),
            FloatWidth::F64
        );
    }

    #[test]
    fn float_width_count_over_2_pow_24_is_f64() {
        // N = 20_000_001 > 2^24 even though both bounds are representable.
        assert_eq!(
            derive_float_width(&float_range("0.0", "20000000.0", Some("1.0"))),
            FloatWidth::F64
        );
    }

    #[test]
    fn float_width_unrepresentable_bound_is_f64() {
        // Small N (3), but 0.1 is not exactly representable in binary32.
        assert_eq!(
            derive_float_width(&float_range("0.1", "0.2", Some("0.05"))),
            FloatWidth::F64
        );
    }

    #[test]
    fn float_width_ignores_step_representability() {
        // Bounds representable and N = 1001 <= 2^24, so F32 — even though the
        // step 0.1 is itself not binary32-representable. §4.3 checks the bounds
        // only; the scaled-integer wire form carries the quantization.
        assert_eq!(
            derive_float_width(&float_range("0.0", "100.0", Some("0.1"))),
            FloatWidth::F32
        );
    }

    // --- binary32 representability: 0.5 yes, 0.1 no, plus the edges. ---

    #[test]
    fn binary32_representable_values() {
        for text in ["0.0", "0.5", "0.25", "250.0", "1000000.0", "-128.0", "1.0"] {
            assert!(
                is_binary32_representable(&exact(text).0),
                "{text} must be representable in binary32"
            );
        }
    }

    #[test]
    fn binary32_unrepresentable_values() {
        // 0.1 = 1/10 and 0.2 = 1/5 have a factor of 5 in the denominator: no
        // finite binary expansion. 0.001 = 1/1000 likewise.
        for text in ["0.1", "0.2", "0.001", "0.3"] {
            assert!(
                !is_binary32_representable(&exact(text).0),
                "{text} must not be representable in binary32"
            );
        }
    }

    #[test]
    fn binary32_exponent_and_mantissa_edges() {
        // Smallest positive subnormal 2^-149 is representable; 2^-150 is not.
        let two_pow_149 = BigRational::new(BigInt::from(1), BigInt::from(1) << 149u32);
        assert!(is_binary32_representable(&two_pow_149));
        let two_pow_150 = BigRational::new(BigInt::from(1), BigInt::from(1) << 150u32);
        assert!(!is_binary32_representable(&two_pow_150));

        // Largest finite binary32 = (2^24 − 1) · 2^104 is representable;
        // 2^128 (the next power of two) is not.
        let max_mantissa = BigInt::from((1u64 << 24) - 1);
        let max_finite = BigRational::from_integer(max_mantissa << 104u32);
        assert!(is_binary32_representable(&max_finite));
        let two_pow_128 = BigRational::from_integer(BigInt::from(1) << 128u32);
        assert!(!is_binary32_representable(&two_pow_128));

        // A 25-bit odd mantissa exceeds the 24-bit significand.
        let mantissa_25_bits = BigRational::from_integer(BigInt::from((1u64 << 24) + 1));
        assert!(!is_binary32_representable(&mantissa_25_bits));
    }

    // --- enum-set widths: all four §9.3 rows. ---

    #[test]
    fn enumset_width_rows() {
        assert_eq!(enumset_width(0), IntWidth::U8);
        assert_eq!(enumset_width(7), IntWidth::U8);
        assert_eq!(enumset_width(8), IntWidth::U16);
        assert_eq!(enumset_width(15), IntWidth::U16);
        assert_eq!(enumset_width(16), IntWidth::U32);
        assert_eq!(enumset_width(31), IntWidth::U32);
        assert_eq!(enumset_width(32), IntWidth::U64);
        assert_eq!(enumset_width(63), IntWidth::U64);
    }

    // --- TYPL-104 range, TYPL-105 step validation. ---

    #[test]
    fn range_min_greater_than_max_is_typl_104() {
        assert_eq!(
            validate_range(&exact("5"), &exact("3")),
            Some(DiagKind::MinGreaterThanMax)
        );
        assert_eq!(DiagKind::MinGreaterThanMax.code(), "TYPL-104");
        assert_eq!(validate_range(&exact("3"), &exact("5")), None);
        assert_eq!(validate_range(&exact("5"), &exact("5")), None);
    }

    #[test]
    fn step_non_positive_is_typl_105() {
        assert_eq!(
            validate_step(&float_range("0.0", "250.0", Some("0.0"))),
            Some(DiagKind::StepNonPositive)
        );
        assert_eq!(
            validate_step(&float_range("0.0", "250.0", Some("-0.5"))),
            Some(DiagKind::StepNonPositive)
        );
        assert_eq!(DiagKind::StepNonPositive.code(), "TYPL-105");
    }

    #[test]
    fn step_larger_than_range_is_typl_105() {
        assert_eq!(
            validate_step(&float_range("0.0", "250.0", Some("300.0"))),
            Some(DiagKind::StepLargerThanRange)
        );
        assert_eq!(DiagKind::StepLargerThanRange.code(), "TYPL-105");
    }

    #[test]
    fn step_type_mismatch_maps_to_typl_105() {
        // A type mismatch is caller-constructed (it needs the declared type),
        // but it shares the TYPL-105 code with the numeric step failures.
        assert_eq!(DiagKind::StepTypeMismatch.code(), "TYPL-105");
    }

    #[test]
    fn valid_step_and_no_step_pass() {
        assert_eq!(
            validate_step(&float_range("0.0", "250.0", Some("0.5"))),
            None
        );
        assert_eq!(validate_step(&float_range("0.0", "250.0", None)), None);
        // An inverted range is left to validate_range, not reported as a step
        // failure.
        assert_eq!(
            validate_step(&float_range("250.0", "0.0", Some("0.5"))),
            None
        );
    }

    // --- exact value parsing and canonical decimal rendering. ---

    #[test]
    fn to_decimal_string_round_trips_representative_values() {
        for text in ["0.1", "9223372036854775807"] {
            let value = exact(text);
            let rendered = value.to_decimal_string();
            assert_eq!(rendered, text, "{text} must render canonically");
            assert_eq!(
                ExactValue::parse(&rendered),
                Some(value),
                "{text} must round-trip through parse"
            );
        }
    }

    #[test]
    fn to_decimal_string_is_canonical() {
        // Integer-valued floats drop the fractional part; trailing zeros are
        // stripped; equal values render identically regardless of spelling.
        assert_eq!(exact("250.0").to_decimal_string(), "250");
        assert_eq!(exact("0.5").to_decimal_string(), "0.5");
        assert_eq!(exact("0.001").to_decimal_string(), "0.001");
        assert_eq!(exact("-0.1").to_decimal_string(), "-0.1");
        assert_eq!(exact("-128").to_decimal_string(), "-128");
        assert_eq!(exact("250.50").to_decimal_string(), "250.5");
        assert_eq!(
            exact("0.10").to_decimal_string(),
            exact("0.1").to_decimal_string()
        );
    }

    #[test]
    fn parse_accepts_integer_and_decimal_forms() {
        assert!(ExactValue::parse("0").is_some());
        assert!(ExactValue::parse("-128").is_some());
        assert!(ExactValue::parse("250.0").is_some());
        // Leading zeros carry no value meaning and parse fine here.
        assert_eq!(ExactValue::parse("042"), ExactValue::parse("42"));
    }

    #[test]
    fn parse_rejects_malformed_text() {
        for text in ["", "-", ".", ".5", "1.", "1.2.3", "abc", "1e3", "+5", " 5"] {
            assert_eq!(ExactValue::parse(text), None, "`{text}` must not parse");
        }
    }

    // --- sem width enums lower into the IR v1 enums. ---

    #[test]
    fn int_width_lowers_to_ir() {
        let cases = [
            (IntWidth::U8, ridl_ir::v1::IntWidth::U8),
            (IntWidth::I8, ridl_ir::v1::IntWidth::I8),
            (IntWidth::U16, ridl_ir::v1::IntWidth::U16),
            (IntWidth::I16, ridl_ir::v1::IntWidth::I16),
            (IntWidth::U32, ridl_ir::v1::IntWidth::U32),
            (IntWidth::I32, ridl_ir::v1::IntWidth::I32),
            (IntWidth::U64, ridl_ir::v1::IntWidth::U64),
            (IntWidth::I64, ridl_ir::v1::IntWidth::I64),
        ];
        for (sem, ir) in cases {
            assert_eq!(ridl_ir::v1::IntWidth::from(sem), ir);
        }
    }

    #[test]
    fn float_width_lowers_to_ir() {
        assert_eq!(
            ridl_ir::v1::FloatWidth::from(FloatWidth::F32),
            ridl_ir::v1::FloatWidth::F32
        );
        assert_eq!(
            ridl_ir::v1::FloatWidth::from(FloatWidth::F64),
            ridl_ir::v1::FloatWidth::F64
        );
    }
}

#[cfg(test)]
mod properties {
    use super::*;
    use num_bigint::BigInt;
    use num_rational::BigRational;
    use proptest::prelude::*;

    fn contains(lo: &BigRational, hi: &BigRational, min: &BigRational, max: &BigRational) -> bool {
        min >= lo && max <= hi
    }

    proptest! {
        /// For any integer range within the int64 domain, the derived width's
        /// domain contains the whole range (so every value min..=max fits), and
        /// no narrower width in table order would have contained it.
        #[test]
        fn derived_int_width_is_the_narrowest_containing(
            a in i64::MIN..=i64::MAX,
            b in i64::MIN..=i64::MAX,
        ) {
            let (lo, hi) = if a <= b { (a, b) } else { (b, a) };
            let range = IntRange {
                min: ExactValue(BigRational::from_integer(BigInt::from(lo))),
                max: ExactValue(BigRational::from_integer(BigInt::from(hi))),
            };
            let width = derive_int_width(&range).expect("an int64-domain range always has a width");

            let mut found = false;
            for (candidate, clo, chi) in int_width_domains() {
                if candidate == width {
                    // The chosen width contains the range.
                    prop_assert!(contains(&clo, &chi, &range.min.0, &range.max.0));
                    found = true;
                    break;
                }
                // Every earlier (narrower) width fails to contain the range.
                prop_assert!(!contains(&clo, &chi, &range.min.0, &range.max.0));
            }
            prop_assert!(found);
        }

        /// For a random float range built from a positive step and a value
        /// count, the derived width matches the exact §4.3 rule, and whenever it
        /// is F32 the value count stays below 2^24 — so every scaled-integer
        /// index fits binary32's mantissa exactly.
        #[test]
        fn derived_float_width_matches_the_count_rule(
            min_num in -1000i64..=1000,
            step_den in 1i64..=1000,
            count in 1u64..=40_000_000,
        ) {
            // min = min_num, step = 1/step_den (positive), max = min + (count-1)*step.
            let min = BigRational::from_integer(BigInt::from(min_num));
            let step = BigRational::new(BigInt::from(1), BigInt::from(step_den));
            let span = &step * BigRational::from_integer(BigInt::from(count - 1));
            let max = &min + &span;
            let range = FloatRange {
                min: ExactValue(min.clone()),
                max: ExactValue(max.clone()),
                step: Some(ExactValue(step)),
            };

            let width = derive_float_width(&range);

            let two_pow_24 = 1u64 << 24;
            let count_ok = count <= two_pow_24;
            let bounds_ok = is_binary32_representable(&min) && is_binary32_representable(&max);
            let expected = if count_ok && bounds_ok {
                FloatWidth::F32
            } else {
                FloatWidth::F64
            };
            prop_assert_eq!(width, expected);

            if width == FloatWidth::F32 {
                // Every scaled index n (0..count) is below 2^24, hence exactly
                // representable in binary32's 24-bit mantissa.
                prop_assert!(count <= two_pow_24);
            }
        }
    }
}
