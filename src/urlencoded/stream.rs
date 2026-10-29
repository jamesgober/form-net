//! The streaming urlencoded parser: chunks in, pairs out, identical for
//! every split of the body.

use core::fmt;

use crate::error::{FormError, Limit, Location};
use crate::limits::Limits;
use crate::urlencoded::raw::{Budget, RawPair};

/// A streaming urlencoded parser over a caller scratch buffer.
///
/// A sequence that ends inside the current chunk is returned borrowed from
/// the input; one that straddles chunks is assembled in the scratch buffer
/// ([`Limits::urlencoded_buffer_len`] bytes) and returned borrowed from it.
/// A sequence too long for its budgets is refused as soon as its first
/// bytes prove it, before it is copied, so the scratch never overflows.
/// Pairs and errors are the same for every split of the same body.
///
/// # Examples
///
/// ```
/// use form_net::{Limits, UrlencodedEvent, UrlencodedParser};
///
/// // A small route: the scratch fits on the stack.
/// const LIMITS: Limits = Limits::DEFAULT.with_max_name_bytes(32).with_max_value_bytes(256);
/// let mut scratch = [0u8; LIMITS.urlencoded_buffer_len()];
/// let mut parser = UrlencodedParser::with_buffer(&LIMITS, &mut scratch[..])?;
/// let mut names = Vec::new();
/// for (chunk, last) in [(&b"na"[..], false), (b"me=Al&ci", false), (b"ty=Paris", true)] {
///     let mut rest = chunk;
///     loop {
///         let step = parser.parse(rest, last)?;
///         rest = &rest[step.consumed..];
///         match step.event {
///             UrlencodedEvent::Pair(p) => names.push(p.name_raw().to_vec()),
///             _ => break,
///         }
///     }
/// }
/// assert_eq!(names, [b"name".to_vec(), b"city".to_vec()]);
/// # Ok::<(), form_net::FormError>(())
/// ```
pub struct UrlencodedParser<B: AsMut<[u8]> + AsRef<[u8]>> {
    scratch: B,
    /// Bytes of the current, unfinished sequence held in `scratch`.
    held: usize,
    /// Body offset of the held sequence.
    held_at: u64,
    /// The last call handed out a pair borrowed from `scratch`.
    release: bool,
    /// Body bytes consumed so far.
    seen: u64,
    /// The most of `scratch` ever held: only that is wiped (D-103).
    dirty: usize,
    budget: Budget,
    poisoned: bool,
    ended: bool,
}

impl<B: AsMut<[u8]> + AsRef<[u8]>> fmt::Debug for UrlencodedParser<B> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("UrlencodedParser")
            .field("bytes_seen", &self.seen)
            .field("held", &self.held)
            .field("ended", &self.ended)
            .finish_non_exhaustive()
    }
}

/// One step of [`UrlencodedParser::parse`]: how much input was consumed and
/// what it completed.
#[non_exhaustive]
#[derive(Debug)]
pub struct UrlencodedStep<'p> {
    /// Bytes of the input consumed (or held in the scratch buffer).
    pub consumed: usize,
    /// What the consumed bytes completed.
    pub event: UrlencodedEvent<'p>,
}

/// What a step of the streaming parser completed.
#[non_exhaustive]
#[derive(Clone, Copy, Debug)]
pub enum UrlencodedEvent<'p> {
    /// One non-empty `name=value` sequence, undecoded.
    Pair(RawPair<'p>),
    /// Every byte of the input was consumed or held; pass the next chunk.
    NeedInput,
    /// The body is complete (only after `eof = true`).
    End,
}

/// Where the pair of a step lives, decided before any borrow is taken.
enum Found {
    Input { start: usize, end: usize, at: u64 },
    Scratch { len: usize, at: u64 },
    NeedInput,
    End,
}

impl<B: AsMut<[u8]> + AsRef<[u8]>> UrlencodedParser<B> {
    /// A parser over `scratch`, which must hold at least
    /// [`Limits::urlencoded_buffer_len`] bytes.
    ///
    /// # Errors
    ///
    /// `InvalidLimits`; `BufferTooSmall { needed }`.
    pub fn with_buffer(limits: &Limits, scratch: B) -> Result<Self, FormError> {
        limits.validate()?;
        let needed = limits.urlencoded_buffer_len();
        if scratch.as_ref().len() < needed {
            return Err(FormError::BufferTooSmall { needed });
        }
        Ok(UrlencodedParser {
            scratch,
            held: 0,
            held_at: 0,
            release: false,
            seen: 0,
            dirty: 0,
            budget: Budget::new(limits),
            poisoned: false,
            ended: false,
        })
    }

    /// Feeds `input` (`eof = true` says it ends the body) and returns the
    /// next pair, a request for more input, or the end.
    ///
    /// # Errors
    ///
    /// The errors of [`raw_pairs`](crate::raw_pairs); `Poisoned` after an
    /// error until [`reset`](Self::reset).
    pub fn parse<'p>(
        &'p mut self,
        input: &'p [u8],
        eof: bool,
    ) -> Result<UrlencodedStep<'p>, FormError> {
        if self.poisoned {
            return Err(FormError::Poisoned);
        }
        if self.release {
            self.release = false;
            self.clear_held();
        }
        let (consumed, found) = match self.step(input, eof) {
            Ok(r) => r,
            Err(e) => {
                self.poisoned = true;
                self.clear_held();
                return Err(e);
            }
        };
        let event = match found {
            Found::Input { start, end, at } => {
                UrlencodedEvent::Pair(RawPair::split(input.get(start..end).unwrap_or(&[]), at))
            }
            Found::Scratch { len, at } => {
                self.release = true;
                let seq = self.scratch.as_ref().get(..len).unwrap_or(&[]);
                UrlencodedEvent::Pair(RawPair::split(seq, at))
            }
            Found::NeedInput => UrlencodedEvent::NeedInput,
            Found::End => UrlencodedEvent::End,
        };
        Ok(UrlencodedStep { consumed, event })
    }

    /// Clears the counters and the poisoned state and wipes the scratch, for
    /// the next body.
    pub fn reset(&mut self) {
        self.wipe_used();
        self.held = 0;
        self.held_at = 0;
        self.release = false;
        self.seen = 0;
        self.budget.reset();
        self.poisoned = false;
        self.ended = false;
    }

    /// Body bytes consumed so far.
    #[must_use]
    pub const fn bytes_seen(&self) -> u64 {
        self.seen
    }

    fn wipe_used(&mut self) {
        let used = self.dirty;
        if let Some(region) = self.scratch.as_mut().get_mut(..used) {
            crate::wipe::wipe(region);
        }
        self.dirty = 0;
    }

    fn clear_held(&mut self) {
        if let Some(held) = self.scratch.as_mut().get_mut(..self.held) {
            crate::wipe::wipe(held);
        }
        self.held = 0;
    }

    fn step(&mut self, input: &[u8], eof: bool) -> Result<(usize, Found), FormError> {
        if self.ended {
            return Ok((0, Found::End));
        }
        let body = self.budget.body;
        let mut pos = 0usize;
        loop {
            let rest = input.get(pos..).unwrap_or(&[]);
            let allowed = usize::try_from(body.saturating_sub(self.seen)).unwrap_or(usize::MAX);
            let window = rest.get(..allowed.min(rest.len())).unwrap_or(&[]);
            let over = rest.len() > window.len();
            let amp = crate::scan::find_any(window, 0, *b"&&&&").min(window.len());
            let terminated = amp < window.len();
            let piece = window.get(..amp).unwrap_or(&[]);
            if !terminated && (!eof || over) {
                // The sequence goes on past this chunk (or past the body
                // budget): check what is known, then hold it or refuse.
                self.check_partial(piece)?;
                if over {
                    return Err(FormError::limit(Limit::Body, Location::at(body)));
                }
                self.hold(piece);
                pos = pos.saturating_add(piece.len());
                self.seen = self.seen.saturating_add(piece.len() as u64);
                return Ok((pos, Found::NeedInput));
            }
            // The sequence is complete: `piece` ends at `&` or at the end of
            // the body.
            let start = pos;
            let at = self.seen;
            let seq_len = self.held.saturating_add(piece.len());
            pos = pos.saturating_add(piece.len());
            self.seen = self.seen.saturating_add(piece.len() as u64);
            if terminated {
                pos = pos.saturating_add(1);
                self.seen = self.seen.saturating_add(1);
            }
            if seq_len == 0 {
                if terminated {
                    continue;
                }
                self.ended = true;
                return Ok((pos, Found::End));
            }
            if self.held == 0 {
                self.budget.admit(&RawPair::split(piece, at))?;
                return Ok((
                    pos,
                    Found::Input {
                        start,
                        end: start.saturating_add(piece.len()),
                        at,
                    },
                ));
            }
            // Part of the sequence is held: the rest joins it in scratch.
            self.check_partial(piece)?;
            self.hold(piece);
            let at = self.held_at;
            let len = self.held;
            let seq = self.scratch.as_ref().get(..len).unwrap_or(&[]);
            self.budget.admit(&RawPair::split(seq, at))?;
            return Ok((pos, Found::Scratch { len, at }));
        }
    }

    /// Refuses the held sequence plus `more` when its complete form is
    /// certain to be refused.
    fn check_partial(&self, more: &[u8]) -> Result<(), FormError> {
        let held = self.scratch.as_ref().get(..self.held).unwrap_or(&[]);
        let at = if self.held == 0 {
            self.seen
        } else {
            self.held_at
        };
        let (name_len, has_equals, value_len) = match held.iter().position(|&b| b == b'=') {
            Some(e) => (
                e,
                true,
                held.len()
                    .saturating_sub(e)
                    .saturating_sub(1)
                    .saturating_add(more.len()),
            ),
            None => match more.iter().position(|&b| b == b'=') {
                Some(e) => (
                    held.len().saturating_add(e),
                    true,
                    more.len().saturating_sub(e).saturating_sub(1),
                ),
                None => (held.len().saturating_add(more.len()), false, 0),
            },
        };
        self.budget
            .admit_partial(at, name_len, has_equals, value_len)
    }

    /// Appends `bytes` to the held sequence. `check_partial` passed, so
    /// they fit.
    fn hold(&mut self, bytes: &[u8]) {
        if bytes.is_empty() {
            return;
        }
        if self.held == 0 {
            self.held_at = self.seen;
        }
        let start = self.held;
        let end = start.saturating_add(bytes.len());
        if let Some(dst) = self.scratch.as_mut().get_mut(start..end) {
            dst.copy_from_slice(bytes);
            self.held = end;
            self.dirty = self.dirty.max(end);
        }
    }
}

impl<B: AsMut<[u8]> + AsRef<[u8]>> Drop for UrlencodedParser<B> {
    fn drop(&mut self) {
        self.wipe_used();
    }
}

#[cfg(feature = "alloc")]
impl UrlencodedParser<alloc::vec::Vec<u8>> {
    /// A parser that owns its scratch buffer.
    ///
    /// # Errors
    ///
    /// `InvalidLimits`.
    ///
    /// # Examples
    ///
    /// ```
    /// use form_net::{Limits, UrlencodedEvent, UrlencodedParser};
    ///
    /// let mut parser = UrlencodedParser::new(&Limits::DEFAULT)?;
    /// let step = parser.parse(b"a=1", true)?;
    /// assert!(matches!(step.event, UrlencodedEvent::Pair(p) if p.value_raw() == b"1"));
    /// # Ok::<(), form_net::FormError>(())
    /// ```
    pub fn new(limits: &Limits) -> Result<Self, FormError> {
        limits.validate()?;
        Self::with_buffer(limits, alloc::vec![0u8; limits.urlencoded_buffer_len()])
    }
}
