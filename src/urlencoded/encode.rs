//! The urlencoded serializer, byte-identical to the WHATWG serializer for
//! UTF-8.

use core::fmt;

/// `%XX` for every byte, so an escape is one `write_str` of three bytes
/// with nothing to check.
const ESCAPES: [&str; 256] = [
    "%00", "%01", "%02", "%03", "%04", "%05", "%06", "%07", "%08", "%09", "%0A", "%0B", "%0C",
    "%0D", "%0E", "%0F", "%10", "%11", "%12", "%13", "%14", "%15", "%16", "%17", "%18", "%19",
    "%1A", "%1B", "%1C", "%1D", "%1E", "%1F", "%20", "%21", "%22", "%23", "%24", "%25", "%26",
    "%27", "%28", "%29", "%2A", "%2B", "%2C", "%2D", "%2E", "%2F", "%30", "%31", "%32", "%33",
    "%34", "%35", "%36", "%37", "%38", "%39", "%3A", "%3B", "%3C", "%3D", "%3E", "%3F", "%40",
    "%41", "%42", "%43", "%44", "%45", "%46", "%47", "%48", "%49", "%4A", "%4B", "%4C", "%4D",
    "%4E", "%4F", "%50", "%51", "%52", "%53", "%54", "%55", "%56", "%57", "%58", "%59", "%5A",
    "%5B", "%5C", "%5D", "%5E", "%5F", "%60", "%61", "%62", "%63", "%64", "%65", "%66", "%67",
    "%68", "%69", "%6A", "%6B", "%6C", "%6D", "%6E", "%6F", "%70", "%71", "%72", "%73", "%74",
    "%75", "%76", "%77", "%78", "%79", "%7A", "%7B", "%7C", "%7D", "%7E", "%7F", "%80", "%81",
    "%82", "%83", "%84", "%85", "%86", "%87", "%88", "%89", "%8A", "%8B", "%8C", "%8D", "%8E",
    "%8F", "%90", "%91", "%92", "%93", "%94", "%95", "%96", "%97", "%98", "%99", "%9A", "%9B",
    "%9C", "%9D", "%9E", "%9F", "%A0", "%A1", "%A2", "%A3", "%A4", "%A5", "%A6", "%A7", "%A8",
    "%A9", "%AA", "%AB", "%AC", "%AD", "%AE", "%AF", "%B0", "%B1", "%B2", "%B3", "%B4", "%B5",
    "%B6", "%B7", "%B8", "%B9", "%BA", "%BB", "%BC", "%BD", "%BE", "%BF", "%C0", "%C1", "%C2",
    "%C3", "%C4", "%C5", "%C6", "%C7", "%C8", "%C9", "%CA", "%CB", "%CC", "%CD", "%CE", "%CF",
    "%D0", "%D1", "%D2", "%D3", "%D4", "%D5", "%D6", "%D7", "%D8", "%D9", "%DA", "%DB", "%DC",
    "%DD", "%DE", "%DF", "%E0", "%E1", "%E2", "%E3", "%E4", "%E5", "%E6", "%E7", "%E8", "%E9",
    "%EA", "%EB", "%EC", "%ED", "%EE", "%EF", "%F0", "%F1", "%F2", "%F3", "%F4", "%F5", "%F6",
    "%F7", "%F8", "%F9", "%FA", "%FB", "%FC", "%FD", "%FE", "%FF",
];

/// The escape of `b` as text.
#[inline]
fn escape(b: u8) -> &'static str {
    ESCAPES.get(usize::from(b)).copied().unwrap_or("%00")
}

/// Whether each byte is written as itself: ASCII alphanumerics and `*-._`.
// Built at compile time; a `u8` index into 256 entries cannot go out of
// bounds, and `i += 1` stops before 255 overflows.
#[allow(clippy::indexing_slicing, clippy::arithmetic_side_effects)]
const UNRESERVED: [bool; 256] = {
    let mut t = [false; 256];
    let mut i = 0u8;
    loop {
        t[i as usize] = i.is_ascii_alphanumeric() || matches!(i, b'*' | b'-' | b'.' | b'_');
        if i == 255 {
            break;
        }
        i += 1;
    }
    t
};

#[inline]
fn is_unreserved(b: u8) -> bool {
    *UNRESERVED.get(usize::from(b)).unwrap_or(&false)
}

/// The index of the first byte at or after `i` that must be escaped, or
/// `bytes.len()`. Form names and values are short and their runs shorter,
/// so a table lookup per byte beats a word test (measured).
#[inline]
fn next_reserved(bytes: &[u8], mut i: usize) -> usize {
    while let Some(&b) = bytes.get(i) {
        if !is_unreserved(b) {
            return i;
        }
        i = i.wrapping_add(1);
    }
    bytes.len()
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
    let mut i = 0usize;
    while i < bytes.len() {
        let end = next_reserved(bytes, i);
        // Runs end before a byte that is escaped, so they end on a char
        // boundary (escaped bytes include every non-ASCII byte).
        if let Some(run) = text.get(i..end) {
            if !run.is_empty() {
                out.write_str(run)?;
            }
        }
        let Some(&b) = bytes.get(end) else { break };
        if b == b' ' {
            out.write_char('+')?;
        } else {
            out.write_str(escape(b))?;
        }
        i = end.wrapping_add(1);
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
        // Room for a typical form up front: growing from empty took four
        // reallocations for ten pairs.
        let mut out = String::with_capacity(256);
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
            // Exact when nothing needs escaping, the common case: one
            // allocation step per pair at most.
            self.out
                .reserve(name.len().saturating_add(value.len()).saturating_add(2));
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
