//! SHA-1 (FIPS 180-4), used only to compute `Sec-WebSocket-Accept`.

/// Returns the SHA-1 digest of `data`.
pub(super) fn digest(data: &[u8]) -> [u8; 20] {
    let mut h: [u32; 5] = [
        0x6745_2301,
        0xEFCD_AB89,
        0x98BA_DCFE,
        0x1032_5476,
        0xC3D2_E1F0,
    ];

    // Padding: one 0x80 byte, zero bytes up to 56 modulo 64, then the message
    // length in bits as a big-endian u64.
    let bit_len = (data.len() as u64).wrapping_mul(8);
    let mut padded = data.to_vec();
    padded.push(0x80);
    while padded.len() % 64 != 56 {
        padded.push(0);
    }
    padded.extend_from_slice(&bit_len.to_be_bytes());

    for block in padded.chunks_exact(64) {
        let mut w = [0u32; 80];
        for (i, word) in block.chunks_exact(4).enumerate() {
            w[i] = u32::from_be_bytes([word[0], word[1], word[2], word[3]]);
        }
        for i in 16..80 {
            w[i] = (w[i - 3] ^ w[i - 8] ^ w[i - 14] ^ w[i - 16]).rotate_left(1);
        }

        let [mut a, mut b, mut c, mut d, mut e] = h;
        for (i, &wi) in w.iter().enumerate() {
            let (f, k) = match i {
                0..=19 => ((b & c) | (!b & d), 0x5A82_7999),
                20..=39 => (b ^ c ^ d, 0x6ED9_EBA1),
                40..=59 => ((b & c) | (b & d) | (c & d), 0x8F1B_BCDC),
                _ => (b ^ c ^ d, 0xCA62_C1D6),
            };
            let t = a
                .rotate_left(5)
                .wrapping_add(f)
                .wrapping_add(e)
                .wrapping_add(k)
                .wrapping_add(wi);
            e = d;
            d = c;
            c = b.rotate_left(30);
            b = a;
            a = t;
        }
        for (hi, v) in h.iter_mut().zip([a, b, c, d, e]) {
            *hi = hi.wrapping_add(v);
        }
    }

    let mut out = [0u8; 20];
    for (chunk, word) in out.chunks_exact_mut(4).zip(h) {
        chunk.copy_from_slice(&word.to_be_bytes());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::digest;

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{b:02x}")).collect()
    }

    #[test]
    fn the_empty_message_has_the_published_digest() {
        assert_eq!(
            hex(&digest(b"")),
            "da39a3ee5e6b4b0d3255bfef95601890afd80709"
        );
    }

    #[test]
    fn abc_has_the_published_digest() {
        assert_eq!(
            hex(&digest(b"abc")),
            "a9993e364706816aba3e25717850c26c9cd0d89d"
        );
    }

    #[test]
    fn the_448_bit_message_has_the_published_digest() {
        let msg = b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq";
        assert_eq!(
            hex(&digest(msg)),
            "84983e441c3bd26ebaae4aa1f95129e5e54670f1"
        );
    }
}
