//! Budgets ([`Limits`]) and behaviours ([`Policy`]).
//!
//! Both are small `Copy` values with `const` setters, so a route's
//! configuration is a `const` checked once at startup.

use crate::error::{FormError, Limit};

/// The bytes the multipart parser keeps for the `_charset_` field's value.
pub(crate) const CHARSET_CAPTURE: usize = 32;

/// Every budget form-net enforces.
///
/// Each budget is checked before the bytes it governs are buffered, stored,
/// or passed on. A limit of 0 means none allowed. A limit larger than the one
/// containing it is capped by it (the effective value is the smaller), so a
/// route sets only the limits it cares about.
///
/// | Field | Default |
/// |---|---|
/// | `max_body_bytes` | 10 MiB |
/// | `max_fields` | 1,000 |
/// | `max_name_bytes` | 256 |
/// | `max_value_bytes` | 1 MiB |
/// | `max_text_bytes` | 2 MiB |
/// | `max_parts` | 100 |
/// | `max_files` | 20 |
/// | `max_file_bytes` | 10 MiB |
/// | `max_part_header_bytes` | 8 KiB |
/// | `max_part_headers` | 8 |
/// | `max_filename_bytes` | 1,024 |
/// | `max_preamble_bytes` | 1 KiB |
/// | `max_epilogue_bytes` | 1 KiB |
/// | `max_padding_bytes` | 32 |
/// | `sniff_bytes` | 0 |
/// | `max_token_bytes` | 128 |
///
/// # Examples
///
/// ```
/// use form_net::Limits;
///
/// // A login route: a small body, a few fields, no files.
/// const LOGIN: Limits = Limits::DEFAULT
///     .with_max_body_bytes(4 * 1024)
///     .with_max_fields(8)
///     .with_max_files(0);
/// assert!(LOGIN.validate().is_ok());
/// ```
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    /// Every byte fed, both formats, preamble and epilogue included.
    pub max_body_bytes: u64,
    /// urlencoded pairs; multipart text fields.
    pub max_fields: u32,
    /// Raw (encoded) name length. Capped by `max_body_bytes`.
    pub max_name_bytes: u32,
    /// One urlencoded value or one multipart text field. Capped by
    /// `max_text_bytes`.
    pub max_value_bytes: u32,
    /// All text values of one body together. Capped by `max_body_bytes`.
    pub max_text_bytes: u64,
    /// Multipart parts of either kind.
    pub max_parts: u32,
    /// Multipart file parts.
    pub max_files: u32,
    /// One file part. Capped by `max_body_bytes`.
    pub max_file_bytes: u64,
    /// One part's header block, CRLF CRLF included (at least 64).
    pub max_part_header_bytes: u32,
    /// Header lines per part.
    pub max_part_headers: u16,
    /// A raw `filename`, or a decoded `filename*`. Capped by
    /// `max_part_header_bytes`.
    pub max_filename_bytes: u32,
    /// Bytes before the first delimiter. Capped by `max_body_bytes`.
    pub max_preamble_bytes: u32,
    /// Bytes after the close delimiter. Capped by `max_body_bytes`.
    pub max_epilogue_bytes: u32,
    /// Transport padding after a delimiter.
    pub max_padding_bytes: u8,
    /// The sniff window per file part (0: off). Capped by `max_file_bytes`.
    pub sniff_bytes: u32,
    /// The captured anti-forgery token. Capped by `max_value_bytes`.
    pub max_token_bytes: u16,
}

macro_rules! setter {
    ($(#[$doc:meta])* $name:ident, $field:ident, $ty:ty) => {
        $(#[$doc])*
        #[must_use]
        pub const fn $name(mut self, value: $ty) -> Self {
            self.$field = value;
            self
        }
    };
}

const fn min_u64(a: u64, b: u64) -> u64 {
    if a < b { a } else { b }
}

/// `value` as `usize`, or `usize::MAX` when it does not fit.
#[cfg(feature = "alloc")]
const fn to_usize(value: u64) -> usize {
    if value > usize::MAX as u64 {
        usize::MAX
    } else {
        value as usize
    }
}

const fn add_usize(a: usize, b: usize) -> usize {
    match a.checked_add(b) {
        Some(v) => v,
        None => usize::MAX,
    }
}

impl Limits {
    /// The defaults in the table above.
    pub const DEFAULT: Limits = Limits {
        max_body_bytes: 10 * 1024 * 1024,
        max_fields: 1_000,
        max_name_bytes: 256,
        max_value_bytes: 1024 * 1024,
        max_text_bytes: 2 * 1024 * 1024,
        max_parts: 100,
        max_files: 20,
        max_file_bytes: 10 * 1024 * 1024,
        max_part_header_bytes: 8 * 1024,
        max_part_headers: 8,
        max_filename_bytes: 1_024,
        max_preamble_bytes: 1_024,
        max_epilogue_bytes: 1_024,
        max_padding_bytes: 32,
        sniff_bytes: 0,
        max_token_bytes: 128,
    };

    setter!(
        /// Sets `max_body_bytes`.
        with_max_body_bytes, max_body_bytes, u64);
    setter!(
        /// Sets `max_fields`.
        with_max_fields, max_fields, u32);
    setter!(
        /// Sets `max_name_bytes`.
        with_max_name_bytes, max_name_bytes, u32);
    setter!(
        /// Sets `max_value_bytes`.
        with_max_value_bytes, max_value_bytes, u32);
    setter!(
        /// Sets `max_text_bytes`.
        with_max_text_bytes, max_text_bytes, u64);
    setter!(
        /// Sets `max_parts`.
        with_max_parts, max_parts, u32);
    setter!(
        /// Sets `max_files`.
        with_max_files, max_files, u32);
    setter!(
        /// Sets `max_file_bytes`.
        with_max_file_bytes, max_file_bytes, u64);
    setter!(
        /// Sets `max_part_header_bytes`.
        with_max_part_header_bytes, max_part_header_bytes, u32);
    setter!(
        /// Sets `max_part_headers`.
        with_max_part_headers, max_part_headers, u16);
    setter!(
        /// Sets `max_filename_bytes`.
        with_max_filename_bytes, max_filename_bytes, u32);
    setter!(
        /// Sets `max_preamble_bytes`.
        with_max_preamble_bytes, max_preamble_bytes, u32);
    setter!(
        /// Sets `max_epilogue_bytes`.
        with_max_epilogue_bytes, max_epilogue_bytes, u32);
    setter!(
        /// Sets `max_padding_bytes`.
        with_max_padding_bytes, max_padding_bytes, u8);
    setter!(
        /// Sets `sniff_bytes`.
        with_sniff_bytes, sniff_bytes, u32);
    setter!(
        /// Sets `max_token_bytes`.
        with_max_token_bytes, max_token_bytes, u16);

    /// Refuses the values no body could satisfy, so a misconfiguration is
    /// found at startup: a `max_part_header_bytes` below 64 (the shortest
    /// useful `Content-Disposition` line must fit), or buffer sizes that do
    /// not fit in `usize`. Every parser constructor and every `*_with`
    /// function calls it.
    ///
    /// # Errors
    ///
    /// [`FormError::InvalidLimits`] naming the refused field.
    ///
    /// # Examples
    ///
    /// ```
    /// use form_net::Limits;
    ///
    /// assert!(Limits::DEFAULT.validate().is_ok());
    /// let broken = Limits::DEFAULT.with_max_part_header_bytes(16);
    /// assert_eq!(broken.validate().unwrap_err().code(), "invalid_limits");
    /// ```
    pub fn validate(&self) -> Result<(), FormError> {
        if self.max_part_header_bytes < 64 {
            return Err(FormError::InvalidLimits {
                limit: Limit::PartHeaderBytes,
            });
        }
        let parts = [
            u64::from(self.max_part_header_bytes),
            u64::from(self.name()),
            u64::from(self.filename()),
            u64::from(self.token()),
            CHARSET_CAPTURE as u64,
        ];
        let total = parts.iter().fold(0u64, |a, &b| a.saturating_add(b));
        if total > usize::MAX as u64 {
            return Err(FormError::InvalidLimits {
                limit: Limit::PartHeaderBytes,
            });
        }
        let url = u64::from(self.name())
            .saturating_add(u64::from(self.value()))
            .saturating_add(1);
        if url > usize::MAX as u64 {
            return Err(FormError::InvalidLimits {
                limit: Limit::ValueLength,
            });
        }
        Ok(())
    }

    /// Validates these limits together with a policy: a token field with an
    /// effective `max_token_bytes` of 0 can never be captured.
    pub(crate) fn validate_with(&self, policy: &crate::Policy) -> Result<(), FormError> {
        self.validate()?;
        if policy.token_field.is_some() && self.token() == 0 {
            return Err(FormError::InvalidLimits {
                limit: Limit::TokenLength,
            });
        }
        Ok(())
    }

    /// The buffer length the multipart parser needs with these limits and
    /// this policy: the header block, the decoded field name, the decoded
    /// `filename*` (only under `ExtendedFilenamePolicy::AcceptAlone`), the
    /// token (only with a token field), and 32 bytes for `_charset_`.
    ///
    /// A `const fn`, so a stack buffer can be sized at compile time.
    ///
    /// # Examples
    ///
    /// ```
    /// use form_net::{Limits, Policy};
    ///
    /// const LIMITS: Limits = Limits::DEFAULT.with_max_part_header_bytes(1024);
    /// const LEN: usize = LIMITS.multipart_buffer_len(&Policy::DEFAULT);
    /// assert_eq!(LEN, 1024 + 256 + 32);
    /// let buffer = [0u8; LEN];
    /// ```
    #[must_use]
    pub const fn multipart_buffer_len(&self, policy: &crate::Policy) -> usize {
        let mut len = add_usize(self.max_part_header_bytes as usize, self.name() as usize);
        if matches!(
            policy.extended_filename,
            crate::ExtendedFilenamePolicy::AcceptAlone
        ) {
            len = add_usize(len, self.filename() as usize);
        }
        if policy.token_field.is_some() {
            len = add_usize(len, self.token() as usize);
        }
        add_usize(len, CHARSET_CAPTURE)
    }

    /// The scratch length the streaming urlencoded parser needs:
    /// `max_name_bytes + max_value_bytes + 1` (effective values).
    ///
    /// # Examples
    ///
    /// ```
    /// use form_net::Limits;
    ///
    /// const LIMITS: Limits = Limits::DEFAULT.with_max_text_bytes(1000);
    /// assert_eq!(LIMITS.urlencoded_buffer_len(), 256 + 1000 + 1);
    /// ```
    #[must_use]
    pub const fn urlencoded_buffer_len(&self) -> usize {
        add_usize(add_usize(self.name() as usize, self.value() as usize), 1)
    }

    /// The worst-case bytes one parse with these limits can hold in memory,
    /// for admission control before a body is read (FR-40, D-093).
    ///
    /// The sum of: parser state (256); the parser buffer and the driver's
    /// copy of the current part head (each `multipart_buffer_len` under the
    /// most demanding policy); the sniff window; the field being received
    /// (`max_value_bytes`); the text kept (`max_text_bytes`) and its names
    /// (`max_fields × max_name_bytes`, capped by `max_body_bytes`); 96 bytes
    /// of index per field and 320 per part; owned copies of part names, file
    /// names, and content types (`max_parts × max_part_header_bytes`, capped
    /// by `max_body_bytes`); the files kept in memory,
    /// `max_files × min(file_in_memory_bytes, max_file_bytes)`, capped by
    /// twice `max_body_bytes` (a buffer that grew by doubling can hold twice
    /// its bytes); and half the largest growing buffer (a buffer that
    /// doubles holds its old half while it moves). `file_in_memory_bytes`
    /// is the most bytes of one file a store keeps in memory
    /// (`Store::memory_bound` passes it). The caller's own read buffer is
    /// not included. All values are effective values; the arithmetic
    /// saturates. `tests/memory_bound.rs` measures it against hostile bodies.
    ///
    /// # Examples
    ///
    /// ```
    /// use form_net::Limits;
    ///
    /// const LOGIN: Limits = Limits::DEFAULT
    ///     .with_max_body_bytes(4 * 1024)
    ///     .with_max_fields(8)
    ///     .with_max_parts(8)
    ///     .with_max_files(0)
    ///     .with_max_part_header_bytes(1024);
    /// assert!(LOGIN.memory_bound(64 * 1024) < 32 * 1024);
    /// // The defaults with files over 64 KiB spilled: about 6.5 MiB.
    /// assert!(Limits::DEFAULT.memory_bound(64 * 1024) < 7 << 20);
    /// ```
    #[must_use]
    pub const fn memory_bound(&self, file_in_memory_bytes: u64) -> u64 {
        const PARSER_STATE: u64 = 256;
        const FIELD_INDEX: u64 = 96;
        const PART_INDEX: u64 = 320;
        let head = (self.max_part_header_bytes as u64)
            .saturating_add(self.name() as u64)
            .saturating_add(self.filename() as u64)
            .saturating_add(self.token() as u64)
            .saturating_add(CHARSET_CAPTURE as u64);
        let parts = self.max_parts as u64;
        let copies = min_u64(
            parts.saturating_mul(self.max_part_header_bytes as u64),
            self.body(),
        );
        let sniff = min_u64(self.sniff_bytes as u64, self.file());
        let file = min_u64(file_in_memory_bytes, self.file());
        let files = min_u64(
            (self.max_files as u64).saturating_mul(file),
            self.body().saturating_mul(2),
        );
        let text = self.text_and_names();
        let growth = if file > text { file } else { text } / 2;
        PARSER_STATE
            .saturating_add(head.saturating_mul(2))
            .saturating_add(sniff)
            .saturating_add(self.value() as u64)
            .saturating_add(text)
            .saturating_add((self.max_fields as u64).saturating_mul(FIELD_INDEX))
            .saturating_add(parts.saturating_mul(PART_INDEX))
            .saturating_add(copies)
            .saturating_add(files)
            .saturating_add(growth)
    }

    /// The text values and field names one body can hold: `max_text_bytes`
    /// plus `max_fields × max_name_bytes`, the latter capped by
    /// `max_body_bytes` (effective values).
    pub(crate) const fn text_and_names(&self) -> u64 {
        let names = min_u64(
            (self.max_fields as u64).saturating_mul(self.name() as u64),
            self.body(),
        );
        self.text().saturating_add(names)
    }

    // Effective values: each capped by the limit that contains it.

    pub(crate) const fn body(&self) -> u64 {
        self.max_body_bytes
    }

    pub(crate) const fn text(&self) -> u64 {
        min_u64(self.max_text_bytes, self.body())
    }

    pub(crate) const fn value(&self) -> u32 {
        min_u64(self.max_value_bytes as u64, self.text()) as u32
    }

    pub(crate) const fn name(&self) -> u32 {
        min_u64(self.max_name_bytes as u64, self.body()) as u32
    }

    pub(crate) const fn file(&self) -> u64 {
        min_u64(self.max_file_bytes, self.body())
    }

    #[cfg(feature = "alloc")]
    pub(crate) const fn sniff(&self) -> u32 {
        min_u64(self.sniff_bytes as u64, self.file()) as u32
    }

    pub(crate) const fn filename(&self) -> u32 {
        min_u64(
            self.max_filename_bytes as u64,
            self.max_part_header_bytes as u64,
        ) as u32
    }

    pub(crate) const fn token(&self) -> u16 {
        min_u64(self.max_token_bytes as u64, self.value() as u64) as u16
    }

    pub(crate) const fn preamble(&self) -> u64 {
        min_u64(self.max_preamble_bytes as u64, self.body())
    }

    pub(crate) const fn epilogue(&self) -> u64 {
        min_u64(self.max_epilogue_bytes as u64, self.body())
    }

    #[cfg(feature = "alloc")]
    pub(crate) const fn body_usize(&self) -> usize {
        to_usize(self.body())
    }
}

impl Default for Limits {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// The behaviours a route may choose. The defaults are the strict ones;
/// every alternative is named after what it allows.
///
/// # Examples
///
/// ```
/// use form_net::{EmptyFilePolicy, HeaderPolicy, Policy, TokenField};
///
/// const UPLOADS: Policy = Policy::DEFAULT
///     .with_token_field(TokenField::new("csrf_token"))
///     .with_empty_files(EmptyFilePolicy::Skip)
///     .with_unknown_headers(HeaderPolicy::Refuse);
/// assert_eq!(UPLOADS.token_field.map(|t| t.name()), Some("csrf_token"));
/// ```
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Policy {
    /// A malformed `%` in urlencoded input. Default `Refuse`.
    pub percent: PercentPolicy,
    /// `filename*` in a part. Default `Refuse`.
    pub extended_filename: ExtendedFilenamePolicy,
    /// Part headers other than the three known ones. Default `Ignore`.
    pub unknown_headers: HeaderPolicy,
    /// A file input left empty. Default `Report`.
    pub empty_files: EmptyFilePolicy,
    /// The anti-forgery token field to capture. Default none.
    pub token_field: Option<TokenField>,
}

impl Policy {
    /// The strict defaults.
    pub const DEFAULT: Policy = Policy {
        percent: PercentPolicy::Refuse,
        extended_filename: ExtendedFilenamePolicy::Refuse,
        unknown_headers: HeaderPolicy::Ignore,
        empty_files: EmptyFilePolicy::Report,
        token_field: None,
    };

    /// Sets `percent`.
    #[must_use]
    pub const fn with_percent(mut self, policy: PercentPolicy) -> Self {
        self.percent = policy;
        self
    }

    /// Sets `extended_filename`.
    #[must_use]
    pub const fn with_extended_filename(mut self, policy: ExtendedFilenamePolicy) -> Self {
        self.extended_filename = policy;
        self
    }

    /// Sets `unknown_headers`.
    #[must_use]
    pub const fn with_unknown_headers(mut self, policy: HeaderPolicy) -> Self {
        self.unknown_headers = policy;
        self
    }

    /// Sets `empty_files`.
    #[must_use]
    pub const fn with_empty_files(mut self, policy: EmptyFilePolicy) -> Self {
        self.empty_files = policy;
        self
    }

    /// Sets the token field to capture.
    #[must_use]
    pub const fn with_token_field(mut self, field: TokenField) -> Self {
        self.token_field = Some(field);
        self
    }
}

impl Default for Policy {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// What a `%` not followed by two hex digits means in urlencoded input.
///
/// # Examples
///
/// ```
/// use form_net::{decode_into, PercentPolicy};
///
/// let mut out = [0u8; 8];
/// assert!(decode_into(b"100%", &mut out, PercentPolicy::Refuse).is_err());
/// assert_eq!(decode_into(b"100%", &mut out, PercentPolicy::Literal)?, "100%");
/// # Ok::<(), form_net::FormError>(())
/// ```
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PercentPolicy {
    /// Refuse it (default).
    Refuse,
    /// Keep it as a literal byte, as WHATWG percent-decode does.
    Literal,
}

/// What a `filename*` parameter (RFC 8187) in a part means. Browsers never
/// send it; RFC 7578 forbids senders from using it.
///
/// Both `filename` and `filename*` in one part are refused under every
/// policy.
///
/// # Examples
///
/// ```
/// use form_net::{ExtendedFilenamePolicy, Policy};
///
/// let accept = Policy::DEFAULT.with_extended_filename(ExtendedFilenamePolicy::AcceptAlone);
/// assert_eq!(Policy::DEFAULT.extended_filename, ExtendedFilenamePolicy::Refuse);
/// assert_ne!(accept, Policy::DEFAULT);
/// ```
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExtendedFilenamePolicy {
    /// Refuse any `filename*` (default).
    Refuse,
    /// Accept and decode it (UTF-8 only) when it is the only file name.
    AcceptAlone,
    /// Drop it; a part with only `filename*` is then a text field.
    Ignore,
}

/// Part headers other than `Content-Disposition`, `Content-Type`, and
/// `Content-Transfer-Encoding`.
///
/// # Examples
///
/// ```
/// use form_net::{HeaderPolicy, Policy};
///
/// assert_eq!(Policy::DEFAULT.unknown_headers, HeaderPolicy::Ignore);
/// ```
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HeaderPolicy {
    /// Skip them, as RFC 7578 section 4.8 requires (default). They still
    /// count toward the header budgets.
    Ignore,
    /// Refuse them.
    Refuse,
}

/// A file input left empty (`filename=""` and no data).
///
/// # Examples
///
/// ```
/// use form_net::{EmptyFilePolicy, Policy};
///
/// assert_eq!(Policy::DEFAULT.empty_files, EmptyFilePolicy::Report);
/// ```
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EmptyFilePolicy {
    /// Report it as a file part with an empty file name (default).
    Report,
    /// Produce nothing for it; it does not count against `max_files`.
    Skip,
}

/// The anti-forgery token field to capture from a multipart body.
///
/// # Examples
///
/// ```
/// use form_net::{TokenField, TokenPlacement};
///
/// let strict = TokenField::new("csrf_token");
/// assert_eq!(strict.placement(), TokenPlacement::BeforeFirstFile);
/// assert_eq!(strict.anywhere().placement(), TokenPlacement::Anywhere);
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TokenField {
    name: &'static str,
    placement: TokenPlacement,
}

impl TokenField {
    /// A token field named `name` that must come before the first file.
    #[must_use]
    pub const fn new(name: &'static str) -> TokenField {
        TokenField {
            name,
            placement: TokenPlacement::BeforeFirstFile,
        }
    }

    /// The same field, allowed anywhere in the body.
    #[must_use]
    pub const fn anywhere(self) -> TokenField {
        TokenField {
            name: self.name,
            placement: TokenPlacement::Anywhere,
        }
    }

    /// The field name, matched byte for byte.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        self.name
    }

    /// Where the field must be.
    #[must_use]
    pub const fn placement(&self) -> TokenPlacement {
        self.placement
    }
}

/// Where the token field must be.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TokenPlacement {
    /// Before the first file part (default of [`TokenField::new`]).
    BeforeFirstFile,
    /// Anywhere.
    Anywhere,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_limits_default_validates() {
        assert!(Limits::DEFAULT.validate().is_ok());
        assert_eq!(Limits::default(), Limits::DEFAULT);
    }

    #[test]
    fn test_limits_header_budget_below_64_refused() {
        for bytes in [0, 1, 63] {
            let l = Limits::DEFAULT.with_max_part_header_bytes(bytes);
            assert_eq!(
                l.validate(),
                Err(FormError::InvalidLimits {
                    limit: Limit::PartHeaderBytes
                })
            );
        }
        assert!(
            Limits::DEFAULT
                .with_max_part_header_bytes(64)
                .validate()
                .is_ok()
        );
    }

    #[test]
    fn test_limits_containing_limit_caps_contained() {
        let l = Limits::DEFAULT.with_max_body_bytes(4096);
        assert_eq!(l.text(), 4096);
        assert_eq!(l.value(), 4096);
        assert_eq!(l.file(), 4096);
        assert_eq!(l.name(), 256);
        assert_eq!(l.token(), 128);
        let l = l.with_max_value_bytes(10);
        assert_eq!((l.value(), l.token()), (10, 10));
        let l = Limits::DEFAULT.with_max_part_header_bytes(100);
        assert_eq!(l.filename(), 100);
        let l = Limits::DEFAULT.with_sniff_bytes(64).with_max_file_bytes(16);
        assert!(l.validate().is_ok());
    }

    #[test]
    fn test_limits_zero_token_with_token_field_refused() {
        let policy = Policy::DEFAULT.with_token_field(TokenField::new("t"));
        let l = Limits::DEFAULT.with_max_token_bytes(0);
        assert_eq!(
            l.validate_with(&policy),
            Err(FormError::InvalidLimits {
                limit: Limit::TokenLength
            })
        );
        assert!(l.validate_with(&Policy::DEFAULT).is_ok());
    }

    #[test]
    fn test_buffer_lengths_follow_policy() {
        let l = Limits::DEFAULT;
        assert_eq!(l.multipart_buffer_len(&Policy::DEFAULT), 8192 + 256 + 32);
        let p = Policy::DEFAULT
            .with_extended_filename(ExtendedFilenamePolicy::AcceptAlone)
            .with_token_field(TokenField::new("t"));
        assert_eq!(l.multipart_buffer_len(&p), 8192 + 256 + 1024 + 128 + 32);
        assert_eq!(l.urlencoded_buffer_len(), 256 + 1024 * 1024 + 1);
    }

    #[test]
    fn test_setters_set_one_field_each() {
        let l = Limits::DEFAULT
            .with_max_body_bytes(1)
            .with_max_fields(2)
            .with_max_name_bytes(3)
            .with_max_value_bytes(4)
            .with_max_text_bytes(5)
            .with_max_parts(6)
            .with_max_files(7)
            .with_max_file_bytes(8)
            .with_max_part_header_bytes(9)
            .with_max_part_headers(10)
            .with_max_filename_bytes(11)
            .with_max_preamble_bytes(12)
            .with_max_epilogue_bytes(13)
            .with_max_padding_bytes(14)
            .with_sniff_bytes(15)
            .with_max_token_bytes(16);
        assert_eq!(
            (
                l.max_body_bytes,
                l.max_fields,
                l.max_name_bytes,
                l.max_value_bytes
            ),
            (1, 2, 3, 4)
        );
        assert_eq!(
            (l.max_text_bytes, l.max_parts, l.max_files, l.max_file_bytes),
            (5, 6, 7, 8)
        );
        assert_eq!(
            (
                l.max_part_header_bytes,
                l.max_part_headers,
                l.max_filename_bytes,
                l.max_preamble_bytes
            ),
            (9, 10, 11, 12)
        );
        assert_eq!(
            (
                l.max_epilogue_bytes,
                l.max_padding_bytes,
                l.sniff_bytes,
                l.max_token_bytes
            ),
            (13, 14, 15, 16)
        );
    }

    #[test]
    fn test_policy_setters() {
        let p = Policy::DEFAULT
            .with_percent(PercentPolicy::Literal)
            .with_extended_filename(ExtendedFilenamePolicy::Ignore)
            .with_unknown_headers(HeaderPolicy::Refuse)
            .with_empty_files(EmptyFilePolicy::Skip)
            .with_token_field(TokenField::new("t").anywhere());
        assert_eq!(p.percent, PercentPolicy::Literal);
        assert_eq!(p.extended_filename, ExtendedFilenamePolicy::Ignore);
        assert_eq!(p.unknown_headers, HeaderPolicy::Refuse);
        assert_eq!(p.empty_files, EmptyFilePolicy::Skip);
        assert_eq!(
            p.token_field.map(|t| (t.name(), t.placement())),
            Some(("t", TokenPlacement::Anywhere))
        );
        assert_eq!(Policy::default(), Policy::DEFAULT);
    }
}
