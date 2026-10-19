//! Base64 (RFC 4648 §4): the encoder writes `Sec-WebSocket-Accept`, and the
//! decoder checks that `Sec-WebSocket-Key` decodes to 16 bytes.

/// Returns `data` in the base64 alphabet of RFC 4648 §4, with padding.
pub(super) fn encode(data: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for group in data.chunks(3) {
        let b0 = group[0];
        let b1 = group.get(1).copied().unwrap_or(0);
        let b2 = group.get(2).copied().unwrap_or(0);
        let n = (u32::from(b0) << 16) | (u32::from(b1) << 8) | u32::from(b2);
        let sextet = |shift: u32| char::from(ALPHABET[((n >> shift) & 0x3F) as usize]);
        out.push(sextet(18));
        out.push(sextet(12));
        out.push(if group.len() > 1 { sextet(6) } else { '=' });
        out.push(if group.len() > 2 { sextet(0) } else { '=' });
    }
    out
}

/// Decodes `text` from the base64 alphabet of RFC 4648 §4, with padding, or
/// returns `None` when `text` is not such an encoding: a length that is not
/// a multiple of 4, a byte outside the alphabet, or padding of more than two
/// `=` or anywhere but at the end.
pub(super) fn decode(text: &str) -> Option<Vec<u8>> {
    let bytes = text.as_bytes();
    if bytes.len() % 4 != 0 {
        return None;
    }
    let groups = bytes.len() / 4;
    let mut out = Vec::with_capacity(groups * 3);
    for (i, group) in bytes.chunks(4).enumerate() {
        let pad = group.iter().rev().take_while(|&&b| b == b'=').count();
        if pad > 2 || (pad > 0 && i + 1 != groups) {
            return None;
        }
        let mut n: u32 = 0;
        for &b in &group[..4 - pad] {
            n = (n << 6) | u32::from(sextet(b)?);
        }
        n <<= 6 * pad as u32;
        out.push((n >> 16) as u8);
        if pad < 2 {
            out.push((n >> 8) as u8);
        }
        if pad < 1 {
            out.push(n as u8);
        }
    }
    Some(out)
}

/// The value of one byte of the alphabet.
fn sextet(b: u8) -> Option<u8> {
    match b {
        b'A'..=b'Z' => Some(b - b'A'),
        b'a'..=b'z' => Some(b - b'a' + 26),
        b'0'..=b'9' => Some(b - b'0' + 52),
        b'+' => Some(62),
        b'/' => Some(63),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{decode, encode};

    #[test]
    fn the_rfc_4648_test_vectors_encode() {
        let vectors: [(&str, &str); 7] = [
            ("", ""),
            ("f", "Zg=="),
            ("fo", "Zm8="),
            ("foo", "Zm9v"),
            ("foob", "Zm9vYg=="),
            ("fooba", "Zm9vYmE="),
            ("foobar", "Zm9vYmFy"),
        ];
        for (input, expected) in vectors {
            assert_eq!(encode(input.as_bytes()), expected, "input {input:?}");
            assert_eq!(
                decode(expected).as_deref(),
                Some(input.as_bytes()),
                "text {expected:?}"
            );
        }
    }

    /// The two bytes of the alphabet after the letters and digits, which no
    /// RFC 4648 vector uses.
    #[test]
    fn plus_and_slash_encode_and_decode() {
        assert_eq!(encode(&[0xfb, 0xff, 0xbf]), "+/+/");
        assert_eq!(decode("+/+/").as_deref(), Some(&[0xfb, 0xff, 0xbf][..]));
    }

    #[test]
    fn the_rfc_6455_sample_key_decodes_to_16_bytes() {
        let key = decode("dGhlIHNhbXBsZSBub25jZQ==").unwrap();
        assert_eq!(key, b"the sample nonce");
    }

    #[test]
    fn text_that_is_not_base64_does_not_decode() {
        let cases = [
            "Zg",       // not a multiple of 4
            "Zg=",      // not a multiple of 4
            "Zg==Zg==", // padding before the end
            "Z===",     // three padding bytes
            "Zm9!",     // outside the alphabet
            "Zm 9",     // a space
        ];
        for text in cases {
            assert_eq!(decode(text), None, "text {text:?}");
        }
    }
}
