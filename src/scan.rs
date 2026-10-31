//! Byte scanning shared by both formats: eight bytes per step with plain
//! `u64` arithmetic (SWAR), safe Rust.

const LO: u64 = 0x0101_0101_0101_0101;
const HI: u64 = 0x8080_8080_8080_8080;

/// A word with the high bit set in every byte of `word` equal to `byte`.
///
/// The classic "has zero byte" test on `word ^ splat(byte)`. It can set
/// false bits only above a true match (borrow propagation), so the lowest set
/// bit always marks the first match.
#[inline]
const fn matches(word: u64, byte: u8) -> u64 {
    let x = word ^ LO.wrapping_mul(byte as u64);
    x.wrapping_sub(LO) & !x & HI
}

#[inline]
fn word_at(chunk: &[u8]) -> u64 {
    let mut bytes = [0u8; 8];
    bytes.copy_from_slice(chunk);
    u64::from_le_bytes(bytes)
}

/// The index of the first byte of `hay` at or after `from` that is any of
/// the bytes in `set` (at most four), or `hay.len()`.
#[inline]
pub(crate) fn find_any<const N: usize>(hay: &[u8], from: usize, set: [u8; N]) -> usize {
    let start = from.min(hay.len());
    let rest = hay.get(start..).unwrap_or(&[]);
    let mut chunks = rest.chunks_exact(8);
    let mut base = start;
    for chunk in &mut chunks {
        let word = word_at(chunk);
        let mut hits = 0u64;
        for &b in &set {
            hits |= matches(word, b);
        }
        if hits != 0 {
            return base.wrapping_add((hits.trailing_zeros() / 8) as usize);
        }
        base = base.wrapping_add(8);
    }
    for (i, &b) in chunks.remainder().iter().enumerate() {
        if set.contains(&b) {
            return base.wrapping_add(i);
        }
    }
    hay.len()
}

/// The little-endian word of the 8 bytes at `at`, or 0 past the end.
#[inline]
fn word(hay: &[u8], at: usize) -> u64 {
    hay.get(at..)
        .and_then(<[u8]>::first_chunk::<8>)
        .map_or(0, |c| u64::from_le_bytes(*c))
}

/// Whether `c` ends a line to some reader: CR or LF.
#[inline]
pub(crate) const fn is_break(c: u8) -> bool {
    c == b'\r' || c == b'\n'
}

/// The index of the first CR or LF at or after `from` that is followed by
/// `-`, or of a CR or LF that is the last byte of `hay` (the pair may
/// complete in the next chunk), or `hay.len()`.
///
/// Thirty-two positions per step: four words one byte on are matched
/// against `-`, and only a word that holds one has the word before it
/// matched against CR and LF. So data with few `-` bytes is crossed at the
/// cost of four matches per 32 bytes, and data made of line breaks (with no
/// `-` after them) or of dashes (with no line break before them) is still
/// crossed 32 bytes at a time.
#[inline]
pub(crate) fn find_break_dash(hay: &[u8], from: usize) -> usize {
    let mut i = from;
    // Thirty-two positions per step while 33 bytes remain (the `-` words
    // read one byte past the block). One OR of four masks decides most
    // blocks; only a block holding a `-` looks for the line break.
    while let Some(block) = hay.get(i..i.wrapping_add(33)) {
        let d = [
            matches(word(block, 1), b'-'),
            matches(word(block, 9), b'-'),
            matches(word(block, 17), b'-'),
            matches(word(block, 25), b'-'),
        ];
        if d[0] | d[1] | d[2] | d[3] != 0 {
            for (k, &dk) in d.iter().enumerate() {
                if dk == 0 {
                    continue;
                }
                let base = k.wrapping_mul(8);
                let w = word(block, base);
                let mut hits = dk & (matches(w, b'\r') | matches(w, b'\n'));
                while hits != 0 {
                    // The zero-byte test can set a bit above a true match;
                    // each candidate is checked against the bytes.
                    let at = base.wrapping_add((hits.trailing_zeros() / 8) as usize);
                    if block.get(at).is_some_and(|&c| is_break(c))
                        && block.get(at.wrapping_add(1)) == Some(&b'-')
                    {
                        return i.wrapping_add(at);
                    }
                    hits &= hits.wrapping_sub(1);
                }
            }
        }
        i = i.wrapping_add(32);
    }
    while let Some(&c) = hay.get(i) {
        if is_break(c) {
            match hay.get(i.wrapping_add(1)) {
                None | Some(b'-') => return i,
                _ => {}
            }
        }
        i = i.wrapping_add(1);
    }
    hay.len()
}

/// How many leading bytes `a` and `b` share, eight at a time.
#[inline]
pub(crate) fn common_prefix(a: &[u8], b: &[u8]) -> usize {
    let mut n = 0usize;
    let (mut x, mut y) = (a.chunks_exact(8), b.chunks_exact(8));
    for (p, q) in (&mut x).zip(&mut y) {
        let diff = word_at(p) ^ word_at(q);
        if diff != 0 {
            return n.wrapping_add((diff.trailing_zeros() / 8) as usize);
        }
        n = n.wrapping_add(8);
    }
    let (p, q) = (a.get(n..).unwrap_or(&[]), b.get(n..).unwrap_or(&[]));
    n.wrapping_add(p.iter().zip(q).take_while(|(c, d)| c == d).count())
}

/// The index of the first `byte` in `hay` at or after `from`, or
/// `hay.len()`.
#[inline]
pub(crate) fn find_byte(hay: &[u8], from: usize, byte: u8) -> usize {
    find_any(hay, from, [byte])
}

/// How many times `byte` occurs in `hay`.
#[cfg(feature = "alloc")]
pub(crate) fn count_byte(hay: &[u8], byte: u8) -> usize {
    let mut n = 0usize;
    let mut i = 0usize;
    loop {
        i = find_byte(hay, i, byte);
        if i >= hay.len() {
            return n;
        }
        n = n.wrapping_add(1);
        i = i.wrapping_add(1);
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_common_prefix_at_every_length_and_difference() {
        let a: [u8; 40] = core::array::from_fn(|i| i as u8);
        for len in 0..=a.len() {
            assert_eq!(super::common_prefix(&a[..len], &a), len);
            assert_eq!(super::common_prefix(&a, &a[..len]), len);
            for at in 0..len {
                let mut b = a;
                b[at] ^= 0x80;
                assert_eq!(super::common_prefix(&a[..len], &b[..len]), at);
            }
        }
    }

    use super::*;

    fn naive(hay: &[u8], from: usize, set: &[u8]) -> usize {
        (from.min(hay.len())..hay.len())
            .find(|&i| set.contains(&hay[i]))
            .unwrap_or(hay.len())
    }

    #[test]
    fn test_find_any_matches_naive_every_position() {
        let hay = b"abcdefgh&ijklmnop=qrstu%vwxyz+0123456789abcdefghij&";
        for from in 0..=hay.len() + 2 {
            assert_eq!(
                find_any(hay, from, *b"&=%+"),
                naive(hay, from, b"&=%+"),
                "from {from}"
            );
        }
    }

    fn naive_break_dash(hay: &[u8], from: usize) -> usize {
        (from..hay.len())
            .find(|&i| is_break(hay[i]) && hay.get(i + 1).is_none_or(|&n| n == b'-'))
            .unwrap_or(hay.len())
    }

    #[test]
    fn test_find_break_dash_matches_naive_on_dense_and_borrow_patterns() {
        // CR, LF, and `-` in every arrangement, the other bytes the mask
        // admits (0x08 to 0x0F), high bytes that exercise the borrow of the
        // zero-byte test, and every start position.
        let mut seed = 7u64;
        for len in 0..64 {
            for _ in 0..40 {
                let hay: alloc::vec::Vec<u8> = (0..len)
                    .map(|_| {
                        seed ^= seed << 13;
                        seed ^= seed >> 7;
                        seed ^= seed << 17;
                        [
                            b'\r', b'\n', b'-', 0x08, 0x0c, 0x0e, 0x8d, 0x8a, 0xad, b'x', 0x00,
                        ][(seed % 11) as usize]
                    })
                    .collect();
                for from in 0..=len {
                    assert_eq!(
                        find_break_dash(&hay, from),
                        naive_break_dash(&hay, from),
                        "{hay:?} from {from}"
                    );
                }
            }
        }
    }

    #[test]
    fn test_find_byte_high_bytes_and_none() {
        let hay = [0x80u8, 0xff, 0x0d, 0x01, 0x0d, 0, 0, 0, 0, 0x0d];
        assert_eq!(find_byte(&hay, 0, 0x0d), 2);
        assert_eq!(find_byte(&hay, 3, 0x0d), 4);
        assert_eq!(find_byte(&hay, 5, 0x0d), 9);
        assert_eq!(find_byte(&hay, 0, b'x'), hay.len());
        assert_eq!(find_byte(&[], 0, b'x'), 0);
    }

    #[test]
    fn test_matches_borrow_never_hides_first_match() {
        // 0x01 next to the target byte is the borrow case.
        for target in [0u8, 1, 0x0d, 0x80, 0xff] {
            for pos in 0..8 {
                let mut bytes = [target.wrapping_add(1); 8];
                bytes[pos] = target;
                if pos > 0 {
                    bytes[pos - 1] = target ^ 1;
                }
                let w = u64::from_le_bytes(bytes);
                let first = naive(&bytes, 0, &[target]);
                assert_eq!((matches(w, target).trailing_zeros() / 8) as usize, first);
            }
        }
    }
}
