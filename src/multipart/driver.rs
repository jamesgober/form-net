//! The streaming driver: one implementation of the hook order over the
//! parser, an inspector, and a sink.

use alloc::vec::Vec;
use core::fmt;

use crate::error::{FormError, Location};
use crate::hook::{Inspector, Sink, Verdict};
use crate::limits::EmptyFilePolicy;
use crate::multipart::owned_head::OwnedPartHead;
use crate::multipart::parser::{Event, MultipartParser, PartHead, PartKind, PartSummary};
use crate::wipe::WipeVec;

/// What completed during a [`Driver::push`]. `Debug` shows a field's name
/// and value length, never the value.
#[non_exhaustive]
pub enum Progress<'d, F> {
    /// A text field ended: its name and value.
    Field {
        /// The field name.
        name: &'d str,
        /// The value bytes.
        value: &'d [u8],
    },
    /// A file part ended: the sink's output.
    File(F),
    /// Every byte given was consumed; give more.
    NeedInput,
    /// The form is complete.
    End,
}

impl<F> fmt::Debug for Progress<'_, F> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Progress::Field { name, value } => f
                .debug_struct("Field")
                .field("name", name)
                .field("value_len", &value.len())
                .finish(),
            Progress::File(_) => f.write_str("File(..)"),
            Progress::NeedInput => f.write_str("NeedInput"),
            Progress::End => f.write_str("End"),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Part {
    Idle,
    Field {
        skip: bool,
    },
    File {
        open: bool,
        deferred: bool,
        sniffing: bool,
        skip: bool,
    },
}

/// Everything but the parser, so its fields can be borrowed while an event
/// borrows the parser.
struct State<S> {
    sink: S,
    head: OwnedPartHead,
    text: WipeVec,
    sniff: WipeVec,
    sniff_len: usize,
    value_cap: usize,
    skip_empty: bool,
    part: Part,
}

/// What a part handler asks of the driver once the event is released.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Next {
    Continue,
    SkipPart,
}

fn refused(v: Verdict, at: Location) -> Result<bool, FormError> {
    match v {
        Verdict::Refuse(refusal) => Err(FormError::Refused { refusal, at }),
        Verdict::Skip => Ok(true),
        _ => Ok(false),
    }
}

impl<S: Sink> State<S> {
    fn start_file<I: Inspector>(
        &mut self,
        head: &PartHead<'_>,
        inspector: &mut I,
        at: Location,
    ) -> Result<Next, FormError> {
        if refused(inspector.part_start(head), at)? {
            self.part = Part::File {
                open: false,
                deferred: false,
                sniffing: false,
                skip: true,
            };
            return Ok(Next::SkipPart);
        }
        // The sink opens on the first byte the inspector has accepted (or at
        // the end of an empty file), never before `sniff`: a refused or
        // skipped part touches no storage at all.
        crate::wipe::wipe(&mut self.sniff);
        self.sniff.clear();
        self.part = Part::File {
            open: false,
            deferred: false,
            sniffing: self.sniff_len > 0,
            skip: false,
        };
        Ok(Next::Continue)
    }

    fn on_start<I: Inspector>(
        &mut self,
        head: &PartHead<'_>,
        inspector: &mut I,
        at: Location,
    ) -> Result<Next, FormError> {
        self.head.copy_from(head);
        match head.kind() {
            PartKind::File => {
                let empty = head.filename().is_some_and(|f| f.is_empty());
                if empty && self.skip_empty {
                    self.part = Part::File {
                        open: false,
                        deferred: true,
                        sniffing: false,
                        skip: false,
                    };
                    return Ok(Next::Continue);
                }
                self.start_file(head, inspector, at)
            }
            _ => {
                crate::wipe::wipe(&mut self.text);
                self.text.clear();
                if refused(inspector.part_start(head), at)? {
                    self.part = Part::Field { skip: true };
                    return Ok(Next::SkipPart);
                }
                self.part = Part::Field { skip: false };
                Ok(Next::Continue)
            }
        }
    }

    /// Opens the sink for the current file if it is not open yet.
    fn ensure_open(&mut self) -> Result<(), FormError> {
        if let Part::File { open: false, .. } = self.part {
            self.sink.open(&self.head.view())?;
            if let Part::File { open, .. } = &mut self.part {
                *open = true;
            }
        }
        Ok(())
    }

    /// Aborts the sink if this part opened it, and marks the part skipped.
    fn skip_file(&mut self) {
        if let Part::File { open: true, .. } = self.part {
            self.sink.abort();
        }
        self.part = Part::File {
            open: false,
            deferred: false,
            sniffing: false,
            skip: true,
        };
    }

    /// Passes file bytes to the inspector and the sink.
    fn file_bytes<I: Inspector>(
        &mut self,
        index: u32,
        bytes: &[u8],
        inspector: &mut I,
        at: Location,
    ) -> Result<Next, FormError> {
        if bytes.is_empty() {
            return Ok(Next::Continue);
        }
        if refused(inspector.data(index, bytes), at)? {
            self.skip_file();
            return Ok(Next::SkipPart);
        }
        self.ensure_open()?;
        self.sink.write(bytes).map(|()| Next::Continue)
    }

    /// Hands the sniff window to the inspector, then on as data.
    fn flush_sniff<I: Inspector>(
        &mut self,
        index: u32,
        inspector: &mut I,
        at: Location,
    ) -> Result<Next, FormError> {
        if let Part::File { sniffing, .. } = &mut self.part {
            *sniffing = false;
        }
        let window = core::mem::take(&mut self.sniff);
        let verdict = inspector.sniff(&self.head.view(), &window);
        let result = match refused(verdict, at) {
            Err(e) => Err(e),
            Ok(true) => {
                self.skip_file();
                Ok(Next::SkipPart)
            }
            Ok(false) => self.file_bytes(index, &window, inspector, at),
        };
        self.sniff = window;
        crate::wipe::wipe(&mut self.sniff);
        self.sniff.clear();
        result
    }

    fn on_data<I: Inspector>(
        &mut self,
        bytes: &[u8],
        inspector: &mut I,
        at: Location,
    ) -> Result<Next, FormError> {
        let index = self.head.view().index();
        match self.part {
            Part::Field { skip: false } => {
                if refused(inspector.data(index, bytes), at)? {
                    self.part = Part::Field { skip: true };
                    return Ok(Next::SkipPart);
                }
                crate::wipe::append_wiping(&mut self.text, bytes, 64 * 1024, self.value_cap);
                Ok(Next::Continue)
            }
            Part::File { deferred: true, .. } => {
                let head = self.head.clone();
                if self.start_file(&head.view(), inspector, at)? == Next::SkipPart {
                    return Ok(Next::SkipPart);
                }
                self.on_data(bytes, inspector, at)
            }
            Part::File {
                skip: false,
                sniffing: true,
                ..
            } => {
                let room = self.sniff_len.saturating_sub(self.sniff.len());
                let (head, rest) = bytes.split_at(room.min(bytes.len()));
                self.sniff.extend_from_slice(head);
                if self.sniff.len() < self.sniff_len {
                    return Ok(Next::Continue);
                }
                if self.flush_sniff(index, inspector, at)? == Next::SkipPart {
                    return Ok(Next::SkipPart);
                }
                self.file_bytes(index, rest, inspector, at)
            }
            Part::File { skip: false, .. } => self.file_bytes(index, bytes, inspector, at),
            _ => Ok(Next::Continue),
        }
    }
}

/// What a step of the driver produced, without borrows.
enum Done<F> {
    Field,
    File(F),
    NeedInput,
    End,
}

/// The one streaming implementation of the hook order: parser, inspector,
/// sink, the text buffer, the sniff window, and the empty-file delay.
///
/// It takes its limits and policy from the parser. Because a `PartHead`
/// cannot outlive the `parse` call that produced it, the driver copies each
/// head once into an owned head and hands that copy to the delayed calls.
///
/// # Examples
///
/// ```
/// use form_net::{Boundary, Driver, Limits, MemorySink, MultipartParser, NoInspector, Policy, Progress};
///
/// let parser = MultipartParser::new(Boundary::new(b"X")?, &Limits::DEFAULT, &Policy::DEFAULT)?;
/// let mut driver = Driver::new(parser, MemorySink::new());
/// let body = b"--X\r\nContent-Disposition: form-data; name=\"a\"\r\n\r\n1\r\n--X--";
/// let (consumed, progress) = driver.push(body, true, &mut NoInspector)?;
/// assert!(matches!(progress, Progress::Field { name: "a", value: b"1" }));
/// let (_, progress) = driver.push(&body[consumed..], true, &mut NoInspector)?;
/// assert!(matches!(progress, Progress::End));
/// # Ok::<(), form_net::FormError>(())
/// ```
pub struct Driver<B: AsMut<[u8]> + AsRef<[u8]>, S: Sink> {
    parser: MultipartParser<B>,
    state: State<S>,
    ended: bool,
    failed: bool,
}

impl<B: AsMut<[u8]> + AsRef<[u8]>, S: Sink> fmt::Debug for Driver<B, S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Driver")
            .field("parser", &self.parser)
            .field("ended", &self.ended)
            .finish_non_exhaustive()
    }
}

impl<B: AsMut<[u8]> + AsRef<[u8]>, S: Sink> Driver<B, S> {
    /// A driver over `parser` writing files to `sink`.
    #[must_use]
    pub fn new(parser: MultipartParser<B>, sink: S) -> Self {
        let limits = *parser.limits();
        let policy = *parser.policy();
        let sniff_len = limits.sniff() as usize;
        Driver {
            state: State {
                sink,
                head: OwnedPartHead::with_capacity(limits.multipart_buffer_len(&policy)),
                text: WipeVec::default(),
                sniff: WipeVec(Vec::with_capacity(sniff_len)),
                sniff_len,
                value_cap: limits.value() as usize,
                skip_empty: policy.empty_files == EmptyFilePolicy::Skip,
                part: Part::Idle,
            },
            parser,
            ended: false,
            failed: false,
        }
    }

    /// The parser.
    #[must_use]
    pub const fn parser(&self) -> &MultipartParser<B> {
        &self.parser
    }

    /// The parser and the sink.
    pub fn into_parts(self) -> (MultipartParser<B>, S) {
        (self.parser, self.state.sink)
    }

    /// Feeds input and returns how much was consumed and what completed: a
    /// text field, a closed file (the sink's output), a request for more
    /// input, or the end. `eof = true` says `input` ends the body.
    ///
    /// # Errors
    ///
    /// Every multipart error, `Refused`, and the sink's errors. On error the
    /// sink has already been aborted and `inspector.failed` called; later
    /// calls return `Poisoned`.
    pub fn push<'d, I: Inspector>(
        &'d mut self,
        input: &[u8],
        eof: bool,
        inspector: &mut I,
    ) -> Result<(usize, Progress<'d, S::Output>), FormError> {
        if self.failed {
            return Err(FormError::Poisoned);
        }
        match self.run(input, eof, inspector) {
            Ok((consumed, done)) => Ok((
                consumed,
                match done {
                    Done::Field => Progress::Field {
                        name: self.state.head_name(),
                        value: &self.state.text,
                    },
                    Done::File(f) => Progress::File(f),
                    Done::NeedInput => Progress::NeedInput,
                    Done::End => Progress::End,
                },
            )),
            Err(e) => {
                self.failed = true;
                self.state.sink.abort();
                inspector.failed(&e);
                Err(e)
            }
        }
    }

    fn at(&self) -> Location {
        Location::in_part(
            self.parser.parts_seen().checked_sub(1),
            self.parser.bytes_seen(),
        )
    }

    fn run<I: Inspector>(
        &mut self,
        input: &[u8],
        eof: bool,
        inspector: &mut I,
    ) -> Result<(usize, Done<S::Output>), FormError> {
        let mut pos = 0usize;
        loop {
            let rest = input.get(pos..).unwrap_or(&[]);
            let seen = self.parser.bytes_seen();
            let parts = self.parser.parts_seen();
            let step = self.parser.parse(rest, eof)?;
            pos = pos.saturating_add(step.consumed);
            let index = match &step.event {
                Event::PartStart(head) => Some(head.index()),
                _ => parts.checked_sub(1),
            };
            let at = Location::in_part(index, seen.saturating_add(step.consumed as u64));
            let next = match step.event {
                Event::PartStart(head) => self.state.on_start(&head, inspector, at)?,
                Event::Data(bytes) => self.state.on_data(bytes, inspector, at)?,
                Event::PartEnd(summary) => {
                    if let Some(done) = self.on_end(summary, inspector)? {
                        return Ok((pos, done));
                    }
                    Next::Continue
                }
                Event::End => {
                    if !self.ended {
                        self.ended = true;
                        if let Verdict::Refuse(refusal) = inspector.end() {
                            return Err(FormError::Refused { refusal, at });
                        }
                    }
                    return Ok((pos, Done::End));
                }
                _ => return Ok((pos, Done::NeedInput)),
            };
            if next == Next::SkipPart {
                self.parser.skip_part();
            }
        }
    }

    fn on_end<I: Inspector>(
        &mut self,
        summary: PartSummary,
        inspector: &mut I,
    ) -> Result<Option<Done<S::Output>>, FormError> {
        let at = self.at();
        let part = core::mem::replace(&mut self.state.part, Part::Idle);
        let skipped = summary.skipped
            || matches!(
                part,
                Part::Field { skip: true } | Part::File { skip: true, .. }
            );
        let summary = PartSummary { skipped, ..summary };
        match part {
            Part::File { deferred: true, .. } => Ok(None),
            Part::File {
                skip: false,
                sniffing,
                ..
            } => {
                // The flush and the open read the part's state: give it back
                // until the sink is closed.
                self.state.part = part;
                let flushed = if sniffing {
                    self.state.flush_sniff(summary.index, inspector, at)
                } else {
                    Ok(Next::Continue)
                };
                let opened = match flushed {
                    Ok(Next::Continue) => self.state.ensure_open().map(|()| true),
                    Ok(Next::SkipPart) => Ok(false),
                    Err(e) => Err(e),
                };
                self.state.part = Part::Idle;
                if !opened? {
                    let summary = PartSummary {
                        skipped: true,
                        ..summary
                    };
                    refused(inspector.part_end(&summary), at)?;
                    return Ok(None);
                }
                let output = self.state.sink.close()?;
                refused(inspector.part_end(&summary), at)?;
                Ok(Some(Done::File(output)))
            }
            Part::Field { skip: false } => {
                let name = self.state.head.view().name();
                // The name is copied out of the head: the inspector borrows
                // the text buffer.
                let skip = refused(inspector.field(name, &self.state.text), at)?;
                let summary = PartSummary {
                    skipped: skip,
                    ..summary
                };
                refused(inspector.part_end(&summary), at)?;
                if skip {
                    crate::wipe::wipe(&mut self.state.text);
                    return Ok(None);
                }
                Ok(Some(Done::Field))
            }
            _ => {
                refused(inspector.part_end(&summary), at)?;
                Ok(None)
            }
        }
    }
}

impl<S> State<S> {
    fn head_name(&self) -> &str {
        self.head.view().name()
    }
}

impl<B: AsMut<[u8]> + AsRef<[u8]>, S: Sink> Driver<B, S> {
    /// Whether a push failed (the driver is poisoned).
    #[cfg(feature = "fs")]
    pub(crate) const fn has_failed(&self) -> bool {
        self.failed
    }

    /// The charset the current (or last) part declared.
    #[cfg(feature = "fs")]
    pub(crate) fn current_head_charset(&self) -> crate::multipart::parser::Charset {
        self.state.head.view().charset()
    }
}
