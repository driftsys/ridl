//! Whether the value 0 is a legal value of a typl type.
//!
//! 0 is the default a FlatBuffers reader gives an absent scalar or enum
//! field. Two consumers ask the same question and must agree on the answer:
//!
//! - the FlatBuffers codec of `ridl-backend-rust`, which reads an absent
//!   non-optional scalar or enum field as 0 when 0 is legal for its type, and
//!   refuses it as a missing required field when it is not
//!   (driftsys/ridl#472);
//! - `ridl-diff`, which classifies appending such a field as compatible only
//!   when a reader of the new version reads the field absent from an old
//!   payload as 0 (driftsys/ridl#598), and under podbuf accepts an append
//!   on the same condition (ADR-0031 decision 10);
//! - the RIDL-155 check of `ridlc`, which reports a deprecated podbuf field
//!   whose init value is neither declared nor derivable and whose type
//!   excludes 0.
//!
//! The codec reads the lowered codegen model and the others read IR v2, so
//! the call sites hold different message types. The definitions here take
//! what both types carry: a constraint's exact decimal text, and an enum's
//! declared values; [`field_type_holds_zero`] states the rule once over an
//! IR v2 field type for the two readers of IR v2.

use crate::v2;

/// Whether 0 is a value of a numeric scalar's declared range and step, read
/// from the IR's exact decimal text: `min`, `max` and `step` of a
/// `Constraint`, each absent when unstated.
///
/// 0 must lie within `[min..max]`, and when a `step` is declared it must also
/// be on the grid `min + n·step` for a whole `n` (typl §4.3):
/// `[-1.5..1.5 step 1.0]` holds -1.5, -0.5, 0.5 and 1.5, and not 0. A `step`
/// with no `min` uses zero as its origin. A bound that is not plain decimal
/// text (`-`, digits, and an optional fractional part) is not read here; the
/// answer is `false`, so an absent field is refused rather than read as a
/// value that may not be legal. A type with no constraint at all
/// holds 0: the caller passes three `None`s.
///
/// This is decided from the declaration, for a named type and for an inline
/// constraint alike, without depending on generated validation functions.
/// A zero or omitted origin is decided directly from canonical decimal signs
/// and has no coefficient precision limit.
pub fn range_holds_zero(min: Option<&str>, max: Option<&str>, step: Option<&str>) -> bool {
    // A zero origin lies on every positive step lattice. This does not need
    // coefficient arithmetic, so arbitrarily fine or large canonical decimal
    // steps remain decidable without the bounded Decimal representation.
    if min.is_none_or(|min| decimal_sign(min) == Some(0))
        && max.is_none_or(|max| decimal_sign(max).is_some_and(|sign| sign >= 0))
    {
        return step.is_none_or(|step| decimal_sign(step) == Some(1));
    }
    let read = |text: Option<&str>| -> Result<Option<Decimal>, ()> {
        match text {
            None => Ok(None),
            Some(text) => Decimal::parse(text).map(Some).ok_or(()),
        }
    };
    let (Ok(min), Ok(max), Ok(step)) = (read(min), read(max), read(step)) else {
        return false;
    };
    if min.is_some_and(|min| min.units > 0) || max.is_some_and(|max| max.units < 0) {
        return false;
    }
    match (step, min) {
        (None, _) => true,
        (Some(step), None) => step.units > 0,
        // 0 = min + n·step with a whole n: -min is a whole multiple of step.
        (Some(step), Some(min)) => {
            let scale = min.scale.max(step.scale);
            match (min.rescaled(scale), step.rescaled(scale)) {
                (Some(min), Some(step)) if step > 0 => min % step == 0,
                _ => false,
            }
        }
    }
}

/// The sign of a canonical decimal, without allocating its coefficient.
fn decimal_sign(text: &str) -> Option<i8> {
    let (negative, unsigned) = text
        .strip_prefix('-')
        .map_or((false, text), |text| (true, text));
    let (whole, fraction) = unsigned.split_once('.').unwrap_or((unsigned, ""));
    if whole.is_empty()
        || !whole.bytes().all(|byte| byte.is_ascii_digit())
        || !fraction.bytes().all(|byte| byte.is_ascii_digit())
        || (unsigned.contains('.') && fraction.is_empty())
    {
        return None;
    }
    if whole
        .bytes()
        .chain(fraction.bytes())
        .all(|byte| byte == b'0')
    {
        Some(0)
    } else {
        Some(if negative { -1 } else { 1 })
    }
}

/// The index of the enum value whose number is 0 — the enum's zero member —
/// or `None` when the enum declares none. An enum is legal at 0 exactly when
/// it has a zero member (typl §5.8 names the same member as the enum's
/// init, when it exists).
pub fn enum_zero_member(values: &[v2::EnumValue]) -> Option<usize> {
    values.iter().position(|value| value.value == 0)
}

/// Whether 0 is a legal value of the field type `ty`, the rule `ridl diff`
/// applies to an appended field and RIDL-155 to a deprecated one: an
/// optional field, whose flag bit reads 0 and so means absent; a boolean,
/// integer or float primitive; a named or inline scalar that
/// [`scalar_holds_zero`]; an enum with a zero member; or an enum set, whose
/// 0 is the empty set. A string, bytes, array, map, struct, tuple or union
/// field is not. `resolve` maps a named reference, as the IR writes it, to
/// the declaration it names; a reference it cannot resolve is not legal at 0.
pub fn field_type_holds_zero<'a>(
    ty: &v2::FieldType,
    resolve: impl FnOnce(&str) -> Option<&'a v2::Decl>,
) -> bool {
    if ty.optional {
        return true;
    }
    use v2::field_type::Kind;
    match &ty.kind {
        Some(Kind::Primitive(primitive)) => primitive_holds_zero(*primitive),
        Some(Kind::InlineScalar(def)) => scalar_holds_zero(def),
        Some(Kind::Named(reference)) => {
            use v2::decl::Kind as Decl;
            match resolve(reference).and_then(|decl| decl.kind.as_ref()) {
                Some(Decl::TypeDef(def)) => scalar_holds_zero(def),
                Some(Decl::EnumDef(def)) => enum_zero_member(&def.values).is_some(),
                Some(Decl::EnumSetDef(_)) => true,
                _ => false,
            }
        }
        _ => false,
    }
}

/// Whether a named or inline scalar is legal at 0: a boolean, integer, float
/// or unit backing whose constraint holds 0 ([`range_holds_zero`]). A string
/// or bytes backing is not.
pub fn scalar_holds_zero(def: &v2::TypeDef) -> bool {
    let numeric = match def
        .backing
        .as_ref()
        .and_then(|backing| backing.kind.as_ref())
    {
        Some(v2::backing::Kind::Unit(_)) => true,
        Some(v2::backing::Kind::Primitive(primitive)) => primitive_holds_zero(*primitive),
        None => false,
    };
    numeric
        && def.constraint.as_ref().is_none_or(|constraint| {
            range_holds_zero(
                constraint.min.as_deref(),
                constraint.max.as_deref(),
                constraint.step.as_deref(),
            )
        })
}

/// Whether a primitive is numeric, so that 0 is one of its values: a boolean
/// (`false`), an integer or a float.
fn primitive_holds_zero(primitive: i32) -> bool {
    matches!(
        v2::PrimitiveType::try_from(primitive),
        Ok(v2::PrimitiveType::Boolean | v2::PrimitiveType::Integer | v2::PrimitiveType::Float)
    )
}

/// An exact decimal read from the IR's canonical text: `units / 10^scale`.
#[derive(Debug, Clone, Copy)]
struct Decimal {
    units: i128,
    scale: u32,
}

impl Decimal {
    /// Reads `-`, digits, and an optional `.` followed by digits. Anything
    /// else, or more than 30 digits written, is `None`. The guard counts the
    /// digits of the text, leading zeros included, not the significant
    /// digits of the value, so `0.` followed by thirty zeros and a `1` is
    /// refused although it is one significant digit.
    ///
    /// Thirty is a conservative cutoff below the 38 an `i128` holds exactly.
    /// It is not a bound that makes every rescaling fit: [`Decimal::rescaled`]
    /// multiplies a coefficient by a power of ten to bring two bounds to a
    /// common scale, and two bounds whose scales are far apart overflow an
    /// `i128` even inside this limit. Such a rescaling is refused by the
    /// checked multiply, and [`range_holds_zero`] then answers `false`, so a
    /// pair it cannot decide reads as "0 is not legal" and an absent field is
    /// refused rather than read as a value that may not be legal.
    fn parse(text: &str) -> Option<Self> {
        let (negative, unsigned) = match text.strip_prefix('-') {
            Some(rest) => (true, rest),
            None => (false, text),
        };
        let (whole, fraction) = match unsigned.split_once('.') {
            Some((whole, fraction)) => (whole, fraction),
            None => (unsigned, ""),
        };
        let digits_only = |part: &str| part.bytes().all(|byte| byte.is_ascii_digit());
        if whole.is_empty()
            || !digits_only(whole)
            || !digits_only(fraction)
            || (unsigned.contains('.') && fraction.is_empty())
            || whole.len() + fraction.len() > 30
        {
            return None;
        }
        let units: i128 = format!("{whole}{fraction}").parse().ok()?;
        Some(Decimal {
            units: if negative { -units } else { units },
            scale: u32::try_from(fraction.len()).ok()?,
        })
    }

    /// `units` at a larger `scale`, or `None` on overflow.
    fn rescaled(self, scale: u32) -> Option<i128> {
        self.units
            .checked_mul(10i128.checked_pow(scale.checked_sub(self.scale)?)?)
    }
}

#[cfg(test)]
mod tests {
    use super::{enum_zero_member, range_holds_zero, v2};

    #[test]
    fn range_holds_zero_follows_the_range_and_the_step() {
        let cases = [
            (None, None, None, true),
            (Some("0"), Some("10"), None, true),
            (Some("1"), Some("10"), None, false),
            (Some("-10"), Some("-1"), None, false),
            // A bound of exactly 0 holds it, from either side.
            (Some("-10"), Some("0"), None, true),
            (Some("0"), Some("0"), None, true),
            (Some("-1.5"), Some("1.5"), Some("1.0"), false),
            (Some("-1.0"), Some("1.0"), Some("1.0"), true),
            (Some("0.0"), Some("1.0"), Some("0.01"), true),
            (Some("-0.25"), Some("1"), Some("0.125"), true),
            (Some("-0.3"), Some("1"), Some("0.2"), false),
            // A step with no lower bound has zero as its origin.
            (None, Some("10"), Some("1"), true),
            (None, None, Some("0.5"), true),
            (None, None, Some("0"), false),
            (None, None, Some("-0.5"), false),
            // Text this reader does not accept is not decided either.
            (Some("-1e3"), Some("10"), None, false),
            (Some("-1."), Some("10"), None, false),
        ];
        for (min, max, step, legal) in cases {
            assert_eq!(
                range_holds_zero(min, max, step),
                legal,
                "min {min:?}, max {max:?}, step {step:?}"
            );
        }
    }

    /// A pair this reader cannot decide answers `false`, so an absent field
    /// is refused rather than read as a value that may not be legal: a `min`
    /// and a `step` whose scales are far enough apart overflow the rescaling
    /// even though each is inside the 30-digit limit of `Decimal::parse`.
    /// 0 is on this grid, and the answer is still `false`.
    #[test]
    fn a_rescaling_that_overflows_answers_that_zero_is_not_legal() {
        let min = format!("-1{}", "0".repeat(29));
        let step = format!("0.{}1", "0".repeat(28));
        assert_eq!(min.trim_start_matches('-').len(), 30, "min is at the limit");
        assert_eq!(step.len() - 2, 29, "step is at the limit");
        assert!(!range_holds_zero(Some(&min), None, Some(&step)));
    }

    #[test]
    fn zero_origin_holds_zero_at_any_canonical_step_precision() {
        let fine = format!("0.{}1", "0".repeat(399));
        let large = format!("1{}.0", "0".repeat(400));
        for step in [&fine, &large] {
            assert!(range_holds_zero(Some("0.0"), None, Some(step)));
            assert!(range_holds_zero(None, Some("1.0"), Some(step)));
            assert!(!range_holds_zero(None, None, Some(&format!("-{step}"))));
        }
    }

    #[test]
    fn the_zero_member_is_the_value_numbered_zero_in_any_position() {
        let value = |name: &str, value: i64| v2::EnumValue {
            name: name.to_string(),
            value,
            doc: String::new(),
            links: Vec::new(),
            see: Vec::new(),
            since: Vec::new(),
        };
        assert_eq!(
            enum_zero_member(&[value("ON", 1), value("OFF", 0)]),
            Some(1)
        );
        assert_eq!(
            enum_zero_member(&[value("PARK", 1), value("DRIVE", 2)]),
            None
        );
    }
}
