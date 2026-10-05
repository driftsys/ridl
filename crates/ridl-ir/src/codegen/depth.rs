//! The ring depth of an event channel, derived from its timing bounds.
//!
//! The depth is `ceil(max / min)` over the event's resolved timing, computed on
//! the exact-decimal microsecond strings of the IR in integer arithmetic and
//! never in floating point (`docs/decisions/ADR-0015-qos-absorption-and-rpc-bounds.md`, decision 21).

/// Splits `digits[.digits]` into its integer and fractional digits.
///
/// Returns `None` for any other shape: a sign, a space, an exponent, an empty
/// operand, or a `.` without digits on either side.
fn split_decimal(text: &str) -> Option<(&str, &str)> {
    let (int, frac) = match text.split_once('.') {
        Some((int, frac)) => (int, frac),
        None => (text, ""),
    };
    let digits = |part: &str| part.bytes().all(|b| b.is_ascii_digit());
    if int.is_empty() || !digits(int) || !digits(frac) {
        return None;
    }
    if text.contains('.') && frac.is_empty() {
        return None;
    }
    Some((int, frac))
}

/// Scales a decimal to an integer by `places` fractional digits.
///
/// `None` when the scaled value does not fit in `u128`.
fn scaled(int: &str, frac: &str, places: usize) -> Option<u128> {
    let mut digits = String::with_capacity(int.len() + places);
    digits.push_str(int);
    digits.push_str(frac);
    digits.extend(std::iter::repeat_n('0', places - frac.len()));
    digits.parse().ok()
}

/// Returns `ceil(max_us / min_us)` over two non-negative decimal strings.
///
/// Each operand must be `digits[.digits]`. The result is `None` when an
/// operand has another shape, when `min_us` is zero, when a scaled operand does
/// not fit in `u128`, or when the quotient exceeds `u32::MAX`.
#[allow(dead_code)]
pub fn ceil_ratio(max_us: &str, min_us: &str) -> Option<u32> {
    let (max_int, max_frac) = split_decimal(max_us)?;
    let (min_int, min_frac) = split_decimal(min_us)?;
    let places = max_frac.len().max(min_frac.len());
    let max = scaled(max_int, max_frac, places)?;
    let min = scaled(min_int, min_frac, places)?;
    if min == 0 {
        return None;
    }
    let quotient = max / min + u128::from(max % min != 0);
    u32::try_from(quotient).ok()
}

#[cfg(test)]
mod tests {
    use super::ceil_ratio;

    #[test]
    fn ratio_is_rounded_up_over_exact_decimals() {
        assert_eq!(ceil_ratio("1000000", "100000"), Some(10)); // @[100ms..1s]
        assert_eq!(ceil_ratio("1000000", "300000"), Some(4)); // 3.33 rounds up
        assert_eq!(ceil_ratio("500.5", "0.25"), Some(2002));
        assert_eq!(ceil_ratio("10000", "10000"), Some(1));
        assert_eq!(ceil_ratio("1", "0"), None);
        assert_eq!(ceil_ratio("1e3", "1"), None);
        assert_eq!(ceil_ratio("4294967296", "1"), None);
    }

    #[test]
    fn unlisted_shapes_are_decided() {
        assert_eq!(ceil_ratio("4294967295", "1"), Some(u32::MAX));
        assert_eq!(ceil_ratio("+1", "1"), None);
        assert_eq!(ceil_ratio(" 1", "1"), None);
        assert_eq!(ceil_ratio("1", "1 "), None);
        assert_eq!(ceil_ratio("", "1"), None);
        assert_eq!(ceil_ratio("1", ""), None);
        assert_eq!(ceil_ratio(".", "1"), None);
        assert_eq!(ceil_ratio(".5", "1"), None);
        assert_eq!(ceil_ratio("5.", "1"), None);
        assert_eq!(ceil_ratio("1", "0.000"), None);
        assert_eq!(ceil_ratio("0", "7"), Some(0));
        assert_eq!(ceil_ratio("1", &format!("0.{}1", "0".repeat(60))), None);
    }
}
