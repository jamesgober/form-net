//! An owned copy of a part head, for code that needs the head after the
//! `parse` call that produced it.

use alloc::vec::Vec;
use core::fmt;
use core::ops::Range;

use crate::multipart::filename::{FileName, FileNameSource};
use crate::multipart::parser::{Charset, PartHead, PartKind};

/// An owned copy of a [`PartHead`]. Its buffer is reused for the next head
/// copied into it, so a driver that keeps one never allocates per part once
/// the buffer has grown to the largest head.
///
/// # Examples
///
/// ```
/// use form_net::{Boundary, Event, Limits, MultipartParser, OwnedPartHead, Policy};
///
/// let mut parser = MultipartParser::new(Boundary::new(b"X")?, &Limits::DEFAULT, &Policy::DEFAULT)?;
/// let body = b"--X\r\nContent-Disposition: form-data; name=\"a\"\r\n\r\n1\r\n--X--";
/// let mut kept = OwnedPartHead::new();
/// let step = parser.parse(body, true)?;
/// if let Event::PartStart(head) = step.event {
///     kept.copy_from(&head);
/// }
/// assert_eq!(kept.view().name(), "a");
/// # Ok::<(), form_net::FormError>(())
/// ```
#[derive(Clone, Default)]
pub struct OwnedPartHead {
    buf: Vec<u8>,
    index: u32,
    block: Range<usize>,
    name: Range<usize>,
    name_raw: Range<usize>,
    kind: Option<PartKind>,
    filename: Option<(Range<usize>, FileNameSource)>,
    content_type: Option<Range<usize>>,
    charset: Option<Charset>,
}

impl fmt::Debug for OwnedPartHead {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.view(), f)
    }
}

impl OwnedPartHead {
    /// An empty head; [`view`](Self::view) of it is a field with no name.
    #[must_use]
    pub fn new() -> OwnedPartHead {
        OwnedPartHead::default()
    }

    /// An empty head whose buffer already holds `capacity` bytes.
    #[must_use]
    pub fn with_capacity(capacity: usize) -> OwnedPartHead {
        let mut head = OwnedPartHead::default();
        head.buf = Vec::with_capacity(capacity);
        head
    }

    fn push(&mut self, bytes: &[u8]) -> Range<usize> {
        let start = self.buf.len();
        // The capacity was reserved in `copy_from`, so this never moves.
        crate::wipe::append_wiping(&mut self.buf, bytes, bytes.len(), usize::MAX);
        start..self.buf.len()
    }

    /// Replaces this copy with `head`. The old bytes are wiped first, and a
    /// buffer too small for the new head is replaced, not reallocated, so
    /// no copy of a head is freed unwiped.
    pub fn copy_from(&mut self, head: &PartHead<'_>) {
        crate::wipe::wipe(&mut self.buf);
        self.buf.clear();
        let needed = [
            head.block().len(),
            head.name().len(),
            head.name_raw().len(),
            head.filename().map_or(0, |f| f.raw().len()),
            head.content_type().map_or(0, |c| c.raw().len()),
        ]
        .iter()
        .fold(0usize, |a, n| a.saturating_add(*n));
        if needed > self.buf.capacity() {
            let mut old = core::mem::replace(&mut self.buf, Vec::with_capacity(needed));
            crate::wipe::wipe_vec(&mut old);
        }
        self.index = head.index();
        self.block = self.push(head.block());
        self.name = self.push(head.name().as_bytes());
        self.name_raw = self.push(head.name_raw());
        self.kind = Some(head.kind());
        self.filename = head.filename().map(|f| (self.push(f.raw()), f.source()));
        self.content_type = head.content_type().map(|c| self.push(c.raw()));
        self.charset = Some(head.charset());
    }

    /// The usual borrowed view.
    #[must_use]
    pub fn view(&self) -> PartHead<'_> {
        let get = |r: &Range<usize>| self.buf.get(r.clone()).unwrap_or(&[]);
        PartHead::from_parts(
            self.index,
            get(&self.block),
            core::str::from_utf8(get(&self.name)).unwrap_or(""),
            get(&self.name_raw),
            self.kind.unwrap_or(PartKind::Field),
            self.filename
                .as_ref()
                .map(|(r, source)| FileName::new(get(r), *source)),
            self.content_type.as_ref().map(get),
            self.charset.unwrap_or(Charset::Unspecified),
        )
    }
}

impl Drop for OwnedPartHead {
    fn drop(&mut self) {
        crate::wipe::wipe_vec(&mut self.buf);
    }
}
