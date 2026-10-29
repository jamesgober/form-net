//! `application/x-www-form-urlencoded`: the WHATWG URL standard's parser
//! (section 5.1) with strict refusal where WHATWG would repair, and its
//! serializer.

pub(crate) mod decode;
pub(crate) mod encode;
pub(crate) mod raw;
pub(crate) mod stream;

#[cfg(feature = "alloc")]
mod owned {
    use alloc::borrow::Cow;
    use alloc::vec::Vec;
    use core::fmt;

    use super::decode::{decode_append, decode_to_vec};
    use super::raw::{RawPairs, raw_pairs};
    use crate::error::{ExtractError, FormError};
    use crate::limits::{Limits, PercentPolicy, Policy};

    #[derive(Clone, Copy)]
    struct Span {
        offset: u64,
        name_start: usize,
        name_len: usize,
        value_start: usize,
        value_len: usize,
    }

    /// The owned result of a urlencoded parse: every field, in input order,
    /// duplicates kept.
    ///
    /// Every decoded name and value lives in one buffer allocated once, and
    /// one table of positions; neither reallocates. Lookups are linear scans
    /// (there is no hash map, so hash flooding cannot apply; `max_fields`
    /// bounds every scan). On drop the buffer is wiped. `Debug` prints names
    /// and value lengths, never values. There is no `Clone`: a clone would
    /// be a second copy of every secret.
    ///
    /// # Examples
    ///
    /// ```
    /// let form = form_net::parse_urlencoded(b"role=user&role=admin&tag=red&tag=blue")?;
    /// assert_eq!(form.get("role"), Some("user"));                 // first wins
    /// assert!(form.get_one("role").is_err());                     // strict
    /// assert_eq!(form.get_one("missing")?, None);
    /// assert_eq!(form.get_all("tag").collect::<Vec<_>>(), ["red", "blue"]);
    /// assert_eq!(form.iter().count(), 4);
    /// assert_eq!(
    ///     format!("{form:?}"),
    ///     "Form { role: <4 bytes>, role: <5 bytes>, tag: <3 bytes>, tag: <4 bytes> }"
    /// );
    /// # Ok::<(), form_net::FormError>(())
    /// ```
    #[must_use]
    pub struct Form {
        arena: Vec<u8>,
        spans: Vec<Span>,
    }

    impl Form {
        fn text(&self, start: usize, len: usize) -> &str {
            let bytes = self
                .arena
                .get(start..start.saturating_add(len))
                .unwrap_or(&[]);
            // Every span was checked as UTF-8 when it was decoded.
            core::str::from_utf8(bytes).unwrap_or("")
        }

        fn name_of(&self, s: &Span) -> &str {
            self.text(s.name_start, s.name_len)
        }

        fn value_of(&self, s: &Span) -> &str {
            self.text(s.value_start, s.value_len)
        }

        /// The first value given for `name`, as browsers'
        /// `URLSearchParams.get` returns. Use [`get_one`](Self::get_one) where a
        /// repeated name must be refused.
        ///
        /// # Examples
        ///
        /// ```
        /// let form = form_net::parse_urlencoded(b"email=a%40b.example")?;
        /// assert_eq!(form.get("email"), Some("a@b.example"));
        /// assert_eq!(form.get("other"), None);
        /// # Ok::<(), form_net::FormError>(())
        /// ```
        #[must_use]
        pub fn get(&self, name: &str) -> Option<&str> {
            self.spans
                .iter()
                .find(|s| self.name_of(s) == name)
                .map(|s| self.value_of(s))
        }

        /// The value given for `name`, refusing a name given more than once
        /// (the parameter pollution defence).
        ///
        /// # Errors
        ///
        /// `Extract { reason: Duplicate, field: None }`.
        ///
        /// # Examples
        ///
        /// ```
        /// let form = form_net::parse_urlencoded(b"a=1&b=2&b=3")?;
        /// assert_eq!(form.get_one("a")?, Some("1"));
        /// assert!(form.get_one("b").is_err());
        /// # Ok::<(), form_net::FormError>(())
        /// ```
        pub fn get_one(&self, name: &str) -> Result<Option<&str>, FormError> {
            let mut found = None;
            for s in &self.spans {
                if self.name_of(s) == name {
                    if found.is_some() {
                        return Err(FormError::Extract {
                            reason: ExtractError::Duplicate,
                            field: None,
                        });
                    }
                    found = Some(self.value_of(s));
                }
            }
            Ok(found)
        }

        /// Every value given for `name`, in input order.
        ///
        /// # Examples
        ///
        /// ```
        /// let form = form_net::parse_urlencoded(b"t=a&x=1&t=b")?;
        /// assert_eq!(form.get_all("t").collect::<Vec<_>>(), ["a", "b"]);
        /// # Ok::<(), form_net::FormError>(())
        /// ```
        pub fn get_all<'f>(&'f self, name: &'f str) -> impl Iterator<Item = &'f str> + 'f {
            self.spans
                .iter()
                .filter(move |s| self.name_of(s) == name)
                .map(move |s| self.value_of(s))
        }

        /// Every `(name, value)`, in input order.
        ///
        /// # Examples
        ///
        /// ```
        /// let form = form_net::parse_urlencoded(b"a=1&b=2")?;
        /// assert_eq!(form.iter().collect::<Vec<_>>(), [("a", "1"), ("b", "2")]);
        /// # Ok::<(), form_net::FormError>(())
        /// ```
        pub fn iter(&self) -> impl Iterator<Item = (&str, &str)> {
            self.spans
                .iter()
                .map(|s| (self.name_of(s), self.value_of(s)))
        }

        /// Every name, in input order, repeats included.
        ///
        /// # Examples
        ///
        /// ```
        /// let form = form_net::parse_urlencoded(b"a=1&b=2&a=3")?;
        /// assert_eq!(form.names().collect::<Vec<_>>(), ["a", "b", "a"]);
        /// # Ok::<(), form_net::FormError>(())
        /// ```
        pub fn names(&self) -> impl Iterator<Item = &str> {
            self.spans.iter().map(|s| self.name_of(s))
        }

        /// The number of fields, repeats included.
        ///
        /// # Examples
        ///
        /// ```
        /// assert_eq!(form_net::parse_urlencoded(b"a&&b")?.len(), 2);
        /// # Ok::<(), form_net::FormError>(())
        /// ```
        #[must_use]
        pub fn len(&self) -> usize {
            self.spans.len()
        }

        /// Whether there are no fields.
        ///
        /// # Examples
        ///
        /// ```
        /// assert!(form_net::parse_urlencoded(b"&&")?.is_empty());
        /// # Ok::<(), form_net::FormError>(())
        /// ```
        #[must_use]
        pub fn is_empty(&self) -> bool {
            self.spans.is_empty()
        }

        /// Whether `name` was given at least once.
        ///
        /// # Examples
        ///
        /// ```
        /// let form = form_net::parse_urlencoded(b"remember")?;
        /// assert!(form.contains("remember"));
        /// # Ok::<(), form_net::FormError>(())
        /// ```
        #[must_use]
        pub fn contains(&self, name: &str) -> bool {
            self.spans.iter().any(|s| self.name_of(s) == name)
        }
    }

    impl Drop for Form {
        fn drop(&mut self) {
            crate::wipe::wipe_vec(&mut self.arena);
        }
    }

    impl fmt::Debug for Form {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            struct Len(usize);
            impl fmt::Debug for Len {
                fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                    if self.0 == 1 {
                        f.write_str("<1 byte>")
                    } else {
                        write!(f, "<{} bytes>", self.0)
                    }
                }
            }
            let mut m = f.debug_struct("Form");
            for s in &self.spans {
                m.field(self.name_of(s), &Len(s.value_len));
            }
            m.finish()
        }
    }

    fn build(input: &[u8], limits: &Limits, percent: PercentPolicy) -> Result<Form, FormError> {
        let mut pairs = raw_pairs(input, limits);
        // The span table is sized from the separator count, capped by the
        // field limit; the arena from the input length (decoding never makes
        // text longer). Neither grows afterwards.
        let separators = crate::scan::count_byte(input, b'&');
        let cap = separators
            .saturating_add(1)
            .min(usize::try_from(limits.max_fields).unwrap_or(usize::MAX))
            .min(input.len());
        let mut spans = Vec::with_capacity(cap);
        let mut arena = alloc::vec![0u8; input.len().min(limits.body_usize())];
        let mut used = 0usize;
        let result = collect(&mut pairs, &mut arena, &mut used, &mut spans, percent);
        if let Err(e) = result {
            crate::wipe::wipe(&mut arena);
            return Err(e);
        }
        arena.truncate(used);
        Ok(Form { arena, spans })
    }

    fn collect(
        pairs: &mut RawPairs<'_>,
        arena: &mut [u8],
        used: &mut usize,
        spans: &mut Vec<Span>,
        percent: PercentPolicy,
    ) -> Result<(), FormError> {
        for pair in pairs {
            let pair = pair?;
            let name_start = *used;
            let name_len = decode_append(pair.name_raw(), arena, name_start, percent)
                .map_err(|e| e.shifted(pair.offset()))?;
            let value_start = name_start.saturating_add(name_len);
            let value_at = pair
                .offset()
                .saturating_add(pair.name_raw().len() as u64)
                .saturating_add(1);
            let value_len = decode_append(pair.value_raw(), arena, value_start, percent)
                .map_err(|e| e.shifted(value_at))?;
            *used = value_start.saturating_add(value_len);
            spans.push(Span {
                offset: pair.offset(),
                name_start,
                name_len,
                value_start,
                value_len,
            });
        }
        Ok(())
    }

    /// Parses a whole `application/x-www-form-urlencoded` body with the
    /// default limits and policy, and returns the fields, owned, in input
    /// order.
    ///
    /// `&` is the only separator (`;` is an ordinary byte), `+` is a space,
    /// and a malformed `%` or invalid UTF-8 is refused, never repaired.
    ///
    /// # Errors
    ///
    /// `Limit { Body | Fields | NameLength | ValueLength | TextTotal }`;
    /// `Encoding { InvalidPercent | InvalidUtf8 }`.
    ///
    /// # Examples
    ///
    /// ```
    /// let form = form_net::parse_urlencoded(b"email=a%40b.example&remember=on")?;
    /// assert_eq!(form.get("email"), Some("a@b.example"));
    /// assert_eq!(form.get("remember"), Some("on"));
    ///
    /// let form = form_net::parse_urlencoded(b"a=1;b=2")?;
    /// assert_eq!(form.get("a"), Some("1;b=2"));
    ///
    /// assert!(form_net::parse_urlencoded(b"name=%zz").is_err());
    /// assert!(form_net::parse_urlencoded(b"name=%C0%AF").is_err());
    /// # Ok::<(), form_net::FormError>(())
    /// ```
    pub fn parse_urlencoded(input: &[u8]) -> Result<Form, FormError> {
        build(input, &Limits::DEFAULT, PercentPolicy::Refuse)
    }

    /// As [`parse_urlencoded`], after stripping exactly one leading `?` if
    /// there is one.
    ///
    /// # Errors
    ///
    /// As [`parse_urlencoded`].
    ///
    /// # Examples
    ///
    /// ```
    /// let q = form_net::parse_query(b"?q=rust+forms&page=2")?;
    /// assert_eq!(q.get("q"), Some("rust forms"));
    /// assert_eq!(form_net::parse_query(b"??q=x")?.get("?q"), Some("x"));
    /// # Ok::<(), form_net::FormError>(())
    /// ```
    pub fn parse_query(query: &[u8]) -> Result<Form, FormError> {
        parse_urlencoded(query.strip_prefix(b"?").unwrap_or(query))
    }

    /// As [`parse_urlencoded`] with explicit limits and policy.
    /// `limits.validate()` runs first. Of the policy, only `percent` applies
    /// to urlencoded bodies.
    ///
    /// # Errors
    ///
    /// As [`parse_urlencoded`]; `InvalidLimits` from `validate`.
    ///
    /// # Examples
    ///
    /// ```
    /// use form_net::{Limits, Policy, PercentPolicy};
    ///
    /// const LOGIN: Limits = Limits::DEFAULT.with_max_body_bytes(4 * 1024).with_max_fields(8);
    /// let form = form_net::parse_urlencoded_with(b"email=a%40b.example&password=x", &LOGIN, &Policy::DEFAULT)?;
    /// assert_eq!(form.len(), 2);
    ///
    /// const LIKE_BROWSERS: Policy = Policy::DEFAULT.with_percent(PercentPolicy::Literal);
    /// let form = form_net::parse_urlencoded_with(b"discount=100%", &Limits::DEFAULT, &LIKE_BROWSERS)?;
    /// assert_eq!(form.get("discount"), Some("100%"));
    /// # Ok::<(), form_net::FormError>(())
    /// ```
    pub fn parse_urlencoded_with(
        input: &[u8],
        limits: &Limits,
        policy: &Policy,
    ) -> Result<Form, FormError> {
        limits.validate()?;
        build(input, limits, policy.percent)
    }

    /// As [`parse_urlencoded_with`], calling `inspector.field(name, value)`
    /// for every pair in input order (decoded value bytes), then
    /// `inspector.end()`. On any error, including a refusal,
    /// `inspector.failed(&error)` is called once before the error is
    /// returned. A `Verdict::Skip` from `field` drops that pair. Inspection
    /// never runs on a body form-net refuses.
    ///
    /// # Errors
    ///
    /// As [`parse_urlencoded_with`]; `Refused`.
    ///
    /// # Examples
    ///
    /// ```
    /// use form_net::{Inspector, Limits, Policy, Refusal, Verdict};
    ///
    /// struct NoAdmins;
    /// impl Inspector for NoAdmins {
    ///     fn field(&mut self, name: &str, value: &[u8]) -> Verdict {
    ///         if name == "role" && value == b"admin" {
    ///             Verdict::Refuse(Refusal::new(422, "example.role_not_allowed"))
    ///         } else {
    ///             Verdict::Continue
    ///         }
    ///     }
    /// }
    ///
    /// let err = form_net::parse_urlencoded_inspected(b"role=admin", &Limits::DEFAULT, &Policy::DEFAULT, &mut NoAdmins)
    ///     .unwrap_err();
    /// assert_eq!(err.status_hint(), 422);
    /// ```
    pub fn parse_urlencoded_inspected<I: crate::Inspector>(
        input: &[u8],
        limits: &Limits,
        policy: &Policy,
        inspector: &mut I,
    ) -> Result<Form, FormError> {
        let result = parse_urlencoded_with(input, limits, policy).and_then(|mut form| {
            let mut keep = Vec::with_capacity(form.spans.len());
            for s in &form.spans {
                let at = crate::error::Location::at(s.offset);
                match inspector.field(form.name_of(s), form.value_of(s).as_bytes()) {
                    crate::Verdict::Refuse(refusal) => {
                        return Err(FormError::Refused { refusal, at });
                    }
                    crate::Verdict::Skip => keep.push(false),
                    _ => keep.push(true),
                }
            }
            let mut flags = keep.into_iter();
            form.spans.retain(|_| flags.next().unwrap_or(false));
            if let crate::Verdict::Refuse(refusal) = inspector.end() {
                let at = crate::error::Location::at(input.len() as u64);
                return Err(FormError::Refused { refusal, at });
            }
            Ok(form)
        });
        if let Err(e) = &result {
            inspector.failed(e);
        }
        result
    }

    /// Iterates the fields of a urlencoded body lazily, with the default
    /// limits. A name or value that needs no decoding is borrowed from the
    /// input; others are decoded into a new `String` (which belongs to the
    /// caller and is not wiped: use [`Form`] for secret fields). After the
    /// first error the iterator returns `None`.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::borrow::Cow;
    ///
    /// let mut it = form_net::pairs(b"a=1&b=x+y");
    /// let (name, value) = it.next().unwrap()?;
    /// assert!(matches!((name, value), (Cow::Borrowed("a"), Cow::Borrowed("1"))));
    /// let (_, value) = it.next().unwrap()?;
    /// assert!(matches!(value, Cow::Owned(ref s) if s == "x y"));
    /// assert!(it.next().is_none());
    /// # Ok::<(), form_net::FormError>(())
    /// ```
    pub fn pairs(input: &[u8]) -> Pairs<'_> {
        Pairs {
            raw: raw_pairs(input, &Limits::DEFAULT),
            done: false,
        }
    }

    /// The iterator returned by [`pairs`].
    #[derive(Debug, Clone)]
    pub struct Pairs<'a> {
        raw: RawPairs<'a>,
        done: bool,
    }

    fn component(raw: &[u8], needs_decoding: bool, offset: u64) -> Result<Cow<'_, str>, FormError> {
        if !needs_decoding {
            if let Ok(s) = core::str::from_utf8(raw) {
                return Ok(Cow::Borrowed(s));
            }
        }
        decode_to_vec(raw, PercentPolicy::Refuse)
            .map(Cow::Owned)
            .map_err(|e| e.shifted(offset))
    }

    impl<'a> Iterator for Pairs<'a> {
        type Item = Result<(Cow<'a, str>, Cow<'a, str>), FormError>;

        fn next(&mut self) -> Option<Self::Item> {
            if self.done {
                return None;
            }
            let item = match self.raw.next()? {
                Ok(p) => {
                    let value_at = p
                        .offset()
                        .saturating_add(p.name_raw().len() as u64)
                        .saturating_add(1);
                    component(p.name_raw(), p.name_needs_decoding(), p.offset()).and_then(|name| {
                        component(p.value_raw(), p.value_needs_decoding(), value_at)
                            .map(|value| (name, value))
                    })
                }
                Err(e) => Err(e),
            };
            if item.is_err() {
                self.done = true;
            }
            Some(item)
        }
    }

    impl core::iter::FusedIterator for Pairs<'_> {}

    #[cfg(test)]
    mod tests {
        use super::*;
        use crate::error::{EncodingError, Limit, Location};
        use alloc::format;

        #[test]
        fn test_parse_urlencoded_whatwg_cases() {
            type Case<'a> = (&'a [u8], &'a [(&'a str, &'a str)]);
            let cases: &[Case<'_>] = &[
                (b"", &[]),
                (b"a", &[("a", "")]),
                (b"a=", &[("a", "")]),
                (b"=a", &[("", "a")]),
                (b"a==b", &[("a", "=b")]),
                (b"a=b&c=d", &[("a", "b"), ("c", "d")]),
                (b"&&a=1&&", &[("a", "1")]),
                (b"+=+", &[(" ", " ")]),
                (b"%2B=%20", &[("+", " ")]),
                (b"a%26b=c%3Dd", &[("a&b", "c=d")]),
            ];
            for (input, expected) in cases {
                let form = parse_urlencoded(input).unwrap();
                assert_eq!(form.iter().collect::<Vec<_>>(), *expected, "{input:?}");
            }
        }

        #[test]
        fn test_parse_urlencoded_error_locations_are_absolute() {
            let e = parse_urlencoded(b"ok=1&bad=%zz").unwrap_err();
            assert_eq!(
                e,
                FormError::encoding(EncodingError::InvalidPercent, Location::at(9))
            );
            let e = parse_urlencoded(b"ok=1&%C0=x").unwrap_err();
            assert_eq!(
                e,
                FormError::encoding(EncodingError::InvalidUtf8, Location::at(5))
            );
        }

        #[test]
        fn test_parse_urlencoded_with_validates_limits() {
            let bad = Limits::DEFAULT.with_max_part_header_bytes(1);
            assert!(matches!(
                parse_urlencoded_with(b"a=1", &bad, &Policy::DEFAULT),
                Err(FormError::InvalidLimits { .. })
            ));
            let small = Limits::DEFAULT.with_max_body_bytes(3);
            assert!(matches!(
                parse_urlencoded_with(b"a=12", &small, &Policy::DEFAULT),
                Err(FormError::Limit {
                    limit: Limit::Body,
                    ..
                })
            ));
        }

        #[test]
        fn test_form_debug_hides_values() {
            let form = parse_urlencoded(b"password=hunter2&x=1").unwrap();
            let s = format!("{form:?}");
            assert_eq!(s, "Form { password: <7 bytes>, x: <1 byte> }");
        }

        #[test]
        fn test_pairs_error_then_none() {
            let mut it = pairs(b"a=%&b=1");
            assert!(it.next().unwrap().is_err());
            assert!(it.next().is_none());
        }

        #[test]
        fn test_form_two_allocations_sizes() {
            let form = parse_urlencoded(b"a=%41%42&b=c").unwrap();
            assert_eq!(form.arena.len(), 5);
            assert_eq!(form.get("a"), Some("AB"));
        }
    }
}

#[cfg(feature = "alloc")]
pub use owned::{
    Form, Pairs, pairs, parse_query, parse_urlencoded, parse_urlencoded_inspected,
    parse_urlencoded_with,
};
