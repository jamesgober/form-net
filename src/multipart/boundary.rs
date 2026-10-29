//! The multipart boundary: validation, the delimiter bytes, and the failure
//! table of the streaming matcher.

use core::fmt;

use crate::error::{FormError, Location, SyntaxError};

/// The longest boundary RFC 2046 allows.
pub const MAX_BOUNDARY_LEN: usize = 70;

/// The longest delimiter: CRLF, `--`, and a 70-byte boundary.
pub const MAX_DELIMITER_LEN: usize = 74;

/// RFC 2046 `bchars`: digits, letters, `'()+_,-./:=?`, and space.
const fn is_bchar(b: u8) -> bool {
    b.is_ascii_alphanumeric()
        || matches!(
            b,
            b'\''
                | b'('
                | b')'
                | b'+'
                | b'_'
                | b','
                | b'-'
                | b'.'
                | b'/'
                | b':'
                | b'='
                | b'?'
                | b' '
        )
}

/// A validated boundary with its delimiter (`CRLF "--" boundary`, 5 to 74
/// bytes) and the failure table of the streaming matcher, built once.
///
/// `Debug` prints the boundary: it is not secret.
///
/// # Examples
///
/// ```
/// use form_net::Boundary;
///
/// let b = Boundary::new(b"----WebKitFormBoundaryx7Y")?;
/// assert_eq!(b.delimiter(), b"\r\n------WebKitFormBoundaryx7Y");
/// assert!(Boundary::new(b"").is_err());
/// assert!(Boundary::new(&[b'a'; 71]).is_err());
/// assert!(Boundary::new(b"ends with space ").is_err());
/// assert!(Boundary::new(b"has\"quote").is_err());
/// assert_eq!(Boundary::parse(b"multipart/form-data; boundary=\"a b\"")?.as_bytes(), b"a b");
/// # Ok::<(), form_net::FormError>(())
/// ```
#[must_use]
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Boundary {
    delimiter: [u8; MAX_DELIMITER_LEN],
    fail: [u8; MAX_DELIMITER_LEN],
    len: u8,
}

impl Boundary {
    /// Validates a bare boundary (without `--`): 1 to 70 bytes of RFC 2046
    /// `bchars`, not ending in a space.
    ///
    /// # Errors
    ///
    /// `Syntax { InvalidBoundary }`, at the offending byte.
    ///
    /// # Examples
    ///
    /// ```
    /// assert_eq!(form_net::Boundary::new(b"X")?.delimiter(), b"\r\n--X");
    /// # Ok::<(), form_net::FormError>(())
    /// ```
    pub fn new(boundary: &[u8]) -> Result<Boundary, FormError> {
        let invalid =
            |at: usize| FormError::syntax(SyntaxError::InvalidBoundary, Location::at(at as u64));
        if boundary.is_empty() {
            return Err(invalid(0));
        }
        if boundary.len() > MAX_BOUNDARY_LEN {
            return Err(invalid(MAX_BOUNDARY_LEN));
        }
        if let Some(at) = boundary.iter().position(|&b| !is_bchar(b)) {
            return Err(invalid(at));
        }
        if boundary.last() == Some(&b' ') {
            return Err(invalid(boundary.len().wrapping_sub(1)));
        }
        let mut delimiter = [0u8; MAX_DELIMITER_LEN];
        let len = boundary.len().wrapping_add(4);
        if let Some(head) = delimiter.get_mut(..4) {
            head.copy_from_slice(b"\r\n--");
        }
        if let Some(rest) = delimiter.get_mut(4..len) {
            rest.copy_from_slice(boundary);
        }
        let fail = failure_table(&delimiter, len);
        Ok(Boundary {
            delimiter,
            fail,
            len: len as u8,
        })
    }

    /// A fresh boundary: `form-net-` and 32 lowercase hex characters from 16
    /// bytes of the OS random source (41 bytes, all `bchars`).
    ///
    /// # Errors
    ///
    /// `Random` when the OS random source fails (fail closed).
    ///
    /// # Examples
    ///
    /// ```
    /// let b = form_net::Boundary::generate()?;
    /// assert_eq!(b.as_bytes().len(), 41);
    /// assert!(b.as_bytes().starts_with(b"form-net-"));
    /// assert_ne!(b, form_net::Boundary::generate()?);
    /// # Ok::<(), form_net::FormError>(())
    /// ```
    #[cfg(feature = "random")]
    #[cfg_attr(docsrs, doc(cfg(feature = "random")))]
    pub fn generate() -> Result<Boundary, FormError> {
        let hex = crate::random::random_hex()?;
        let mut name = [0u8; 41];
        if let Some(head) = name.get_mut(..9) {
            head.copy_from_slice(b"form-net-");
        }
        if let Some(tail) = name.get_mut(9..) {
            tail.copy_from_slice(&hex);
        }
        Boundary::new(&name)
    }

    /// Reads the boundary from a whole `multipart/form-data` `Content-Type`
    /// value.
    ///
    /// # Errors
    ///
    /// `UnsupportedMediaType` for any other media type;
    /// `Syntax { InvalidContentType | MissingBoundary | DuplicateBoundary | InvalidBoundary }`.
    ///
    /// # Examples
    ///
    /// ```
    /// let b = form_net::Boundary::parse(b"multipart/form-data; boundary=----x")?;
    /// assert_eq!(b.as_bytes(), b"----x");
    /// assert!(form_net::Boundary::parse(b"text/plain").is_err());
    /// # Ok::<(), form_net::FormError>(())
    /// ```
    pub fn parse(content_type: &[u8]) -> Result<Boundary, FormError> {
        match crate::kind::FormKind::detect(content_type)? {
            crate::kind::FormKind::Multipart(b) => Ok(b),
            crate::kind::FormKind::Urlencoded => Err(FormError::UnsupportedMediaType),
        }
    }

    /// The boundary, without `--`.
    ///
    /// # Examples
    ///
    /// ```
    /// assert_eq!(form_net::Boundary::new(b"abc")?.as_bytes(), b"abc");
    /// # Ok::<(), form_net::FormError>(())
    /// ```
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        self.delimiter.get(4..usize::from(self.len)).unwrap_or(&[])
    }

    /// The full delimiter: CRLF, `--`, and the boundary.
    ///
    /// # Examples
    ///
    /// ```
    /// assert_eq!(form_net::Boundary::new(b"abc")?.delimiter(), b"\r\n--abc");
    /// # Ok::<(), form_net::FormError>(())
    /// ```
    #[must_use]
    pub fn delimiter(&self) -> &[u8] {
        self.delimiter.get(..usize::from(self.len)).unwrap_or(&[])
    }

    /// One step of the matcher: the new state after byte `c` from state `k`
    /// (the number of delimiter bytes matched so far, below the length).
    #[inline]
    pub(crate) fn step(&self, mut k: usize, c: u8) -> usize {
        #[cfg(test)]
        tests::count_visit();
        loop {
            if self.delimiter.get(k) == Some(&c) {
                return k.wrapping_add(1);
            }
            if k == 0 {
                return 0;
            }
            k = usize::from(*self.fail.get(k.wrapping_sub(1)).unwrap_or(&0));
        }
    }

    pub(crate) fn len(&self) -> usize {
        usize::from(self.len)
    }

    /// One step of the bare-break matcher: the new state after byte `c`
    /// from state `m`. It matches a lone CR or LF followed by `--boundary`,
    /// the delimiter with its first byte made a choice of CR or LF and its
    /// LF dropped. The boundary holds neither, so the only restart point is
    /// the line break itself and no failure table is needed (D-113).
    #[inline]
    pub(crate) fn bare_step(&self, m: usize, c: u8) -> usize {
        if m > 0 && self.delimiter.get(m.wrapping_add(1)) == Some(&c) {
            m.wrapping_add(1)
        } else {
            usize::from(crate::scan::is_break(c))
        }
    }

    /// Runs both matchers over `hay` from states `k` (the delimiter) and
    /// `m` (a line break followed by `--boundary`). Returns the final
    /// states and what completed first; a completed delimiter wins over
    /// the bare-break match that ends at the same byte inside it.
    ///
    /// From states 0 a match can only start at a CR or LF followed by `-`:
    /// the delimiter's LF is followed by `--`, and the boundary holds no
    /// line break. So the search skips to the next such pair 32 positions
    /// at a time and compares `--boundary` after it whole; the byte before
    /// tells a delimiter (CR LF) from a bare break. A miss resumes one byte
    /// on. The automata are stepped byte by byte only where a match spans
    /// chunks: at the start of `hay`, until no match carried in reaches back
    /// before it, and near the end, from the CR before the LF; so the
    /// states returned are exactly the automata's (D-102, D-113). The
    /// boundary's lack of line breaks also keeps the compares linear: a
    /// candidate can never start inside the bytes the one before it matched.
    pub(crate) fn run(&self, mut k: usize, mut m: usize, hay: &[u8]) -> (usize, usize, Hit) {
        let len = self.len();
        let bare = len.wrapping_sub(1);
        let delimiter = self.delimiter();
        let dashes = delimiter.get(2..).unwrap_or(&[]);
        let mut i = 0usize;
        if k > 0 || m > 0 {
            // A match carried in: step until every live partial match lies
            // inside `hay`, then search again from where the longest starts.
            while let Some(&c) = hay.get(i) {
                k = self.step(k, c);
                m = self.bare_step(m, c);
                i = i.wrapping_add(1);
                if k == len {
                    return (k, m, Hit::Delimiter(i));
                }
                if m == bare {
                    return (k, m, Hit::BareBreak(i));
                }
                if k.max(m) <= i {
                    i = i.wrapping_sub(k.max(m));
                    (k, m) = (0, 0);
                    break;
                }
            }
            if k > 0 || m > 0 {
                // All of `hay` extended the match.
                return (k, m, Hit::None);
            }
        }
        while i < hay.len() {
            let at = crate::scan::find_break_dash(hay, i);
            if at >= hay.len() {
                break;
            }
            let start = at.wrapping_add(1);
            let after_cr = hay.get(at) == Some(&b'\n')
                && at > 0
                && hay.get(at.wrapping_sub(1)) == Some(&b'\r');
            if let Some(candidate) = hay.get(start..start.wrapping_add(dashes.len())) {
                // The last byte first: a near miss usually differs there.
                if candidate.last() == dashes.last() && candidate == dashes {
                    let end = start.wrapping_add(dashes.len());
                    return if after_cr {
                        (len, 0, Hit::Delimiter(end))
                    } else {
                        (0, bare, Hit::BareBreak(end))
                    };
                }
                i = start;
                continue;
            }
            // Too close to the end to compare whole: step the automata from
            // here (from the CR of a CR LF) to the end.
            let mut j = if after_cr { at.wrapping_sub(1) } else { at };
            while let Some(&c) = hay.get(j) {
                k = self.step(k, c);
                m = self.bare_step(m, c);
                j = j.wrapping_add(1);
                if k == len {
                    return (k, m, Hit::Delimiter(j));
                }
                if m == bare {
                    return (k, m, Hit::BareBreak(j));
                }
            }
            return (k, m, Hit::None);
        }
        (0, 0, Hit::None)
    }
}

/// What a run of the matchers found.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Hit {
    /// Nothing: all of `hay` was consumed.
    None,
    /// The delimiter, ending just before this index.
    Delimiter(usize),
    /// `--boundary` after a lone CR or LF, ending just before this index
    /// (D-113).
    BareBreak(usize),
}

/// The Knuth-Morris-Pratt failure table: `fail[i]` is the length of the
/// longest proper prefix of `d[..=i]` that is also its suffix.
fn failure_table(d: &[u8; MAX_DELIMITER_LEN], len: usize) -> [u8; MAX_DELIMITER_LEN] {
    let mut fail = [0u8; MAX_DELIMITER_LEN];
    let mut k = 0usize;
    for i in 1..len {
        let c = *d.get(i).unwrap_or(&0);
        while k > 0 && d.get(k) != Some(&c) {
            k = usize::from(*fail.get(k.wrapping_sub(1)).unwrap_or(&0));
        }
        if d.get(k) == Some(&c) {
            k = k.wrapping_add(1);
        }
        if let Some(slot) = fail.get_mut(i) {
            *slot = k as u8;
        }
    }
    fail
}

impl fmt::Debug for Boundary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // bchars are printable ASCII, so this is always valid text.
        let text = core::str::from_utf8(self.as_bytes()).unwrap_or("");
        f.debug_tuple("Boundary").field(&text).finish()
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use alloc::vec::Vec;
    use core::cell::Cell;

    std::thread_local! {
        static VISITS: Cell<u64> = const { Cell::new(0) };
    }

    /// Counts one byte the matcher compares (one call to `step`, which may
    /// follow the failure table several times: KMP bounds those by the
    /// bytes seen).
    pub(crate) fn count_visit() {
        VISITS.with(|v| v.set(v.get() + 1));
    }

    #[cfg(feature = "alloc")]
    pub(crate) fn visits() -> u64 {
        VISITS.with(Cell::get)
    }

    /// Runs `body` through a parser in `chunk`-byte pieces and returns the
    /// matcher's byte visits.
    #[cfg(feature = "alloc")]
    fn visits_for(body: &[u8], chunk: usize) -> u64 {
        use crate::{Event, Limits, MultipartParser, Policy};
        let limits = Limits::DEFAULT
            .with_max_body_bytes(64 << 20)
            .with_max_file_bytes(64 << 20);
        let mut p = MultipartParser::new(
            Boundary::new(b"boundary").unwrap(),
            &limits,
            &Policy::DEFAULT,
        )
        .unwrap();
        let before = visits();
        let chunks: Vec<&[u8]> = body.chunks(chunk).collect();
        for (i, c) in chunks.iter().enumerate() {
            let mut rest: &[u8] = c;
            loop {
                let step = p.parse(rest, i + 1 == chunks.len()).unwrap();
                rest = &rest[step.consumed..];
                if matches!(step.event, Event::NeedInput | Event::End) {
                    break;
                }
            }
        }
        visits() - before
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn test_matcher_visits_are_linear_for_adversarial_bodies() {
        let head = b"--boundary\r\nContent-Disposition: form-data; name=f; filename=a\r\n\r\n";
        let shapes: [&[u8]; 4] = [b"\r", b"\r\n--boundar", b"\r\n-", b"-"];
        for shape in shapes {
            for chunk in [1usize, 7, 4096] {
                let mut body = head.to_vec();
                while body.len() < head.len() + 64 * 1024 {
                    body.extend_from_slice(shape);
                }
                let data = (body.len() - head.len()) as u64;
                body.extend_from_slice(b"\r\n--boundary--");
                let v = visits_for(&body, chunk);
                // Each data byte is examined a bounded number of times
                // (KMP), plus at most a delimiter's length rescanned per
                // chunk boundary.
                let bound = 2 * data + (body.len() / chunk + 1) as u64 * 16 + 64;
                assert!(
                    v <= bound,
                    "{shape:?} chunk {chunk}: {v} visits for {data} bytes"
                );
            }
        }
    }

    #[test]
    fn test_boundary_every_byte_value() {
        for b in 0u8..=255 {
            let r = Boundary::new(&[b'a', b, b'z']);
            assert_eq!(r.is_ok(), is_bchar(b), "byte {b:#x}");
        }
    }

    #[test]
    fn test_boundary_lengths_and_trailing_space() {
        assert!(Boundary::new(b"").is_err());
        assert!(Boundary::new(b"a").is_ok());
        assert!(Boundary::new(&[b'a'; 69]).is_ok());
        assert!(Boundary::new(&[b'a'; 70]).is_ok());
        assert!(Boundary::new(&[b'a'; 71]).is_err());
        assert!(Boundary::new(b"a ").is_err());
        assert!(Boundary::new(b"a b").is_ok());
        assert!(Boundary::new(b" ").is_err());
    }

    #[test]
    fn test_failure_table_matches_definition() {
        for raw in [&b"a"[..], b"aaaa", b"-", b"--", b"-a-", b"---", b"a-b--a"] {
            let b = Boundary::new(raw).unwrap();
            let d = b.delimiter();
            for i in 0..d.len() {
                let best = (0..=i)
                    .rev()
                    .find(|&l| l <= i && d[..l] == d[i + 1 - l..=i])
                    .unwrap();
                assert_eq!(usize::from(b.fail[i]), best, "{raw:?} at {i}");
            }
        }
    }

    /// The first match in `hay` by a plain search: the delimiter, or a
    /// lone CR or LF followed by `--boundary`, by where it ends.
    fn naive_hit(b: &Boundary, hay: &[u8]) -> Hit {
        let d = b.delimiter();
        let dashes = &d[2..];
        for end in 0..=hay.len() {
            if end >= d.len() && &hay[end - d.len()..end] == d {
                return Hit::Delimiter(end);
            }
            if end > dashes.len() && &hay[end - dashes.len()..end] == dashes {
                let p = end - dashes.len() - 1;
                if hay[p] == b'\r' || hay[p] == b'\n' {
                    return Hit::BareBreak(end);
                }
            }
        }
        Hit::None
    }

    /// Runs the matchers over `chunks` in turn, carrying the states, and
    /// returns the first hit with its index in the whole.
    fn chunked_hit(b: &Boundary, chunks: &[&[u8]]) -> Hit {
        let (mut k, mut m, mut base) = (0, 0, 0);
        for chunk in chunks {
            let (k2, m2, hit) = b.run(k, m, chunk);
            match hit {
                Hit::Delimiter(e) => return Hit::Delimiter(base + e),
                Hit::BareBreak(e) => return Hit::BareBreak(base + e),
                Hit::None => (k, m, base) = (k2, m2, base + chunk.len()),
            }
        }
        Hit::None
    }

    #[test]
    fn test_run_finds_like_naive_and_carries_state() {
        let b = Boundary::new(b"-ab").unwrap();
        let d = b.delimiter().to_vec();
        let mut hay = Vec::new();
        hay.extend_from_slice(b"x\r\n--a\r\n\r\n--\r\n--");
        hay.extend_from_slice(&d);
        hay.extend_from_slice(b"tail");
        let expected = naive_hit(&b, &hay);
        assert!(matches!(expected, Hit::Delimiter(_)));
        assert_eq!(chunked_hit(&b, &[&hay]), expected);
        for split in 0..hay.len() {
            let (x, y) = hay.split_at(split);
            assert_eq!(chunked_hit(&b, &[x, y]), expected, "split {split}");
        }
    }

    /// D-113: the delimiter and bare breaks against the plain search, on
    /// hays dense in CR, LF, and dashes, whole, at every split, and byte
    /// by byte, for boundaries with and without dashes of their own.
    #[test]
    fn test_bare_breaks_and_delimiters_match_naive_at_every_split() {
        let mut seed = 11u64;
        for boundary in [&b"-ab"[..], b"--", b"x", b"ab-"] {
            let b = Boundary::new(boundary).unwrap();
            for len in 0..40 {
                for _ in 0..60 {
                    let hay: Vec<u8> = (0..len)
                        .map(|_| {
                            seed ^= seed << 13;
                            seed ^= seed >> 7;
                            seed ^= seed << 17;
                            b"\r\n--abx"[(seed % 7) as usize]
                        })
                        .collect();
                    let expected = naive_hit(&b, &hay);
                    assert_eq!(chunked_hit(&b, &[&hay]), expected, "{hay:?}");
                    for split in 0..=hay.len() {
                        let (x, y) = hay.split_at(split);
                        assert_eq!(chunked_hit(&b, &[x, y]), expected, "{hay:?} at {split}");
                    }
                    let bytes: Vec<&[u8]> = hay.chunks(1).collect();
                    assert_eq!(chunked_hit(&b, &bytes), expected, "{hay:?} bytewise");
                }
            }
        }
    }

    #[test]
    fn test_parse_reports_unsupported_for_urlencoded() {
        assert_eq!(
            Boundary::parse(b"application/x-www-form-urlencoded"),
            Err(FormError::UnsupportedMediaType)
        );
    }
}
