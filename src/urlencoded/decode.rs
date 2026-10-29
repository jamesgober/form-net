//! The urlencoded component decoder: `+` and `%XX` in one pass, then a
//! strict UTF-8 check.

use crate::error::{EncodingError, FormError, Location};
use crate::limits::PercentPolicy;

/// The value of each byte as a hex digit (either case), or `0xFF`.
// Built at compile time; a `u8` index into 256 entries cannot go out of
// bounds, and the digit arithmetic stays inside each match arm's range.
#[allow(clippy::indexing_slicing, clippy::arithmetic_side_effects)]
const HEX: [u8; 256] = {
    let mut t = [0xFFu8; 256];
    let mut i = 0u8;
    loop {
        t[i as usize] = match i {
            b'0'..=b'9' => i - b'0',
            b'a'..=b'f' => i - b'a' + 10,
            b'A'..=b'F' => i - b'A' + 10,
            _ => 0xFF,
        };
        if i == 255 {
            break;
        }
        i += 1;
    }
    t
};

#[inline]
fn hex(b: Option<&u8>) -> u8 {
    match b {
        Some(&b) => *HEX.get(usize::from(b)).unwrap_or(&0xFF),
        None => 0xFF,
    }
}

/// Decodes one urlencoded component (a name or a value) into `out`.
///
/// `+` becomes a space, `%XX` becomes the byte (either hex case), and the
/// result must be valid UTF-8: no overlong forms, no surrogates, no truncated
/// sequences. Invalid UTF-8 is refused, never replaced. The output is never
/// longer than `raw`, so `out.len() >= raw.len()` always suffices; a shorter
/// buffer is refused before anything is written.
///
/// # Errors
///
/// `Encoding { InvalidPercent }` (under `PercentPolicy::Refuse`) and
/// `Encoding { InvalidUtf8 }`, located relative to `raw`;
/// `BufferTooSmall { needed: raw.len() }`.
///
/// # Examples
///
/// ```
/// use form_net::PercentPolicy;
///
/// let mut out = [0u8; 32];
/// assert_eq!(form_net::decode_into(b"a+b%2Fc", &mut out, PercentPolicy::Refuse)?, "a b/c");
/// assert!(form_net::decode_into(b"%ED%A0%80", &mut out, PercentPolicy::Refuse).is_err());
/// assert_eq!(form_net::decode_into(b"%00", &mut out, PercentPolicy::Refuse)?, "\0");
/// # Ok::<(), form_net::FormError>(())
/// ```
pub fn decode_into<'b>(
    raw: &[u8],
    out: &'b mut [u8],
    policy: PercentPolicy,
) -> Result<&'b str, FormError> {
    decode_component(raw, out, policy)
}

pub(crate) fn decode_component<'b>(
    raw: &[u8],
    out: &'b mut [u8],
    policy: PercentPolicy,
) -> Result<&'b str, FormError> {
    if out.len() < raw.len() {
        return Err(FormError::BufferTooSmall { needed: raw.len() });
    }
    let n = decode_bytes(raw, out, policy)?;
    let text = out.get(..n).unwrap_or(&[]);
    core::str::from_utf8(text)
        .map_err(|_| FormError::encoding(EncodingError::InvalidUtf8, Location::at(0)))
}

/// Where decoded bytes go: a caller slice, or a growing `Vec`.
trait Out {
    fn run(&mut self, bytes: &[u8]);
    fn byte(&mut self, b: u8);
}

/// A caller slice at least as long as the input: decoding never lengthens.
struct SliceOut<'o> {
    out: &'o mut [u8],
    w: usize,
}

impl Out for SliceOut<'_> {
    #[inline]
    fn run(&mut self, bytes: &[u8]) {
        if let Some(dst) = self.out.get_mut(self.w..self.w.wrapping_add(bytes.len())) {
            dst.copy_from_slice(bytes);
        }
        self.w = self.w.wrapping_add(bytes.len());
    }
    #[inline]
    fn byte(&mut self, b: u8) {
        if let Some(slot) = self.out.get_mut(self.w) {
            *slot = b;
        }
        self.w = self.w.wrapping_add(1);
    }
}

/// A `Vec` with the input's length reserved: no zeroing, no regrowth.
#[cfg(feature = "alloc")]
impl Out for alloc::vec::Vec<u8> {
    #[inline]
    fn run(&mut self, bytes: &[u8]) {
        self.extend_from_slice(bytes);
    }
    #[inline]
    fn byte(&mut self, b: u8) {
        self.push(b);
    }
}

/// Decodes `raw` into `out`. No UTF-8 check.
///
/// Escapes are decoded where they stand, so a run of them (`%41%41...`)
/// costs one table load per digit; only runs of plain bytes are scanned for
/// the next `%` or `+`, eight bytes at a time.
#[inline]
fn decode_with<O: Out>(raw: &[u8], out: &mut O, policy: PercentPolicy) -> Result<(), FormError> {
    let mut i = 0usize;
    while let Some(&b) = raw.get(i) {
        let byte = match b {
            b'%' => {
                let h = hex(raw.get(i.wrapping_add(1)));
                let l = hex(raw.get(i.wrapping_add(2)));
                if h | l < 16 {
                    i = i.wrapping_add(3);
                    (h << 4) | l
                } else if policy == PercentPolicy::Literal {
                    i = i.wrapping_add(1);
                    b'%'
                } else {
                    return Err(FormError::encoding(
                        EncodingError::InvalidPercent,
                        Location::at(i as u64),
                    ));
                }
            }
            b'+' => {
                i = i.wrapping_add(1);
                b' '
            }
            _ => {
                let next = crate::scan::find_any(raw, i, *b"%+");
                out.run(raw.get(i..next).unwrap_or(&[]));
                i = next;
                continue;
            }
        };
        out.byte(byte);
    }
    Ok(())
}

/// Writes the decoded bytes of `raw` into `out` (at least `raw.len()` long)
/// and returns how many were written. No UTF-8 check.
fn decode_bytes(raw: &[u8], out: &mut [u8], policy: PercentPolicy) -> Result<usize, FormError> {
    let mut sink = SliceOut { out, w: 0 };
    decode_with(raw, &mut sink, policy)?;
    Ok(sink.w)
}

/// Decodes `raw` into a new `String`. On an error, the bytes decoded so
/// far are wiped before the buffer is freed.
#[cfg(feature = "alloc")]
pub(crate) fn decode_to_vec(
    raw: &[u8],
    policy: PercentPolicy,
) -> Result<alloc::string::String, FormError> {
    let mut buf = alloc::vec::Vec::with_capacity(raw.len());
    if let Err(e) = decode_with(raw, &mut buf, policy) {
        crate::wipe::wipe_vec(&mut buf);
        return Err(e);
    }
    alloc::string::String::from_utf8(buf).map_err(|e| {
        let mut bytes = e.into_bytes();
        crate::wipe::wipe_vec(&mut bytes);
        FormError::encoding(EncodingError::InvalidUtf8, Location::at(0))
    })
}

/// Appends the decoded `raw` to `arena` (which must have room for
/// `raw.len()` more bytes past `at`) and returns the decoded length.
#[cfg(feature = "alloc")]
pub(crate) fn decode_append(
    raw: &[u8],
    arena: &mut [u8],
    at: usize,
    policy: PercentPolicy,
) -> Result<usize, FormError> {
    let Some(dst) = arena.get_mut(at..) else {
        return Err(FormError::BufferTooSmall { needed: raw.len() });
    };
    if dst.len() < raw.len() {
        return Err(FormError::BufferTooSmall { needed: raw.len() });
    }
    let n = decode_bytes(raw, dst, policy)?;
    let text = dst.get(..n).unwrap_or(&[]);
    if core::str::from_utf8(text).is_err() {
        return Err(FormError::encoding(
            EncodingError::InvalidUtf8,
            Location::at(0),
        ));
    }
    Ok(n)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dec(raw: &[u8], policy: PercentPolicy) -> Result<alloc::string::String, FormError> {
        let mut out = alloc::vec![0u8; raw.len()];
        decode_into(raw, &mut out, policy).map(alloc::string::ToString::to_string)
    }

    #[test]
    fn test_decode_plus_and_hex_both_cases() {
        assert_eq!(dec(b"+%20+", PercentPolicy::Refuse).unwrap(), "   ");
        assert_eq!(dec(b"%2f%2F", PercentPolicy::Refuse).unwrap(), "//");
        assert_eq!(dec(b"%2B", PercentPolicy::Refuse).unwrap(), "+");
        assert_eq!(dec(b"", PercentPolicy::Refuse).unwrap(), "");
    }

    #[test]
    fn test_decode_bad_percent_refused_or_literal() {
        for (raw, at) in [
            (&b"%"[..], 0),
            (b"%4", 0),
            (b"%4g", 0),
            (b"%zz", 0),
            (b"a%%41", 1),
        ] {
            let e = dec(raw, PercentPolicy::Refuse).unwrap_err();
            assert_eq!(
                e,
                FormError::encoding(EncodingError::InvalidPercent, Location::at(at)),
                "{raw:?}"
            );
        }
        assert_eq!(dec(b"%", PercentPolicy::Literal).unwrap(), "%");
        assert_eq!(dec(b"%4g", PercentPolicy::Literal).unwrap(), "%4g");
        assert_eq!(dec(b"%%41", PercentPolicy::Literal).unwrap(), "%A");
    }

    #[test]
    fn test_decode_invalid_utf8_refused() {
        for raw in [
            &b"%C0%AF"[..],
            b"%ED%A0%80",
            b"%E2%82",
            b"%FF",
            b"%F8%88%80%80%80",
            b"\xff",
        ] {
            assert_eq!(
                dec(raw, PercentPolicy::Refuse).unwrap_err(),
                FormError::encoding(EncodingError::InvalidUtf8, Location::at(0)),
                "{raw:?}"
            );
        }
        assert_eq!(
            dec(b"%EF%BB%BFx", PercentPolicy::Refuse).unwrap(),
            "\u{feff}x"
        );
        assert_eq!(dec("é".as_bytes(), PercentPolicy::Refuse).unwrap(), "é");
    }

    #[test]
    fn test_decode_short_buffer_refused_before_writing() {
        let mut out = [7u8; 2];
        assert_eq!(
            decode_into(b"abc", &mut out, PercentPolicy::Refuse),
            Err(FormError::BufferTooSmall { needed: 3 })
        );
        assert_eq!(out, [7, 7]);
    }

    #[test]
    fn test_decode_never_longer_than_input() {
        let raw = b"a%41+b%%";
        let mut out = [0u8; 8];
        let s = decode_into(raw, &mut out, PercentPolicy::Literal).unwrap();
        assert!(s.len() <= raw.len());
    }
}
