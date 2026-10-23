//! The urlencoded serializer, byte-identical to the WHATWG serializer for
//! UTF-8.

use core::fmt;

const UPPER_HEX: &[u8; 16] = b"0123456789ABCDEF";

/// Whether `b` is written as itself: ASCII alphanumerics and `*-._`.
#[inline]
const fn is_unreserved(b: u8) -> bool {
    b.is_ascii_alphanumeric() || matches!(b, b'*' | b'-' | b'.' | b'_')
}

/// Encodes one component (a name or a value) without allocating: every
/// byte outside ASCII alphanumerics and `*-._` is percent-encoded with
/// uppercase hex, and space is written as `+`. Runs of bytes that need no
/// escape are written with one `write_str`.
///
/// # Errors
///
/// Only what `out` returns.
///
/// # Examples
///
/// ```
/// let mut s = String::new();
/// form_net::encode_into("a b&c", &mut s).unwrap();
/// assert_eq!(s, "a+b%26c");
/// ```
pub fn encode_into<W: fmt::Write>(text: &str, out: &mut W) -> fmt::Result {
    let bytes = text.as_bytes();
    let mut run_start = 0usize;
    for (i, &b) in bytes.iter().enumerate() {
        if is_unreserved(b) {
            continue;
        }
        if let Some(run) = text.get(run_start..i) {
            if !run.is_empty() {
                out.write_str(run)?;
            }
        }
        if b == b' ' {
            out.write_char('+')?;
        } else {
            let hi = UPPER_HEX.get(usize::from(b >> 4)).copied().unwrap_or(b'0');
            let lo = UPPER_HEX
                .get(usize::from(b & 0x0F))
                .copied()
                .unwrap_or(b'0');
            out.write_char('%')?;
            out.write_char(char::from(hi))?;
            out.write_char(char::from(lo))?;
        }
        run_start = i.wrapping_add(1);
    }
    if let Some(run) = text.get(run_start..) {
        if !run.is_empty() {
            out.write_str(run)?;
        }
    }
    Ok(())
}

#[cfg(feature = "alloc")]
mod owned {
    use alloc::string::String;

    use super::encode_into;

    /// Writes into a `String`, which cannot fail.
    fn push_encoded(out: &mut String, text: &str) {
        // Writing into a String never returns an error.
        let _ = encode_into(text, out);
    }

    /// Builds a urlencoded body or query from pairs, byte-identical to the
    /// WHATWG serializer for UTF-8: names and values are encoded as
    /// [`encode_into`](crate::encode_into) does, joined with `=`, and pairs
    /// are joined with `&`. Infallible: every `str` is valid input.
    ///
    /// # Examples
    ///
    /// ```
    /// assert_eq!(form_net::serialize_urlencoded([("q", "rust forms"), ("page", "2")]), "q=rust+forms&page=2");
    /// assert_eq!(form_net::serialize_urlencoded([("a+b", "1&2=3")]), "a%2Bb=1%262%3D3");
    /// assert_eq!(form_net::serialize_urlencoded([("city", "Zürich")]), "city=Z%C3%BCrich");
    /// assert_eq!(form_net::serialize_urlencoded(Vec::<(&str, &str)>::new()), "");
    /// ```
    #[must_use]
    pub fn serialize_urlencoded<I, N, V>(pairs: I) -> String
    where
        I: IntoIterator<Item = (N, V)>,
        N: AsRef<str>,
        V: AsRef<str>,
    {
        let mut out = String::new();
        Serializer::new(&mut out).append_pairs(pairs);
        out
    }

    /// Appends urlencoded pairs to an existing string, for example a URL that
    /// already ends in `?`. The first pair is not preceded by `&`; whatever
    /// the string held before is left untouched.
    ///
    /// # Examples
    ///
    /// ```
    /// let mut url = String::from("https://example.com/search?");
    /// form_net::Serializer::new(&mut url)
    ///     .append_pair("q", "rust forms")
    ///     .append_pairs([("page", "2"), ("sort", "new")]);
    /// assert_eq!(url, "https://example.com/search?q=rust+forms&page=2&sort=new");
    /// ```
    #[derive(Debug)]
    pub struct Serializer<'s> {
        out: &'s mut String,
        first: bool,
    }

    impl<'s> Serializer<'s> {
        /// A serializer that appends to `out`.
        #[must_use]
        pub fn new(out: &'s mut String) -> Serializer<'s> {
            Serializer { out, first: true }
        }

        /// Appends one pair.
        pub fn append_pair(&mut self, name: &str, value: &str) -> &mut Self {
            if !self.first {
                self.out.push('&');
            }
            self.first = false;
            push_encoded(self.out, name);
            self.out.push('=');
            push_encoded(self.out, value);
            self
        }

        /// Appends every pair, in order.
        pub fn append_pairs<I, N, V>(&mut self, pairs: I) -> &mut Self
        where
            I: IntoIterator<Item = (N, V)>,
            N: AsRef<str>,
            V: AsRef<str>,
        {
            for (name, value) in pairs {
                self.append_pair(name.as_ref(), value.as_ref());
            }
            self
        }

        /// Ends the serializer and returns the string.
        pub fn finish(self) -> &'s mut String {
            self.out
        }
    }
}

#[cfg(feature = "alloc")]
pub use owned::{Serializer, serialize_urlencoded};

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::string::String;

    #[test]
    fn test_encode_every_ascii_byte_matches_whatwg() {
        for b in 0u8..=0x7F {
            let s = String::from(char::from(b));
            let mut out = String::new();
            encode_into(&s, &mut out).unwrap();
            let expected = if b == b' ' {
                String::from("+")
            } else if b.is_ascii_alphanumeric() || b"*-._".contains(&b) {
                s.clone()
            } else {
                alloc::format!("%{b:02X}")
            };
            assert_eq!(out, expected, "byte {b:#x}");
        }
    }

    #[test]
    fn test_encode_non_ascii_and_empty() {
        let mut out = String::new();
        encode_into("", &mut out).unwrap();
        assert_eq!(out, "");
        encode_into("€", &mut out).unwrap();
        assert_eq!(out, "%E2%82%AC");
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn test_serializer_appends_and_finishes() {
        let mut s = String::from("x?");
        let mut ser = Serializer::new(&mut s);
        ser.append_pair("", "").append_pair("a", "b");
        assert_eq!(ser.finish().as_str(), "x?=&a=b");
    }
}
