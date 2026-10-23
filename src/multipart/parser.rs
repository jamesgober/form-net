//! The sans-IO multipart parser: a state machine over the delimiter matcher
//! and the header reader.

use core::fmt;

use crate::error::{EncodingError, FormError, Limit, Location, PolicyRefusal, SyntaxError};
use crate::limits::{CHARSET_CAPTURE, EmptyFilePolicy, Limits, Policy, TokenPlacement};
use crate::multipart::boundary::Boundary;
use crate::multipart::filename::FileName;
use crate::multipart::header::{Areas, BlockScan, HeadFacts, Text, read_block};

/// Whether a part is a text field or a file.
///
/// # Examples
///
/// ```
/// # let body = b"--X\r\nContent-Disposition: form-data; name=a\r\n\r\n1\r\n--X--";
/// let mp = form_net::parse_multipart(b"multipart/form-data; boundary=X", body)?;
/// assert_eq!(mp.parts()[0].kind(), form_net::PartKind::Field);
/// # Ok::<(), form_net::FormError>(())
/// ```
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PartKind {
    /// A text field: no `filename` parameter.
    Field,
    /// A file: a `filename` parameter (even an empty one) or an accepted
    /// `filename*`.
    File,
}

/// The `charset` a part declared.
///
/// `Unspecified` means no `charset` parameter: UTF-8 is assumed unless a
/// `_charset_` field before the part said otherwise. `Other` is any label
/// that is not UTF-8 or US-ASCII; its text is refused by every text accessor.
///
/// # Examples
///
/// ```
/// use form_net::Charset;
///
/// # let body = b"--X\r\nContent-Disposition: form-data; name=a\r\nContent-Type: text/plain; charset=UTF-8\r\n\r\n1\r\n--X--";
/// # let mp = form_net::parse_multipart(b"multipart/form-data; boundary=X", body)?;
/// assert_eq!(mp.parts()[0].content_type().unwrap().charset(), Charset::Utf8);
/// # Ok::<(), form_net::FormError>(())
/// ```
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Charset {
    /// `utf-8`.
    Utf8,
    /// `us-ascii`.
    Ascii,
    /// No `charset` parameter.
    Unspecified,
    /// Any other label.
    Other,
}

impl Charset {
    pub(crate) fn from_label(label: &[u8]) -> Charset {
        if label.eq_ignore_ascii_case(b"utf-8") {
            Charset::Utf8
        } else if label.eq_ignore_ascii_case(b"us-ascii") {
            Charset::Ascii
        } else {
            Charset::Other
        }
    }
}

/// One step of the parser: how many input bytes it consumed, and what
/// happened.
#[non_exhaustive]
#[derive(Debug)]
pub struct Step<'p> {
    /// Bytes of the input consumed; pass the rest (and later new bytes) to
    /// the next call.
    pub consumed: usize,
    /// What happened.
    pub event: Event<'p>,
}

/// What the parser found.
///
/// Per part the order is always `PartStart`, any number of `Data` (none for
/// an empty part), `PartEnd`. A `Data` slice is never empty. After the last
/// part comes `End`. `NeedInput` can appear anywhere between.
///
/// `Debug` prints the length of `Data`, never the bytes.
#[non_exhaustive]
pub enum Event<'p> {
    /// A part's headers were read and checked.
    PartStart(PartHead<'p>),
    /// Bytes of the current part's data.
    Data(&'p [u8]),
    /// The current part ended.
    PartEnd(PartSummary),
    /// The close delimiter was read: the form is complete.
    End,
    /// Every byte given was consumed; give more.
    NeedInput,
}

impl fmt::Debug for Event<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PartStart(head) => f.debug_tuple("PartStart").field(head).finish(),
            Self::Data(bytes) => write!(f, "Data({} bytes)", bytes.len()),
            Self::PartEnd(summary) => f.debug_tuple("PartEnd").field(summary).finish(),
            Self::End => f.write_str("End"),
            Self::NeedInput => f.write_str("NeedInput"),
        }
    }
}

/// What one part was.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PartSummary {
    /// The part index, from 0.
    pub index: u32,
    /// Field or file.
    pub kind: PartKind,
    /// The length of the part's data.
    pub bytes: u64,
    /// Whether the part was skipped (by `skip_part` or an inspector).
    pub skipped: bool,
}

/// The validated header block of one part, borrowed.
///
/// A head is only produced after every header budget and rule passed.
#[derive(Clone, Copy)]
pub struct PartHead<'p> {
    index: u32,
    block: &'p [u8],
    name: &'p str,
    name_raw: &'p [u8],
    kind: PartKind,
    filename: Option<FileName<'p>>,
    content_type: Option<&'p [u8]>,
    charset: Charset,
}

impl fmt::Debug for PartHead<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PartHead")
            .field("index", &self.index)
            .field("name", &self.name)
            .field("kind", &self.kind)
            .field("filename", &self.filename)
            .finish_non_exhaustive()
    }
}

impl<'p> PartHead<'p> {
    /// The part index, from 0, in body order.
    #[must_use]
    pub const fn index(&self) -> u32 {
        self.index
    }

    /// The field name, with `%0A`, `%0D`, `%22` decoded to LF, CR, `"` (no
    /// other escape is decoded). Always valid UTF-8 and never empty.
    #[must_use]
    pub const fn name(&self) -> &'p str {
        self.name
    }

    /// The field name as sent.
    #[must_use]
    pub const fn name_raw(&self) -> &'p [u8] {
        self.name_raw
    }

    /// Field or file.
    #[must_use]
    pub const fn kind(&self) -> PartKind {
        self.kind
    }

    /// Whether this is a file part.
    #[must_use]
    pub const fn is_file(&self) -> bool {
        matches!(self.kind, PartKind::File)
    }

    /// The client's file name, for file parts.
    #[must_use]
    pub const fn filename(&self) -> Option<FileName<'p>> {
        self.filename
    }

    /// The declared `Content-Type`: a hint, never trusted.
    #[must_use]
    pub fn content_type(&self) -> Option<ContentType<'p>> {
        self.content_type.and_then(ContentType::parse)
    }

    /// The part's own `charset` parameter (`Unspecified` when absent).
    #[must_use]
    pub const fn charset(&self) -> Charset {
        self.charset
    }

    /// Every header line, in order, as raw `(name, value)` bytes.
    #[must_use]
    pub fn headers(&self) -> Headers<'p> {
        Headers { rest: self.block }
    }

    /// The size of the header block, the final CRLF CRLF included.
    #[must_use]
    pub const fn header_bytes(&self) -> u32 {
        self.block.len() as u32
    }
}

/// A part's declared `Content-Type`, already checked. **A hint**: the client
/// chose it.
///
/// # Examples
///
/// ```
/// # let body = b"--X\r\nContent-Disposition: form-data; name=f; filename=a.png\r\nContent-Type: Image/PNG; q=\"x y\"\r\n\r\n\r\n--X--";
/// # let mp = form_net::parse_multipart(b"multipart/form-data; boundary=X", body)?;
/// let ct = mp.file("f").unwrap().content_type().unwrap();
/// assert!(ct.is("image", "png"));
/// assert_eq!((ct.type_(), ct.subtype()), ("Image", "PNG"));
/// assert_eq!(ct.parameter("Q"), Some(&b"x y"[..]));
/// # Ok::<(), form_net::FormError>(())
/// ```
#[derive(Clone, Copy)]
pub struct ContentType<'p> {
    raw: &'p [u8],
    media: parse_net::MediaType<'p>,
}

impl fmt::Debug for ContentType<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ContentType")
            .field("type", &self.type_())
            .field("subtype", &self.subtype())
            .finish_non_exhaustive()
    }
}

impl<'p> ContentType<'p> {
    pub(crate) fn parse(raw: &'p [u8]) -> Option<ContentType<'p>> {
        parse_net::media::parse(raw, &crate::kind::HEADER_LIMITS)
            .ok()
            .map(|media| ContentType { raw, media })
    }

    /// The value as sent.
    #[must_use]
    pub const fn raw(&self) -> &'p [u8] {
        self.raw
    }

    /// The type, as sent.
    #[must_use]
    pub fn type_(&self) -> &'p str {
        core::str::from_utf8(self.media.type_()).unwrap_or("")
    }

    /// The subtype, as sent.
    #[must_use]
    pub fn subtype(&self) -> &'p str {
        core::str::from_utf8(self.media.subtype()).unwrap_or("")
    }

    /// Whether the type and subtype are these, ASCII case-insensitively.
    #[must_use]
    pub fn is(&self, type_: &str, subtype: &str) -> bool {
        self.media.is(type_, subtype)
    }

    /// A parameter's value by name (ASCII case-insensitive), without
    /// surrounding quotes; backslashes are literal, as browsers send them.
    #[must_use]
    pub fn parameter(&self, name: &str) -> Option<&'p [u8]> {
        self.media
            .params()
            .get(name)
            .map(|w| crate::kind::word_raw(&w))
    }

    /// The `charset` parameter.
    #[must_use]
    pub fn charset(&self) -> Charset {
        self.parameter("charset")
            .map_or(Charset::Unspecified, Charset::from_label)
    }
}

/// Every header line of a part, in order, as raw `(name, value)` bytes with
/// the value trimmed.
///
/// `Debug` prints the header names only: values carry file names.
#[derive(Clone)]
pub struct Headers<'p> {
    rest: &'p [u8],
}

impl fmt::Debug for Headers<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut list = f.debug_list();
        for (name, _) in self.clone() {
            list.entry(&core::str::from_utf8(name).unwrap_or("?"));
        }
        list.finish()
    }
}

impl<'p> Iterator for Headers<'p> {
    type Item = (&'p [u8], &'p [u8]);

    fn next(&mut self) -> Option<Self::Item> {
        let lf = self.rest.iter().position(|&b| b == b'\n')?;
        let line = self.rest.get(..lf.saturating_sub(1)).unwrap_or(&[]);
        self.rest = self.rest.get(lf.wrapping_add(1)..).unwrap_or(&[]);
        if line.is_empty() {
            self.rest = &[];
            return None;
        }
        let colon = line.iter().position(|&b| b == b':').unwrap_or(line.len());
        let name = line.get(..colon).unwrap_or(&[]);
        let value = line.get(colon.wrapping_add(1)..).unwrap_or(&[]);
        Some((name, parse_net::text::trim_ows(value)))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum State {
    Preamble,
    /// After a delimiter: expecting `--`, padding, or CRLF.
    AfterDelimiter(After),
    Headers,
    Body,
    /// The delimiter that ends the current part was found; `PartEnd` is due.
    EndOfPart,
    Epilogue,
    Poisoned,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum After {
    Start,
    Dash,
    Padding(u8),
    Cr,
}

/// The token field's progress.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Token {
    NotSeen,
    Capturing,
    Captured,
}

const SKIPPING: u8 = 1;
const IS_TOKEN: u8 = 2;
const IS_CHARSET: u8 = 4;
const DEFERRED: u8 = 8;

/// Where a head's block is.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Source {
    /// In the input, from this index.
    Input(usize),
    /// In the parser buffer, from 0.
    Buffer,
}

/// What a call to `advance` produced, as positions only.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Outcome {
    PartStart(HeadFacts, Source),
    /// `input[start..end]`.
    DataInput(usize, usize),
    /// The first bytes of the delimiter.
    DataDelimiter(usize),
    PartEnd(PartSummary),
    End,
    NeedInput,
}

/// The sans-IO `multipart/form-data` parser.
///
/// Feed it bytes as they arrive, in chunks of any size split at any byte;
/// it returns one [`Event`] per call and how many bytes it consumed. The
/// events are identical for every split of the same body. It never
/// allocates after construction and never rescans a byte: the delimiter
/// search is a streaming Knuth-Morris-Pratt automaton with an eight-byte skip
/// to the next CR.
///
/// The buffer holds the header block, the decoded field name, the decoded
/// `filename*` (under `AcceptAlone`), the token, and the `_charset_` value:
/// [`Limits::multipart_buffer_len`] bytes. It is wiped on `reset` and on
/// drop.
///
/// # Examples
///
/// ```
/// use form_net::{Event, FormKind, Limits, MultipartParser, Policy};
///
/// let content_type = b"multipart/form-data; boundary=X";
/// let body = b"--X\r\nContent-Disposition: form-data; name=\"title\"\r\n\r\nHoliday\r\n--X--\r\n";
/// let FormKind::Multipart(boundary) = FormKind::detect(content_type)? else { unreachable!() };
/// let mut parser = MultipartParser::new(boundary, &Limits::DEFAULT, &Policy::DEFAULT)?;
///
/// let mut events = Vec::new();
/// let mut chunks = body.chunks(7).peekable();     // any split gives the same events
/// while let Some(chunk) = chunks.next() {
///     let last = chunks.peek().is_none();
///     let mut rest = chunk;
///     loop {
///         let step = parser.parse(rest, last)?;
///         rest = &rest[step.consumed..];
///         match step.event {
///             Event::PartStart(head) => events.push(format!("start {}", head.name())),
///             Event::Data(bytes) => events.push(format!("data {}", bytes.len())),
///             Event::PartEnd(summary) => events.push(format!("end {}", summary.bytes)),
///             Event::End => break,
///             Event::NeedInput => break,
///             _ => {}
///         }
///     }
/// }
/// assert!(parser.is_done());
/// assert_eq!(events.first().map(String::as_str), Some("start title"));
/// assert_eq!(events.last().map(String::as_str), Some("end 7"));
/// # Ok::<(), form_net::FormError>(())
/// ```
pub struct MultipartParser<B: AsMut<[u8]> + AsRef<[u8]>> {
    boundary: Boundary,
    limits: Limits,
    policy: Policy,
    buffer: B,
    state: State,
    /// Delimiter bytes matched and not yet known to be data.
    k: u8,
    flags: u8,
    token: Token,
    token_len: u16,
    charset_len: u8,
    charset_done: bool,
    scan: BlockScan,
    /// Header bytes copied into the buffer.
    copied: u32,
    parts: u32,
    files: u32,
    fields: u32,
    kind: PartKind,
    part_bytes: u64,
    text_total: u64,
    data_start: u64,
    preamble: u64,
    epilogue: u64,
    /// Where the current header block starts in the body.
    head_start: u64,
    /// Where the epilogue starts in the body.
    epilogue_start: u64,
    bytes_seen: u64,
}

impl<B: AsMut<[u8]> + AsRef<[u8]>> fmt::Debug for MultipartParser<B> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MultipartParser")
            .field("boundary", &self.boundary)
            .field("state", &self.state)
            .field("bytes_seen", &self.bytes_seen)
            .field("parts_seen", &self.parts)
            .finish_non_exhaustive()
    }
}

#[cfg(feature = "alloc")]
impl MultipartParser<alloc::vec::Vec<u8>> {
    /// A parser with its own buffer of
    /// [`Limits::multipart_buffer_len`] bytes, allocated once.
    ///
    /// # Errors
    ///
    /// `InvalidLimits` from validating the limits with the policy.
    ///
    /// # Examples
    ///
    /// ```
    /// use form_net::{Boundary, Limits, MultipartParser, Policy};
    ///
    /// let parser = MultipartParser::new(Boundary::new(b"X")?, &Limits::DEFAULT, &Policy::DEFAULT)?;
    /// assert_eq!(parser.bytes_seen(), 0);
    /// # Ok::<(), form_net::FormError>(())
    /// ```
    pub fn new(boundary: Boundary, limits: &Limits, policy: &Policy) -> Result<Self, FormError> {
        limits.validate_with(policy)?;
        let buffer = alloc::vec![0u8; limits.multipart_buffer_len(policy)];
        Self::with_buffer(boundary, limits, policy, buffer)
    }
}

impl<B: AsMut<[u8]> + AsRef<[u8]>> MultipartParser<B> {
    /// A parser over a buffer the caller provides; it never allocates.
    ///
    /// # Errors
    ///
    /// `InvalidLimits`; `BufferTooSmall` when `buffer` is shorter than
    /// [`Limits::multipart_buffer_len`].
    ///
    /// # Examples
    ///
    /// ```
    /// use form_net::{Boundary, Limits, MultipartParser, Policy};
    ///
    /// const LIMITS: Limits = Limits::DEFAULT.with_max_part_header_bytes(1024);
    /// const LEN: usize = LIMITS.multipart_buffer_len(&Policy::DEFAULT);
    /// let mut buffer = [0u8; LEN];
    /// let parser = MultipartParser::with_buffer(Boundary::new(b"X")?, &LIMITS, &Policy::DEFAULT, &mut buffer[..])?;
    /// # Ok::<(), form_net::FormError>(())
    /// ```
    pub fn with_buffer(
        boundary: Boundary,
        limits: &Limits,
        policy: &Policy,
        buffer: B,
    ) -> Result<Self, FormError> {
        limits.validate_with(policy)?;
        let needed = limits.multipart_buffer_len(policy);
        if buffer.as_ref().len() < needed {
            return Err(FormError::BufferTooSmall { needed });
        }
        Ok(Self {
            boundary,
            limits: *limits,
            policy: *policy,
            buffer,
            state: State::Preamble,
            // The body may start with `--boundary`: start as if CRLF had
            // been seen.
            k: 2,
            flags: 0,
            token: Token::NotSeen,
            token_len: 0,
            charset_len: 0,
            charset_done: false,
            scan: BlockScan::default(),
            copied: 0,
            parts: 0,
            files: 0,
            fields: 0,
            kind: PartKind::Field,
            part_bytes: 0,
            text_total: 0,
            data_start: 0,
            preamble: 0,
            epilogue: 0,
            head_start: 0,
            epilogue_start: 0,
            bytes_seen: 0,
        })
    }

    /// Consumes a prefix of `input` and returns one event. `eof = true` says
    /// `input` ends the body.
    ///
    /// # Errors
    ///
    /// Every multipart error (see the crate's `docs/API.md`); `Poisoned`
    /// after any error until [`reset`](Self::reset).
    pub fn parse<'p>(&'p mut self, input: &'p [u8], eof: bool) -> Result<Step<'p>, FormError> {
        let (consumed, outcome) = self.advance(input, eof)?;
        Ok(Step {
            consumed,
            event: self.view(outcome, input),
        })
    }

    /// Discards the rest of the current part: its bytes are consumed and
    /// counted against the budgets but produce no `Data` events, and its
    /// summary has `skipped = true`. Outside a part it does nothing.
    pub fn skip_part(&mut self) {
        if matches!(self.state, State::Body) {
            self.flags |= SKIPPING;
        }
    }

    /// Returns the parser to its initial state for a new body, wipes its
    /// buffer, and clears the poisoned state. It never allocates.
    pub fn reset(&mut self, boundary: Boundary) {
        crate::wipe::wipe(self.buffer.as_mut());
        let limits = self.limits;
        let policy = self.policy;
        self.boundary = boundary;
        self.state = State::Preamble;
        self.k = 2;
        self.flags = 0;
        self.token = Token::NotSeen;
        self.token_len = 0;
        self.charset_len = 0;
        self.charset_done = false;
        self.scan = BlockScan::default();
        self.copied = 0;
        self.parts = 0;
        self.files = 0;
        self.fields = 0;
        self.kind = PartKind::Field;
        self.part_bytes = 0;
        self.text_total = 0;
        self.data_start = 0;
        self.preamble = 0;
        self.epilogue = 0;
        self.head_start = 0;
        self.epilogue_start = 0;
        self.bytes_seen = 0;
        self.limits = limits;
        self.policy = policy;
    }

    /// Body bytes consumed so far.
    #[must_use]
    pub const fn bytes_seen(&self) -> u64 {
        self.bytes_seen
    }

    /// Parts started so far.
    #[must_use]
    pub const fn parts_seen(&self) -> u32 {
        self.parts
    }

    /// File parts counted so far.
    #[must_use]
    pub const fn files_seen(&self) -> u32 {
        self.files
    }

    /// Whether the close delimiter was read.
    #[must_use]
    pub fn is_done(&self) -> bool {
        self.state == State::Epilogue
    }

    /// The limits the parser was built with.
    #[must_use]
    pub const fn limits(&self) -> &Limits {
        &self.limits
    }

    /// The policy the parser was built with.
    #[must_use]
    pub const fn policy(&self) -> &Policy {
        &self.policy
    }

    /// The token field's value, once its part has ended.
    #[must_use]
    pub fn captured_token(&self) -> Option<&str> {
        if self.token != Token::Captured {
            return None;
        }
        let start = self.token_area();
        let bytes = self
            .buffer
            .as_ref()
            .get(start..start.saturating_add(usize::from(self.token_len)))?;
        core::str::from_utf8(bytes).ok()
    }

    /// The value of the last complete `_charset_` field.
    #[must_use]
    pub fn declared_charset(&self) -> Option<&[u8]> {
        if !self.charset_done {
            return None;
        }
        let start = self.charset_area();
        self.buffer
            .as_ref()
            .get(start..start.saturating_add(usize::from(self.charset_len)))
    }

    /// The decoded name of the last head, `len` bytes in the name area.
    #[cfg(feature = "alloc")]
    pub(crate) fn area_name(&self, len: u32) -> &str {
        let start = self.name_area();
        let bytes = self
            .buffer
            .as_ref()
            .get(start..start.saturating_add(len as usize))
            .unwrap_or(&[]);
        core::str::from_utf8(bytes).unwrap_or("")
    }

    /// The decoded `filename*` of the last head, `len` bytes.
    #[cfg(feature = "alloc")]
    pub(crate) fn area_filename(&self, len: u32) -> &[u8] {
        let start = self.filename_area();
        self.buffer
            .as_ref()
            .get(start..start.saturating_add(len as usize))
            .unwrap_or(&[])
    }

    /// Where the current part's data starts in the body.
    #[cfg(feature = "alloc")]
    pub(crate) const fn data_start(&self) -> u64 {
        self.data_start
    }

    // Buffer layout: header block, name, filename (AcceptAlone), token, charset.

    fn name_area(&self) -> usize {
        self.limits.max_part_header_bytes as usize
    }

    fn filename_area(&self) -> usize {
        self.name_area().saturating_add(self.limits.name() as usize)
    }

    fn token_area(&self) -> usize {
        let filename = if matches!(
            self.policy.extended_filename,
            crate::ExtendedFilenamePolicy::AcceptAlone
        ) {
            self.limits.filename() as usize
        } else {
            0
        };
        self.filename_area().saturating_add(filename)
    }

    fn charset_area(&self) -> usize {
        let token = if self.policy.token_field.is_some() {
            usize::from(self.limits.token())
        } else {
            0
        };
        self.token_area().saturating_add(token)
    }

    /// The location of the current part's header block.
    fn head_at(&self) -> Location {
        Location::in_part(self.parts.checked_sub(1), self.head_start)
    }

    /// The location of the first data byte past a budget of `limit` of
    /// which `used` is spent, in the current part.
    fn excess(&self, used: u64, limit: u64) -> Location {
        let into_part = self.part_bytes.saturating_add(limit.saturating_sub(used));
        Location::in_part(
            self.parts.checked_sub(1),
            self.data_start.saturating_add(into_part),
        )
    }

    fn at(&self, pos: usize) -> Location {
        let part = if matches!(self.state, State::Preamble | State::Epilogue) {
            None
        } else {
            self.parts.checked_sub(1)
        };
        Location::in_part(part, self.bytes_seen.saturating_add(pos as u64))
    }

    /// Runs the state machine and returns positions only, so callers that
    /// own the input for longer (the buffered parser) can slice it
    /// themselves.
    pub(crate) fn advance(
        &mut self,
        input: &[u8],
        eof: bool,
    ) -> Result<(usize, Outcome), FormError> {
        if self.state == State::Poisoned {
            return Err(FormError::Poisoned);
        }
        match self.run(input, eof) {
            Ok((consumed, outcome)) => {
                self.bytes_seen = self.bytes_seen.saturating_add(consumed as u64);
                Ok((consumed, outcome))
            }
            Err(e) => {
                self.state = State::Poisoned;
                Err(e)
            }
        }
    }

    fn run(&mut self, input: &[u8], eof: bool) -> Result<(usize, Outcome), FormError> {
        if self.bytes_seen.saturating_add(input.len() as u64) > self.limits.body() {
            return Err(FormError::limit(
                Limit::Body,
                Location::at(self.limits.body()),
            ));
        }
        let mut pos = 0usize;
        loop {
            let hay = input.get(pos..).unwrap_or(&[]);
            match self.state {
                State::Poisoned => return Err(FormError::Poisoned),
                State::Preamble => {
                    if let Some(after) = self.preamble_step(hay)? {
                        pos = pos.wrapping_add(after);
                        self.state = State::AfterDelimiter(After::Start);
                        continue;
                    }
                    if eof {
                        return Err(FormError::syntax(
                            SyntaxError::NoInitialDelimiter,
                            self.at(input.len()),
                        ));
                    }
                    return Ok((input.len(), Outcome::NeedInput));
                }
                State::AfterDelimiter(sub) => {
                    let Some(&c) = hay.first() else {
                        return self.need_input(input.len(), eof);
                    };
                    pos = pos.wrapping_add(1);
                    let at = self.at(pos);
                    self.state = match (sub, c) {
                        (After::Start, b'-') => State::AfterDelimiter(After::Dash),
                        (After::Dash, b'-') => {
                            self.state = State::Epilogue;
                            self.epilogue_start = self.bytes_seen.saturating_add(pos as u64);
                            return Ok((pos, Outcome::End));
                        }
                        (After::Start, b' ' | b'\t') => {
                            if self.limits.max_padding_bytes == 0 {
                                return Err(FormError::limit(Limit::Padding, at));
                            }
                            State::AfterDelimiter(After::Padding(1))
                        }
                        (After::Padding(n), b' ' | b'\t') => {
                            if n >= self.limits.max_padding_bytes {
                                return Err(FormError::limit(Limit::Padding, at));
                            }
                            State::AfterDelimiter(After::Padding(n.saturating_add(1)))
                        }
                        (After::Start | After::Padding(_), b'\r') => {
                            State::AfterDelimiter(After::Cr)
                        }
                        (After::Cr, b'\n') => {
                            self.parts = self.parts.saturating_add(1);
                            if self.parts > self.limits.max_parts {
                                return Err(FormError::limit(Limit::Parts, self.at(pos)));
                            }
                            self.scan = BlockScan::default();
                            self.copied = 0;
                            self.head_start = self.bytes_seen.saturating_add(pos as u64);
                            State::Headers
                        }
                        (After::Start | After::Padding(_), b'\n') => {
                            return Err(FormError::syntax(SyntaxError::BareLineFeed, at));
                        }
                        _ => return Err(FormError::syntax(SyntaxError::BoundaryInContent, at)),
                    };
                }
                State::Headers => {
                    let at = self.head_at();
                    match self.scan.feed(hay, &self.limits, at)? {
                        Some(end) => {
                            let source = if self.copied == 0 {
                                Source::Input(pos)
                            } else {
                                self.copy_header(hay.get(..end).unwrap_or(&[]), at)?;
                                Source::Buffer
                            };
                            let block_start_in_input = pos;
                            pos = pos.wrapping_add(end);
                            let facts =
                                self.read_head(input, source, block_start_in_input, end, at)?;
                            let (is_token, is_charset) = {
                                let block = self.block(&facts, source, input);
                                let name = self.name_bytes(&facts, block);
                                let token = self
                                    .policy
                                    .token_field
                                    .is_some_and(|t| name == t.name().as_bytes());
                                (token, name == b"_charset_")
                            };
                            self.start_part(&facts, is_token, is_charset, at)?;
                            self.data_start = self.bytes_seen.saturating_add(pos as u64);
                            return Ok((pos, Outcome::PartStart(facts, source)));
                        }
                        None => {
                            self.copy_header(hay, at)?;
                            return self.need_input(input.len(), eof);
                        }
                    }
                }
                State::Body => {
                    if hay.is_empty() {
                        return self.need_input(input.len(), eof);
                    }
                    match self.body_step(hay, pos, input.len(), eof)? {
                        BodyStep::Emit(consumed, outcome) => return Ok((consumed, outcome)),
                        BodyStep::Continue(next) => pos = next,
                    }
                }
                State::EndOfPart => {
                    let summary = self.end_part(self.at(pos))?;
                    self.state = State::AfterDelimiter(After::Start);
                    return Ok((pos, Outcome::PartEnd(summary)));
                }
                State::Epilogue => {
                    self.epilogue = self.epilogue.saturating_add(hay.len() as u64);
                    if self.epilogue > self.limits.epilogue() {
                        let at = self.epilogue_start.saturating_add(self.limits.epilogue());
                        return Err(FormError::limit(Limit::Epilogue, Location::at(at)));
                    }
                    return Ok((input.len(), Outcome::End));
                }
            }
        }
    }

    fn need_input(&self, len: usize, eof: bool) -> Result<(usize, Outcome), FormError> {
        if eof {
            Err(FormError::syntax(SyntaxError::Truncated, self.at(len)))
        } else {
            Ok((len, Outcome::NeedInput))
        }
    }

    /// Scans the preamble. Returns the index in `hay` just after the first
    /// delimiter, or `None` when all of `hay` was consumed.
    fn preamble_step(&mut self, hay: &[u8]) -> Result<Option<usize>, FormError> {
        let len = self.boundary.len() as u64;
        // A delimiter must end within max_preamble_bytes + its own length.
        let budget = self.limits.preamble().saturating_add(len);
        let room = usize::try_from(budget.saturating_sub(self.preamble)).unwrap_or(usize::MAX);
        let window = hay.get(..room.min(hay.len())).unwrap_or(hay);
        let (k, found) = self.boundary.run(usize::from(self.k), window);
        self.k = k as u8;
        match found {
            Some(after) => {
                // Real bytes up to the end of the delimiter; a delimiter that
                // includes the two virtual bytes (CRLF) starts the body.
                let end = self.preamble.saturating_add(after as u64);
                let preamble = end.saturating_sub(len);
                if preamble > self.limits.preamble() {
                    return Err(FormError::limit(
                        Limit::Preamble,
                        Location::at(self.limits.preamble()),
                    ));
                }
                self.k = 0;
                Ok(Some(after))
            }
            None => {
                self.preamble = self.preamble.saturating_add(window.len() as u64);
                if window.len() < hay.len() || self.preamble >= budget {
                    return Err(FormError::limit(
                        Limit::Preamble,
                        Location::at(self.limits.preamble()),
                    ));
                }
                Ok(None)
            }
        }
    }

    fn copy_header(&mut self, bytes: &[u8], at: Location) -> Result<(), FormError> {
        let start = self.copied as usize;
        let end = start.saturating_add(bytes.len());
        if end > self.limits.max_part_header_bytes as usize {
            return Err(FormError::limit(Limit::PartHeaderBytes, at));
        }
        match self.buffer.as_mut().get_mut(start..end) {
            Some(dst) => dst.copy_from_slice(bytes),
            None => return Err(FormError::limit(Limit::PartHeaderBytes, at)),
        }
        self.copied = end as u32;
        Ok(())
    }

    fn read_head(
        &mut self,
        input: &[u8],
        source: Source,
        start: usize,
        end: usize,
        at: Location,
    ) -> Result<HeadFacts, FormError> {
        let name_area = self.name_area();
        let filename_area = self.filename_area();
        let filename_end = self.token_area();
        let limits = self.limits;
        let policy = self.policy;
        let copied = self.copied as usize;
        let buf = self.buffer.as_mut();
        let (head, rest) = buf.split_at_mut(name_area.min(buf.len()));
        let (name, rest) =
            rest.split_at_mut(filename_area.saturating_sub(name_area).min(rest.len()));
        let filename = rest
            .get_mut(..filename_end.saturating_sub(filename_area))
            .unwrap_or(&mut []);
        let block = match source {
            Source::Input(_) => input.get(start..start.saturating_add(end)).unwrap_or(&[]),
            Source::Buffer => head.get(..copied).unwrap_or(&[]),
        };
        read_block(block, &limits, &policy, Areas { name, filename }, at)
    }

    /// The budgets and rules that apply when a part starts.
    fn start_part(
        &mut self,
        facts: &HeadFacts,
        is_token: bool,
        is_charset: bool,
        at: Location,
    ) -> Result<(), FormError> {
        self.state = State::Body;
        self.k = 0;
        self.flags = 0;
        self.kind = facts.kind;
        self.part_bytes = 0;
        match facts.kind {
            PartKind::Field => {
                self.fields = self.fields.saturating_add(1);
                if self.fields > self.limits.max_fields {
                    return Err(FormError::limit(Limit::Fields, at));
                }
                if is_token {
                    if self.token != Token::NotSeen {
                        return Err(FormError::policy(PolicyRefusal::DuplicateToken, at));
                    }
                    self.token = Token::Capturing;
                    self.token_len = 0;
                    self.flags |= IS_TOKEN;
                }
                if is_charset {
                    self.flags |= IS_CHARSET;
                    self.charset_len = 0;
                    self.charset_done = false;
                }
            }
            PartKind::File => {
                let empty = matches!(facts.filename, Some((Text::Block(span), _)) if span.len == 0)
                    || matches!(facts.filename, Some((Text::Area(0), _)));
                if empty && self.policy.empty_files == EmptyFilePolicy::Skip {
                    self.flags |= DEFERRED;
                } else {
                    self.count_file(at)?;
                }
            }
        }
        Ok(())
    }

    /// The block of a head, wherever it is.
    fn block<'a>(&'a self, facts: &HeadFacts, source: Source, input: &'a [u8]) -> &'a [u8] {
        let len = facts.block_len as usize;
        match source {
            Source::Input(start) => input.get(start..start.saturating_add(len)).unwrap_or(&[]),
            Source::Buffer => self.buffer.as_ref().get(..len).unwrap_or(&[]),
        }
    }

    /// The decoded name of a head.
    fn name_bytes<'a>(&'a self, facts: &HeadFacts, block: &'a [u8]) -> &'a [u8] {
        match facts.name {
            Text::Block(span) => span.get(block),
            Text::Area(len) => {
                let start = self.name_area();
                self.buffer
                    .as_ref()
                    .get(start..start.saturating_add(len as usize))
                    .unwrap_or(&[])
            }
        }
    }

    /// The borrowed view of a head.
    fn head_view<'p>(&'p self, facts: &HeadFacts, source: Source, input: &'p [u8]) -> PartHead<'p> {
        let block = self.block(facts, source, input);
        let name = core::str::from_utf8(self.name_bytes(facts, block)).unwrap_or("");
        let filename = facts.filename.map(|(text, origin)| {
            let raw = match text {
                Text::Block(span) => span.get(block),
                Text::Area(len) => {
                    let start = self.filename_area();
                    self.buffer
                        .as_ref()
                        .get(start..start.saturating_add(len as usize))
                        .unwrap_or(&[])
                }
            };
            FileName::new(raw, origin)
        });
        PartHead {
            index: self.parts.saturating_sub(1),
            block,
            name,
            name_raw: facts.name_raw.get(block),
            kind: facts.kind,
            filename,
            content_type: facts.content_type.map(|span| span.get(block)),
            charset: facts.charset,
        }
    }

    /// Turns positions into borrowed events.
    fn view<'p>(&'p self, outcome: Outcome, input: &'p [u8]) -> Event<'p> {
        match outcome {
            Outcome::PartStart(facts, source) => {
                Event::PartStart(self.head_view(&facts, source, input))
            }
            Outcome::DataInput(start, end) => Event::Data(input.get(start..end).unwrap_or(&[])),
            Outcome::DataDelimiter(n) => {
                Event::Data(self.boundary.delimiter().get(..n).unwrap_or(&[]))
            }
            Outcome::PartEnd(summary) => Event::PartEnd(summary),
            Outcome::End => Event::End,
            Outcome::NeedInput => Event::NeedInput,
        }
    }

    fn count_file(&mut self, at: Location) -> Result<(), FormError> {
        self.files = self.files.saturating_add(1);
        if self.files > self.limits.max_files {
            return Err(FormError::limit(Limit::Files, at));
        }
        if let Some(t) = self.policy.token_field {
            if t.placement() == TokenPlacement::BeforeFirstFile && self.token != Token::Captured {
                return Err(FormError::policy(PolicyRefusal::TokenFieldLate, at));
            }
        }
        Ok(())
    }

    /// Checks budgets for `n` more data bytes of the current part, and
    /// captures the token or `_charset_` value.
    fn take_data(&mut self, bytes: &[u8]) -> Result<(), FormError> {
        let n = bytes.len() as u64;
        if n == 0 {
            return Ok(());
        }
        if self.flags & DEFERRED != 0 {
            self.flags &= !DEFERRED;
            let at = Location::in_part(self.parts.checked_sub(1), self.data_start);
            self.count_file(at)?;
        }
        let total = self.part_bytes.saturating_add(n);
        match self.kind {
            PartKind::File => {
                let limit = self.limits.file();
                if total > limit {
                    let at = self.excess(self.part_bytes, limit);
                    return Err(FormError::limit(Limit::FileSize, at));
                }
            }
            PartKind::Field => {
                let limit = u64::from(self.limits.value());
                if total > limit {
                    let at = self.excess(self.part_bytes, limit);
                    return Err(FormError::limit(Limit::ValueLength, at));
                }
                let text = self.text_total.saturating_add(n);
                let limit = self.limits.text();
                if text > limit {
                    let at = self.excess(self.text_total, limit);
                    return Err(FormError::limit(Limit::TextTotal, at));
                }
                self.text_total = text;
            }
        }
        if self.flags & IS_TOKEN != 0 {
            let start = self
                .token_area()
                .saturating_add(usize::from(self.token_len));
            let end = start.saturating_add(bytes.len());
            let limit = u64::from(self.limits.token());
            let at = self.excess(u64::from(self.token_len), limit);
            if end
                > self
                    .token_area()
                    .saturating_add(usize::from(self.limits.token()))
            {
                return Err(FormError::limit(Limit::TokenLength, at));
            }
            match self.buffer.as_mut().get_mut(start..end) {
                Some(dst) => dst.copy_from_slice(bytes),
                None => return Err(FormError::limit(Limit::TokenLength, at)),
            }
            self.token_len = self.token_len.saturating_add(bytes.len() as u16);
        }
        if self.flags & IS_CHARSET != 0 {
            let start = self
                .charset_area()
                .saturating_add(usize::from(self.charset_len));
            let end = start.saturating_add(bytes.len());
            let at = self.excess(u64::from(self.charset_len), CHARSET_CAPTURE as u64);
            if end > self.charset_area().saturating_add(CHARSET_CAPTURE) {
                return Err(FormError::encoding(EncodingError::UnsupportedCharset, at));
            }
            match self.buffer.as_mut().get_mut(start..end) {
                Some(dst) => dst.copy_from_slice(bytes),
                None => return Err(FormError::encoding(EncodingError::UnsupportedCharset, at)),
            }
            self.charset_len = self.charset_len.saturating_add(bytes.len() as u8);
        }
        self.part_bytes = total;
        Ok(())
    }

    fn end_part(&mut self, at: Location) -> Result<PartSummary, FormError> {
        if self.flags & IS_TOKEN != 0 {
            let start = self.token_area();
            let bytes = self
                .buffer
                .as_ref()
                .get(start..start.saturating_add(usize::from(self.token_len)))
                .unwrap_or(&[]);
            if core::str::from_utf8(bytes).is_err() {
                return Err(FormError::encoding(EncodingError::InvalidUtf8, at));
            }
            self.token = Token::Captured;
        }
        if self.flags & IS_CHARSET != 0 {
            self.charset_done = true;
        }
        let summary = PartSummary {
            index: self.parts.saturating_sub(1),
            kind: self.kind,
            bytes: self.part_bytes,
            skipped: self.flags & SKIPPING != 0,
        };
        self.flags = 0;
        self.k = 0;
        Ok(summary)
    }

    /// One step in a part's data. `pos` is where `hay` starts in the input.
    fn body_step(
        &mut self,
        hay: &[u8],
        pos: usize,
        input_len: usize,
        eof: bool,
    ) -> Result<BodyStep, FormError> {
        let len = self.boundary.len();
        let k0 = usize::from(self.k);
        if k0 > 0 {
            // Bytes from earlier chunks that matched the start of the
            // delimiter: resolve them before anything else.
            let mut k = k0;
            for (i, &c) in hay.iter().enumerate() {
                k = self.boundary.step(k, c);
                let n = i.wrapping_add(1);
                if k == len {
                    // The delimiter started inside the held bytes.
                    let data = k0.saturating_add(n).saturating_sub(len);
                    self.k = 0;
                    self.state = State::EndOfPart;
                    return self.emit_delimiter_bytes(data, pos.wrapping_add(n));
                }
                if k <= n {
                    // Every held byte is data; scan the input again from 0.
                    self.k = 0;
                    return self.emit_delimiter_bytes(k0, pos);
                }
            }
            // All of `hay` extended the partial match.
            let n = hay.len();
            if eof {
                return Err(FormError::syntax(
                    SyntaxError::Truncated,
                    self.at(input_len),
                ));
            }
            let data = k0.saturating_add(n).saturating_sub(k);
            self.k = k as u8;
            return self.emit_delimiter_bytes(data, input_len);
        }
        let (k, found) = self.boundary.run(0, hay);
        match found {
            Some(after) => {
                let data = after.saturating_sub(len);
                self.k = 0;
                self.state = State::EndOfPart;
                let bytes = hay.get(..data).unwrap_or(&[]);
                self.emit_input_bytes(bytes, pos, pos.wrapping_add(data), pos.wrapping_add(after))
            }
            None => {
                if eof {
                    return Err(FormError::syntax(
                        SyntaxError::Truncated,
                        self.at(input_len),
                    ));
                }
                self.k = k as u8;
                let data_end = hay.len().saturating_sub(k);
                let bytes = hay.get(..data_end).unwrap_or(&[]);
                self.emit_input_bytes(bytes, pos, pos.wrapping_add(data_end), input_len)
            }
        }
    }

    /// Emits `delimiter[..n]` as data (or skips it), consuming up to
    /// `consumed`.
    fn emit_delimiter_bytes(&mut self, n: usize, consumed: usize) -> Result<BodyStep, FormError> {
        let boundary = self.boundary;
        let bytes = boundary.delimiter().get(..n).unwrap_or(&[]);
        self.take_data(bytes)?;
        if n == 0 || self.flags & SKIPPING != 0 {
            return Ok(BodyStep::Continue(consumed));
        }
        Ok(BodyStep::Emit(consumed, Outcome::DataDelimiter(n)))
    }

    /// Emits `input[start..end]` as data (or skips it), consuming up to
    /// `consumed`.
    fn emit_input_bytes(
        &mut self,
        bytes: &[u8],
        start: usize,
        end: usize,
        consumed: usize,
    ) -> Result<BodyStep, FormError> {
        self.take_data(bytes)?;
        if start == end || self.flags & SKIPPING != 0 {
            return Ok(BodyStep::Continue(consumed));
        }
        Ok(BodyStep::Emit(consumed, Outcome::DataInput(start, end)))
    }
}

/// What a body step did.
enum BodyStep {
    Emit(usize, Outcome),
    Continue(usize),
}

impl<B: AsMut<[u8]> + AsRef<[u8]>> Drop for MultipartParser<B> {
    fn drop(&mut self) {
        crate::wipe::wipe(self.buffer.as_mut());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parser_size_is_bounded() {
        // NFR-18 as measured: the boundary and its matcher table are 149 of
        // these bytes; the buffer is separate.
        assert!(core::mem::size_of::<MultipartParser<&mut [u8]>>() <= 384);
        assert!(core::mem::size_of::<Boundary>() <= 150);
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn test_debug_never_shows_data_or_file_names() {
        let body = b"--X\r\nContent-Disposition: form-data; name=f; filename=\"secret.pdf\"\r\n\r\nhunter2\r\n--X--";
        let mut p = MultipartParser::new(
            Boundary::new(b"X").unwrap(),
            &Limits::DEFAULT,
            &Policy::DEFAULT,
        )
        .unwrap();
        let mut rest: &[u8] = body;
        let mut seen = alloc::string::String::new();
        loop {
            let step = p.parse(rest, true).unwrap();
            rest = &rest[step.consumed..];
            if let Event::PartStart(head) = &step.event {
                seen.push_str(&alloc::format!("{:?}", head.headers()));
            }
            seen.push_str(&alloc::format!("{step:?}"));
            if matches!(step.event, Event::End) {
                break;
            }
        }
        assert!(seen.contains("Data(7 bytes)"), "{seen}");
        assert!(
            !seen.contains("hunter2") && !seen.contains("secret"),
            "{seen}"
        );
    }

    #[test]
    fn test_charset_labels() {
        assert_eq!(Charset::from_label(b"UTF-8"), Charset::Utf8);
        assert_eq!(Charset::from_label(b"us-ascii"), Charset::Ascii);
        assert_eq!(Charset::from_label(b"utf8"), Charset::Other);
    }
}
