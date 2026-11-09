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
    /// buffer too small for the new head is wiped and freed before its
    /// replacement is allocated, so no copy of a head is freed unwiped and
    /// two buffers are never held at once. The header block is copied once:
    /// the name, file name, and content type that lie inside it are kept as
    /// positions in that copy (D-140).
    pub fn copy_from(&mut self, head: &PartHead<'_>) {
        crate::wipe::wipe(&mut self.buf);
        self.buf.clear();
        let block = head.block();
        let outside = |s: &[u8]| {
            if within(block, s).is_some() {
                0
            } else {
                s.len()
            }
        };
        let needed = [
            block.len(),
            outside(head.name().as_bytes()),
            outside(head.name_raw()),
            head.filename().map_or(0, |f| outside(f.raw())),
            head.content_type().map_or(0, |c| outside(c.raw())),
        ]
        .iter()
        .fold(0usize, |a, n| a.saturating_add(*n));
        if needed > self.buf.capacity() {
            let mut old = core::mem::take(&mut self.buf);
            crate::wipe::wipe_vec(&mut old);
            drop(old);
            self.buf = Vec::with_capacity(needed);
        }
        self.index = head.index();
        self.block = self.push(block);
        let base = self.block.start;
        let mut place = |s: &[u8]| match within(block, s) {
            Some(r) => r.start.saturating_add(base)..r.end.saturating_add(base),
            None => {
                // As `push`, on the buffer alone, so the fields stay free.
                let start = self.buf.len();
                crate::wipe::append_wiping(&mut self.buf, s, s.len(), usize::MAX);
                start..self.buf.len()
            }
        };
        self.name = place(head.name().as_bytes());
        self.name_raw = place(head.name_raw());
        self.kind = Some(head.kind());
        self.filename = head.filename().map(|f| (place(f.raw()), f.source()));
        self.content_type = head.content_type().map(|c| place(c.raw()));
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
        // Only the length can hold head bytes: `copy_from` wipes the old
        // bytes before every reuse and moves to a fresh buffer (wiping the
        // old one) when it must grow, so the spare capacity, 9.6 KiB under
        // the default limits, was never written (D-103).
        crate::wipe::wipe(&mut self.buf);
    }
}

/// Where `part` lies inside `whole`, when it is a subslice of it: compared
/// by address, so equal bytes elsewhere do not count.
fn within(whole: &[u8], part: &[u8]) -> Option<Range<usize>> {
    let start = (part.as_ptr() as usize).checked_sub(whole.as_ptr() as usize)?;
    let end = start.checked_add(part.len())?;
    (end <= whole.len()).then_some(start..end)
}
