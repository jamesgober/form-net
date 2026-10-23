//! The urlencoded component decoder: `+` and `%XX` in one pass, then a
//! strict UTF-8 check.

use crate::error::{EncodingError, FormError, Location};
use crate::limits::PercentPolicy;

#[inline]
fn hex(b: Option<&u8>) -> Option<u8> {
    parse_net::class::hex_value(*b?)
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

/// Writes the decoded bytes of `raw` into `out` (at least `raw.len()` long)
/// and returns how many were written. No UTF-8 check.
fn decode_bytes(raw: &[u8], out: &mut [u8], policy: PercentPolicy) -> Result<usize, FormError> {
    let mut i = 0usize;
    let mut w = 0usize;
    while i < raw.len() {
        let next = crate::scan::find_any(raw, i, *b"%+");
        let run = raw.get(i..next).unwrap_or(&[]);
        if let Some(dst) = out.get_mut(w..w.wrapping_add(run.len())) {
            dst.copy_from_slice(run);
        }
        w = w.wrapping_add(run.len());
        i = next;
        let Some(&b) = raw.get(i) else { break };
        let byte = if b == b'+' {
            i = i.wrapping_add(1);
            b' '
        } else {
            match (
                hex(raw.get(i.wrapping_add(1))),
                hex(raw.get(i.wrapping_add(2))),
            ) {
                (Some(h), Some(l)) => {
                    i = i.wrapping_add(3);
                    (h << 4) | l
                }
                _ => match policy {
                    PercentPolicy::Literal => {
                        i = i.wrapping_add(1);
                        b'%'
                    }
                    _ => {
                        return Err(FormError::encoding(
                            EncodingError::InvalidPercent,
                            Location::at(i as u64),
                        ));
                    }
                },
            }
        };
        if let Some(slot) = out.get_mut(w) {
            *slot = byte;
        }
        w = w.wrapping_add(1);
    }
    Ok(w)
}

/// The decoded length and validity of `raw` without writing anything:
/// used to size owned buffers.
#[cfg(feature = "alloc")]
pub(crate) fn decode_to_vec(
    raw: &[u8],
    policy: PercentPolicy,
) -> Result<alloc::string::String, FormError> {
    let mut buf = alloc::vec![0u8; raw.len()];
    let n = decode_bytes(raw, &mut buf, policy)?;
    buf.truncate(n);
    alloc::string::String::from_utf8(buf)
        .map_err(|_| FormError::encoding(EncodingError::InvalidUtf8, Location::at(0)))
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
