//! The error type: one `Copy` enum for the whole crate, its reason enums,
//! and where an error was found.
//!
//! No error ever holds input bytes, a field value, a file name, or a path, so
//! every error is safe to log.

use core::fmt;

use crate::hook::Refusal;

/// Everything that can go wrong in form-net.
///
/// Input errors (`Limit`, `Syntax`, `Encoding`, `Policy`) carry a
/// [`Location`]; caller mistakes (`InvalidLimits`, `BufferTooSmall`,
/// `Poisoned`) and environment failures (`Io`, `Random`) do not.
/// [`status_hint`](Self::status_hint) maps every error to an HTTP status and
/// [`code`](Self::code) to a stable name.
///
/// # Examples
///
/// ```
/// use form_net::{FormError, Limit};
///
/// let err = form_net::parse_urlencoded(&vec![b'a'; 11 << 20]).unwrap_err();
/// assert!(matches!(err, FormError::Limit { limit: Limit::Body, .. }));
/// assert_eq!((err.status_hint(), err.code()), (413, "limit.body"));
/// ```
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FormError {
    /// A budget of [`Limits`](crate::Limits) was exceeded.
    Limit {
        /// Which limit.
        limit: Limit,
        /// Where.
        at: Location,
    },
    /// The input breaks the grammar.
    Syntax {
        /// What was wrong.
        reason: SyntaxError,
        /// Where.
        at: Location,
    },
    /// Bytes that do not decode: percent escapes, UTF-8, charsets.
    Encoding {
        /// What was wrong.
        reason: EncodingError,
        /// Where.
        at: Location,
    },
    /// The input is well formed but a [`Policy`](crate::Policy) refuses it.
    Policy {
        /// Which rule.
        reason: PolicyRefusal,
        /// Where.
        at: Location,
    },
    /// An inspector refused the input.
    Refused {
        /// The inspector's status and code.
        refusal: Refusal,
        /// Where.
        at: Location,
    },
    /// Typed extraction failed.
    Extract {
        /// What was wrong.
        reason: ExtractError,
        /// The Rust-side field name, when there is one.
        field: Option<&'static str>,
    },
    /// The content type is not one form-net parses.
    UnsupportedMediaType,
    /// [`Limits::validate`](crate::Limits::validate) refused the
    /// configuration.
    InvalidLimits {
        /// The field it refused.
        limit: Limit,
    },
    /// A buffer the caller provided is too short.
    BufferTooSmall {
        /// The length it must have.
        needed: usize,
    },
    /// The reader, the writer, or the file system failed.
    #[cfg(feature = "std")]
    #[cfg_attr(docsrs, doc(cfg(feature = "std")))]
    Io {
        /// The kind of failure; never a path or an OS message.
        kind: std::io::ErrorKind,
    },
    /// The operating system's random source failed.
    #[cfg(feature = "std")]
    #[cfg_attr(docsrs, doc(cfg(feature = "std")))]
    Random,
    /// A parser was called again after an error, without `reset`.
    Poisoned,
}

impl FormError {
    pub(crate) const fn limit(limit: Limit, at: Location) -> Self {
        Self::Limit { limit, at }
    }

    pub(crate) const fn syntax(reason: SyntaxError, at: Location) -> Self {
        Self::Syntax { reason, at }
    }

    pub(crate) const fn encoding(reason: EncodingError, at: Location) -> Self {
        Self::Encoding { reason, at }
    }

    pub(crate) const fn policy(reason: PolicyRefusal, at: Location) -> Self {
        Self::Policy { reason, at }
    }

    /// The same error with its location moved `by` bytes later, for errors
    /// found inside a component that starts at `by`.
    #[must_use]
    pub(crate) const fn shifted(self, by: u64) -> Self {
        match self {
            Self::Limit { limit, at } => Self::Limit {
                limit,
                at: at.shifted(by),
            },
            Self::Syntax { reason, at } => Self::Syntax {
                reason,
                at: at.shifted(by),
            },
            Self::Encoding { reason, at } => Self::Encoding {
                reason,
                at: at.shifted(by),
            },
            Self::Policy { reason, at } => Self::Policy {
                reason,
                at: at.shifted(by),
            },
            Self::Refused { refusal, at } => Self::Refused {
                refusal,
                at: at.shifted(by),
            },
            other => other,
        }
    }

    /// The HTTP status a server should answer with.
    ///
    /// 413 for every limit, 400 for syntax and most encoding and policy
    /// errors, 415 for unsupported media types, charsets, transfer encodings,
    /// and nested multipart, 422 for extraction errors, 503 when the shared
    /// spill budget is full, 500 for caller mistakes and environment
    /// failures, and the inspector's own status for a refusal (400 if that
    /// status is outside 400 to 599).
    ///
    /// # Examples
    ///
    /// ```
    /// use form_net::FormError;
    ///
    /// assert_eq!(FormError::UnsupportedMediaType.status_hint(), 415);
    /// assert_eq!(FormError::Poisoned.status_hint(), 500);
    /// ```
    #[must_use]
    pub const fn status_hint(&self) -> u16 {
        match self {
            Self::Limit { .. } => 413,
            Self::Syntax { .. } => 400,
            Self::Encoding { reason, .. } => match reason {
                EncodingError::UnsupportedCharset => 415,
                _ => 400,
            },
            Self::Policy { reason, .. } => match reason {
                PolicyRefusal::TransferEncoding | PolicyRefusal::NestedMultipart => 415,
                _ => 400,
            },
            Self::Refused { refusal, .. } => {
                if refusal.status >= 400 && refusal.status <= 599 {
                    refusal.status
                } else {
                    400
                }
            }
            Self::Extract { .. } => 422,
            Self::UnsupportedMediaType => 415,
            Self::InvalidLimits { .. } | Self::BufferTooSmall { .. } | Self::Poisoned => 500,
            #[cfg(feature = "std")]
            Self::Io { .. } | Self::Random => 500,
        }
    }

    /// A stable, machine-readable name for the error: the variant and the
    /// reason in snake case, joined with a dot (`limit.parts`,
    /// `syntax.boundary_in_content`). A refusal returns the inspector's own
    /// [`Refusal::code`]. Codes never change meaning after 1.0.
    ///
    /// # Examples
    ///
    /// ```
    /// use form_net::FormError;
    ///
    /// assert_eq!(FormError::UnsupportedMediaType.code(), "unsupported_media_type");
    /// let err = form_net::parse_urlencoded(b"a=%zz").unwrap_err();
    /// assert_eq!(err.code(), "encoding.invalid_percent");
    /// ```
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self {
            Self::Limit { limit, .. } => limit.code(),
            Self::Syntax { reason, .. } => reason.code(),
            Self::Encoding { reason, .. } => reason.code(),
            Self::Policy { reason, .. } => reason.code(),
            Self::Refused { refusal, .. } => refusal.code,
            Self::Extract { reason, .. } => reason.code(),
            Self::UnsupportedMediaType => "unsupported_media_type",
            Self::InvalidLimits { .. } => "invalid_limits",
            Self::BufferTooSmall { .. } => "buffer_too_small",
            #[cfg(feature = "std")]
            Self::Io { .. } => "io",
            #[cfg(feature = "std")]
            Self::Random => "random",
            Self::Poisoned => "poisoned",
        }
    }

    /// Where the error was found, for input errors and refusals.
    ///
    /// # Examples
    ///
    /// ```
    /// let err = form_net::parse_urlencoded(b"ok=1&bad=%zz").unwrap_err();
    /// assert_eq!(err.location().map(|l| l.offset), Some(9));
    /// assert_eq!(form_net::FormError::Poisoned.location(), None);
    /// ```
    #[must_use]
    pub const fn location(&self) -> Option<Location> {
        match self {
            Self::Limit { at, .. }
            | Self::Syntax { at, .. }
            | Self::Encoding { at, .. }
            | Self::Policy { at, .. }
            | Self::Refused { at, .. } => Some(*at),
            _ => None,
        }
    }
}

impl fmt::Display for FormError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Limit { limit, at } => {
                write!(f, "{} ({at}): {}", limit.describe(), limit.action())
            }
            Self::Syntax { reason, at } => write!(f, "{} ({at})", reason.describe()),
            Self::Encoding { reason, at } => write!(f, "{} ({at})", reason.describe()),
            Self::Policy { reason, at } => write!(f, "{} ({at})", reason.describe()),
            Self::Refused { refusal, at } => {
                write!(f, "refused by the inspector: {} ({at})", refusal.code)
            }
            Self::Extract { reason, field } => match field {
                Some(name) => write!(f, "{} (field `{name}`)", reason.describe()),
                None => f.write_str(reason.describe()),
            },
            Self::UnsupportedMediaType => f.write_str(
                "unsupported media type: only application/x-www-form-urlencoded and multipart/form-data are parsed",
            ),
            Self::InvalidLimits { limit } => write!(
                f,
                "invalid limits: {} cannot be satisfied by any body; fix the configuration",
                limit.field_name()
            ),
            Self::BufferTooSmall { needed } => write!(
                f,
                "buffer too small: {needed} bytes needed; size it with the Limits buffer functions"
            ),
            #[cfg(feature = "std")]
            Self::Io { kind } => write!(f, "I/O failure: {kind}"),
            #[cfg(feature = "std")]
            Self::Random => f.write_str(
                "the operating system's random source failed; no predictable fallback is used",
            ),
            Self::Poisoned => {
                f.write_str("parser used after an error; call reset before parsing again")
            }
        }
    }
}

impl core::error::Error for FormError {}

/// Where an error was found.
///
/// # Examples
///
/// ```
/// let err = form_net::parse_urlencoded(b"a=1&b=%").unwrap_err();
/// let at = err.location().unwrap();
/// assert_eq!((at.part, at.offset), (None, 6));
/// ```
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Location {
    /// The multipart part index (from 0), when the error is inside a part.
    pub part: Option<u32>,
    /// The byte offset in the body (or in the component, for `decode_into`).
    pub offset: u64,
}

impl Location {
    /// No location: configuration errors.
    pub const NONE: Location = Location {
        part: None,
        offset: 0,
    };

    pub(crate) const fn at(offset: u64) -> Self {
        Self { part: None, offset }
    }

    pub(crate) const fn in_part(part: Option<u32>, offset: u64) -> Self {
        Self { part, offset }
    }

    const fn shifted(self, by: u64) -> Self {
        Self {
            part: self.part,
            offset: self.offset.saturating_add(by),
        }
    }
}

impl fmt::Display for Location {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.part {
            Some(part) => write!(f, "part {part}, byte {}", self.offset),
            None => write!(f, "byte {}", self.offset),
        }
    }
}

/// Which budget of [`Limits`](crate::Limits) was exceeded.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Limit {
    /// `max_body_bytes`.
    Body,
    /// `max_fields`.
    Fields,
    /// `max_name_bytes`.
    NameLength,
    /// `max_value_bytes`.
    ValueLength,
    /// `max_text_bytes`.
    TextTotal,
    /// `max_parts`.
    Parts,
    /// `max_files`.
    Files,
    /// `max_file_bytes`.
    FileSize,
    /// `max_part_header_bytes`.
    PartHeaderBytes,
    /// `max_part_headers`.
    PartHeaderCount,
    /// `max_filename_bytes`.
    FilenameLength,
    /// `max_preamble_bytes`.
    Preamble,
    /// `max_epilogue_bytes`.
    Epilogue,
    /// `max_padding_bytes`.
    Padding,
    /// `max_token_bytes`.
    TokenLength,
    /// The per-request disk limit of a store.
    Disk,
}

impl Limit {
    const fn code(self) -> &'static str {
        match self {
            Self::Body => "limit.body",
            Self::Fields => "limit.fields",
            Self::NameLength => "limit.name_length",
            Self::ValueLength => "limit.value_length",
            Self::TextTotal => "limit.text_total",
            Self::Parts => "limit.parts",
            Self::Files => "limit.files",
            Self::FileSize => "limit.file_size",
            Self::PartHeaderBytes => "limit.part_header_bytes",
            Self::PartHeaderCount => "limit.part_header_count",
            Self::FilenameLength => "limit.filename_length",
            Self::Preamble => "limit.preamble",
            Self::Epilogue => "limit.epilogue",
            Self::Padding => "limit.padding",
            Self::TokenLength => "limit.token_length",
            Self::Disk => "limit.disk",
        }
    }

    const fn field_name(self) -> &'static str {
        match self {
            Self::Body => "max_body_bytes",
            Self::Fields => "max_fields",
            Self::NameLength => "max_name_bytes",
            Self::ValueLength => "max_value_bytes",
            Self::TextTotal => "max_text_bytes",
            Self::Parts => "max_parts",
            Self::Files => "max_files",
            Self::FileSize => "max_file_bytes",
            Self::PartHeaderBytes => "max_part_header_bytes",
            Self::PartHeaderCount => "max_part_headers",
            Self::FilenameLength => "max_filename_bytes",
            Self::Preamble => "max_preamble_bytes",
            Self::Epilogue => "max_epilogue_bytes",
            Self::Padding => "max_padding_bytes",
            Self::TokenLength => "max_token_bytes",
            Self::Disk => "max_disk_bytes",
        }
    }

    const fn describe(self) -> &'static str {
        match self {
            Self::Body => "body too large",
            Self::Fields => "too many fields",
            Self::NameLength => "field name too long",
            Self::ValueLength => "field value too long",
            Self::TextTotal => "text fields too large in total",
            Self::Parts => "too many parts",
            Self::Files => "too many files",
            Self::FileSize => "file too large",
            Self::PartHeaderBytes => "part headers too large",
            Self::PartHeaderCount => "too many part header lines",
            Self::FilenameLength => "file name too long",
            Self::Preamble => "too many bytes before the first boundary",
            Self::Epilogue => "too many bytes after the last boundary",
            Self::Padding => "too much padding after a boundary",
            Self::TokenLength => "anti-forgery token too long",
            Self::Disk => "too many bytes stored on disk for one request",
        }
    }

    const fn action(self) -> &'static str {
        match self {
            Self::Body => "raise max_body_bytes if this route accepts larger bodies",
            Self::Fields => "raise max_fields if this route accepts more fields",
            Self::NameLength => "raise max_name_bytes if this route uses longer names",
            Self::ValueLength => "raise max_value_bytes, or use a file input for large text",
            Self::TextTotal => "raise max_text_bytes if this route accepts more text",
            Self::Parts => "raise max_parts if this route accepts more parts",
            Self::Files => "raise max_files if this route accepts more files",
            Self::FileSize => "raise max_file_bytes if this route accepts larger files",
            Self::PartHeaderBytes => "raise max_part_header_bytes if real clients need it",
            Self::PartHeaderCount => "raise max_part_headers if real clients need it",
            Self::FilenameLength => "raise max_filename_bytes if real clients need it",
            Self::Preamble | Self::Epilogue => "the boundary is probably wrong",
            Self::Padding => "the body is not what browsers send",
            Self::TokenLength => "treat the request as forged",
            Self::Disk => "raise the store's max_disk_bytes if this route accepts more",
        }
    }
}

/// What part of the grammar the input broke.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SyntaxError {
    /// A `Content-Type` value (of the request or of a part) is malformed.
    InvalidContentType,
    /// `multipart/form-data` without a `boundary` parameter.
    MissingBoundary,
    /// Two `boundary` parameters.
    DuplicateBoundary,
    /// A boundary that is empty, longer than 70 bytes, has a byte outside
    /// RFC 2046 `bchars`, or ends with a space.
    InvalidBoundary,
    /// The body ended without any delimiter.
    NoInitialDelimiter,
    /// A full delimiter followed by something other than `--`, padding, or
    /// CRLF; or writer data that contains the delimiter.
    BoundaryInContent,
    /// LF without CR where CRLF is required.
    BareLineFeed,
    /// A part header line without a colon.
    MalformedHeaderLine,
    /// A folded part header line.
    HeaderFolding,
    /// A part header name that is not a token, or has whitespace before the
    /// colon.
    InvalidHeaderName,
    /// A control byte other than HTAB in a part header value.
    ControlInHeader,
    /// A part without `Content-Disposition`.
    MissingContentDisposition,
    /// A known part header given twice.
    DuplicateHeader,
    /// A `Content-Disposition` type other than `form-data`.
    NotFormData,
    /// A `Content-Disposition` without `name`.
    MissingName,
    /// An empty `name`.
    EmptyName,
    /// A parameter given twice.
    DuplicateParameter,
    /// Malformed parameters (or more than 16 on one header).
    MalformedParameter,
    /// A quoted parameter value that contains `\"`.
    AmbiguousQuote,
    /// The body ended before the close delimiter.
    Truncated,
}

impl SyntaxError {
    const fn code(self) -> &'static str {
        match self {
            Self::InvalidContentType => "syntax.invalid_content_type",
            Self::MissingBoundary => "syntax.missing_boundary",
            Self::DuplicateBoundary => "syntax.duplicate_boundary",
            Self::InvalidBoundary => "syntax.invalid_boundary",
            Self::NoInitialDelimiter => "syntax.no_initial_delimiter",
            Self::BoundaryInContent => "syntax.boundary_in_content",
            Self::BareLineFeed => "syntax.bare_line_feed",
            Self::MalformedHeaderLine => "syntax.malformed_header_line",
            Self::HeaderFolding => "syntax.header_folding",
            Self::InvalidHeaderName => "syntax.invalid_header_name",
            Self::ControlInHeader => "syntax.control_in_header",
            Self::MissingContentDisposition => "syntax.missing_content_disposition",
            Self::DuplicateHeader => "syntax.duplicate_header",
            Self::NotFormData => "syntax.not_form_data",
            Self::MissingName => "syntax.missing_name",
            Self::EmptyName => "syntax.empty_name",
            Self::DuplicateParameter => "syntax.duplicate_parameter",
            Self::MalformedParameter => "syntax.malformed_parameter",
            Self::AmbiguousQuote => "syntax.ambiguous_quote",
            Self::Truncated => "syntax.truncated",
        }
    }

    const fn describe(self) -> &'static str {
        match self {
            Self::InvalidContentType => "malformed Content-Type",
            Self::MissingBoundary => "multipart/form-data without a boundary",
            Self::DuplicateBoundary => "two boundary parameters",
            Self::InvalidBoundary => "invalid multipart boundary",
            Self::NoInitialDelimiter => {
                "no multipart delimiter found: the body is not multipart or the boundary is wrong"
            }
            Self::BoundaryInContent => "the boundary appears inside a part",
            Self::BareLineFeed => "LF without CR where CRLF is required",
            Self::MalformedHeaderLine => "malformed part header line",
            Self::HeaderFolding => "folded part header line",
            Self::InvalidHeaderName => "invalid part header name",
            Self::ControlInHeader => "control byte in a part header",
            Self::MissingContentDisposition => "part without Content-Disposition",
            Self::DuplicateHeader => "part header given twice",
            Self::NotFormData => "Content-Disposition type is not form-data",
            Self::MissingName => "part without a name",
            Self::EmptyName => "part with an empty name",
            Self::DuplicateParameter => "parameter given twice",
            Self::MalformedParameter => "malformed header parameter",
            Self::AmbiguousQuote => "quoted value contains \\\"",
            Self::Truncated => "body ended before the closing boundary",
        }
    }
}

/// Bytes that do not decode.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EncodingError {
    /// `%` not followed by two hex digits, under `PercentPolicy::Refuse`.
    InvalidPercent,
    /// Text that is not valid UTF-8.
    InvalidUtf8,
    /// A charset other than UTF-8 or US-ASCII.
    UnsupportedCharset,
    /// A malformed `filename*`.
    InvalidExtendedValue,
}

impl EncodingError {
    const fn code(self) -> &'static str {
        match self {
            Self::InvalidPercent => "encoding.invalid_percent",
            Self::InvalidUtf8 => "encoding.invalid_utf8",
            Self::UnsupportedCharset => "encoding.unsupported_charset",
            Self::InvalidExtendedValue => "encoding.invalid_extended_value",
        }
    }

    const fn describe(self) -> &'static str {
        match self {
            Self::InvalidPercent => "invalid percent escape",
            Self::InvalidUtf8 => "text is not valid UTF-8",
            Self::UnsupportedCharset => "unsupported charset: only UTF-8 and US-ASCII are accepted",
            Self::InvalidExtendedValue => "malformed filename*",
        }
    }
}

/// Well-formed input that a [`Policy`](crate::Policy) refuses.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PolicyRefusal {
    /// A `Content-Transfer-Encoding` other than `7bit`, `8bit`, `binary`.
    TransferEncoding,
    /// A part declared `multipart/*`.
    NestedMultipart,
    /// An unknown part header under `HeaderPolicy::Refuse`.
    UnknownHeader,
    /// Both `filename` and `filename*` in one part.
    BothFilenames,
    /// `filename*` under `ExtendedFilenamePolicy::Refuse`.
    ExtendedFilename,
    /// A file part before the token field under
    /// `TokenPlacement::BeforeFirstFile`.
    TokenFieldLate,
    /// The token field given twice.
    DuplicateToken,
}

impl PolicyRefusal {
    const fn code(self) -> &'static str {
        match self {
            Self::TransferEncoding => "policy.transfer_encoding",
            Self::NestedMultipart => "policy.nested_multipart",
            Self::UnknownHeader => "policy.unknown_header",
            Self::BothFilenames => "policy.both_filenames",
            Self::ExtendedFilename => "policy.extended_filename",
            Self::TokenFieldLate => "policy.token_field_late",
            Self::DuplicateToken => "policy.duplicate_token",
        }
    }

    const fn describe(self) -> &'static str {
        match self {
            Self::TransferEncoding => "Content-Transfer-Encoding other than identity",
            Self::NestedMultipart => "nested multipart part",
            Self::UnknownHeader => "unknown part header",
            Self::BothFilenames => "both filename and filename*",
            Self::ExtendedFilename => "filename* is not accepted",
            Self::TokenFieldLate => {
                "file part before the anti-forgery token; treat as a possible forgery"
            }
            Self::DuplicateToken => "anti-forgery token given twice; treat as a possible forgery",
        }
    }
}

/// Why typed extraction failed.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExtractError {
    /// A name the struct neither declares nor ignores.
    Unknown,
    /// A required field is absent.
    Missing,
    /// A single-valued field was given twice.
    Duplicate,
    /// A value that does not convert; the text says why.
    Invalid(&'static str),
    /// A file where text was expected, or the reverse.
    WrongKind,
}

impl ExtractError {
    const fn code(self) -> &'static str {
        match self {
            Self::Unknown => "extract.unknown",
            Self::Missing => "extract.missing",
            Self::Duplicate => "extract.duplicate",
            Self::Invalid(_) => "extract.invalid",
            Self::WrongKind => "extract.wrong_kind",
        }
    }

    const fn describe(self) -> &'static str {
        match self {
            Self::Unknown => "unknown field",
            Self::Missing => "missing field",
            Self::Duplicate => "field given more than once",
            Self::Invalid(why) => why,
            Self::WrongKind => "file where text was expected, or text where a file was expected",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::string::ToString;

    #[test]
    fn test_status_hint_every_family_maps() {
        let at = Location::NONE;
        assert_eq!(FormError::limit(Limit::Parts, at).status_hint(), 413);
        assert_eq!(
            FormError::syntax(SyntaxError::Truncated, at).status_hint(),
            400
        );
        assert_eq!(
            FormError::encoding(EncodingError::UnsupportedCharset, at).status_hint(),
            415
        );
        assert_eq!(
            FormError::encoding(EncodingError::InvalidUtf8, at).status_hint(),
            400
        );
        assert_eq!(
            FormError::policy(PolicyRefusal::NestedMultipart, at).status_hint(),
            415
        );
        assert_eq!(
            FormError::policy(PolicyRefusal::TokenFieldLate, at).status_hint(),
            400
        );
        let extract = FormError::Extract {
            reason: ExtractError::Missing,
            field: Some("email"),
        };
        assert_eq!(extract.status_hint(), 422);
        assert_eq!(
            FormError::InvalidLimits { limit: Limit::Body }.status_hint(),
            500
        );
        assert_eq!(FormError::BufferTooSmall { needed: 3 }.status_hint(), 500);
    }

    #[test]
    fn test_status_hint_refusal_out_of_range_reports_400() {
        let at = Location::NONE;
        let ok = FormError::Refused {
            refusal: Refusal::new(422, "x"),
            at,
        };
        assert_eq!(ok.status_hint(), 422);
        for status in [0, 200, 399, 600, 999] {
            let e = FormError::Refused {
                refusal: Refusal::new(status, "x"),
                at,
            };
            assert_eq!(e.status_hint(), 400);
        }
    }

    #[test]
    fn test_code_refusal_returns_inspector_code() {
        let e = FormError::Refused {
            refusal: Refusal::new(422, "customs.unexpected_field"),
            at: Location::NONE,
        };
        assert_eq!(e.code(), "customs.unexpected_field");
    }

    #[test]
    fn test_display_never_contains_the_field_value() {
        let e = FormError::Extract {
            reason: ExtractError::Invalid("not a number"),
            field: Some("age"),
        };
        assert_eq!(e.to_string(), "not a number (field `age`)");
        let e = FormError::limit(Limit::Parts, Location::in_part(Some(3), 99));
        assert_eq!(
            e.to_string(),
            "too many parts (part 3, byte 99): raise max_parts if this route accepts more parts"
        );
    }

    #[test]
    fn test_shifted_moves_input_errors_only() {
        let e = FormError::encoding(EncodingError::InvalidPercent, Location::at(2)).shifted(10);
        assert_eq!(e.location().map(|l| l.offset), Some(12));
        assert_eq!(FormError::Poisoned.shifted(10), FormError::Poisoned);
    }

    #[test]
    fn test_form_error_is_small_and_copy() {
        fn is_copy<T: Copy>() {}
        is_copy::<FormError>();
        assert!(core::mem::size_of::<FormError>() <= 48);
    }
}
