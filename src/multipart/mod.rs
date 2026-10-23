//! `multipart/form-data` (RFC 7578, framed by RFC 2046): the streaming
//! parser, and the buffered entry points built on it.

pub(crate) mod boundary;
pub(crate) mod filename;
pub(crate) mod header;
pub(crate) mod parser;

#[cfg(feature = "alloc")]
mod buffered {
    use alloc::borrow::Cow;
    use alloc::string::String;
    use alloc::vec::Vec;
    use core::fmt;

    use super::filename::{FileName, FileNameSource};
    use super::header::Text;
    use super::parser::{Charset, ContentType, MultipartParser, Outcome, PartKind, Source};
    use crate::error::{EncodingError, ExtractError, FormError, Location};
    use crate::limits::{Limits, Policy};

    /// One part of a buffered multipart body. Its data borrows the body.
    pub struct Part<'a> {
        index: u32,
        name: Cow<'a, str>,
        kind: PartKind,
        filename: Option<(Cow<'a, [u8]>, FileNameSource)>,
        content_type: Option<&'a [u8]>,
        body: &'a [u8],
        offset: u64,
    }

    impl fmt::Debug for Part<'_> {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.debug_struct("Part")
                .field("index", &self.index)
                .field("name", &self.name)
                .field("kind", &self.kind)
                .field("len", &self.body.len())
                .finish_non_exhaustive()
        }
    }

    impl<'a> Part<'a> {
        /// The part index, from 0, in body order.
        ///
        /// # Examples
        ///
        /// ```
        /// # let body = b"--X\r\nContent-Disposition: form-data; name=a\r\n\r\n1\r\n--X--";
        /// # let mp = form_net::parse_multipart(b"multipart/form-data; boundary=X", body)?;
        /// assert_eq!(mp.parts()[0].index(), 0);
        /// # Ok::<(), form_net::FormError>(())
        /// ```
        #[must_use]
        pub const fn index(&self) -> u32 {
            self.index
        }

        /// The field name, HTML escapes (`%0A`, `%0D`, `%22`) decoded.
        ///
        /// # Examples
        ///
        /// ```
        /// # let body = b"--X\r\nContent-Disposition: form-data; name=\"a%22b\"\r\n\r\n1\r\n--X--";
        /// # let mp = form_net::parse_multipart(b"multipart/form-data; boundary=X", body)?;
        /// assert_eq!(mp.parts()[0].name(), "a\"b");
        /// # Ok::<(), form_net::FormError>(())
        /// ```
        #[must_use]
        pub fn name(&self) -> &str {
            &self.name
        }

        /// Field or file.
        ///
        /// # Examples
        ///
        /// ```
        /// # let body = b"--X\r\nContent-Disposition: form-data; name=a; filename=b\r\n\r\n1\r\n--X--";
        /// # let mp = form_net::parse_multipart(b"multipart/form-data; boundary=X", body)?;
        /// assert_eq!(mp.parts()[0].kind(), form_net::PartKind::File);
        /// # Ok::<(), form_net::FormError>(())
        /// ```
        #[must_use]
        pub const fn kind(&self) -> PartKind {
            self.kind
        }

        /// Whether this is a file part.
        ///
        /// # Examples
        ///
        /// ```
        /// # let body = b"--X\r\nContent-Disposition: form-data; name=a\r\n\r\n1\r\n--X--";
        /// # let mp = form_net::parse_multipart(b"multipart/form-data; boundary=X", body)?;
        /// assert!(!mp.parts()[0].is_file());
        /// # Ok::<(), form_net::FormError>(())
        /// ```
        #[must_use]
        pub const fn is_file(&self) -> bool {
            matches!(self.kind, PartKind::File)
        }

        /// The client's file name, for file parts.
        ///
        /// # Examples
        ///
        /// ```
        /// # let body = b"--X\r\nContent-Disposition: form-data; name=a; filename=\"me.png\"\r\n\r\n1\r\n--X--";
        /// # let mp = form_net::parse_multipart(b"multipart/form-data; boundary=X", body)?;
        /// assert_eq!(mp.parts()[0].filename().unwrap().raw(), b"me.png");
        /// # Ok::<(), form_net::FormError>(())
        /// ```
        #[must_use]
        pub fn filename(&self) -> Option<FileName<'_>> {
            self.filename
                .as_ref()
                .map(|(raw, source)| FileName::new(raw, *source))
        }

        /// The declared `Content-Type`: a hint, never trusted.
        ///
        /// # Examples
        ///
        /// ```
        /// # let body = b"--X\r\nContent-Disposition: form-data; name=a; filename=b\r\nContent-Type: image/png\r\n\r\n1\r\n--X--";
        /// # let mp = form_net::parse_multipart(b"multipart/form-data; boundary=X", body)?;
        /// assert!(mp.parts()[0].content_type().unwrap().is("image", "png"));
        /// # Ok::<(), form_net::FormError>(())
        /// ```
        #[must_use]
        pub fn content_type(&self) -> Option<ContentType<'a>> {
            self.content_type.and_then(ContentType::parse)
        }

        /// The part's data, borrowed from the body.
        ///
        /// # Examples
        ///
        /// ```
        /// # let body = b"--X\r\nContent-Disposition: form-data; name=a\r\n\r\nhello\r\n--X--";
        /// # let mp = form_net::parse_multipart(b"multipart/form-data; boundary=X", body)?;
        /// assert_eq!(mp.parts()[0].bytes(), b"hello");
        /// # Ok::<(), form_net::FormError>(())
        /// ```
        #[must_use]
        pub const fn bytes(&self) -> &'a [u8] {
            self.body
        }

        /// The data as text. Text fields were checked at parse time, so this
        /// cannot fail for them; for file parts it checks UTF-8 now.
        ///
        /// # Errors
        ///
        /// `Encoding { InvalidUtf8 }`.
        ///
        /// # Examples
        ///
        /// ```
        /// # let body = b"--X\r\nContent-Disposition: form-data; name=a\r\n\r\nhello\r\n--X--";
        /// # let mp = form_net::parse_multipart(b"multipart/form-data; boundary=X", body)?;
        /// assert_eq!(mp.parts()[0].text()?, "hello");
        /// # Ok::<(), form_net::FormError>(())
        /// ```
        pub fn text(&self) -> Result<&'a str, FormError> {
            core::str::from_utf8(self.body).map_err(|_| {
                FormError::encoding(
                    EncodingError::InvalidUtf8,
                    Location::in_part(Some(self.index), self.offset),
                )
            })
        }

        /// The data length.
        ///
        /// # Examples
        ///
        /// ```
        /// # let body = b"--X\r\nContent-Disposition: form-data; name=a\r\n\r\nhello\r\n--X--";
        /// # let mp = form_net::parse_multipart(b"multipart/form-data; boundary=X", body)?;
        /// assert_eq!(mp.parts()[0].len(), 5);
        /// # Ok::<(), form_net::FormError>(())
        /// ```
        #[must_use]
        pub const fn len(&self) -> usize {
            self.body.len()
        }

        /// Whether the data is empty.
        ///
        /// # Examples
        ///
        /// ```
        /// # let body = b"--X\r\nContent-Disposition: form-data; name=a\r\n\r\n\r\n--X--";
        /// # let mp = form_net::parse_multipart(b"multipart/form-data; boundary=X", body)?;
        /// assert!(mp.parts()[0].is_empty());
        /// # Ok::<(), form_net::FormError>(())
        /// ```
        #[must_use]
        pub const fn is_empty(&self) -> bool {
            self.body.is_empty()
        }

        fn field_text(&self) -> &'a str {
            // Text fields were checked as UTF-8 at parse time.
            core::str::from_utf8(self.body).unwrap_or("")
        }
    }

    /// The parts of a buffered multipart body, in body order.
    ///
    /// `Debug` prints names, kinds, and lengths, never data.
    #[must_use]
    pub struct Multipart<'a> {
        parts: Vec<Part<'a>>,
    }

    impl fmt::Debug for Multipart<'_> {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.debug_list().entries(&self.parts).finish()
        }
    }

    impl<'a> Multipart<'a> {
        /// Every part, in body order.
        ///
        /// # Examples
        ///
        /// ```
        /// # let body = b"--X\r\nContent-Disposition: form-data; name=a\r\n\r\n1\r\n--X--";
        /// let mp = form_net::parse_multipart(b"multipart/form-data; boundary=X", body)?;
        /// assert_eq!(mp.parts().len(), 1);
        /// # Ok::<(), form_net::FormError>(())
        /// ```
        #[must_use]
        pub fn parts(&self) -> &[Part<'a>] {
            &self.parts
        }

        /// The first text field named `name`.
        ///
        /// # Examples
        ///
        /// ```
        /// # let body = b"--X\r\nContent-Disposition: form-data; name=title\r\n\r\nHoliday\r\n--X--";
        /// let mp = form_net::parse_multipart(b"multipart/form-data; boundary=X", body)?;
        /// assert_eq!(mp.field("title"), Some("Holiday"));
        /// assert_eq!(mp.field("other"), None);
        /// # Ok::<(), form_net::FormError>(())
        /// ```
        #[must_use]
        pub fn field(&self, name: &str) -> Option<&'a str> {
            self.parts
                .iter()
                .find(|p| !p.is_file() && p.name() == name)
                .map(Part::field_text)
        }

        /// The text field named `name`, refusing a name given twice.
        ///
        /// # Errors
        ///
        /// `Extract { reason: Duplicate, field: None }`.
        ///
        /// # Examples
        ///
        /// ```
        /// # let body = b"--X\r\nContent-Disposition: form-data; name=a\r\n\r\n1\r\n--X\r\nContent-Disposition: form-data; name=a\r\n\r\n2\r\n--X--";
        /// let mp = form_net::parse_multipart(b"multipart/form-data; boundary=X", body)?;
        /// assert!(mp.field_one("a").is_err());
        /// assert_eq!(mp.field_one("b")?, None);
        /// # Ok::<(), form_net::FormError>(())
        /// ```
        pub fn field_one(&self, name: &str) -> Result<Option<&'a str>, FormError> {
            let mut found = None;
            for p in self
                .parts
                .iter()
                .filter(|p| !p.is_file() && p.name() == name)
            {
                if found.is_some() {
                    return Err(FormError::Extract {
                        reason: ExtractError::Duplicate,
                        field: None,
                    });
                }
                found = Some(p.field_text());
            }
            Ok(found)
        }

        /// Every text field named `name`, in body order.
        ///
        /// # Examples
        ///
        /// ```
        /// # let body = b"--X\r\nContent-Disposition: form-data; name=t\r\n\r\na\r\n--X\r\nContent-Disposition: form-data; name=t\r\n\r\nb\r\n--X--";
        /// let mp = form_net::parse_multipart(b"multipart/form-data; boundary=X", body)?;
        /// assert_eq!(mp.fields("t").collect::<Vec<_>>(), ["a", "b"]);
        /// # Ok::<(), form_net::FormError>(())
        /// ```
        pub fn fields<'m>(&'m self, name: &'m str) -> impl Iterator<Item = &'a str> + 'm {
            self.parts
                .iter()
                .filter(move |p| !p.is_file() && p.name() == name)
                .map(Part::field_text)
        }

        /// The first file part named `name`.
        ///
        /// # Examples
        ///
        /// ```
        /// # let body = b"--X\r\nContent-Disposition: form-data; name=f; filename=a.txt\r\n\r\nhi\r\n--X--";
        /// let mp = form_net::parse_multipart(b"multipart/form-data; boundary=X", body)?;
        /// assert_eq!(mp.file("f").map(|p| p.bytes()), Some(&b"hi"[..]));
        /// # Ok::<(), form_net::FormError>(())
        /// ```
        #[must_use]
        pub fn file(&self, name: &str) -> Option<&Part<'a>> {
            self.parts.iter().find(|p| p.is_file() && p.name() == name)
        }

        /// Every file part named `name`, in body order.
        ///
        /// # Examples
        ///
        /// ```
        /// # let body = b"--X\r\nContent-Disposition: form-data; name=f; filename=a\r\n\r\n1\r\n--X\r\nContent-Disposition: form-data; name=f; filename=b\r\n\r\n2\r\n--X--";
        /// let mp = form_net::parse_multipart(b"multipart/form-data; boundary=X", body)?;
        /// assert_eq!(mp.files("f").count(), 2);
        /// # Ok::<(), form_net::FormError>(())
        /// ```
        pub fn files<'m>(&'m self, name: &'m str) -> impl Iterator<Item = &'m Part<'a>> + 'm {
            self.parts
                .iter()
                .filter(move |p| p.is_file() && p.name() == name)
        }

        /// The number of parts.
        ///
        /// # Examples
        ///
        /// ```
        /// let mp = form_net::parse_multipart(b"multipart/form-data; boundary=X", b"--X--")?;
        /// assert_eq!(mp.len(), 0);
        /// # Ok::<(), form_net::FormError>(())
        /// ```
        #[must_use]
        pub fn len(&self) -> usize {
            self.parts.len()
        }

        /// Whether there are no parts.
        ///
        /// # Examples
        ///
        /// ```
        /// let mp = form_net::parse_multipart(b"multipart/form-data; boundary=X", b"--X--\r\n")?;
        /// assert!(mp.is_empty());
        /// # Ok::<(), form_net::FormError>(())
        /// ```
        #[must_use]
        pub fn is_empty(&self) -> bool {
            self.parts.is_empty()
        }
    }

    /// Whether a text field's charset (its own, else the `_charset_` field
    /// before it, else UTF-8) is one form-net reads as text.
    fn text_charset_ok(own: Charset, declared: Option<&[u8]>) -> bool {
        match own {
            Charset::Utf8 | Charset::Ascii => true,
            Charset::Other => false,
            Charset::Unspecified => declared.is_none_or(|label| {
                label.eq_ignore_ascii_case(b"utf-8") || label.eq_ignore_ascii_case(b"us-ascii")
            }),
        }
    }

    /// Parses a buffered `multipart/form-data` body with the default limits
    /// and policy. Part data borrows the body (zero copy); a name borrows too,
    /// except where an HTML escape had to be decoded.
    ///
    /// Text fields are checked as text at parse time: a field that is not
    /// valid UTF-8, or whose charset (its own, else the `_charset_` field
    /// before it, else UTF-8) is not UTF-8 or US-ASCII, refuses the body.
    ///
    /// # Errors
    ///
    /// Every multipart error; `UnsupportedMediaType` when the content type is
    /// not `multipart/form-data`.
    ///
    /// # Examples
    ///
    /// ```
    /// let content_type = b"multipart/form-data; boundary=X";
    /// let body = b"--X\r\n\
    /// Content-Disposition: form-data; name=\"title\"\r\n\r\n\
    /// Holiday\r\n\
    /// --X\r\n\
    /// Content-Disposition: form-data; name=\"avatar\"; filename=\"me.png\"\r\n\
    /// Content-Type: image/png\r\n\r\n\
    /// \x89PNG...\r\n\
    /// --X--\r\n";
    ///
    /// let mp = form_net::parse_multipart(content_type, body)?;
    /// assert_eq!(mp.field("title"), Some("Holiday"));
    /// let avatar = mp.file("avatar").unwrap();
    /// assert_eq!(avatar.filename().unwrap().raw(), b"me.png");
    /// assert!(avatar.content_type().unwrap().is("image", "png"));
    /// assert_eq!(avatar.bytes(), b"\x89PNG...");
    /// # Ok::<(), form_net::FormError>(())
    /// ```
    pub fn parse_multipart<'a>(
        content_type: &[u8],
        body: &'a [u8],
    ) -> Result<Multipart<'a>, FormError> {
        parse_multipart_with(content_type, body, &Limits::DEFAULT, &Policy::DEFAULT)
    }

    /// As [`parse_multipart`] with explicit limits and policy.
    /// `limits.validate()` runs first. The token rule of `Policy::token_field`
    /// applies; the captured token is the value of the text field with that
    /// name.
    ///
    /// # Errors
    ///
    /// As [`parse_multipart`]; `InvalidLimits`.
    ///
    /// # Examples
    ///
    /// ```
    /// use form_net::{Limits, Policy};
    ///
    /// const AVATAR: Limits = Limits::DEFAULT
    ///     .with_max_body_bytes(512 * 1024)
    ///     .with_max_parts(4)
    ///     .with_max_files(1);
    /// let body = b"--X\r\nContent-Disposition: form-data; name=f; filename=a\r\n\r\n1\r\n--X--";
    /// let mp = form_net::parse_multipart_with(b"multipart/form-data; boundary=X", body, &AVATAR, &Policy::DEFAULT)?;
    /// assert_eq!(mp.len(), 1);
    /// # Ok::<(), form_net::FormError>(())
    /// ```
    pub fn parse_multipart_with<'a>(
        content_type: &[u8],
        body: &'a [u8],
        limits: &Limits,
        policy: &Policy,
    ) -> Result<Multipart<'a>, FormError> {
        let boundary = crate::Boundary::parse(content_type)?;
        let mut parser = MultipartParser::new(boundary, limits, policy)?;
        let mut parts = Vec::new();
        let mut pos = 0usize;
        let mut current: Option<(Part<'a>, bool)> = None;
        loop {
            let rest = body.get(pos..).unwrap_or(&[]);
            let (consumed, outcome) = parser.advance(rest, true)?;
            let base = pos;
            pos = pos.saturating_add(consumed);
            match outcome {
                Outcome::PartStart(facts, source) => {
                    let block_start = match source {
                        Source::Input(start) => base.saturating_add(start),
                        // The whole body is one chunk, so a head is always in it.
                        Source::Buffer => base,
                    };
                    let block = body
                        .get(block_start..block_start.saturating_add(facts.block_len as usize))
                        .unwrap_or(&[]);
                    let name: Cow<'a, str> = match facts.name {
                        Text::Block(span) => {
                            Cow::Borrowed(core::str::from_utf8(span.get(block)).unwrap_or(""))
                        }
                        Text::Area(len) => Cow::Owned(String::from(parser.area_name(len))),
                    };
                    let filename = facts.filename.map(|(text, source)| {
                        let raw: Cow<'a, [u8]> = match text {
                            Text::Block(span) => Cow::Borrowed(span.get(block)),
                            Text::Area(len) => Cow::Owned(parser.area_filename(len).to_vec()),
                        };
                        (raw, source)
                    });
                    let charset_ok = text_charset_ok(facts.charset, parser.declared_charset());
                    let part = Part {
                        index: parser.parts_seen().saturating_sub(1),
                        name,
                        kind: facts.kind,
                        filename,
                        content_type: facts.content_type.map(|span| span.get(block)),
                        body: &[],
                        offset: parser.data_start(),
                    };
                    current = Some((part, charset_ok));
                }
                Outcome::PartEnd(summary) => {
                    if let Some((mut part, charset_ok)) = current.take() {
                        let start = usize::try_from(part.offset).unwrap_or(usize::MAX);
                        let end = start.saturating_add(usize::try_from(summary.bytes).unwrap_or(0));
                        part.body = body.get(start..end).unwrap_or(&[]);
                        let skipped_empty = part.is_file()
                            && part.body.is_empty()
                            && part
                                .filename
                                .as_ref()
                                .is_some_and(|(raw, _)| raw.is_empty())
                            && parser.policy().empty_files == crate::EmptyFilePolicy::Skip;
                        if skipped_empty {
                            // A file input left empty produces nothing.
                            continue;
                        }
                        if !part.is_file() {
                            let at = Location::in_part(Some(part.index), part.offset);
                            if !charset_ok {
                                return Err(FormError::encoding(
                                    EncodingError::UnsupportedCharset,
                                    at,
                                ));
                            }
                            if core::str::from_utf8(part.body).is_err() {
                                return Err(FormError::encoding(EncodingError::InvalidUtf8, at));
                            }
                        }
                        parts.push(part);
                    }
                }
                Outcome::End => {
                    // Whatever follows the close delimiter is epilogue: it
                    // still counts against its budget.
                    if pos < body.len() {
                        let rest = body.get(pos..).unwrap_or(&[]);
                        let _ = parser.advance(rest, true)?;
                    }
                    break;
                }
                Outcome::DataInput(..) | Outcome::DataDelimiter(_) | Outcome::NeedInput => {}
            }
        }
        Ok(Multipart { parts })
    }
}

#[cfg(feature = "alloc")]
pub use buffered::{Multipart, Part, parse_multipart, parse_multipart_with};
