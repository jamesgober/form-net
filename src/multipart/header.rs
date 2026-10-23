//! Part headers: finding the end of a header block, and reading a complete
//! block into the facts the parser keeps about a part.

use parse_net::{ExtValue, Params, Word};

use crate::error::{EncodingError, FormError, Limit, Location, PolicyRefusal, SyntaxError};
use crate::kind::{HEADER_LIMITS, repeated_name, word_raw};
use crate::limits::{ExtendedFilenamePolicy, HeaderPolicy, Limits, Policy};
use crate::multipart::filename::{FileNameSource, has_html_escape, html_unescape};
use crate::multipart::parser::{Charset, PartKind};

/// The state of a header block being read, kept across chunks.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct BlockScan {
    /// Bytes of the block seen so far.
    pub(crate) len: u32,
    /// Where the current line starts, in the block.
    line_start: u32,
    /// Complete non-empty lines so far.
    lines: u16,
    /// The previous byte was CR.
    prev_cr: bool,
}

impl BlockScan {
    /// Scans `hay` as the next bytes of the block. Returns the number of
    /// bytes of `hay` that complete the block (through its final CRLF), or
    /// `None` when all of `hay` belongs to it and more is needed.
    pub(crate) fn feed(
        &mut self,
        hay: &[u8],
        limits: &Limits,
        at: Location,
    ) -> Result<Option<usize>, FormError> {
        let max = limits.max_part_header_bytes;
        for (j, &c) in hay.iter().enumerate() {
            let pos = self.len;
            if pos >= max {
                return Err(FormError::limit(Limit::PartHeaderBytes, at));
            }
            self.len = pos.wrapping_add(1);
            if c == b'\n' {
                if !self.prev_cr {
                    return Err(FormError::syntax(SyntaxError::BareLineFeed, at));
                }
                self.prev_cr = false;
                // The line is [line_start, pos - 1): empty when CR is at line_start.
                if pos.wrapping_sub(1) == self.line_start {
                    return Ok(Some(j.wrapping_add(1)));
                }
                self.lines = self.lines.saturating_add(1);
                if self.lines > limits.max_part_headers {
                    return Err(FormError::limit(Limit::PartHeaderCount, at));
                }
                self.line_start = self.len;
                continue;
            }
            if self.prev_cr {
                return Err(FormError::syntax(SyntaxError::ControlInHeader, at));
            }
            self.prev_cr = c == b'\r';
        }
        Ok(None)
    }
}

/// A range inside the header block.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Span {
    pub(crate) start: u32,
    pub(crate) len: u32,
}

impl Span {
    fn of(block: &[u8], part: &[u8]) -> Span {
        let start = (part.as_ptr() as usize).wrapping_sub(block.as_ptr() as usize);
        Span {
            start: start as u32,
            len: part.len() as u32,
        }
    }

    pub(crate) fn get<'a>(&self, bytes: &'a [u8]) -> &'a [u8] {
        let start = self.start as usize;
        bytes
            .get(start..start.saturating_add(self.len as usize))
            .unwrap_or(&[])
    }
}

/// Where a decoded value lives.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Text {
    /// Unchanged, inside the block.
    Block(Span),
    /// Decoded into the parser buffer's area for it; the length.
    Area(u32),
}

/// What the parser keeps about a part's head, as positions only.
#[derive(Clone, Copy, Debug)]
pub(crate) struct HeadFacts {
    pub(crate) block_len: u32,
    pub(crate) name_raw: Span,
    pub(crate) name: Text,
    pub(crate) kind: PartKind,
    pub(crate) filename: Option<(Text, FileNameSource)>,
    pub(crate) content_type: Option<Span>,
    pub(crate) charset: Charset,
}

/// The areas of the parser buffer that follow the header block.
pub(crate) struct Areas<'b> {
    pub(crate) name: &'b mut [u8],
    pub(crate) filename: &'b mut [u8],
}

/// Whether `a` and `b` are equal ignoring ASCII case.
fn is(a: &[u8], b: &[u8]) -> bool {
    a.eq_ignore_ascii_case(b)
}

/// Reads parameters after a value's leading token, with form-net's rules:
/// no repeated name, no `\"` in a quoted value, nothing after them.
fn params(rest: &[u8], at: Location, bad: SyntaxError) -> Result<Params<'_>, FormError> {
    if has_escaped_quote(rest) {
        return Err(FormError::syntax(SyntaxError::AmbiguousQuote, at));
    }
    let (p, used) =
        parse_net::param::parse(rest, &HEADER_LIMITS).map_err(|_| FormError::syntax(bad, at))?;
    if parse_net::text::skip_ows(rest.get(used..).unwrap_or(&[])) != rest.len().saturating_sub(used)
    {
        return Err(FormError::syntax(bad, at));
    }
    Ok(p)
}

/// Whether `s` contains `\"`: in a header value that can only be a quoted
/// pair, the one sequence browsers and RFC 9110 read differently (D-015).
fn has_escaped_quote(s: &[u8]) -> bool {
    s.windows(2).any(|w| w == b"\\\"")
}

/// Reads a complete header block (ending with its empty line).
pub(crate) fn read_block(
    block: &[u8],
    limits: &Limits,
    policy: &Policy,
    areas: Areas<'_>,
    at: Location,
) -> Result<HeadFacts, FormError> {
    let mut disposition: Option<&[u8]> = None;
    let mut content_type: Option<&[u8]> = None;
    let mut transfer_encoding: Option<&[u8]> = None;
    let mut rest = block;
    while let Some(lf) = rest.iter().position(|&b| b == b'\n') {
        // Lines end with CRLF; BlockScan already refused anything else.
        let line = rest.get(..lf.saturating_sub(1)).unwrap_or(&[]);
        rest = rest.get(lf.wrapping_add(1)..).unwrap_or(&[]);
        if line.is_empty() {
            break;
        }
        if matches!(line.first(), Some(b' ' | b'\t')) {
            return Err(FormError::syntax(SyntaxError::HeaderFolding, at));
        }
        let Some(colon) = line.iter().position(|&b| b == b':') else {
            return Err(FormError::syntax(SyntaxError::MalformedHeaderLine, at));
        };
        let name = line.get(..colon).unwrap_or(&[]);
        if !parse_net::text::is_token(name) {
            return Err(FormError::syntax(SyntaxError::InvalidHeaderName, at));
        }
        let value = parse_net::text::trim_ows(line.get(colon.wrapping_add(1)..).unwrap_or(&[]));
        if value.iter().any(|&b| (b < 0x20 && b != b'\t') || b == 0x7F) {
            return Err(FormError::syntax(SyntaxError::ControlInHeader, at));
        }
        let slot = if is(name, b"content-disposition") {
            &mut disposition
        } else if is(name, b"content-type") {
            &mut content_type
        } else if is(name, b"content-transfer-encoding") {
            &mut transfer_encoding
        } else {
            if policy.unknown_headers == HeaderPolicy::Refuse {
                return Err(FormError::policy(PolicyRefusal::UnknownHeader, at));
            }
            continue;
        };
        if slot.is_some() {
            return Err(FormError::syntax(SyntaxError::DuplicateHeader, at));
        }
        *slot = Some(value);
    }

    if let Some(te) = transfer_encoding {
        if !(is(te, b"7bit") || is(te, b"8bit") || is(te, b"binary")) {
            return Err(FormError::policy(PolicyRefusal::TransferEncoding, at));
        }
    }

    let (ct_span, charset) = match content_type {
        None => (None, Charset::Unspecified),
        Some(ct) => {
            if has_escaped_quote(ct) {
                return Err(FormError::syntax(SyntaxError::AmbiguousQuote, at));
            }
            let mt = parse_net::media::parse(ct, &HEADER_LIMITS)
                .map_err(|_| FormError::syntax(SyntaxError::InvalidContentType, at))?;
            let p = mt.params();
            if repeated_name(&p).is_some() {
                return Err(FormError::syntax(SyntaxError::InvalidContentType, at));
            }
            if mt.type_().eq_ignore_ascii_case(b"multipart") {
                return Err(FormError::policy(PolicyRefusal::NestedMultipart, at));
            }
            let charset = match p.get("charset") {
                None => Charset::Unspecified,
                Some(w) => Charset::from_label(word_raw(&w)),
            };
            (Some(Span::of(block, ct)), charset)
        }
    };

    let Some(cd) = disposition else {
        return Err(FormError::syntax(
            SyntaxError::MissingContentDisposition,
            at,
        ));
    };
    let type_len = cd
        .iter()
        .take_while(|&&b| parse_net::class::is_tchar(b))
        .count();
    if type_len == 0 {
        return Err(FormError::syntax(SyntaxError::MalformedParameter, at));
    }
    if !is(cd.get(..type_len).unwrap_or(&[]), b"form-data") {
        return Err(FormError::syntax(SyntaxError::NotFormData, at));
    }
    let p = params(
        cd.get(type_len..).unwrap_or(&[]),
        at,
        SyntaxError::MalformedParameter,
    )?;
    if repeated_name(&p).is_some() {
        return Err(FormError::syntax(SyntaxError::DuplicateParameter, at));
    }
    let mut name = None;
    let mut filename = None;
    let mut filename_ext = None;
    for q in p.iter() {
        if is(q.name, b"name") {
            name = Some(q.value);
        } else if is(q.name, b"filename") {
            filename = Some(q.value);
        } else if is(q.name, b"filename*") {
            filename_ext = Some(q.value);
        } else if q.name.contains(&b'*') {
            // RFC 2231 continuations and other extended parameters
            // (`name*`, `filename*0`): readers that join or prefer them see a
            // different field or a file where form-net would not (D-081).
            return Err(FormError::syntax(SyntaxError::MalformedParameter, at));
        }
    }

    let Some(name) = name else {
        return Err(FormError::syntax(SyntaxError::MissingName, at));
    };
    let name_raw = word_raw(&name);
    if name_raw.contains(&b'\\') {
        // A backslash in a name reads as a quoted pair to RFC 9110 readers
        // and as itself to browsers; no form needs one (D-082).
        return Err(FormError::syntax(SyntaxError::MalformedParameter, at));
    }
    if name_raw.len() as u64 > u64::from(limits.name()) {
        return Err(FormError::limit(Limit::NameLength, at));
    }
    let name_text = if has_html_escape(name_raw) {
        if areas.name.len() < name_raw.len() {
            return Err(FormError::limit(Limit::NameLength, at));
        }
        let n = html_unescape(name_raw, areas.name);
        if core::str::from_utf8(areas.name.get(..n).unwrap_or(&[])).is_err() {
            return Err(FormError::encoding(EncodingError::InvalidUtf8, at));
        }
        if n == 0 {
            return Err(FormError::syntax(SyntaxError::EmptyName, at));
        }
        Text::Area(n as u32)
    } else {
        if core::str::from_utf8(name_raw).is_err() {
            return Err(FormError::encoding(EncodingError::InvalidUtf8, at));
        }
        if name_raw.is_empty() {
            return Err(FormError::syntax(SyntaxError::EmptyName, at));
        }
        Text::Block(Span::of(block, name_raw))
    };

    let file = match (filename, filename_ext) {
        (Some(_), Some(_)) => return Err(FormError::policy(PolicyRefusal::BothFilenames, at)),
        (Some(plain), None) => {
            let raw = word_raw(&plain);
            if raw.len() as u64 > u64::from(limits.filename()) {
                return Err(FormError::limit(Limit::FilenameLength, at));
            }
            Some((Text::Block(Span::of(block, raw)), FileNameSource::Plain))
        }
        (None, Some(ext)) => match policy.extended_filename {
            ExtendedFilenamePolicy::AcceptAlone => {
                let bad = || FormError::encoding(EncodingError::InvalidExtendedValue, at);
                let Word::Token(t) = ext else {
                    return Err(bad());
                };
                let value = ExtValue::parse(t).map_err(|_| bad())?;
                if value.charset() != parse_net::Charset::Utf8 {
                    return Err(bad());
                }
                if value.max_decoded_len() as u64 > u64::from(limits.filename())
                    && value.max_decoded_len() > areas.filename.len()
                {
                    return Err(FormError::limit(Limit::FilenameLength, at));
                }
                let decoded = value.decode_into(areas.filename).map_err(|e| {
                    if e.kind() == parse_net::ErrorKind::BufferTooSmall {
                        FormError::limit(Limit::FilenameLength, at)
                    } else {
                        bad()
                    }
                })?;
                if decoded.len() as u64 > u64::from(limits.filename()) {
                    return Err(FormError::limit(Limit::FilenameLength, at));
                }
                Some((Text::Area(decoded.len() as u32), FileNameSource::Extended))
            }
            ExtendedFilenamePolicy::Ignore => None,
            _ => return Err(FormError::policy(PolicyRefusal::ExtendedFilename, at)),
        },
        (None, None) => None,
    };

    Ok(HeadFacts {
        block_len: block.len() as u32,
        name_raw: Span::of(block, name_raw),
        name: name_text,
        kind: if file.is_some() {
            PartKind::File
        } else {
            PartKind::Field
        },
        filename: file,
        content_type: ct_span,
        charset,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scan_all(block: &[u8], limits: &Limits) -> Result<Option<usize>, FormError> {
        BlockScan::default().feed(block, limits, Location::NONE)
    }

    fn read(block: &[u8], policy: &Policy) -> Result<HeadFacts, FormError> {
        let mut name = [0u8; 256];
        let mut filename = [0u8; 1024];
        read_block(
            block,
            &Limits::DEFAULT,
            policy,
            Areas {
                name: &mut name,
                filename: &mut filename,
            },
            Location::NONE,
        )
    }

    fn reason(block: &[u8]) -> Option<SyntaxError> {
        match read(block, &Policy::DEFAULT) {
            Err(FormError::Syntax { reason, .. }) => Some(reason),
            _ => None,
        }
    }

    #[test]
    fn test_block_scan_finds_end_split_anywhere() {
        let block = b"Content-Disposition: form-data; name=a\r\nX: y\r\n\r\n";
        assert_eq!(
            scan_all(block, &Limits::DEFAULT).unwrap(),
            Some(block.len())
        );
        for split in 0..block.len() {
            let mut s = BlockScan::default();
            let first = s
                .feed(&block[..split], &Limits::DEFAULT, Location::NONE)
                .unwrap();
            assert_eq!(first, None);
            let second = s
                .feed(&block[split..], &Limits::DEFAULT, Location::NONE)
                .unwrap();
            assert_eq!(second, Some(block.len() - split));
        }
        assert_eq!(scan_all(b"\r\n", &Limits::DEFAULT).unwrap(), Some(2));
    }

    #[test]
    fn test_block_scan_budgets_and_line_endings() {
        let l = Limits::DEFAULT.with_max_part_header_bytes(64);
        let mut block = alloc::vec![b'a'; 63];
        block.extend_from_slice(b"\r\n");
        assert!(matches!(
            scan_all(&block, &l),
            Err(FormError::Limit {
                limit: Limit::PartHeaderBytes,
                ..
            })
        ));
        let l = Limits::DEFAULT.with_max_part_headers(1);
        assert!(matches!(
            scan_all(b"A: 1\r\nB: 2\r\n\r\n", &l),
            Err(FormError::Limit {
                limit: Limit::PartHeaderCount,
                ..
            })
        ));
        assert!(matches!(
            scan_all(b"A: 1\n\r\n", &Limits::DEFAULT),
            Err(FormError::Syntax {
                reason: SyntaxError::BareLineFeed,
                ..
            })
        ));
        assert!(matches!(
            scan_all(b"A: 1\rx\r\n\r\n", &Limits::DEFAULT),
            Err(FormError::Syntax {
                reason: SyntaxError::ControlInHeader,
                ..
            })
        ));
    }

    #[test]
    fn test_read_block_field_and_file() {
        let f = read(
            b"Content-Disposition: form-data; name=\"title\"\r\n\r\n",
            &Policy::DEFAULT,
        )
        .unwrap();
        assert_eq!(f.kind, PartKind::Field);
        assert!(f.filename.is_none());
        let f = read(
            b"content-disposition: FORM-DATA; name=f; filename=\"a.png\"\r\nContent-Type: image/png; charset=UTF-8\r\n\r\n",
            &Policy::DEFAULT,
        )
        .unwrap();
        assert_eq!(f.kind, PartKind::File);
        assert_eq!(f.charset, Charset::Utf8);
        assert!(f.content_type.is_some());
    }

    #[test]
    fn test_read_block_refusals() {
        assert_eq!(
            reason(b"\r\n"),
            Some(SyntaxError::MissingContentDisposition)
        );
        assert_eq!(
            reason(b"Content-Disposition: attachment; name=a\r\n\r\n"),
            Some(SyntaxError::NotFormData)
        );
        assert_eq!(
            reason(b"Content-Disposition: form-data\r\n\r\n"),
            Some(SyntaxError::MissingName)
        );
        assert_eq!(
            reason(b"Content-Disposition: form-data; name=\"\"\r\n\r\n"),
            Some(SyntaxError::EmptyName)
        );
        assert_eq!(
            reason(b"Content-Disposition: form-data; name=a; name=b\r\n\r\n"),
            Some(SyntaxError::DuplicateParameter)
        );
        assert_eq!(
            reason(b"Content-Disposition: form-data; name=a\r\nContent-Disposition: form-data; name=b\r\n\r\n"),
            Some(SyntaxError::DuplicateHeader)
        );
        assert_eq!(
            reason(b"Content-Disposition: form-data; name=\"a\\\"; filename=\"x\"\r\n\r\n"),
            Some(SyntaxError::AmbiguousQuote)
        );
        assert_eq!(
            reason(b"Content-Disposition : form-data; name=a\r\n\r\n"),
            Some(SyntaxError::InvalidHeaderName)
        );
        assert_eq!(
            reason(b"X: 1\r\n folded\r\n\r\n"),
            Some(SyntaxError::HeaderFolding)
        );
        assert_eq!(
            reason(b"no colon\r\n\r\n"),
            Some(SyntaxError::MalformedHeaderLine)
        );
        assert_eq!(
            reason(b"X: a\x00b\r\n\r\n"),
            Some(SyntaxError::ControlInHeader)
        );
        assert_eq!(
            reason(b"Content-Disposition: form-data; name=a; junk\r\n\r\n"),
            Some(SyntaxError::MalformedParameter)
        );
    }

    #[test]
    fn test_read_block_policies() {
        let te = read(
            b"Content-Disposition: form-data; name=a\r\nContent-Transfer-Encoding: base64\r\n\r\n",
            &Policy::DEFAULT,
        );
        assert!(matches!(
            te,
            Err(FormError::Policy {
                reason: PolicyRefusal::TransferEncoding,
                ..
            })
        ));
        assert!(read(
            b"Content-Disposition: form-data; name=a\r\nContent-Transfer-Encoding: BINARY\r\n\r\n",
            &Policy::DEFAULT
        )
        .is_ok());
        let nested = read(
            b"Content-Disposition: form-data; name=a\r\nContent-Type: multipart/mixed; boundary=x\r\n\r\n",
            &Policy::DEFAULT,
        );
        assert!(matches!(
            nested,
            Err(FormError::Policy {
                reason: PolicyRefusal::NestedMultipart,
                ..
            })
        ));
        let unknown = b"Content-Disposition: form-data; name=a\r\nX-Foo: 1\r\n\r\n";
        assert!(read(unknown, &Policy::DEFAULT).is_ok());
        let strict = Policy::DEFAULT.with_unknown_headers(HeaderPolicy::Refuse);
        assert!(matches!(
            read(unknown, &strict),
            Err(FormError::Policy {
                reason: PolicyRefusal::UnknownHeader,
                ..
            })
        ));
    }

    #[test]
    fn test_read_block_extended_filename_policies() {
        let ext = b"Content-Disposition: form-data; name=f; filename*=UTF-8''%E2%82%AC.txt\r\n\r\n";
        assert!(matches!(
            read(ext, &Policy::DEFAULT),
            Err(FormError::Policy {
                reason: PolicyRefusal::ExtendedFilename,
                ..
            })
        ));
        let accept = Policy::DEFAULT.with_extended_filename(ExtendedFilenamePolicy::AcceptAlone);
        let f = read(ext, &accept).unwrap();
        assert_eq!(f.filename, Some((Text::Area(7), FileNameSource::Extended)));
        let ignore = Policy::DEFAULT.with_extended_filename(ExtendedFilenamePolicy::Ignore);
        assert_eq!(read(ext, &ignore).unwrap().kind, PartKind::Field);
        let both =
            b"Content-Disposition: form-data; name=f; filename=a; filename*=UTF-8''b\r\n\r\n";
        for p in [Policy::DEFAULT, accept, ignore] {
            assert!(matches!(
                read(both, &p),
                Err(FormError::Policy {
                    reason: PolicyRefusal::BothFilenames,
                    ..
                })
            ));
        }
        let latin = b"Content-Disposition: form-data; name=f; filename*=ISO-8859-1''%E9\r\n\r\n";
        assert!(matches!(
            read(latin, &accept),
            Err(FormError::Encoding {
                reason: EncodingError::InvalidExtendedValue,
                ..
            })
        ));
    }

    #[test]
    fn test_read_block_starred_parameters_and_backslash_names_refused() {
        for block in [
            &b"Content-Disposition: form-data; name=f; filename*0=\"shell.php\"\r\n\r\n"[..],
            b"Content-Disposition: form-data; name=\"comment\"; name*=UTF-8''is_admin\r\n\r\n",
            b"Content-Disposition: form-data; name=f; filename*0*=UTF-8''a\r\n\r\n",
            b"Content-Disposition: form-data; name=\"ro\\le\"\r\n\r\n",
        ] {
            assert_eq!(
                reason(block),
                Some(SyntaxError::MalformedParameter),
                "{block:?}"
            );
        }
        // A backslash in a file name stays literal, as browsers send it.
        let f = read(
            b"Content-Disposition: form-data; name=f; filename=\"a\\b.txt\"\r\n\r\n",
            &Policy::DEFAULT,
        )
        .unwrap();
        assert_eq!(f.kind, PartKind::File);
    }

    #[test]
    fn test_read_block_name_escapes_decoded() {
        let mut name = [0u8; 256];
        let mut filename = [0u8; 16];
        let f = read_block(
            b"Content-Disposition: form-data; name=\"a%22b%0D%0A\"\r\n\r\n",
            &Limits::DEFAULT,
            &Policy::DEFAULT,
            Areas {
                name: &mut name,
                filename: &mut filename,
            },
            Location::NONE,
        )
        .unwrap();
        assert_eq!(f.name, Text::Area(5));
        assert_eq!(&name[..5], b"a\"b\r\n");
    }
}
