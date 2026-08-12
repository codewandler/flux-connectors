//! A vendored SHA-256, so verifying the pack's digest costs no dependency.
//!
//! The reader's acceptance (C-537) is **zero non-optional dependencies**, and the pack's embedded
//! digest is SHA-256 because that is the one hash spelling this repository records anywhere
//! (`connector_spec::sha256_hex`, `connectors.lock`). Those two requirements meet here: FIPS 180-4
//! is a fixed, small, public algorithm, and vendoring ~100 lines of it is the cheaper trade than
//! either a `sha2` dependency in every consumer's tree or a weaker checksum that disagrees with
//! every other digest in the repository.
//!
//! **This is integrity, not authentication.** The digest catches truncation and corruption — a
//! partial download, a bad disk, a hand-edit — exactly as `connectors.lock` does one layer up. An
//! attacker who can rewrite the pack can rewrite its digest line too; trust in the *content* comes
//! from the same place it does for every other committed artifact: review and provenance, not this
//! hash.
//!
//! Checked three ways: the FIPS 180-4 vectors and the one-million-`a` message in this module's own
//! unit tests below, and byte-for-byte agreement with the `sha2` crate (a dev-dependency only)
//! across lengths that cross every padding boundary in
//! `tests/pack.rs::the_vendored_sha256_agrees_with_sha2_across_padding_boundaries`.

/// The eight initial hash values (FIPS 180-4 §5.3.3): the fractional parts of the square roots of
/// the first eight primes.
const H0: [u32; 8] = [
    0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19,
];

/// The sixty-four round constants (§4.2.2): the fractional parts of the cube roots of the first
/// sixty-four primes.
const K: [u32; 64] = [
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
    0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
    0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
    0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
    0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
    0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
    0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
    0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
];

/// The SHA-256 digest of `message`, as 32 raw bytes.
pub(crate) fn digest(message: &[u8]) -> [u8; 32] {
    let mut state = H0;

    // Process every whole 64-byte block of the message itself.
    let mut blocks = message.chunks_exact(64);
    for block in &mut blocks {
        compress(&mut state, block.try_into().expect("a 64-byte chunk"));
    }

    // Padding (§5.1.1): the remainder, `0x80`, zeros to 56 mod 64, then the bit length, big-endian.
    let remainder = blocks.remainder();
    let mut tail = [0u8; 128];
    tail[..remainder.len()].copy_from_slice(remainder);
    tail[remainder.len()] = 0x80;
    let tail_len = if remainder.len() < 56 { 64 } else { 128 };
    let bit_length = (message.len() as u64).wrapping_mul(8);
    tail[tail_len - 8..tail_len].copy_from_slice(&bit_length.to_be_bytes());
    for block in tail[..tail_len].chunks_exact(64) {
        compress(&mut state, block.try_into().expect("a 64-byte chunk"));
    }

    let mut out = [0u8; 32];
    for (chunk, word) in out.chunks_exact_mut(4).zip(state) {
        chunk.copy_from_slice(&word.to_be_bytes());
    }
    out
}

/// The digest as the lowercase-hex spelling every hash in this repository uses.
pub(crate) fn hex_digest(message: &[u8]) -> String {
    let mut hex = String::with_capacity(64);
    for byte in digest(message) {
        hex.push(char::from_digit((byte >> 4) as u32, 16).expect("a nibble is a hex digit"));
        hex.push(char::from_digit((byte & 0xf) as u32, 16).expect("a nibble is a hex digit"));
    }
    hex
}

/// One compression round over one 64-byte block (§6.2.2).
fn compress(state: &mut [u32; 8], block: &[u8; 64]) {
    let mut w = [0u32; 64];
    for (i, chunk) in block.chunks_exact(4).enumerate() {
        w[i] = u32::from_be_bytes(chunk.try_into().expect("a 4-byte chunk"));
    }
    for i in 16..64 {
        let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
        let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
        w[i] = w[i - 16]
            .wrapping_add(s0)
            .wrapping_add(w[i - 7])
            .wrapping_add(s1);
    }

    let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h] = *state;
    for i in 0..64 {
        let big_s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
        let ch = (e & f) ^ (!e & g);
        let t1 = h
            .wrapping_add(big_s1)
            .wrapping_add(ch)
            .wrapping_add(K[i])
            .wrapping_add(w[i]);
        let big_s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
        let maj = (a & b) ^ (a & c) ^ (b & c);
        let t2 = big_s0.wrapping_add(maj);
        h = g;
        g = f;
        f = e;
        e = d.wrapping_add(t1);
        d = c;
        c = b;
        b = a;
        a = t1.wrapping_add(t2);
    }

    state[0] = state[0].wrapping_add(a);
    state[1] = state[1].wrapping_add(b);
    state[2] = state[2].wrapping_add(c);
    state[3] = state[3].wrapping_add(d);
    state[4] = state[4].wrapping_add(e);
    state[5] = state[5].wrapping_add(f);
    state[6] = state[6].wrapping_add(g);
    state[7] = state[7].wrapping_add(h);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The FIPS 180-4 example vectors, plus the empty message every implementation gets asked for.
    #[test]
    fn the_published_vectors_agree() {
        for (message, expected) in [
            (
                &b""[..],
                "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
            ),
            (
                &b"abc"[..],
                "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
            ),
            (
                &b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"[..],
                "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1",
            ),
        ] {
            assert_eq!(hex_digest(message), expected);
        }
    }

    /// The long-message vector: one million `a`s, which exercises many blocks and the length
    /// arithmetic at a size no hand-picked short string reaches.
    #[test]
    fn the_million_a_vector_agrees() {
        let message = vec![b'a'; 1_000_000];
        assert_eq!(
            hex_digest(&message),
            "cdc76e5c9914fb9281a1c7e284d73e67f1809a48a497200e046d39ccc7112cd0"
        );
    }
}
