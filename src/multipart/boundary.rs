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
        tests::count_visits(1);
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
    /// tells a delimiter (CR LF) from a bare break. A miss resumes at the
    /// byte that differed: the bytes before it are boundary, which holds no
    /// line break. A match that spans chunks is compared too, against the bytes it
    /// still needs at the start of `hay` and the bytes `hay` still has at
    /// its end: with no line break in the boundary, at most one match is
    /// live past a lone CR, and the moment it fails nothing else is. So the
    /// states returned are exactly the automata's, at a compare per chunk
    /// (D-102, D-113, D-117). The same fact keeps the compares linear: a
    /// candidate can never start inside the bytes the one before it matched.
    pub(crate) fn run(&self, mut k: usize, mut m: usize, hay: &[u8]) -> (usize, usize, Hit) {
        let len = self.len();
        let bare = len.wrapping_sub(1);
        let delimiter = self.delimiter();
        let dashes = delimiter.get(2..).unwrap_or(&[]);
        let mut i = 0usize;
        if k == 1 {
            // A CR carried in is the one state with two futures: the next
            // byte makes it a delimiter, a bare break, or nothing.
            let Some(&c) = hay.first() else {
                return (k, m, Hit::None);
            };
            k = self.step(k, c);
            m = self.bare_step(m, c);
            i = 1;
            if k == 1 {
                // Another CR: the search below starts at it.
                (i, k, m) = (0, 0, 0);
            }
        }
        if m > 0 {
            // One match carried in, delimiter or bare: compare what it
            // still needs.
            let need = delimiter.get(m.wrapping_add(1)..).unwrap_or(&[]);
            let rest = hay.get(i..).unwrap_or(&[]);
            let same = shared(need, rest);
            if same == need.len() {
                let end = i.wrapping_add(same);
                return if k > 0 {
                    (len, 0, Hit::Delimiter(end))
                } else {
                    (0, bare, Hit::BareBreak(end))
                };
            }
            if same == rest.len() {
                // All of `hay` extended the match.
                let k = if k > 0 { k.wrapping_add(same) } else { 0 };
                return (k, m.wrapping_add(same), Hit::None);
            }
            i = i.wrapping_add(same);
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
            let rest = hay.get(start..).unwrap_or(&[]);
            let same = shared(dashes, rest);
            if same == dashes.len() {
                let end = start.wrapping_add(same);
                return if after_cr {
                    (len, 0, Hit::Delimiter(end))
                } else {
                    (0, bare, Hit::BareBreak(end))
                };
            }
            if same == rest.len() {
                // All of `hay` is the start of a match, which goes on in
                // the next chunk.
                let k = if after_cr {
                    same.wrapping_add(2)
                } else {
                    usize::from(same == 0 && hay.get(at) == Some(&b'\r'))
                };
                return (k, same.wrapping_add(1), Hit::None);
            }
            i = start.wrapping_add(same);
        }
        (0, 0, Hit::None)
    }
}

/// How many leading bytes `a` and `b` share.
#[inline]
pub(crate) fn shared(a: &[u8], b: &[u8]) -> usize {
    let n = crate::scan::common_prefix(a, b);
    #[cfg(test)]
    tests::count_visits(n.wrapping_add(1));
    n
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

    /// Counts bytes the matcher compares: one call to `step`, which may
    /// follow the failure table several times (KMP bounds those by the
    /// bytes seen), or the bytes of one whole compare.
    pub(crate) fn count_visits(n: usize) {
        VISITS.with(|v| v.set(v.get() + n as u64));
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

    /// D-117: `run` compares where the automata step, and the states it
    /// leaves are exactly theirs, so a match carries across any split.
    #[test]
    fn test_run_leaves_the_automata_states_at_every_split() {
        let mut seed = 5u64;
        for boundary in [&b"-ab"[..], b"--", b"x", b"ab-", b"abcdefghijklmnop"] {
            let b = Boundary::new(boundary).unwrap();
            let bare = b.len() - 1;
            let hay_bytes: &[u8] = &[b'\r', b'\n', b'-', b'-', boundary[0], b'x'];
            for len in 0..48 {
                for _ in 0..40 {
                    let mut hay: Vec<u8> = (0..len)
                        .map(|_| {
                            seed ^= seed << 13;
                            seed ^= seed >> 7;
                            seed ^= seed << 17;
                            hay_bytes[(seed % 6) as usize]
                        })
                        .collect();
                    // A partial match at the end, of every length.
                    let d = b.delimiter();
                    hay.extend_from_slice(&d[..len % d.len()]);
                    if !matches!(naive_hit(&b, &hay), Hit::None) {
                        continue;
                    }
                    let (mut k, mut m) = (0, 0);
                    for &c in &hay {
                        k = b.step(k, c);
                        m = b.bare_step(m, c);
                        assert!(k < b.len() && m < bare);
                    }
                    assert_eq!(&b.run(0, 0, &hay), &(k, m, Hit::None), "{hay:?}");
                    for split in 0..=hay.len() {
                        let (x, y) = hay.split_at(split);
                        let (k1, m1, _) = b.run(0, 0, x);
                        assert_eq!(&b.run(k1, m1, y), &(k, m, Hit::None), "{hay:?} at {split}");
                    }
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
