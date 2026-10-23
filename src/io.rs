//! Blocking readers: a pull-style API over `std::io::Read`, free of any
//! runtime. Ask for the next part, read it like a file, ask for the next.

use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;
use std::io::{self, Read};

use crate::error::{EncodingError, FormError, Location};
use crate::limits::{Limits, Policy};
use crate::multipart::owned_head::OwnedPartHead;
use crate::multipart::parser::{Charset, Event, MultipartParser, PartSummary};

const READ_BUFFER: usize = 64 * 1024;

/// Reads a multipart body from a blocking reader, one part at a time.
///
/// It has no hooks: the caller sees each part itself. Dropping a
/// [`PartReader`] before its end skips the rest of the part at the next
/// [`next_part`](Self::next_part).
///
/// # Examples
///
/// ```
/// use std::io::Read;
///
/// let body = b"--X\r\nContent-Disposition: form-data; name=note\r\n\r\nhi\r\n--X\r\nContent-Disposition: form-data; name=f; filename=a.bin\r\n\r\n0123\r\n--X--";
/// let mut reader = form_net::MultipartReader::new(b"multipart/form-data; boundary=X", &body[..])?;
/// let mut seen = Vec::new();
/// while let Some(mut part) = reader.next_part()? {
///     if part.head().view().is_file() {
///         let mut bytes = Vec::new();
///         part.read_to_end(&mut bytes).unwrap();
///         seen.push(bytes.len());
///     } else {
///         seen.push(part.text()?.len());
///     }
/// }
/// assert_eq!(seen, [2, 4]);
/// # Ok::<(), form_net::FormError>(())
/// ```
pub struct MultipartReader<R: Read> {
    reader: R,
    parser: MultipartParser<Vec<u8>>,
    buf: Vec<u8>,
    start: usize,
    end: usize,
    eof: bool,
    head: OwnedPartHead,
    pending: Vec<u8>,
    pending_at: usize,
    in_part: bool,
    last: Option<PartSummary>,
    done: bool,
    declared: Option<Vec<u8>>,
    /// Bytes `save` has put on disk for this request.
    #[cfg(feature = "fs")]
    disk_used: u64,
}

impl<R: Read> Drop for MultipartReader<R> {
    fn drop(&mut self) {
        crate::wipe::wipe_vec(&mut self.buf);
        crate::wipe::wipe_vec(&mut self.pending);
        if let Some(d) = self.declared.as_mut() {
            crate::wipe::wipe_vec(d);
        }
    }
}

impl<R: Read> fmt::Debug for MultipartReader<R> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MultipartReader")
            .field("parser", &self.parser)
            .field("in_part", &self.in_part)
            .finish_non_exhaustive()
    }
}

/// What the reader's loop produced.
enum Got {
    Start,
    Data,
    End(PartSummary),
    Done,
}

impl<R: Read> MultipartReader<R> {
    /// A reader with the default limits and policy.
    ///
    /// # Errors
    ///
    /// `UnsupportedMediaType` and the boundary errors.
    pub fn new(content_type: &[u8], reader: R) -> Result<Self, FormError> {
        Self::with_limits(content_type, reader, &Limits::DEFAULT, &Policy::DEFAULT)
    }

    /// A reader with explicit limits and policy.
    ///
    /// # Errors
    ///
    /// As [`new`](Self::new); `InvalidLimits`.
    pub fn with_limits(
        content_type: &[u8],
        reader: R,
        limits: &Limits,
        policy: &Policy,
    ) -> Result<Self, FormError> {
        let boundary = crate::Boundary::parse(content_type)?;
        let parser = MultipartParser::new(boundary, limits, policy)?;
        Ok(MultipartReader {
            reader,
            parser,
            buf: alloc::vec![0u8; READ_BUFFER],
            start: 0,
            end: 0,
            eof: false,
            head: OwnedPartHead::with_capacity(limits.multipart_buffer_len(policy)),
            pending: Vec::new(),
            pending_at: 0,
            in_part: false,
            last: None,
            done: false,
            declared: None,
            #[cfg(feature = "fs")]
            disk_used: 0,
        })
    }

    /// The token field's value, once its part has ended.
    #[must_use]
    pub fn captured_token(&self) -> Option<&str> {
        self.parser.captured_token()
    }

    fn fill(&mut self) -> Result<(), FormError> {
        if self.start < self.end || self.eof {
            return Ok(());
        }
        loop {
            match self.reader.read(&mut self.buf) {
                Ok(0) => {
                    self.eof = true;
                    self.start = 0;
                    self.end = 0;
                    return Ok(());
                }
                Ok(n) => {
                    self.start = 0;
                    self.end = n.min(self.buf.len());
                    return Ok(());
                }
                Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
                Err(e) => return Err(FormError::Io { kind: e.kind() }),
            }
        }
    }

    /// One parser event; data goes to `pending`.
    fn step(&mut self) -> Result<Got, FormError> {
        loop {
            self.fill()?;
            let input = self.buf.get(self.start..self.end).unwrap_or(&[]);
            let step = self.parser.parse(input, self.eof)?;
            self.start = self.start.saturating_add(step.consumed);
            match step.event {
                Event::PartStart(head) => {
                    self.head.copy_from(&head);
                    return Ok(Got::Start);
                }
                Event::Data(bytes) => {
                    crate::wipe::wipe(&mut self.pending);
                    self.pending.clear();
                    crate::wipe::append_wiping(&mut self.pending, bytes, 4096, usize::MAX);
                    self.pending_at = 0;
                    return Ok(Got::Data);
                }
                Event::PartEnd(s) => return Ok(Got::End(s)),
                Event::End => return Ok(Got::Done),
                _ => {}
            }
        }
    }

    /// The next part, or `None` after the close delimiter. Skips whatever
    /// is left of the previous part.
    ///
    /// # Errors
    ///
    /// Every multipart error; `Io { kind }` from the reader.
    pub fn next_part(&mut self) -> Result<Option<PartReader<'_, R>>, FormError> {
        if self.in_part {
            self.parser.skip_part();
            self.pending.clear();
            while self.in_part {
                if let Got::End(s) = self.step()? {
                    self.end_part(s);
                }
            }
        }
        while !self.done {
            match self.step()? {
                Got::Start => {
                    self.in_part = true;
                    return Ok(Some(PartReader { inner: self }));
                }
                Got::Done => self.done = true,
                _ => {}
            }
        }
        Ok(None)
    }

    fn end_part(&mut self, summary: PartSummary) {
        self.in_part = false;
        self.last = Some(summary);
        self.pending.clear();
    }
}

/// One part of a [`MultipartReader`]: read it like a file.
///
/// `Read` reports a parse error as `std::io::Error` with kind `InvalidData`
/// and the `FormError` as its source.
pub struct PartReader<'r, R: Read> {
    inner: &'r mut MultipartReader<R>,
}

impl<R: Read> fmt::Debug for PartReader<'_, R> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PartReader")
            .field("head", &self.inner.head)
            .finish_non_exhaustive()
    }
}

impl<R: Read> PartReader<'_, R> {
    /// The part's head, copied when the part started.
    #[must_use]
    pub fn head(&self) -> &OwnedPartHead {
        &self.inner.head
    }

    /// Reads the rest of the part and checks it as text (its charset, else
    /// the `_charset_` field before it, else UTF-8), bounded by
    /// `max_value_bytes` for file parts too.
    ///
    /// # Errors
    ///
    /// Every multipart error; `Encoding { InvalidUtf8 | UnsupportedCharset }`;
    /// `Limit { ValueLength }`.
    pub fn text(&mut self) -> Result<String, FormError> {
        let own = self.inner.head.view().charset();
        let is_charset = self.inner.head.view().name() == "_charset_";
        let cap = self.inner.parser.limits().value() as usize;
        let mut out = Vec::new();
        let read = (|| {
            while let Some(chunk) = self.next_chunk()? {
                if out.len().saturating_add(chunk.len()) > cap {
                    return Err(FormError::limit(
                        crate::Limit::ValueLength,
                        Location::in_part(
                            Some(self.inner.head.view().index()),
                            self.inner.parser.bytes_seen(),
                        ),
                    ));
                }
                crate::wipe::append_wiping(&mut out, chunk, 4096, cap);
            }
            Ok(())
        })();
        if let Err(e) = read {
            crate::wipe::wipe_vec(&mut out);
            return Err(e);
        }
        let at = Location::in_part(
            Some(self.inner.head.view().index()),
            self.inner.parser.bytes_seen(),
        );
        let readable = match own {
            Charset::Utf8 | Charset::Ascii => true,
            Charset::Other => false,
            Charset::Unspecified => self.inner.declared.as_deref().is_none_or(|l| {
                l.eq_ignore_ascii_case(b"utf-8") || l.eq_ignore_ascii_case(b"us-ascii")
            }),
        };
        if !readable {
            crate::wipe::wipe_vec(&mut out);
            return Err(FormError::encoding(EncodingError::UnsupportedCharset, at));
        }
        if is_charset {
            self.inner.declared = Some(out.clone());
        }
        String::from_utf8(out).map_err(|e| {
            let mut bytes = e.into_bytes();
            crate::wipe::wipe_vec(&mut bytes);
            FormError::encoding(EncodingError::InvalidUtf8, at)
        })
    }

    /// Discards the rest of the part and returns its data length.
    ///
    /// # Errors
    ///
    /// Every multipart error.
    pub fn skip(&mut self) -> Result<u64, FormError> {
        self.inner.parser.skip_part();
        self.inner.pending.clear();
        while self.inner.in_part {
            if let Got::End(s) = self.inner.step()? {
                self.inner.end_part(s);
            }
        }
        Ok(self.inner.last.map_or(0, |s| s.bytes))
    }

    /// Streams a file part into `store`.
    ///
    /// # Errors
    ///
    /// Every multipart error; `Io`, `Store`, `Limit { Disk }`.
    #[cfg(feature = "fs")]
    #[cfg_attr(docsrs, doc(cfg(feature = "fs")))]
    pub fn save(&mut self, store: &crate::Store) -> Result<crate::StoredFile, FormError> {
        use crate::hook::Sink;
        // One request, one disk limit: the sink starts from what earlier
        // saves of this body already put on disk.
        let mut sink = store
            .sink(self.inner.parser.limits())
            .with_used_disk(self.inner.disk_used);
        let head = self.inner.head.clone();
        sink.open(&head.view())?;
        let r = (|| {
            while let Some(chunk) = self.next_chunk()? {
                sink.write(chunk)?;
            }
            sink.close()
        })();
        if r.is_err() {
            sink.abort();
        } else {
            self.inner.disk_used = sink.used_disk();
        }
        r
    }

    /// The next piece of data, or `None` at the end of the part.
    fn next_chunk(&mut self) -> Result<Option<&[u8]>, FormError> {
        let inner = &mut *self.inner;
        if inner.pending_at < inner.pending.len() {
            let chunk = inner.pending.get(inner.pending_at..).unwrap_or(&[]);
            inner.pending_at = inner.pending.len();
            return Ok(Some(chunk));
        }
        while inner.in_part {
            match inner.step()? {
                Got::Data => {
                    inner.pending_at = inner.pending.len();
                    return Ok(Some(&inner.pending));
                }
                Got::End(s) => inner.end_part(s),
                _ => {}
            }
        }
        Ok(None)
    }
}

impl<R: Read> Read for PartReader<'_, R> {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        let inner = &mut *self.inner;
        loop {
            if inner.pending_at < inner.pending.len() {
                let src = inner.pending.get(inner.pending_at..).unwrap_or(&[]);
                let n = src.len().min(out.len());
                if let (Some(dst), Some(src)) = (out.get_mut(..n), src.get(..n)) {
                    dst.copy_from_slice(src);
                }
                inner.pending_at = inner.pending_at.saturating_add(n);
                return Ok(n);
            }
            if !inner.in_part {
                return Ok(0);
            }
            match inner.step() {
                Ok(Got::End(s)) => inner.end_part(s),
                Ok(_) => {}
                Err(e) => return Err(io::Error::new(io::ErrorKind::InvalidData, e)),
            }
        }
    }
}
