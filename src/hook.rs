//! Hooks for other crates to take part in a parse without a second pass:
//! an [`Inspector`] sees every part before it is stored, and a [`Sink`]
//! decides where file data goes.
//!
//! Hooks are generic parameters, so a hook that does nothing
//! ([`NoInspector`], [`DiscardSink`]) compiles to nothing.

use crate::error::FormError;
use crate::multipart::parser::{PartHead, PartSummary};

/// Why an inspector refused the input: the HTTP status it wants and a
/// stable, machine-readable code for the reason.
///
/// [`FormError::status_hint`](crate::FormError::status_hint) returns the
/// status (400 when it is outside 400 to 599: a refusal is never a success);
/// [`FormError::code`](crate::FormError::code) returns the code. The code
/// must never contain input.
///
/// # Examples
///
/// ```
/// use form_net::Refusal;
///
/// let r = Refusal::new(415, "customs.avatar_not_image");
/// assert_eq!((r.status, r.code), (415, "customs.avatar_not_image"));
/// ```
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Refusal {
    /// The HTTP status the inspector wants.
    pub status: u16,
    /// The inspector's stable code for the reason.
    pub code: &'static str,
}

impl Refusal {
    /// A refusal with this status and code.
    #[must_use]
    pub const fn new(status: u16, code: &'static str) -> Refusal {
        Refusal { status, code }
    }
}

/// What an inspector says about what it saw.
///
/// # Examples
///
/// ```
/// use form_net::{Refusal, Verdict};
///
/// let v = Verdict::Refuse(Refusal::new(422, "example.not_allowed"));
/// assert!(matches!(v, Verdict::Refuse(r) if r.status == 422));
/// ```
#[must_use]
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    /// Go on.
    Continue,
    /// Discard the rest of this part (or this field); it still counts
    /// against the budgets and is left out of the result.
    Skip,
    /// Stop the parse with this refusal.
    Refuse(Refusal),
}

/// Takes part in a parse: called for every part head, the first bytes of
/// every file, every data chunk, every complete text field, every part end,
/// the end of the body, and once on any failure.
///
/// Every method has a default, so an inspector implements only what it
/// checks.
///
/// **Order guarantee**, per part and at every split of the input:
///
/// ```text
/// part_start ─▶ sniff (file parts, when sniff_bytes > 0) ─▶ data* ─▶ field (text fields) ─▶ part_end
/// ... after the last part: end
/// ... on any error, at any point: failed (once), then the error is returned
/// ```
///
/// For urlencoded bodies only `field`, `end`, and `failed` are called.
///
/// # Examples
///
/// ```
/// use form_net::{Inspector, PartHead, Refusal, Verdict};
///
/// /// Allows only PNG and JPEG avatars, judged by their bytes.
/// struct ImagesOnly;
///
/// impl Inspector for ImagesOnly {
///     fn part_start(&mut self, head: &PartHead<'_>) -> Verdict {
///         match (head.name(), head.is_file()) {
///             ("avatar", true) | ("title", false) => Verdict::Continue,
///             _ => Verdict::Refuse(Refusal::new(422, "example.unexpected_field")),
///         }
///     }
///     fn sniff(&mut self, _head: &PartHead<'_>, first: &[u8]) -> Verdict {
///         if first.starts_with(b"\x89PNG\r\n\x1a\n") || first.starts_with(b"\xff\xd8\xff") {
///             Verdict::Continue
///         } else {
///             Verdict::Refuse(Refusal::new(415, "example.avatar_not_image"))
///         }
///     }
/// }
/// ```
pub trait Inspector {
    /// A part's headers were read and passed every rule. Called before any
    /// byte of the part reaches a sink.
    fn part_start(&mut self, head: &PartHead<'_>) -> Verdict {
        let _ = head;
        Verdict::Continue
    }

    /// The first `min(sniff_bytes, part length)` bytes of a file part, as one
    /// contiguous slice however the input was split, before any `data` call
    /// for that part.
    fn sniff(&mut self, head: &PartHead<'_>, first: &[u8]) -> Verdict {
        let _ = (head, first);
        Verdict::Continue
    }

    /// Bytes of part `part`'s data, text fields included.
    fn data(&mut self, part: u32, chunk: &[u8]) -> Verdict {
        let _ = (part, chunk);
        Verdict::Continue
    }

    /// A complete text field (multipart) or a decoded pair (urlencoded): the
    /// value as bytes, so the inspector can apply its own charset rules.
    /// `Skip` leaves the field out of the result.
    fn field(&mut self, name: &str, value: &[u8]) -> Verdict {
        let _ = (name, value);
        Verdict::Continue
    }

    /// A part ended. `Skip` here is treated as `Continue`.
    fn part_end(&mut self, summary: &PartSummary) -> Verdict {
        let _ = summary;
        Verdict::Continue
    }

    /// The body ended. `Skip` here is treated as `Continue`.
    fn end(&mut self) -> Verdict {
        Verdict::Continue
    }

    /// The parse failed with `error` (for any reason, this inspector's
    /// refusal included). Called once; it cannot change the outcome. The
    /// hook for security signals: a stream of refusals from one client is a
    /// probe.
    fn failed(&mut self, error: &FormError) {
        let _ = error;
    }
}

impl<I: Inspector + ?Sized> Inspector for &mut I {
    fn part_start(&mut self, head: &PartHead<'_>) -> Verdict {
        (**self).part_start(head)
    }
    fn sniff(&mut self, head: &PartHead<'_>, first: &[u8]) -> Verdict {
        (**self).sniff(head, first)
    }
    fn data(&mut self, part: u32, chunk: &[u8]) -> Verdict {
        (**self).data(part, chunk)
    }
    fn field(&mut self, name: &str, value: &[u8]) -> Verdict {
        (**self).field(name, value)
    }
    fn part_end(&mut self, summary: &PartSummary) -> Verdict {
        (**self).part_end(summary)
    }
    fn end(&mut self) -> Verdict {
        (**self).end()
    }
    fn failed(&mut self, error: &FormError) {
        (**self).failed(error);
    }
}

/// Two inspectors in one pass, `self.0` first (D-088). A refusal from either
/// wins and the second is not asked; otherwise the part is skipped if either
/// says `Skip`. `failed` reaches both. Nest pairs for longer chains:
/// `(customs, (sentinel, app))`.
///
/// # Examples
///
/// ```
/// use form_net::{Inspector, Limits, Policy, Refusal, Verdict};
///
/// struct NoAdmin;
/// impl Inspector for NoAdmin {
///     fn field(&mut self, _name: &str, value: &[u8]) -> Verdict {
///         if value == b"admin" { Verdict::Refuse(Refusal::new(403, "app.admin")) } else { Verdict::Continue }
///     }
/// }
/// struct Count(u32);
/// impl Inspector for Count {
///     fn field(&mut self, _name: &str, _value: &[u8]) -> Verdict {
///         self.0 += 1;
///         Verdict::Continue
///     }
/// }
///
/// let mut chain = (Count(0), NoAdmin);
/// let r = form_net::parse_urlencoded_inspected(b"a=1&b=admin", &Limits::DEFAULT, &Policy::DEFAULT, &mut chain);
/// assert_eq!(r.unwrap_err().status_hint(), 403);
/// assert_eq!(chain.0 .0, 2);
/// ```
impl<A: Inspector, B: Inspector> Inspector for (A, B) {
    fn part_start(&mut self, head: &PartHead<'_>) -> Verdict {
        both(self.0.part_start(head), || self.1.part_start(head))
    }
    fn sniff(&mut self, head: &PartHead<'_>, first: &[u8]) -> Verdict {
        both(self.0.sniff(head, first), || self.1.sniff(head, first))
    }
    fn data(&mut self, part: u32, chunk: &[u8]) -> Verdict {
        both(self.0.data(part, chunk), || self.1.data(part, chunk))
    }
    fn field(&mut self, name: &str, value: &[u8]) -> Verdict {
        both(self.0.field(name, value), || self.1.field(name, value))
    }
    fn part_end(&mut self, summary: &PartSummary) -> Verdict {
        both(self.0.part_end(summary), || self.1.part_end(summary))
    }
    fn end(&mut self) -> Verdict {
        both(self.0.end(), || self.1.end())
    }
    fn failed(&mut self, error: &FormError) {
        self.0.failed(error);
        self.1.failed(error);
    }
}

/// Combines two verdicts: the first refusal wins, then any skip.
fn both(first: Verdict, second: impl FnOnce() -> Verdict) -> Verdict {
    if matches!(first, Verdict::Refuse(_)) {
        return first;
    }
    match second() {
        Verdict::Refuse(r) => Verdict::Refuse(r),
        Verdict::Skip => Verdict::Skip,
        _ => first,
    }
}

/// A boxed inspector, for a chain chosen at run time (D-088).
#[cfg(feature = "alloc")]
impl<I: Inspector + ?Sized> Inspector for alloc::boxed::Box<I> {
    fn part_start(&mut self, head: &PartHead<'_>) -> Verdict {
        (**self).part_start(head)
    }
    fn sniff(&mut self, head: &PartHead<'_>, first: &[u8]) -> Verdict {
        (**self).sniff(head, first)
    }
    fn data(&mut self, part: u32, chunk: &[u8]) -> Verdict {
        (**self).data(part, chunk)
    }
    fn field(&mut self, name: &str, value: &[u8]) -> Verdict {
        (**self).field(name, value)
    }
    fn part_end(&mut self, summary: &PartSummary) -> Verdict {
        (**self).part_end(summary)
    }
    fn end(&mut self) -> Verdict {
        (**self).end()
    }
    fn failed(&mut self, error: &FormError) {
        (**self).failed(error);
    }
}

/// An inspector that accepts everything; it compiles to nothing.
///
/// # Examples
///
/// ```
/// use form_net::{Limits, NoInspector, Policy};
///
/// let form = form_net::parse_urlencoded_inspected(b"a=1", &Limits::DEFAULT, &Policy::DEFAULT, &mut NoInspector)?;
/// assert_eq!(form.get("a"), Some("1"));
/// # Ok::<(), form_net::FormError>(())
/// ```
#[derive(Clone, Copy, Debug, Default)]
pub struct NoInspector;

impl Inspector for NoInspector {}

/// Where file parts go: one part at a time, `open`, any number of `write`,
/// then `close` or `abort`.
///
/// The [`Driver`](crate::Driver) opens a sink only after the inspector has
/// accepted the part's head and sniff window and the first chunk of data
/// (or at the end of an empty file), so a refused or skipped part never
/// reaches storage. Budgets are enforced before `write` is called, so a sink
/// never has to count. `abort` must release everything, cannot fail, and
/// must do nothing when no part is open; drivers call it on every error
/// path.
///
/// # Examples
///
/// ```
/// use form_net::{FormError, PartHead, Sink};
///
/// /// Counts bytes per file without keeping them.
/// struct Counter(u64);
///
/// impl Sink for Counter {
///     type Output = u64;
///     fn open(&mut self, _head: &PartHead<'_>) -> Result<(), FormError> {
///         self.0 = 0;
///         Ok(())
///     }
///     fn write(&mut self, chunk: &[u8]) -> Result<(), FormError> {
///         self.0 += chunk.len() as u64;
///         Ok(())
///     }
///     fn close(&mut self) -> Result<u64, FormError> {
///         Ok(self.0)
///     }
///     fn abort(&mut self) {}
/// }
/// ```
pub trait Sink {
    /// What a closed part becomes.
    type Output;

    /// A file part starts.
    ///
    /// # Errors
    ///
    /// The sink's own errors (storage, I/O).
    fn open(&mut self, head: &PartHead<'_>) -> Result<(), FormError>;

    /// More bytes of the current file part.
    ///
    /// # Errors
    ///
    /// The sink's own errors.
    fn write(&mut self, chunk: &[u8]) -> Result<(), FormError>;

    /// The current file part ended.
    ///
    /// # Errors
    ///
    /// The sink's own errors.
    fn close(&mut self) -> Result<Self::Output, FormError>;

    /// Discards the current file part, if any, and releases everything.
    fn abort(&mut self);
}

/// A sink that discards file data and reports how many bytes it saw.
///
/// # Examples
///
/// ```
/// use form_net::{DiscardSink, Sink};
///
/// let mut s = DiscardSink::default();
/// s.write(b"abc")?;
/// assert_eq!(s.close()?, 3);
/// # Ok::<(), form_net::FormError>(())
/// ```
#[derive(Clone, Copy, Debug, Default)]
pub struct DiscardSink {
    bytes: u64,
}

impl Sink for DiscardSink {
    type Output = u64;

    fn open(&mut self, _head: &PartHead<'_>) -> Result<(), FormError> {
        self.bytes = 0;
        Ok(())
    }

    fn write(&mut self, chunk: &[u8]) -> Result<(), FormError> {
        self.bytes = self.bytes.saturating_add(chunk.len() as u64);
        Ok(())
    }

    fn close(&mut self) -> Result<u64, FormError> {
        Ok(core::mem::take(&mut self.bytes))
    }

    fn abort(&mut self) {
        self.bytes = 0;
    }
}

#[cfg(feature = "alloc")]
mod memory {
    use alloc::string::String;
    use alloc::vec::Vec;

    use super::Sink;
    use crate::error::FormError;
    use crate::extract::FileData;
    use crate::multipart::parser::PartHead;

    /// A sink that keeps each file in memory as a [`FileData`].
    ///
    /// # Examples
    ///
    /// ```
    /// use form_net::{Driver, MemorySink, MultipartParser, Boundary, Limits, Policy, NoInspector, Progress};
    ///
    /// let parser = MultipartParser::new(Boundary::new(b"X")?, &Limits::DEFAULT, &Policy::DEFAULT)?;
    /// let mut driver = Driver::new(parser, MemorySink::new());
    /// let body = b"--X\r\nContent-Disposition: form-data; name=f; filename=a.txt\r\n\r\nhi\r\n--X--";
    /// let (_, progress) = driver.push(body, true, &mut NoInspector)?;
    /// match progress {
    ///     Progress::File(file) => assert_eq!(file.bytes(), b"hi"),
    ///     _ => unreachable!(),
    /// }
    /// # Ok::<(), form_net::FormError>(())
    /// ```
    #[derive(Debug, Default)]
    pub struct MemorySink {
        current: Option<FileData>,
    }

    impl MemorySink {
        /// An empty sink.
        #[must_use]
        pub fn new() -> MemorySink {
            MemorySink { current: None }
        }
    }

    impl Sink for MemorySink {
        type Output = FileData;

        fn open(&mut self, head: &PartHead<'_>) -> Result<(), FormError> {
            // Text only when it is text: a file name that is not UTF-8 is
            // reported as absent rather than repaired.
            let filename = head
                .filename()
                .and_then(|f| core::str::from_utf8(f.raw()).ok().map(String::from));
            let content_type = head
                .content_type()
                .and_then(|c| core::str::from_utf8(c.raw()).ok().map(String::from));
            self.current = Some(FileData::new(Vec::new(), filename, content_type));
            Ok(())
        }

        fn write(&mut self, chunk: &[u8]) -> Result<(), FormError> {
            if let Some(f) = self.current.as_mut() {
                f.append(chunk);
            }
            Ok(())
        }

        fn close(&mut self) -> Result<FileData, FormError> {
            Ok(self.current.take().unwrap_or_default())
        }

        fn abort(&mut self) {
            self.current = None;
        }
    }
}

#[cfg(feature = "alloc")]
pub use memory::MemorySink;
