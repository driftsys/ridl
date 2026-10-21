//! The platform's size limits (ADR-0030). The typl checker states the advised
//! bounds, `ridlc` states the payload ceiling, and every runtime that enforces
//! the ceiling reads the same number, so the values live in one place.

/// The payload ceiling in bytes: the largest single transfer of 16,384 bytes
/// minus the 128 bytes a transport reserves for its header (ADR-0030
/// decisions 1 and 2). It is a platform rule, changed only by amending that
/// record.
pub const PAYLOAD_CEILING: u32 = 16_256;

/// The largest `string` bound, in characters, that TYPL-116 accepts. A string
/// of this many characters is at most 16,128 bytes of UTF-8, which fits the
/// payload ceiling with room for the encoding overhead (ADR-0030 decision 6).
pub const ADVISED_STRING_MAX_CHARS: u64 = 4_032;

/// The largest `bytes` bound, in bytes, that TYPL-116 accepts: the encoded size
/// of a string at [`ADVISED_STRING_MAX_CHARS`] (ADR-0030 decision 6).
pub const ADVISED_BYTES_MAX: u64 = 16_128;

#[cfg(test)]
mod tests {
    use super::*;

    /// A string at its advised bound is as large in bytes as `bytes` at its
    /// advised bound (four UTF-8 bytes per character), and both fit in the
    /// ceiling, which leaves room for the overhead of an encoding.
    #[test]
    fn the_limits_agree_with_each_other() {
        assert_eq!(ADVISED_STRING_MAX_CHARS * 4, ADVISED_BYTES_MAX);
        assert!(ADVISED_BYTES_MAX < u64::from(PAYLOAD_CEILING));
        assert_eq!(PAYLOAD_CEILING, 16_384 - 128);
    }
}
