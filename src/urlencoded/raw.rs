//! The allocation-free urlencoded scanner.

use core::fmt;

use crate::error::{FormError, Limit, Location};
use crate::limits::{Limits, PercentPolicy};
use crate::urlencoded::decode::decode_component;

const HAS_EQUALS: u8 = 1;
const NAME_DECODE: u8 = 2;
const VALUE_DECODE: u8 = 4;

/// Iterates the pairs of a urlencoded body without allocating or copying.
///
/// Budgets are checked per sequence before the sequence is handed out: body
/// size (every byte, empty sequences included), field count, raw name and
/// value lengths, and the running total of raw value lengths. After the
/// first error the iterator returns `None`.
///
/// # Examples
///
/// ```
/// use form_net::Limits;
///
/// let pairs: Vec<_> = form_net::raw_pairs(b"a=1&&b", &Limits::DEFAULT)
///     .map(|p| p.map(|p| (p.name_raw(), p.value_raw(), p.has_equals())))
///     .collect::<Result<_, _>>()?;
/// assert_eq!(pairs, [(&b"a"[..], &b"1"[..], true), (&b"b"[..], &b""[..], false)]);
/// # Ok::<(), form_net::FormError>(())
/// ```
#[must_use]
pub fn raw_pairs<'a>(input: &'a [u8], limits: &Limits) -> RawPairs<'a> {
    RawPairs {
        input,
        pos: 0,
        budget: Budget::new(limits),
        done: false,
        checked_body: false,
    }
}

/// The per-sequence budgets, shared by [`raw_pairs`] and the streaming
/// parser so both refuse the same sequence with the same error.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Budget {
    pub(crate) limits: Limits,
    fields: u32,
    text: u64,
}

impl Budget {
    pub(crate) const fn new(limits: &Limits) -> Budget {
        Budget {
            limits: *limits,
            fields: 0,
            text: 0,
        }
    }

    /// Counts and checks one complete, non-empty sequence: field count,
    /// name length, value length, then the text total.
    pub(crate) fn admit(&mut self, pair: &RawPair<'_>) -> Result<(), FormError> {
        let at = Location::at(pair.offset);
        self.fields = self.fields.saturating_add(1);
        if self.fields > self.limits.max_fields {
            return Err(FormError::limit(Limit::Fields, at));
        }
        if pair.name.len() as u64 > u64::from(self.limits.name()) {
            return Err(FormError::limit(Limit::NameLength, at));
        }
        let value_at = pair.value_offset();
        if pair.value.len() as u64 > u64::from(self.limits.value()) {
            return Err(FormError::limit(Limit::ValueLength, value_at));
        }
        self.text = self.text.saturating_add(pair.value.len() as u64);
        if self.text > self.limits.text() {
            return Err(FormError::limit(Limit::TextTotal, value_at));
        }
        Ok(())
    }

    /// Checks the start of a sequence that is not complete yet: when its
    /// complete form is certain to be refused, refuses it now with the error
    /// [`admit`](Self::admit) would give, so a held sequence never outgrows
    /// `urlencoded_buffer_len`.
    pub(crate) fn admit_partial(
        &self,
        offset: u64,
        name_len: usize,
        has_equals: bool,
        value_len: usize,
    ) -> Result<(), FormError> {
        let name_over = name_len as u64 > u64::from(self.limits.name());
        let value_over = has_equals && value_len as u64 > u64::from(self.limits.value());
        if !name_over && !value_over {
            return Ok(());
        }
        if self.fields.saturating_add(1) > self.limits.max_fields {
            return Err(FormError::limit(Limit::Fields, Location::at(offset)));
        }
        if name_over {
            return Err(FormError::limit(Limit::NameLength, Location::at(offset)));
        }
        let value_at = offset.saturating_add(name_len as u64).saturating_add(1);
        Err(FormError::limit(Limit::ValueLength, Location::at(value_at)))
    }

    pub(crate) fn reset(&mut self) {
        self.fields = 0;
        self.text = 0;
    }
}

/// The iterator returned by [`raw_pairs`].
#[derive(Clone)]
pub struct RawPairs<'a> {
    input: &'a [u8],
    pos: usize,
    budget: Budget,
    done: bool,
    checked_body: bool,
}

impl fmt::Debug for RawPairs<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RawPairs")
            .field("position", &self.pos)
            .field("fields", &self.budget.fields)
            .finish_non_exhaustive()
    }
}

impl<'a> RawPairs<'a> {
    fn fail(&mut self, err: FormError) -> Option<Result<RawPair<'a>, FormError>> {
        self.done = true;
        Some(Err(err))
    }
}

impl<'a> Iterator for RawPairs<'a> {
    type Item = Result<RawPair<'a>, FormError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.done {
            return None;
        }
        if !self.checked_body {
            self.checked_body = true;
            if self.input.len() > self.budget.limits.body_usize() {
                return self.fail(FormError::limit(
                    Limit::Body,
                    Location::at(self.budget.limits.body()),
                ));
            }
        }
        loop {
            if self.pos >= self.input.len() {
                self.done = true;
                return None;
            }
            let start = self.pos;
            let mut flags = 0u8;
            let mut eq = None;
            let mut i = start;
            let end = loop {
                i = crate::scan::find_any(self.input, i, *b"&=%+");
                match self.input.get(i) {
                    None | Some(b'&') => break i,
                    Some(b'=') if eq.is_none() => {
                        eq = Some(i);
                        flags |= HAS_EQUALS;
                    }
                    Some(b'%' | b'+') => {
                        flags |= if eq.is_some() {
                            VALUE_DECODE
                        } else {
                            NAME_DECODE
                        };
                    }
                    Some(_) => {}
                }
                i = i.wrapping_add(1);
            };
            self.pos = end.saturating_add(1).min(self.input.len());
            if end == start {
                continue;
            }
            let seq = self.input.get(start..end).unwrap_or(&[]);
            let (name, value) = match eq {
                Some(e) => {
                    let at = e.wrapping_sub(start);
                    (
                        seq.get(..at).unwrap_or(&[]),
                        seq.get(at.wrapping_add(1)..).unwrap_or(&[]),
                    )
                }
                None => (seq, &[][..]),
            };
            let pair = RawPair {
                name,
                value,
                flags,
                offset: start as u64,
            };
            if let Err(e) = self.budget.admit(&pair) {
                return self.fail(e);
            }
            return Some(Ok(pair));
        }
    }
}

impl core::iter::FusedIterator for RawPairs<'_> {}

/// One `name=value` sequence, undecoded.
///
/// `Debug` prints the offset and the lengths only: values can be secrets.
#[derive(Clone, Copy)]
pub struct RawPair<'a> {
    name: &'a [u8],
    value: &'a [u8],
    flags: u8,
    offset: u64,
}

impl fmt::Debug for RawPair<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RawPair")
            .field("offset", &self.offset)
            .field("name_len", &self.name.len())
            .field("value_len", &self.value.len())
            .finish()
    }
}

impl<'a> RawPair<'a> {
    /// Splits one sequence (no `&` in it) found at `offset` in the body.
    pub(crate) fn split(seq: &'a [u8], offset: u64) -> RawPair<'a> {
        let mut flags = 0u8;
        let mut eq = None;
        let mut i = 0usize;
        loop {
            i = crate::scan::find_any(seq, i, *b"&=%+");
            match seq.get(i) {
                None => break,
                Some(b'=') if eq.is_none() => {
                    eq = Some(i);
                    flags |= HAS_EQUALS;
                }
                Some(b'%' | b'+') => {
                    flags |= if eq.is_some() {
                        VALUE_DECODE
                    } else {
                        NAME_DECODE
                    };
                }
                Some(_) => {}
            }
            i = i.wrapping_add(1);
        }
        let (name, value) = match eq {
            Some(e) => (
                seq.get(..e).unwrap_or(&[]),
                seq.get(e.wrapping_add(1)..).unwrap_or(&[]),
            ),
            None => (seq, &[][..]),
        };
        RawPair {
            name,
            value,
            flags,
            offset,
        }
    }

    fn value_offset(&self) -> Location {
        Location::at(
            self.offset
                .saturating_add(self.name.len() as u64)
                .saturating_add(1),
        )
    }

    /// The name as sent, before decoding.
    ///
    /// # Examples
    ///
    /// ```
    /// let p = form_net::raw_pairs(b"a+b=1", &form_net::Limits::DEFAULT).next().unwrap()?;
    /// assert_eq!(p.name_raw(), b"a+b");
    /// # Ok::<(), form_net::FormError>(())
    /// ```
    #[must_use]
    pub const fn name_raw(&self) -> &'a [u8] {
        self.name
    }

    /// The value as sent, before decoding.
    ///
    /// # Examples
    ///
    /// ```
    /// let p = form_net::raw_pairs(b"a=%41", &form_net::Limits::DEFAULT).next().unwrap()?;
    /// assert_eq!(p.value_raw(), b"%41");
    /// # Ok::<(), form_net::FormError>(())
    /// ```
    #[must_use]
    pub const fn value_raw(&self) -> &'a [u8] {
        self.value
    }

    /// Whether the sequence contained `=`: `a` and `a=` both have an empty
    /// value, but only `a=` has an equals sign.
    ///
    /// # Examples
    ///
    /// ```
    /// use form_net::Limits;
    ///
    /// let mut it = form_net::raw_pairs(b"a&b=", &Limits::DEFAULT);
    /// assert!(!it.next().unwrap()?.has_equals());
    /// assert!(it.next().unwrap()?.has_equals());
    /// # Ok::<(), form_net::FormError>(())
    /// ```
    #[must_use]
    pub const fn has_equals(&self) -> bool {
        self.flags & HAS_EQUALS != 0
    }

    /// Whether the name contains `%` or `+`.
    ///
    /// # Examples
    ///
    /// ```
    /// let p = form_net::raw_pairs(b"a+b=1", &form_net::Limits::DEFAULT).next().unwrap()?;
    /// assert!(p.name_needs_decoding() && !p.value_needs_decoding());
    /// # Ok::<(), form_net::FormError>(())
    /// ```
    #[must_use]
    pub const fn name_needs_decoding(&self) -> bool {
        self.flags & NAME_DECODE != 0
    }

    /// Whether the value contains `%` or `+`.
    ///
    /// # Examples
    ///
    /// ```
    /// let p = form_net::raw_pairs(b"a=1%20", &form_net::Limits::DEFAULT).next().unwrap()?;
    /// assert!(p.value_needs_decoding());
    /// # Ok::<(), form_net::FormError>(())
    /// ```
    #[must_use]
    pub const fn value_needs_decoding(&self) -> bool {
        self.flags & VALUE_DECODE != 0
    }

    /// The name with no copy, when it needs no decoding and is valid UTF-8.
    ///
    /// # Examples
    ///
    /// ```
    /// let p = form_net::raw_pairs(b"email=x", &form_net::Limits::DEFAULT).next().unwrap()?;
    /// assert_eq!(p.name_str(), Some("email"));
    /// # Ok::<(), form_net::FormError>(())
    /// ```
    #[must_use]
    pub fn name_str(&self) -> Option<&'a str> {
        if self.name_needs_decoding() {
            None
        } else {
            core::str::from_utf8(self.name).ok()
        }
    }

    /// The value with no copy, when it needs no decoding and is valid UTF-8.
    ///
    /// # Examples
    ///
    /// ```
    /// let p = form_net::raw_pairs(b"q=a+b", &form_net::Limits::DEFAULT).next().unwrap()?;
    /// assert_eq!(p.value_str(), None);
    /// # Ok::<(), form_net::FormError>(())
    /// ```
    #[must_use]
    pub fn value_str(&self) -> Option<&'a str> {
        if self.value_needs_decoding() {
            None
        } else {
            core::str::from_utf8(self.value).ok()
        }
    }

    /// Decodes the name into `out` (see [`decode_into`](crate::decode_into)).
    ///
    /// # Errors
    ///
    /// `Encoding { InvalidPercent | InvalidUtf8 }` located in the body;
    /// `BufferTooSmall` when `out` is shorter than the raw name.
    ///
    /// # Examples
    ///
    /// ```
    /// use form_net::PercentPolicy;
    ///
    /// let p = form_net::raw_pairs(b"a%20b=1", &form_net::Limits::DEFAULT).next().unwrap()?;
    /// let mut out = [0u8; 16];
    /// assert_eq!(p.decode_name(&mut out, PercentPolicy::Refuse)?, "a b");
    /// # Ok::<(), form_net::FormError>(())
    /// ```
    pub fn decode_name<'b>(
        &self,
        out: &'b mut [u8],
        policy: PercentPolicy,
    ) -> Result<&'b str, FormError> {
        decode_component(self.name, out, policy).map_err(|e| e.shifted(self.offset))
    }

    /// Decodes the value into `out` (see [`decode_into`](crate::decode_into)).
    ///
    /// # Errors
    ///
    /// As [`decode_name`](Self::decode_name).
    ///
    /// # Examples
    ///
    /// ```
    /// use form_net::PercentPolicy;
    ///
    /// let p = form_net::raw_pairs(b"q=rust+forms", &form_net::Limits::DEFAULT).next().unwrap()?;
    /// let mut out = [0u8; 16];
    /// assert_eq!(p.decode_value(&mut out, PercentPolicy::Refuse)?, "rust forms");
    /// # Ok::<(), form_net::FormError>(())
    /// ```
    pub fn decode_value<'b>(
        &self,
        out: &'b mut [u8],
        policy: PercentPolicy,
    ) -> Result<&'b str, FormError> {
        let at = self
            .offset
            .saturating_add(self.name.len() as u64)
            .saturating_add(1);
        decode_component(self.value, out, policy).map_err(|e| e.shifted(at))
    }

    /// The byte offset of the sequence in the body.
    ///
    /// # Examples
    ///
    /// ```
    /// let mut it = form_net::raw_pairs(b"a=1&bb=2", &form_net::Limits::DEFAULT);
    /// let _ = it.next();
    /// assert_eq!(it.next().unwrap()?.offset(), 4);
    /// # Ok::<(), form_net::FormError>(())
    /// ```
    #[must_use]
    pub const fn offset(&self) -> u64 {
        self.offset
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec::Vec;

    type Pairs<'a> = Vec<(&'a [u8], &'a [u8])>;

    fn pairs(input: &[u8]) -> Result<Pairs<'_>, FormError> {
        raw_pairs(input, &Limits::DEFAULT)
            .map(|p| p.map(|p| (p.name_raw(), p.value_raw())))
            .collect()
    }

    #[test]
    fn test_raw_pairs_whatwg_splitting() {
        assert_eq!(pairs(b"").unwrap(), []);
        assert_eq!(pairs(b"&&&").unwrap(), []);
        assert_eq!(pairs(b"=").unwrap(), [(&b""[..], &b""[..])]);
        assert_eq!(pairs(b"a==b").unwrap(), [(&b"a"[..], &b"=b"[..])]);
        assert_eq!(pairs(b"=a").unwrap(), [(&b""[..], &b"a"[..])]);
        assert_eq!(pairs(b"a;b=c").unwrap(), [(&b"a;b"[..], &b"c"[..])]);
        assert_eq!(
            pairs(b"a=1&b=2&").unwrap(),
            [(&b"a"[..], &b"1"[..]), (&b"b"[..], &b"2"[..])]
        );
    }

    #[test]
    fn test_raw_pairs_flags_per_side() {
        let p = raw_pairs(b"a%=b", &Limits::DEFAULT)
            .next()
            .unwrap()
            .unwrap();
        assert!(p.name_needs_decoding() && !p.value_needs_decoding());
        let p = raw_pairs(b"a=b+%", &Limits::DEFAULT)
            .next()
            .unwrap()
            .unwrap();
        assert!(!p.name_needs_decoding() && p.value_needs_decoding());
        let p = raw_pairs(b"a=b=+", &Limits::DEFAULT)
            .next()
            .unwrap()
            .unwrap();
        assert!(p.value_needs_decoding() && p.has_equals());
    }

    #[test]
    fn test_raw_pairs_field_limit_n_and_n_plus_one() {
        let l = Limits::DEFAULT.with_max_fields(2);
        assert_eq!(raw_pairs(b"a&b", &l).count(), 2);
        let r: Result<Vec<_>, _> = raw_pairs(b"a&b&c", &l).collect();
        assert!(matches!(
            r,
            Err(FormError::Limit {
                limit: Limit::Fields,
                at
            }) if at.offset == 4
        ));
    }

    #[test]
    fn test_raw_pairs_length_limits_at_the_edge() {
        let l = Limits::DEFAULT
            .with_max_name_bytes(3)
            .with_max_value_bytes(3);
        assert!(raw_pairs(b"abc=def", &l).all(|p| p.is_ok()));
        let e = raw_pairs(b"abcd=1", &l).next().unwrap().unwrap_err();
        assert!(matches!(
            e,
            FormError::Limit {
                limit: Limit::NameLength,
                ..
            }
        ));
        let e = raw_pairs(b"a=defg", &l).next().unwrap().unwrap_err();
        assert!(matches!(e, FormError::Limit { limit: Limit::ValueLength, at } if at.offset == 2));
        let l = Limits::DEFAULT.with_max_text_bytes(4);
        let r: Result<Vec<_>, _> = raw_pairs(b"a=12&b=345", &l).collect();
        assert!(matches!(
            r,
            Err(FormError::Limit {
                limit: Limit::TextTotal,
                ..
            })
        ));
    }

    #[test]
    fn test_raw_pairs_body_limit_counts_empty_sequences() {
        let l = Limits::DEFAULT.with_max_body_bytes(4);
        assert_eq!(raw_pairs(b"&&&&", &l).count(), 0);
        let e = raw_pairs(b"&&&&&", &l).next().unwrap().unwrap_err();
        assert!(matches!(
            e,
            FormError::Limit {
                limit: Limit::Body,
                ..
            }
        ));
    }

    #[test]
    fn test_raw_pairs_stops_after_error() {
        let l = Limits::DEFAULT.with_max_fields(0);
        let mut it = raw_pairs(b"a&b", &l);
        assert!(it.next().unwrap().is_err());
        assert!(it.next().is_none());
    }

    #[test]
    fn test_raw_pair_debug_hides_values() {
        let p = raw_pairs(b"password=hunter2", &Limits::DEFAULT)
            .next()
            .unwrap()
            .unwrap();
        let s = alloc::format!("{p:?}");
        assert!(!s.contains("hunter2") && !s.contains("password"));
    }
}
