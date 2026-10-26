//! Views of a client's file name. form-net never uses a file name as a path.

use core::fmt;

use crate::error::{EncodingError, FormError, Location};

/// Where a file name came from.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileNameSource {
    /// The `filename` parameter.
    Plain,
    /// An accepted `filename*` parameter (RFC 8187, decoded).
    Extended,
}

/// The client's file name, with a view for each way a caller might want it.
///
/// **form-net never uses a client file name as a path or as part of one.**
/// `Debug` prints the length only: file names are user data.
///
/// # Examples
///
/// ```
/// let body = b"--X\r\nContent-Disposition: form-data; name=\"doc\"; filename=\"C:\\Users\\me\\a b.pdf\"\r\n\r\nx\r\n--X--";
/// let mp = form_net::parse_multipart(b"multipart/form-data; boundary=X", body)?;
/// let name = mp.file("doc").unwrap().filename().unwrap();
/// assert_eq!(name.raw(), b"C:\\Users\\me\\a b.pdf");
/// assert_eq!(name.basename(), b"a b.pdf");
/// # Ok::<(), form_net::FormError>(())
/// ```
#[derive(Clone, Copy)]
pub struct FileName<'a> {
    raw: &'a [u8],
    source: FileNameSource,
}

impl<'a> FileName<'a> {
    pub(crate) const fn new(raw: &'a [u8], source: FileNameSource) -> Self {
        Self { raw, source }
    }

    /// The bytes as sent between the quotes of `filename`, untouched; or the
    /// decoded bytes of an accepted `filename*`.
    ///
    /// # Examples
    ///
    /// ```
    /// # let body = b"--X\r\nContent-Disposition: form-data; name=f; filename=\"a%22b.txt\"\r\n\r\n\r\n--X--";
    /// # let mp = form_net::parse_multipart(b"multipart/form-data; boundary=X", body)?;
    /// let name = mp.file("f").unwrap().filename().unwrap();
    /// assert_eq!(name.raw(), b"a%22b.txt");
    /// # Ok::<(), form_net::FormError>(())
    /// ```
    #[must_use]
    pub const fn raw(&self) -> &'a [u8] {
        self.raw
    }

    /// The raw bytes as UTF-8.
    ///
    /// # Errors
    ///
    /// `Encoding { InvalidUtf8 }`.
    ///
    /// # Examples
    ///
    /// ```
    /// # let body = "--X\r\nContent-Disposition: form-data; name=f; filename=\"café.txt\"\r\n\r\n\r\n--X--";
    /// # let mp = form_net::parse_multipart(b"multipart/form-data; boundary=X", body.as_bytes())?;
    /// assert_eq!(mp.file("f").unwrap().filename().unwrap().as_str()?, "café.txt");
    /// # Ok::<(), form_net::FormError>(())
    /// ```
    pub fn as_str(&self) -> Result<&'a str, FormError> {
        core::str::from_utf8(self.raw)
            .map_err(|_| FormError::encoding(EncodingError::InvalidUtf8, Location::at(0)))
    }

    /// Whether the name is empty: a file input left empty.
    ///
    /// # Examples
    ///
    /// ```
    /// # let body = b"--X\r\nContent-Disposition: form-data; name=f; filename=\"\"\r\n\r\n\r\n--X--";
    /// # let mp = form_net::parse_multipart(b"multipart/form-data; boundary=X", body)?;
    /// assert!(mp.file("f").unwrap().filename().unwrap().is_empty());
    /// # Ok::<(), form_net::FormError>(())
    /// ```
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.raw.is_empty()
    }

    /// `Plain` for `filename`, `Extended` for `filename*`.
    ///
    /// # Examples
    ///
    /// ```
    /// use form_net::FileNameSource;
    /// # let body = b"--X\r\nContent-Disposition: form-data; name=f; filename=a\r\n\r\n\r\n--X--";
    /// # let mp = form_net::parse_multipart(b"multipart/form-data; boundary=X", body)?;
    /// assert_eq!(mp.file("f").unwrap().filename().unwrap().source(), FileNameSource::Plain);
    /// # Ok::<(), form_net::FormError>(())
    /// ```
    #[must_use]
    pub const fn source(&self) -> FileNameSource {
        self.source
    }

    /// The bytes after the last `/` or `\` (some non-browser clients send
    /// full client paths).
    ///
    /// # Examples
    ///
    /// ```
    /// # let body = b"--X\r\nContent-Disposition: form-data; name=f; filename=\"a/b\\c.txt\"\r\n\r\n\r\n--X--";
    /// # let mp = form_net::parse_multipart(b"multipart/form-data; boundary=X", body)?;
    /// assert_eq!(mp.file("f").unwrap().filename().unwrap().basename(), b"c.txt");
    /// # Ok::<(), form_net::FormError>(())
    /// ```
    #[must_use]
    pub fn basename(&self) -> &'a [u8] {
        match self.raw.iter().rposition(|&b| b == b'/' || b == b'\\') {
            Some(i) => self.raw.get(i.wrapping_add(1)..).unwrap_or(&[]),
            None => self.raw,
        }
    }

    /// The text after the last `.` of the base name, when it is 1 to 16
    /// ASCII letters or digits. A hint that proves nothing about the
    /// content: only the bytes tell what a file is.
    ///
    /// # Examples
    ///
    /// ```
    /// # let body = b"--X\r\nContent-Disposition: form-data; name=f; filename=\"a/b.tar.gz\"\r\n\r\n\r\n--X--";
    /// # let mp = form_net::parse_multipart(b"multipart/form-data; boundary=X", body)?;
    /// assert_eq!(mp.file("f").unwrap().filename().unwrap().extension(), Some("gz"));
    /// # Ok::<(), form_net::FormError>(())
    /// ```
    #[must_use]
    pub fn extension(&self) -> Option<&'a str> {
        let base = self.basename();
        let dot = base.iter().rposition(|&b| b == b'.')?;
        let ext = base.get(dot.wrapping_add(1)..)?;
        if ext.is_empty() || ext.len() > 16 || !ext.iter().all(u8::is_ascii_alphanumeric) {
            return None;
        }
        core::str::from_utf8(ext).ok()
    }

    /// Writes the name into `out` with the HTML escapes `%0A`, `%0D`, `%22`
    /// decoded to LF, CR, `"`, and nothing else, and returns it as text.
    /// Browsers do not escape `%` itself, so a file the user named `a%22b`
    /// reads as `a"b`; that ambiguity is inherent in the HTML encoding.
    ///
    /// # Errors
    ///
    /// `BufferTooSmall` when `out` is shorter than [`raw`](Self::raw);
    /// `Encoding { InvalidUtf8 }`.
    ///
    /// # Examples
    ///
    /// ```
    /// # let body = b"--X\r\nContent-Disposition: form-data; name=f; filename=\"a%22b.txt\"\r\n\r\n\r\n--X--";
    /// # let mp = form_net::parse_multipart(b"multipart/form-data; boundary=X", body)?;
    /// let mut out = [0u8; 32];
    /// let name = mp.file("f").unwrap().filename().unwrap();
    /// assert_eq!(name.html_unescape_into(&mut out)?, "a\"b.txt");
    /// # Ok::<(), form_net::FormError>(())
    /// ```
    pub fn html_unescape_into<'b>(&self, out: &'b mut [u8]) -> Result<&'b str, FormError> {
        if out.len() < self.raw.len() {
            return Err(FormError::BufferTooSmall {
                needed: self.raw.len(),
            });
        }
        let n = html_unescape(self.raw, out);
        core::str::from_utf8(out.get(..n).unwrap_or(&[]))
            .map_err(|_| FormError::encoding(EncodingError::InvalidUtf8, Location::at(0)))
    }

    /// The name as UTF-8 in Unicode NFC (macOS clients may send NFD, so the
    /// same visible name can arrive as different bytes).
    ///
    /// # Errors
    ///
    /// `Encoding { InvalidUtf8 }`.
    ///
    /// # Examples
    ///
    /// ```
    /// # let body = "--X\r\nContent-Disposition: form-data; name=f; filename=\"cafe\u{301}.txt\"\r\n\r\n\r\n--X--";
    /// # let mp = form_net::parse_multipart(b"multipart/form-data; boundary=X", body.as_bytes())?;
    /// let name = mp.file("f").unwrap().filename().unwrap();
    /// assert_eq!(name.nfc()?, "caf\u{e9}.txt");
    /// # Ok::<(), form_net::FormError>(())
    /// ```
    #[cfg(feature = "unicode")]
    #[cfg_attr(docsrs, doc(cfg(feature = "unicode")))]
    pub fn nfc(&self) -> Result<alloc::string::String, FormError> {
        Ok(unicode_lang::normalize(
            self.as_str()?,
            unicode_lang::Form::Nfc,
        ))
    }

    /// As [`html_unescape_into`](Self::html_unescape_into), borrowed when the
    /// name has nothing to decode.
    ///
    /// # Errors
    ///
    /// `Encoding { InvalidUtf8 }`.
    ///
    /// # Examples
    ///
    /// ```
    /// # let body = b"--X\r\nContent-Disposition: form-data; name=f; filename=\"a%0Ab\"\r\n\r\n\r\n--X--";
    /// # let mp = form_net::parse_multipart(b"multipart/form-data; boundary=X", body)?;
    /// assert_eq!(mp.file("f").unwrap().filename().unwrap().html_unescaped()?, "a\nb");
    /// # Ok::<(), form_net::FormError>(())
    /// ```
    #[cfg(feature = "alloc")]
    #[cfg_attr(docsrs, doc(cfg(feature = "alloc")))]
    pub fn html_unescaped(&self) -> Result<alloc::borrow::Cow<'a, str>, FormError> {
        if !has_html_escape(self.raw) {
            return self.as_str().map(alloc::borrow::Cow::Borrowed);
        }
        let mut out = alloc::vec![0u8; self.raw.len()];
        let n = html_unescape(self.raw, &mut out);
        out.truncate(n);
        alloc::string::String::from_utf8(out)
            .map(alloc::borrow::Cow::Owned)
            .map_err(|_| FormError::encoding(EncodingError::InvalidUtf8, Location::at(0)))
    }
}

impl fmt::Debug for FileName<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "FileName({} bytes)", self.raw.len())
    }
}

/// The byte an HTML multipart escape at the start of `s` stands for.
#[inline]
fn html_escape_at(s: &[u8]) -> Option<u8> {
    match s.get(..3)? {
        b"%0A" => Some(b'\n'),
        b"%0D" => Some(b'\r'),
        b"%22" => Some(b'"'),
        _ => None,
    }
}

pub(crate) fn has_html_escape(s: &[u8]) -> bool {
    let mut i = 0usize;
    loop {
        i = crate::scan::find_byte(s, i, b'%');
        if i >= s.len() {
            return false;
        }
        if html_escape_at(s.get(i..).unwrap_or(&[])).is_some() {
            return true;
        }
        i = i.wrapping_add(1);
    }
}

/// Decodes the three HTML escapes of `src` into `out` (at least as long) and
/// returns the length written.
pub(crate) fn html_unescape(src: &[u8], out: &mut [u8]) -> usize {
    let mut i = 0usize;
    let mut w = 0usize;
    while i < src.len() {
        let rest = src.get(i..).unwrap_or(&[]);
        let (byte, step) = match html_escape_at(rest) {
            Some(b) => (b, 3usize),
            None => (*rest.first().unwrap_or(&0), 1usize),
        };
        if let Some(slot) = out.get_mut(w) {
            *slot = byte;
        }
        w = w.wrapping_add(1);
        i = i.wrapping_add(step);
    }
    w
}

#[cfg(test)]
mod tests {
    use super::*;

    fn name(raw: &[u8]) -> FileName<'_> {
        FileName::new(raw, FileNameSource::Plain)
    }

    #[test]
    fn test_basename_cases() {
        assert_eq!(name(b"a/b\\c.txt").basename(), b"c.txt");
        assert_eq!(name(b"c.txt").basename(), b"c.txt");
        assert_eq!(name(b"/").basename(), b"");
        assert_eq!(name(b"").basename(), b"");
        assert_eq!(name(b"\\\\").basename(), b"");
    }

    #[test]
    fn test_html_unescape_only_three_escapes() {
        let mut out = [0u8; 32];
        assert_eq!(
            name(b"%0A%0D%22").html_unescape_into(&mut out).unwrap(),
            "\n\r\""
        );
        assert_eq!(
            name(b"%25%0a%41").html_unescape_into(&mut out).unwrap(),
            "%25%0a%41"
        );
        assert_eq!(name(b"%2").html_unescape_into(&mut out).unwrap(), "%2");
        assert!(name(b"\xff").html_unescape_into(&mut out).is_err());
        let mut small = [0u8; 2];
        assert_eq!(
            name(b"abc").html_unescape_into(&mut small),
            Err(FormError::BufferTooSmall { needed: 3 })
        );
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn test_html_unescaped_borrows_when_plain() {
        assert!(matches!(
            name(b"plain").html_unescaped().unwrap(),
            alloc::borrow::Cow::Borrowed("plain")
        ));
        assert_eq!(name(b"a%22").html_unescaped().unwrap(), "a\"");
    }

    #[test]
    fn test_extension_cases() {
        assert_eq!(name(b".bashrc").extension(), Some("bashrc"));
        assert_eq!(name(b"a.").extension(), None);
        assert_eq!(name(b"a.tar.gz").extension(), Some("gz"));
        assert_eq!(name("a.\u{c4}\u{d6}".as_bytes()).extension(), None);
        assert_eq!(
            name(b"a.1234567890123456").extension(),
            Some("1234567890123456")
        );
        assert_eq!(name(b"a.12345678901234567").extension(), None);
        assert_eq!(name(b"dir.d/file").extension(), None);
        assert_eq!(name(b"noext").extension(), None);
    }

    #[test]
    fn test_debug_hides_the_name() {
        assert_eq!(
            alloc::format!("{:?}", name(b"secret.pdf")),
            "FileName(10 bytes)"
        );
    }
}
