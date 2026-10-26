//! The client side: part heads encoded as browsers encode them, and a
//! writer whose every body parses back with form-net to the same fields.

use crate::error::{FormError, Location, SyntaxError};
use crate::multipart::boundary::Boundary;

const DISPOSITION: &[u8] = b"Content-Disposition: form-data; name=\"";
const FILENAME: &[u8] = b"\"; filename=\"";
const CONTENT_TYPE: &[u8] = b"Content-Type: ";

/// Where encoded bytes go: a caller buffer or a writer.
trait Out {
    fn put(&mut self, bytes: &[u8]) -> Result<(), FormError>;
}

struct SliceOut<'o> {
    out: &'o mut [u8],
    pos: usize,
}

impl Out for SliceOut<'_> {
    fn put(&mut self, bytes: &[u8]) -> Result<(), FormError> {
        let end = self.pos.saturating_add(bytes.len());
        match self.out.get_mut(self.pos..end) {
            Some(dst) => {
                dst.copy_from_slice(bytes);
                self.pos = end;
                Ok(())
            }
            None => Err(FormError::BufferTooSmall { needed: end }),
        }
    }
}

fn syntax(reason: SyntaxError) -> FormError {
    FormError::syntax(reason, Location::NONE)
}

/// Checks a name or file name and returns its length once escaped.
///
/// LF, CR, and `"` are escaped as `%0A`, `%0D`, `%22` (the browser rule);
/// any other control byte but HTAB is refused. So are the sequences that
/// would read back as another character: a literal `%0A`, `%0D`, or `%22`
/// (a reader decodes it), and for names any backslash (form-net refuses one
/// in a name, D-082), for file names a trailing one (it would read as `\"`).
fn escaped_len(text: &str, is_name: bool) -> Result<usize, FormError> {
    let bytes = text.as_bytes();
    if is_name && bytes.is_empty() {
        return Err(syntax(SyntaxError::EmptyName));
    }
    if crate::multipart::filename::has_html_escape(bytes) {
        return Err(syntax(SyntaxError::AmbiguousQuote));
    }
    if (is_name && bytes.contains(&b'\\')) || (!is_name && bytes.last() == Some(&b'\\')) {
        return Err(syntax(SyntaxError::AmbiguousQuote));
    }
    let mut len = 0usize;
    for &b in bytes {
        len = len.saturating_add(match b {
            b'\n' | b'\r' | b'"' => 3,
            b'\t' => 1,
            b if b < 0x20 || b == 0x7F => return Err(syntax(SyntaxError::ControlInHeader)),
            _ => 1,
        });
    }
    Ok(len)
}

/// Writes `text` with the browser escapes. `escaped_len` has checked it.
fn put_escaped(out: &mut impl Out, text: &str) -> Result<(), FormError> {
    let bytes = text.as_bytes();
    let mut from = 0usize;
    for (i, &b) in bytes.iter().enumerate() {
        let esc: &[u8] = match b {
            b'\n' => b"%0A",
            b'\r' => b"%0D",
            b'"' => b"%22",
            _ => continue,
        };
        out.put(bytes.get(from..i).unwrap_or(&[]))?;
        out.put(esc)?;
        from = i.saturating_add(1);
    }
    out.put(bytes.get(from..).unwrap_or(&[]))
}

/// Checks a part's `Content-Type` value the way the parser will read it.
fn check_content_type(ct: &str) -> Result<(), FormError> {
    let raw = ct.as_bytes();
    if raw.iter().any(|&b| (b < 0x20 && b != b'\t') || b == 0x7F) {
        return Err(syntax(SyntaxError::ControlInHeader));
    }
    if raw.windows(2).any(|w| w == b"\\\"") {
        return Err(syntax(SyntaxError::AmbiguousQuote));
    }
    let invalid = || syntax(SyntaxError::InvalidContentType);
    let mt = parse_net::media::parse(raw, &crate::kind::HEADER_LIMITS).map_err(|_| invalid())?;
    if crate::kind::repeated_name(&mt.params()).is_some()
        || mt.type_().eq_ignore_ascii_case(b"multipart")
    {
        return Err(invalid());
    }
    Ok(())
}

/// The exact length of a part head, after checking every input.
fn head_len(
    boundary: &Boundary,
    first: bool,
    name: &str,
    filename: Option<&str>,
    content_type: Option<&str>,
) -> Result<usize, FormError> {
    let mut len = if first { 0usize } else { 2 };
    len = len
        .saturating_add(2)
        .saturating_add(boundary.as_bytes().len())
        .saturating_add(2)
        .saturating_add(DISPOSITION.len())
        .saturating_add(escaped_len(name, true)?);
    if let Some(f) = filename {
        len = len
            .saturating_add(FILENAME.len())
            .saturating_add(escaped_len(f, false)?);
    }
    // The closing quote and CRLF of the disposition line.
    len = len.saturating_add(3);
    if let Some(ct) = content_type {
        check_content_type(ct)?;
        len = len
            .saturating_add(CONTENT_TYPE.len())
            .saturating_add(ct.len())
            .saturating_add(2);
    }
    Ok(len.saturating_add(2))
}

/// Writes a head that `head_len` accepted.
fn put_head(
    out: &mut impl Out,
    boundary: &Boundary,
    first: bool,
    name: &str,
    filename: Option<&str>,
    content_type: Option<&str>,
) -> Result<(), FormError> {
    if !first {
        out.put(b"\r\n")?;
    }
    out.put(b"--")?;
    out.put(boundary.as_bytes())?;
    out.put(b"\r\n")?;
    out.put(DISPOSITION)?;
    put_escaped(out, name)?;
    if let Some(f) = filename {
        out.put(FILENAME)?;
        put_escaped(out, f)?;
    }
    out.put(b"\"\r\n")?;
    if let Some(ct) = content_type {
        out.put(CONTENT_TYPE)?;
        out.put(ct.as_bytes())?;
        out.put(b"\r\n")?;
    }
    out.put(b"\r\n")
}

/// Writes one part's head into `out` without allocating: the delimiter
/// (with its leading CRLF unless `first`), the `Content-Disposition` line
/// with browser escaping, the optional `Content-Type` line, and the blank
/// line. Returns the bytes written.
///
/// After the last part's data the caller writes the close delimiter:
/// [`Boundary::delimiter`] followed by `--\r\n`; for a body with no parts,
/// `--`, the boundary, and `--\r\n`.
///
/// # Errors
///
/// `BufferTooSmall { needed }` (nothing is written);
/// `Syntax { EmptyName | ControlInHeader | AmbiguousQuote | InvalidContentType }`.
///
/// # Examples
///
/// ```
/// let b = form_net::Boundary::new(b"X")?;
/// let mut out = [0u8; 128];
/// let n = form_net::encode_part_head(&b, true, "a\"b", None, None, &mut out)?;
/// assert_eq!(&out[..n], b"--X\r\nContent-Disposition: form-data; name=\"a%22b\"\r\n\r\n");
/// # Ok::<(), form_net::FormError>(())
/// ```
pub fn encode_part_head(
    boundary: &Boundary,
    first: bool,
    name: &str,
    filename: Option<&str>,
    content_type: Option<&str>,
    out: &mut [u8],
) -> Result<usize, FormError> {
    let needed = head_len(boundary, first, name, filename, content_type)?;
    if out.len() < needed {
        return Err(FormError::BufferTooSmall { needed });
    }
    let mut sink = SliceOut { out, pos: 0 };
    put_head(&mut sink, boundary, first, name, filename, content_type)?;
    Ok(sink.pos)
}

#[cfg(feature = "std")]
pub use writer_std::MultipartWriter;

#[cfg(feature = "std")]
mod writer_std {
    use alloc::string::String;
    use core::fmt;
    use std::io::{Read, Write};

    use super::{Out, head_len, put_head};
    use crate::error::{EncodingError, FormError, Location, SyntaxError};
    use crate::multipart::boundary::Boundary;

    struct IoOut<'w, W: Write>(&'w mut W);

    impl<W: Write> Out for IoOut<'_, W> {
        fn put(&mut self, bytes: &[u8]) -> Result<(), FormError> {
            self.0
                .write_all(bytes)
                .map_err(|e| FormError::Io { kind: e.kind() })
        }
    }

    /// Writes a `multipart/form-data` body as browsers do.
    ///
    /// Names and file names are escaped by the browser rule; every input
    /// that could read back differently is refused; text line breaks become
    /// CRLF. With a caller boundary, data is scanned for the delimiter
    /// across chunk splits and refused if it contains one. So every body the
    /// writer produces parses back with form-net to the same fields.
    ///
    /// # Examples
    ///
    /// ```
    /// let mut body = Vec::new();
    /// // `MultipartWriter::new` (feature `random`) generates the boundary.
    /// let boundary = form_net::Boundary::new(b"----form-net-example")?;
    /// let mut w = form_net::MultipartWriter::with_boundary(&mut body, boundary);
    /// w.text("title", "Holiday")?;
    /// w.file("photo", "beach.jpg", Some("image/jpeg"), &b"\xff\xd8 jpeg"[..])?;
    /// let content_type = w.content_type();
    /// w.finish()?;
    ///
    /// let mp = form_net::parse_multipart(content_type.as_bytes(), &body)?;
    /// assert_eq!(mp.field("title"), Some("Holiday"));
    /// assert_eq!(mp.file("photo").map(|p| p.bytes()), Some(&b"\xff\xd8 jpeg"[..]));
    /// # Ok::<(), form_net::FormError>(())
    /// ```
    pub struct MultipartWriter<W: Write> {
        out: W,
        boundary: Boundary,
        verify: bool,
        first: bool,
        in_file: bool,
        poisoned: bool,
        /// The delimiter matcher's state over the current part's data.
        k: usize,
        /// Text normalisation: the last byte written was a CR.
        pending_cr: bool,
    }

    impl<W: Write> fmt::Debug for MultipartWriter<W> {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.debug_struct("MultipartWriter")
                .field("boundary", &self.boundary)
                .field("in_file", &self.in_file)
                .finish_non_exhaustive()
        }
    }

    impl<W: Write> MultipartWriter<W> {
        /// A writer with a generated boundary.
        ///
        /// # Errors
        ///
        /// `Random` when the OS random source fails.
        #[cfg(feature = "random")]
        #[cfg_attr(docsrs, doc(cfg(feature = "random")))]
        pub fn new(out: W) -> Result<Self, FormError> {
            let mut w = Self::with_boundary(out, Boundary::generate()?);
            // 128 random bits: a collision with the data is negligible.
            w.verify = false;
            Ok(w)
        }

        /// A writer with the caller's boundary; data is scanned for it.
        pub fn with_boundary(out: W, boundary: Boundary) -> Self {
            MultipartWriter {
                out,
                boundary,
                verify: true,
                first: true,
                in_file: false,
                poisoned: false,
                k: 2,
                pending_cr: false,
            }
        }

        /// The boundary.
        pub const fn boundary(&self) -> &Boundary {
            &self.boundary
        }

        /// The `Content-Type` header value to send:
        /// `multipart/form-data; boundary=...`, quoted when the boundary
        /// holds a character that is not a token character.
        #[must_use]
        pub fn content_type(&self) -> String {
            let b = self.boundary.as_bytes();
            let token = b.iter().all(|&c| parse_net::class::is_tchar(c));
            let b = core::str::from_utf8(b).unwrap_or("");
            if token {
                alloc::format!("multipart/form-data; boundary={b}")
            } else {
                alloc::format!("multipart/form-data; boundary=\"{b}\"")
            }
        }

        /// Turns the delimiter scan on or off. It is on for a caller
        /// boundary and off for a generated one.
        pub fn verify_boundary(&mut self, on: bool) {
            self.verify = on;
        }

        /// Writes a text field. Line breaks in `value` become CRLF. A refused
        /// head writes nothing; after any error the output is incomplete and
        /// must be discarded.
        ///
        /// # Errors
        ///
        /// The head errors of [`encode_part_head`](crate::encode_part_head);
        /// `Syntax { BoundaryInContent }`; `Encoding { UnsupportedCharset }`
        /// for a `_charset_` field naming anything but UTF-8 or US-ASCII
        /// (the text after it would not read back); `Io`; `Poisoned`.
        pub fn text(&mut self, name: &str, value: &str) -> Result<(), FormError> {
            self.guard(|w| {
                if name == "_charset_"
                    && !(value.eq_ignore_ascii_case("utf-8")
                        || value.eq_ignore_ascii_case("us-ascii"))
                {
                    return Err(FormError::encoding(
                        EncodingError::UnsupportedCharset,
                        Location::NONE,
                    ));
                }
                w.head(name, None, None)?;
                w.in_file = false;
                w.pending_cr = false;
                let bytes = value.as_bytes();
                let mut from = 0usize;
                for (i, &b) in bytes.iter().enumerate() {
                    match b {
                        b'\r' => {
                            w.data(bytes.get(from..i).unwrap_or(&[]))?;
                            w.data(b"\r\n")?;
                            w.pending_cr = true;
                            from = i.saturating_add(1);
                        }
                        b'\n' => {
                            w.data(bytes.get(from..i).unwrap_or(&[]))?;
                            if !w.pending_cr {
                                w.data(b"\r\n")?;
                            }
                            w.pending_cr = false;
                            from = i.saturating_add(1);
                        }
                        _ => w.pending_cr = false,
                    }
                }
                w.data(bytes.get(from..).unwrap_or(&[]))
            })
        }

        /// Starts a file part; its data follows through
        /// [`file_data`](Self::file_data).
        ///
        /// # Errors
        ///
        /// As [`text`](Self::text), without the charset rule.
        pub fn file_start(
            &mut self,
            name: &str,
            filename: &str,
            content_type: Option<&str>,
        ) -> Result<(), FormError> {
            self.guard(|w| {
                w.head(name, Some(filename), content_type)?;
                w.in_file = true;
                Ok(())
            })
        }

        /// More data of the current file part.
        ///
        /// # Errors
        ///
        /// `Syntax { BoundaryInContent }`; `Io`; `Poisoned` outside a file
        /// part.
        pub fn file_data(&mut self, chunk: &[u8]) -> Result<(), FormError> {
            if !self.in_file {
                self.poisoned = true;
                return Err(FormError::Poisoned);
            }
            self.guard(|w| w.data(chunk))
        }

        /// A whole file part read from `data`. Returns its length.
        ///
        /// # Errors
        ///
        /// As [`file_start`](Self::file_start) and
        /// [`file_data`](Self::file_data); `Io` from `data`.
        pub fn file<R: Read>(
            &mut self,
            name: &str,
            filename: &str,
            content_type: Option<&str>,
            mut data: R,
        ) -> Result<u64, FormError> {
            self.file_start(name, filename, content_type)?;
            let mut buf = [0u8; 8192];
            let mut total = 0u64;
            let r = loop {
                let n = match data.read(&mut buf) {
                    Ok(0) => break Ok(total),
                    Ok(n) => n,
                    Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                    Err(e) => {
                        self.poisoned = true;
                        break Err(FormError::Io { kind: e.kind() });
                    }
                };
                if let Err(e) = self.file_data(buf.get(..n).unwrap_or(&[])) {
                    break Err(e);
                }
                total = total.saturating_add(n as u64);
            };
            crate::wipe::wipe(&mut buf);
            r
        }

        /// Writes the close delimiter and returns the writer.
        ///
        /// # Errors
        ///
        /// `Io`; `Poisoned`.
        pub fn finish(mut self) -> Result<W, FormError> {
            self.guard(|w| {
                let mut out = IoOut(&mut w.out);
                if !w.first {
                    out.put(b"\r\n")?;
                }
                out.put(b"--")?;
                out.put(w.boundary.as_bytes())?;
                out.put(b"--\r\n")?;
                w.out.flush().map_err(|e| FormError::Io { kind: e.kind() })
            })?;
            Ok(self.out)
        }

        /// Runs `f`; any error poisons the writer.
        fn guard(
            &mut self,
            f: impl FnOnce(&mut Self) -> Result<(), FormError>,
        ) -> Result<(), FormError> {
            if self.poisoned {
                return Err(FormError::Poisoned);
            }
            let r = f(self);
            if r.is_err() {
                self.poisoned = true;
            }
            r
        }

        fn head(
            &mut self,
            name: &str,
            filename: Option<&str>,
            content_type: Option<&str>,
        ) -> Result<(), FormError> {
            // Everything is checked before the first byte goes out.
            head_len(&self.boundary, self.first, name, filename, content_type)?;
            put_head(
                &mut IoOut(&mut self.out),
                &self.boundary,
                self.first,
                name,
                filename,
                content_type,
            )?;
            self.first = false;
            // The data starts right after a CRLF: as if it had been seen.
            self.k = 2;
            Ok(())
        }

        /// Writes part data, scanning it for the delimiter when verifying.
        fn data(&mut self, bytes: &[u8]) -> Result<(), FormError> {
            if self.verify {
                let (k, found) = self.boundary.run(self.k, bytes);
                if found.is_some() {
                    return Err(FormError::syntax(
                        SyntaxError::BoundaryInContent,
                        Location::NONE,
                    ));
                }
                self.k = k;
            }
            IoOut(&mut self.out).put(bytes)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn head(
        first: bool,
        name: &str,
        filename: Option<&str>,
        ct: Option<&str>,
    ) -> Result<alloc::vec::Vec<u8>, FormError> {
        let b = Boundary::new(b"X").unwrap();
        let mut out = [0u8; 256];
        let n = encode_part_head(&b, first, name, filename, ct, &mut out)?;
        Ok(out[..n].to_vec())
    }

    #[test]
    fn test_head_shape_and_escapes() {
        assert_eq!(
            head(false, "a\r\nb", Some("c\".txt"), Some("text/plain")).unwrap(),
            b"\r\n--X\r\nContent-Disposition: form-data; name=\"a%0D%0Ab\"; filename=\"c%22.txt\"\r\nContent-Type: text/plain\r\n\r\n"
        );
        assert_eq!(
            head(true, "t\tab", None, None).unwrap(),
            b"--X\r\nContent-Disposition: form-data; name=\"t\tab\"\r\n\r\n"
        );
    }

    #[test]
    fn test_head_refusals() {
        let refused = |r: Result<alloc::vec::Vec<u8>, FormError>| match r {
            Err(FormError::Syntax { reason, .. }) => reason,
            other => panic!("{other:?}"),
        };
        assert_eq!(refused(head(true, "", None, None)), SyntaxError::EmptyName);
        assert_eq!(
            refused(head(true, "a\0", None, None)),
            SyntaxError::ControlInHeader
        );
        assert_eq!(
            refused(head(true, "a\\b", None, None)),
            SyntaxError::AmbiguousQuote
        );
        assert_eq!(
            refused(head(true, "a%22", None, None)),
            SyntaxError::AmbiguousQuote
        );
        assert_eq!(
            refused(head(true, "a", Some("x\\"), None)),
            SyntaxError::AmbiguousQuote
        );
        assert!(head(true, "a", Some("C:\\x\\y.txt"), None).is_ok());
        assert_eq!(
            refused(head(true, "a", None, Some("text/plain\r\nX: y"))),
            SyntaxError::ControlInHeader
        );
        assert_eq!(
            refused(head(true, "a", None, Some("not a type"))),
            SyntaxError::InvalidContentType
        );
        assert_eq!(
            refused(head(true, "a", None, Some("multipart/mixed; boundary=y"))),
            SyntaxError::InvalidContentType
        );
        assert_eq!(
            refused(head(true, "a", None, Some("text/plain; a=1; a=2"))),
            SyntaxError::InvalidContentType
        );
    }

    #[test]
    fn test_head_buffer_too_small_writes_nothing() {
        let b = Boundary::new(b"X").unwrap();
        let mut out = [7u8; 10];
        let e = encode_part_head(&b, true, "name", None, None, &mut out).unwrap_err();
        assert_eq!(e, FormError::BufferTooSmall { needed: 52 });
        assert!(out.iter().all(|&x| x == 7));
    }
}
