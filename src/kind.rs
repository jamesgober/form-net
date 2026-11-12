//! Form kind detection from the request's `Content-Type`, and the parameter
//! rules shared with part headers.

use parse_net::{DuplicatePolicy, Params, Word};

use crate::error::{EncodingError, FormError, Location, SyntaxError};
use crate::multipart::boundary::Boundary;

/// parse-net limits for form headers: at most 16 parameters, and repeated
/// names reported to form-net, which applies its own repeat rules (D-063).
pub(crate) const HEADER_LIMITS: parse_net::Limits = parse_net::Limits::DEFAULT
    .max_params(16)
    .duplicate_params(DuplicatePolicy::First);

/// The name of the first parameter whose name was already used, compared
/// ASCII case-insensitively. Quadratic in the parameter count, which
/// `HEADER_LIMITS` caps at 16.
pub(crate) fn repeated_name<'a>(params: &Params<'a>) -> Option<&'a [u8]> {
    for (i, p) in params.iter().enumerate() {
        if params
            .iter()
            .take(i)
            .any(|q| q.name.eq_ignore_ascii_case(p.name))
        {
            return Some(p.name);
        }
    }
    None
}

/// Whether a header value has an empty parameter: `;` then optional
/// whitespace then `;`, outside a quoted string. RFC 9110 allows it; Go's
/// `mime.ParseMediaType` refuses the whole parameter list, so a Go reader in
/// front sees no boundary, or a part with no name, where form-net reads one
/// (D-136). A single trailing `;` is read the same by both and stays.
pub(crate) fn has_empty_parameter(s: &[u8]) -> bool {
    // Two `;` at least, or there is nothing to find: a browser's header has
    // one or two, and the count is a single fast pass.
    if s.iter().filter(|&&b| b == b';').count() < 2 {
        return false;
    }
    let mut quoted = false;
    let mut escaped = false;
    let mut after_semicolon = false;
    for &b in s {
        if quoted {
            if escaped {
                escaped = false;
            } else if b == b'\\' {
                escaped = true;
            } else if b == b'"' {
                quoted = false;
            }
            continue;
        }
        match b {
            b';' if after_semicolon => return true,
            b';' => after_semicolon = true,
            b' ' | b'\t' => {}
            b'"' => {
                quoted = true;
                after_semicolon = false;
            }
            _ => after_semicolon = false,
        }
    }
    false
}

/// Whether a parameter value holds `name=` or `filename=`, in any ASCII
/// case, where a reader that splits the header on `;` or whitespace would
/// take it for a parameter: at the start of the value or after `;`, `,`,
/// SP, or HTAB. multer 3.1 (the reader behind axum) reads another field
/// name there, or a file where form-net reads a field (D-136). After a
/// letter it does not, so a file name like `Surname=Smith.pdf` is accepted
/// (D-152, checked by running multer).
pub(crate) fn holds_name_assignment(value: &[u8]) -> bool {
    if !value.contains(&b'=') {
        return false;
    }
    let at = |i: usize| {
        let rest = value.get(i..).unwrap_or(&[]);
        let starts = |p: &[u8]| {
            rest.get(..p.len())
                .is_some_and(|r| r.eq_ignore_ascii_case(p))
        };
        starts(b"name=") || starts(b"filename=")
    };
    if at(0) {
        return true;
    }
    value
        .iter()
        .enumerate()
        .any(|(i, &b)| matches!(b, b';' | b',' | b' ' | b'\t') && at(i.wrapping_add(1)))
}

/// The bytes of a parameter value as sent: a token, or the content of a
/// quoted string with backslashes kept literal.
pub(crate) const fn word_raw<'a>(w: &Word<'a>) -> &'a [u8] {
    match w {
        Word::Token(t) => t,
        Word::Quoted(q) => q.raw(),
    }
}

/// Which form encoding a request body uses.
///
/// # Examples
///
/// ```
/// use form_net::{FormError, FormKind};
///
/// match FormKind::detect(b"multipart/form-data; boundary=----WebKitFormBoundaryx7Y")? {
///     FormKind::Urlencoded => { /* parse_urlencoded */ }
///     FormKind::Multipart(boundary) => assert_eq!(boundary.as_bytes(), b"----WebKitFormBoundaryx7Y"),
///     _ => return Err(FormError::UnsupportedMediaType),
/// }
/// assert_eq!(FormKind::detect(b"Application/X-WWW-Form-Urlencoded; charset=UTF-8")?, FormKind::Urlencoded);
/// assert_eq!(FormKind::detect(b"text/plain"), Err(FormError::UnsupportedMediaType));
/// # Ok::<(), FormError>(())
/// ```
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FormKind {
    /// `application/x-www-form-urlencoded`.
    Urlencoded,
    /// `multipart/form-data`, with its validated boundary.
    Multipart(Boundary),
}

impl FormKind {
    /// Reads a request's `Content-Type` value. A pure function of the header
    /// bytes; it never looks at the body.
    ///
    /// `application/x-www-form-urlencoded` is accepted with any parameters
    /// except a `charset` other than UTF-8. `multipart/form-data`
    /// needs exactly one valid `boundary`. Everything else, including
    /// `text/plain`, is `UnsupportedMediaType`. A repeated parameter, an
    /// empty parameter (`;;`), and an extended parameter (`boundary*`,
    /// `charset*0`) are refused (D-136).
    ///
    /// # Errors
    ///
    /// `UnsupportedMediaType`; `Syntax { InvalidContentType | MissingBoundary
    /// | DuplicateBoundary | InvalidBoundary }`; `Encoding { UnsupportedCharset }`.
    ///
    /// # Examples
    ///
    /// ```
    /// use form_net::{FormError, FormKind, SyntaxError};
    ///
    /// assert!(matches!(
    ///     FormKind::detect(b"multipart/form-data; boundary=a; boundary=b"),
    ///     Err(FormError::Syntax { reason: SyntaxError::DuplicateBoundary, .. })
    /// ));
    /// ```
    pub fn detect(content_type: &[u8]) -> Result<FormKind, FormError> {
        if parse_net::text::trim_ows(content_type).is_empty() {
            return Err(FormError::UnsupportedMediaType);
        }
        let mt = parse_net::media::parse(content_type, &HEADER_LIMITS).map_err(|e| {
            FormError::syntax(
                SyntaxError::InvalidContentType,
                Location::at(e.offset() as u64),
            )
        })?;
        let urlencoded = mt.is_form_urlencoded();
        if !urlencoded && !mt.is_multipart_form_data() {
            return Err(FormError::UnsupportedMediaType);
        }
        let params = mt.params();
        if has_empty_parameter(content_type) {
            return Err(FormError::syntax(
                SyntaxError::InvalidContentType,
                Location::at(0),
            ));
        }
        // `*` is in no browser's header: the fast byte search keeps the
        // parameter walk off the common path.
        if let Some(p) = content_type
            .contains(&b'*')
            .then(|| params.iter().find(|p| p.name.contains(&b'*')))
            .flatten()
        {
            // `boundary*=UTF-8''Y` beside `boundary=X`: Go's
            // `mime.ParseMediaType` frames the body with Y, form-net would
            // with X (D-136). The same reasoning as D-081 for part headers.
            let reason = if p.name.len() >= 8
                && p.name
                    .get(..8)
                    .is_some_and(|n| n.eq_ignore_ascii_case(b"boundary"))
            {
                SyntaxError::InvalidBoundary
            } else {
                SyntaxError::InvalidContentType
            };
            return Err(FormError::syntax(reason, Location::at(0)));
        }
        if let Some(name) = repeated_name(&params) {
            let reason = if name.eq_ignore_ascii_case(b"boundary") {
                SyntaxError::DuplicateBoundary
            } else {
                SyntaxError::InvalidContentType
            };
            return Err(FormError::syntax(reason, Location::at(0)));
        }
        if urlencoded {
            if let Some(cs) = params.get("charset") {
                // UTF-8 only: the urlencoded parser does not see this label,
                // and WHATWG readers decode `us-ascii` as windows-1252, so a
                // `%E9` would read two ways (D-150).
                if !cs.eq_ignore_ascii_case(b"utf-8") {
                    return Err(FormError::encoding(
                        EncodingError::UnsupportedCharset,
                        Location::at(0),
                    ));
                }
            }
            return Ok(FormKind::Urlencoded);
        }
        let Some(word) = params.get("boundary") else {
            return Err(FormError::syntax(
                SyntaxError::MissingBoundary,
                Location::at(0),
            ));
        };
        if let Word::Quoted(q) = word {
            if q.has_escapes() {
                return Err(FormError::syntax(
                    SyntaxError::InvalidBoundary,
                    Location::at(0),
                ));
            }
        }
        Boundary::new(word_raw(&word)).map(FormKind::Multipart)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::Limit;

    fn reason(ct: &[u8]) -> Option<SyntaxError> {
        match FormKind::detect(ct) {
            Err(FormError::Syntax { reason, .. }) => Some(reason),
            _ => None,
        }
    }

    #[test]
    fn test_detect_urlencoded_cases() {
        for ct in [
            &b"application/x-www-form-urlencoded"[..],
            b"APPLICATION/X-WWW-FORM-URLENCODED",
            b"application/x-www-form-urlencoded; charset=UTF-8",
            b"application/x-www-form-urlencoded; charset=\"utf-8\"",
            b"application/x-www-form-urlencoded; charset=utf-8; other=1",
            b"  application/x-www-form-urlencoded  ",
        ] {
            assert_eq!(FormKind::detect(ct), Ok(FormKind::Urlencoded), "{ct:?}");
        }
        assert!(matches!(
            FormKind::detect(b"application/x-www-form-urlencoded; charset=iso-8859-1"),
            Err(FormError::Encoding {
                reason: EncodingError::UnsupportedCharset,
                ..
            })
        ));
        assert_eq!(
            reason(b"application/x-www-form-urlencoded; charset=utf-8; Charset=utf-8"),
            Some(SyntaxError::InvalidContentType)
        );
    }

    #[test]
    fn test_detect_multipart_boundary_rules() {
        assert!(matches!(
            FormKind::detect(b"multipart/form-data; boundary=x"),
            Ok(FormKind::Multipart(_))
        ));
        assert!(matches!(
            FormKind::detect(b"Multipart/Form-Data; Boundary=\"a b\""),
            Ok(FormKind::Multipart(b)) if b.as_bytes() == b"a b"
        ));
        assert_eq!(
            reason(b"multipart/form-data"),
            Some(SyntaxError::MissingBoundary)
        );
        assert_eq!(
            reason(b"multipart/form-data; boundary=a; boundary=a"),
            Some(SyntaxError::DuplicateBoundary)
        );
        assert_eq!(
            reason(b"multipart/form-data; boundary=\"\""),
            Some(SyntaxError::InvalidBoundary)
        );
        assert_eq!(
            reason(b"multipart/form-data; boundary=\"ab \""),
            Some(SyntaxError::InvalidBoundary)
        );
        assert_eq!(
            reason(b"multipart/form-data; boundary=\"a\\b\""),
            Some(SyntaxError::InvalidBoundary)
        );
        assert_eq!(
            reason(b"multipart/form-data; boundary="),
            Some(SyntaxError::InvalidContentType)
        );
        assert_eq!(
            reason(b"multipart/form-data; boundary=a; x=1; X=2"),
            Some(SyntaxError::InvalidContentType)
        );
        assert_eq!(
            reason(b"multipart/form-data; boundary=x junk"),
            Some(SyntaxError::InvalidContentType)
        );
    }

    #[test]
    fn test_detect_unsupported_types() {
        for ct in [
            &b""[..],
            b"   ",
            b"text/plain",
            b"multipart/mixed; boundary=x",
            b"application/json",
        ] {
            assert_eq!(
                FormKind::detect(ct),
                Err(FormError::UnsupportedMediaType),
                "{ct:?}"
            );
        }
    }

    #[test]
    fn test_detect_too_many_parameters_refused() {
        let mut ct = alloc::vec::Vec::from(&b"multipart/form-data; boundary=x"[..]);
        for i in 0..16 {
            ct.extend_from_slice(alloc::format!("; p{i}=1").as_bytes());
        }
        assert_eq!(reason(&ct), Some(SyntaxError::InvalidContentType));
        let _ = Limit::Body;
    }
}
