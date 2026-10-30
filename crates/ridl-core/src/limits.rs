//! The platform's size limits (ADR-0030): the default ceiling of each kind of
//! interaction, the largest limit an interaction entry may raise a member to,
//! and the bounds above which TYPL-116 warns. The typl checker, the manifest
//! reader and `ridlc` read them from here, so the values live in one place. A
//! runtime keeps its own constants (ADR-0030 decision 4).

use crate::manifest::LimitKind;

/// The default ceiling of a `signal` and an `event`, in bytes of encoded
/// payload: one 4,096-byte page minus 256 bytes of header (ADR-0030 decision
/// 16).
pub const SIGNAL_EVENT_CEILING: u32 = 3_840;

/// The default ceiling of a `command` and a `query`, in bytes: four 4,096-byte
/// pages minus 256 bytes of header.
pub const CALL_CEILING: u32 = 16_128;

/// The default ceiling of a `fixed`, in bytes: sixteen 4,096-byte pages minus
/// 256 bytes of header.
pub const FIXED_CEILING: u32 = 65_280;

/// The largest payload of any kind in bytes: the default ceiling of a `fixed`.
/// It is a platform rule, changed only by amending ADR-0030. No limit in a
/// manifest is above it.
pub const PAYLOAD_CEILING: u32 = FIXED_CEILING;

/// The largest limit, in bytes, that a `[limits.interaction]` entry may give a
/// member that is not a `fixed` (ADR-0030 decision 18).
pub const RAISE_MAX: u32 = 16_128;

/// The largest `string` bound, in characters, that TYPL-116 accepts. A string
/// of this many characters is a multiple of 64 and its encoding fits the
/// largest payload with room for the overhead of a struct of several fields
/// (ADR-0030 decision 7).
pub const STRING_BOUND_MAX_CHARS: u64 = 16_256;

/// The largest `bytes` bound, in bytes, that TYPL-116 accepts: the largest
/// payload of any kind (ADR-0030 decision 7).
pub const BYTES_BOUND_MAX: u64 = 65_280;

/// The default ceiling of an interaction of `kind`, in bytes of encoded
/// payload (ADR-0030 decision 16).
pub const fn default_ceiling(kind: LimitKind) -> u32 {
    match kind {
        LimitKind::Signal | LimitKind::Event => SIGNAL_EVENT_CEILING,
        LimitKind::Command | LimitKind::Query => CALL_CEILING,
        LimitKind::Fixed => FIXED_CEILING,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The ceilings are whole 4,096-byte pages minus 256 bytes of header, the
    /// largest payload is the largest ceiling, and a string at its bound fits
    /// four times, plus the overhead of a small struct, in that payload.
    #[test]
    fn the_limits_agree_with_each_other() {
        assert_eq!(SIGNAL_EVENT_CEILING, 4_096 - 256);
        assert_eq!(CALL_CEILING, 4 * 4_096 - 256);
        assert_eq!(FIXED_CEILING, 16 * 4_096 - 256);
        assert_eq!(PAYLOAD_CEILING, 65_280);
        assert_eq!(STRING_BOUND_MAX_CHARS, 254 * 64);
        assert_eq!(BYTES_BOUND_MAX, u64::from(PAYLOAD_CEILING));
        assert!(4 * STRING_BOUND_MAX_CHARS + 58 <= u64::from(PAYLOAD_CEILING));
        assert_eq!(RAISE_MAX, CALL_CEILING);
    }

    #[test]
    fn each_kind_has_its_default_ceiling() {
        assert_eq!(default_ceiling(LimitKind::Signal), 3_840);
        assert_eq!(default_ceiling(LimitKind::Event), 3_840);
        assert_eq!(default_ceiling(LimitKind::Command), 16_128);
        assert_eq!(default_ceiling(LimitKind::Query), 16_128);
        assert_eq!(default_ceiling(LimitKind::Fixed), 65_280);
    }
}
