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

    /// Runs the matcher over `hay` from state `k`. Returns the final state
    /// and, when the delimiter completed, the index just after it.
    ///
    /// From state 0 nothing before a CR can start a match (the delimiter
    /// starts with CR), so those bytes are skipped eight at a time.
    pub(crate) fn run(&self, mut k: usize, hay: &[u8]) -> (usize, Option<usize>) {
        let len = self.len();
        let mut i = 0usize;
        while i < hay.len() {
            if k == 0 {
                i = crate::scan::find_byte(hay, i, b'\r');
                if i >= hay.len() {
                    break;
                }
            }
            let c = *hay.get(i).unwrap_or(&0);
            k = self.step(k, c);
            i = i.wrapping_add(1);
            if k == len {
                return (k, Some(i));
            }
        }
        (k, None)
    }
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

    fn naive_find(hay: &[u8], needle: &[u8]) -> Option<usize> {
        hay.windows(needle.len())
            .position(|w| w == needle)
            .map(|p| p + needle.len())
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

    #[test]
    fn test_run_finds_like_naive_and_carries_state() {
        let b = Boundary::new(b"-ab").unwrap();
        let d = b.delimiter().to_vec();
        let mut hay = Vec::new();
        hay.extend_from_slice(b"x\r\n--a\r\n\r\n--\r\n--");
        hay.extend_from_slice(&d);
        hay.extend_from_slice(b"tail");
        let expected = naive_find(&hay, &d);
        assert_eq!(b.run(0, &hay).1, expected);
        // Split at every byte: the state carries across.
        for split in 0..hay.len() {
            let (k, found) = b.run(0, &hay[..split]);
            let total = match found {
                Some(i) => Some(i),
                None => b.run(k, &hay[split..]).1.map(|i| i + split),
            };
            assert_eq!(total, expected, "split {split}");
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
