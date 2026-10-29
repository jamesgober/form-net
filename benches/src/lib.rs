//! Inputs and drivers shared by the benchmarks and the allocation counter.
//!
//! Every driver does the same work for every crate: it parses the whole
//! body and touches every byte of every value (sums lengths), with the
//! same limits where the crate supports limits. form-net is never run with
//! its checks off.

#![allow(clippy::missing_panics_doc)]

use std::convert::Infallible;
use std::io::{self, Cursor};

use bytes::Bytes;
use form_net::{Boundary, Event, FromForm, Limits, MultipartParser, NoInspector, Policy};

/// The boundary Chrome sends.
pub const BOUNDARY: &str = "----WebKitFormBoundary7MA4YWxkTrZu0gW";

pub fn content_type() -> String {
    format!("multipart/form-data; boundary={BOUNDARY}")
}

/// Deterministic pseudo-random bytes (xorshift64), so runs compare.
pub fn random_bytes(n: usize, seed: u64) -> Vec<u8> {
    let mut x = seed | 1;
    let mut out = Vec::with_capacity(n);
    while out.len() < n {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        out.extend_from_slice(&x.to_le_bytes());
    }
    out.truncate(n);
    out
}

// ---------------------------------------------------------------- urlencoded

pub fn login() -> Vec<u8> {
    b"email=ada%40example.com&password=correct+horse+battery&remember=on".to_vec()
}

pub fn search() -> Vec<u8> {
    b"q=rust+form+parser&page=2".to_vec()
}

pub fn signup() -> Vec<u8> {
    b"email=ada%40example.com&password=correct+horse+battery&display_name=Ada+Lovelace&age=36&country=GB&newsletter=on&referrer=https%3A%2F%2Fexample.org%2Fblog&bio=Mathematician%2C+writer".to_vec()
}

pub fn thousand_fields() -> Vec<u8> {
    let mut b = Vec::new();
    for i in 0..1000 {
        if i > 0 {
            b.push(b'&');
        }
        b.extend_from_slice(format!("field{i}=value{i}").as_bytes());
    }
    b
}

/// 1 MiB of plain values: 16 fields of 64 KiB, nothing to decode.
pub fn one_mib_plain() -> Vec<u8> {
    let mut b = Vec::new();
    for i in 0..16 {
        if i > 0 {
            b.push(b'&');
        }
        b.extend_from_slice(format!("f{i:02}=").as_bytes());
        b.extend(std::iter::repeat_n(b'a', 65_536 - 5));
    }
    b
}

/// 1 MiB where every value byte is escaped.
pub fn one_mib_escaped() -> Vec<u8> {
    let mut b = Vec::new();
    for i in 0..16 {
        if i > 0 {
            b.push(b'&');
        }
        b.extend_from_slice(format!("f{i:02}=").as_bytes());
        for _ in 0..(65_536 - 5) / 3 {
            b.extend_from_slice(b"%41");
        }
    }
    b
}

/// Limits large enough for every urlencoded input above.
pub const URL_LIMITS: Limits = Limits::DEFAULT
    .with_max_fields(2000)
    .with_max_text_bytes(4 << 20);

pub fn fnet_pairs(body: &[u8]) -> usize {
    let form = form_net::parse_urlencoded_with(body, &URL_LIMITS, &Policy::DEFAULT).unwrap();
    form.iter().map(|(n, v)| n.len() + v.len()).sum()
}

pub fn fnet_raw(body: &[u8], scratch: &mut [u8]) -> usize {
    let mut total = 0;
    for pair in form_net::raw_pairs(body, &URL_LIMITS) {
        let pair = pair.unwrap();
        total += pair
            .decode_name(scratch, form_net::PercentPolicy::Refuse)
            .unwrap()
            .len();
        total += pair
            .decode_value(scratch, form_net::PercentPolicy::Refuse)
            .unwrap()
            .len();
    }
    total
}

/// form-net's lazy pairs: borrowed when nothing needs decoding, like
/// `form_urlencoded::parse`.
pub fn fnet_lazy_pairs(body: &[u8]) -> usize {
    let pairs: Vec<_> = form_net::pairs(body).collect::<Result<_, _>>().unwrap();
    pairs
        .iter()
        .map(|(n, v): &(std::borrow::Cow<'_, str>, std::borrow::Cow<'_, str>)| n.len() + v.len())
        .sum()
}

/// form_urlencoded without collecting: the match for `fnet_raw`, which
/// also decodes every pair and keeps nothing.
pub fn form_urlencoded_sum(body: &[u8]) -> usize {
    form_urlencoded::parse(body)
        .map(|(n, v)| n.len() + v.len())
        .sum()
}

/// form-net's lazy pairs, iterated without collecting: the match for
/// `form_urlencoded_sum`.
pub fn fnet_lazy_sum(body: &[u8]) -> usize {
    form_net::pairs(body)
        .map(|r| {
            let (n, v) = r.unwrap();
            n.len() + v.len()
        })
        .sum()
}

pub fn form_urlencoded_pairs(body: &[u8]) -> usize {
    let pairs: Vec<_> = form_urlencoded::parse(body).collect();
    pairs.iter().map(|(n, v)| n.len() + v.len()).sum()
}

#[derive(serde::Deserialize)]
#[allow(dead_code)]
pub struct SerdeSignup {
    pub email: String,
    pub password: String,
    pub display_name: String,
    pub age: u8,
    pub country: String,
    #[serde(default)]
    pub newsletter: Option<String>,
    pub referrer: String,
    pub bio: String,
}

form_net::form! {
    pub struct Signup {
        pub email: String,
        pub password: form_net::Secret,
        pub display_name: String,
        pub age: u8,
        pub country: String,
        pub newsletter: bool,
        pub referrer: String,
        pub bio: String,
    }
}

pub fn fnet_signup(body: &[u8]) -> u8 {
    Signup::from_urlencoded(body).unwrap().age
}

pub fn serde_urlencoded_signup(body: &[u8]) -> u8 {
    serde_urlencoded::from_bytes::<SerdeSignup>(body)
        .unwrap()
        .age
}

pub fn serde_qs_signup(body: &[u8]) -> u8 {
    serde_qs::from_bytes::<SerdeSignup>(body).unwrap().age
}

pub const TEN_PAIRS: [(&str, &str); 10] = [
    ("email", "ada@example.com"),
    ("password", "correct horse battery"),
    ("display_name", "Ada Lovelace"),
    ("age", "36"),
    ("country", "GB"),
    ("newsletter", "on"),
    ("referrer", "https://example.org/blog?a=1&b=2"),
    ("bio", "Mathematician, writer"),
    ("lang", "en-GB"),
    ("tz", "Europe/London"),
];

pub fn fnet_serialize() -> usize {
    form_net::serialize_urlencoded(TEN_PAIRS).len()
}

pub fn form_urlencoded_serialize() -> usize {
    form_urlencoded::Serializer::new(String::new())
        .extend_pairs(TEN_PAIRS)
        .finish()
        .len()
}

pub fn serde_urlencoded_serialize() -> usize {
    serde_urlencoded::to_string(TEN_PAIRS).unwrap().len()
}

// ----------------------------------------------------------------- multipart

/// A body built by form-net's writer under the browser boundary.
pub fn multipart_body(texts: &[(&str, &str)], files: &[(&str, &[u8])]) -> Vec<u8> {
    let mut body = Vec::new();
    let mut w = form_net::MultipartWriter::with_boundary(
        &mut body,
        Boundary::new(BOUNDARY.as_bytes()).unwrap(),
    );
    // Data is synthetic; the delimiter scan is the parser's job here.
    w.verify_boundary(false);
    for (n, v) in texts {
        w.text(n, v).unwrap();
    }
    for (n, data) in files {
        w.file(
            n,
            &format!("{n}.bin"),
            Some("application/octet-stream"),
            *data,
        )
        .unwrap();
    }
    w.finish().unwrap();
    body
}

pub fn mp_login() -> Vec<u8> {
    multipart_body(
        &[
            ("email", "ada@example.com"),
            ("password", "correct horse battery"),
            ("remember", "on"),
        ],
        &[],
    )
}

pub fn mp_avatar() -> Vec<u8> {
    let img = random_bytes(64 * 1024, 1);
    multipart_body(&[("user", "42"), ("caption", "me")], &[("avatar", &img)])
}

pub fn mp_document() -> Vec<u8> {
    let doc = random_bytes(10 << 20, 2);
    multipart_body(&[("title", "Q3 report")], &[("document", &doc)])
}

pub fn mp_batch() -> Vec<u8> {
    let files: Vec<Vec<u8>> = (0..20).map(|i| random_bytes(1 << 20, 10 + i)).collect();
    let named: Vec<(String, &[u8])> = files
        .iter()
        .enumerate()
        .map(|(i, f)| (format!("f{i}"), &f[..]))
        .collect();
    let refs: Vec<(&str, &[u8])> = named.iter().map(|(n, f)| (n.as_str(), *f)).collect();
    multipart_body(&[], &refs)
}

pub fn mp_hundred_mib() -> Vec<u8> {
    let big = random_bytes(100 << 20, 3);
    multipart_body(&[], &[("file", &big)])
}

pub fn mp_many_parts() -> Vec<u8> {
    let names: Vec<String> = (0..100).map(|i| format!("p{i}")).collect();
    let texts: Vec<(&str, &str)> = names.iter().map(|n| (n.as_str(), "0123456789")).collect();
    multipart_body(&texts, &[])
}

/// 1 MiB of CR in one file part: the matcher's worst byte.
pub fn mp_all_cr() -> Vec<u8> {
    let data = vec![b'\r'; 1 << 20];
    multipart_body(&[], &[("f", &data)])
}

/// 1 MiB of near misses that differ in the middle of the delimiter: they
/// pass the last-byte check and need the full compare.
pub fn mp_near_miss_middle() -> Vec<u8> {
    let delim = format!("\r\n--{BOUNDARY}");
    let mut near = delim.clone().into_bytes();
    let mid = near.len() / 2;
    near[mid] = b'X';
    let mut data = Vec::new();
    while data.len() < (1 << 20) {
        data.extend_from_slice(&near);
    }
    multipart_body(&[], &[("f", &data)])
}

/// 1 MiB of near-miss delimiters (all but the last byte of the delimiter).
pub fn mp_near_miss() -> Vec<u8> {
    let delim = format!("\r\n--{BOUNDARY}");
    let near = &delim.as_bytes()[..delim.len() - 1];
    let mut data = Vec::new();
    while data.len() < (1 << 20) {
        data.extend_from_slice(near);
        data.push(b'X');
    }
    multipart_body(&[], &[("f", &data)])
}

/// Limits large enough for every multipart input above, the same for every
/// crate that takes limits.
pub const MP_LIMITS: Limits = Limits::DEFAULT
    .with_max_body_bytes(128 << 20)
    .with_max_file_bytes(128 << 20)
    .with_max_files(32);

pub fn fnet_stream(body: &[u8], chunk: usize) -> usize {
    let mut p = MultipartParser::new(
        Boundary::new(BOUNDARY.as_bytes()).unwrap(),
        &MP_LIMITS,
        &Policy::DEFAULT,
    )
    .unwrap();
    let mut total = 0;
    let mut chunks = body.chunks(chunk).peekable();
    while let Some(c) = chunks.next() {
        let last = chunks.peek().is_none();
        let mut rest = c;
        loop {
            let step = p.parse(rest, last).unwrap();
            rest = &rest[step.consumed..];
            match step.event {
                Event::Data(d) => total += d.len(),
                Event::NeedInput | Event::End => break,
                _ => {}
            }
        }
    }
    total
}

pub fn fnet_buffered(content_type: &str, body: &[u8]) -> usize {
    let mp =
        form_net::parse_multipart_with(content_type.as_bytes(), body, &MP_LIMITS, &Policy::DEFAULT)
            .unwrap();
    mp.parts().iter().map(|p| p.bytes().len()).sum()
}

fn multer_constraints() -> multer::Constraints {
    multer::Constraints::new().size_limit(
        multer::SizeLimit::new()
            .whole_stream(MP_LIMITS.max_body_bytes)
            .per_field(MP_LIMITS.max_file_bytes),
    )
}

/// multer over a stream of zero-copy `Bytes` slices of the body.
pub fn multer_stream(body: &Bytes, chunk: usize) -> usize {
    // Lazy: slicing `Bytes` bumps a reference count and allocates nothing,
    // so no harness allocation is charged to multer.
    let len = body.len();
    let pieces = (0..len)
        .step_by(chunk.max(1))
        .map(move |s| Ok::<Bytes, Infallible>(body.slice(s..(s + chunk).min(len))));
    let stream = futures_util::stream::iter(pieces);
    let mut m = multer::Multipart::with_constraints(stream, BOUNDARY, multer_constraints());
    futures_executor::block_on(async {
        let mut total = 0;
        while let Some(mut field) = m.next_field().await.unwrap() {
            while let Some(c) = field.chunk().await.unwrap() {
                total += c.len();
            }
        }
        total
    })
}

/// multer over a buffered body, the boundary read from the `Content-Type`
/// as form-net's `parse_multipart` reads it.
pub fn multer_buffered(content_type: &str, body: &Bytes) -> usize {
    let boundary = multer::parse_boundary(content_type).unwrap();
    let once = futures_util::stream::iter([Ok::<Bytes, Infallible>(body.clone())]);
    let mut m = multer::Multipart::with_constraints(once, boundary, multer_constraints());
    futures_executor::block_on(async {
        let mut total = 0;
        while let Some(mut field) = m.next_field().await.unwrap() {
            while let Some(c) = field.chunk().await.unwrap() {
                total += c.len();
            }
        }
        total
    })
}

/// `multipart` 0.18 over a `Cursor`, reading every entry to a sink. Its
/// framework integrations take the boundary from a `mime`-parsed
/// `Content-Type`; this does the same, so all three parse the header.
pub fn multipart_018_buffered(content_type: &str, body: &[u8]) -> usize {
    let boundary = multer::parse_boundary(content_type).unwrap();
    let mut mp = multipart::server::Multipart::with_body(Cursor::new(body), boundary);
    let mut total = 0;
    while let Some(mut entry) = mp.read_entry().unwrap() {
        total += io::copy(&mut entry.data, &mut io::sink()).unwrap() as usize;
    }
    total
}

/// form-net's upload session into a temporary store, in 64 KiB pushes.
pub fn fnet_to_disk(store: &form_net::Store, content_type: &str, body: &[u8]) -> usize {
    let mut s =
        form_net::UploadSession::new(content_type.as_bytes(), &MP_LIMITS, &Policy::DEFAULT, store)
            .unwrap();
    for c in body.chunks(64 * 1024) {
        s.push(c, &mut NoInspector).unwrap();
    }
    let up = s.finish(&mut NoInspector).unwrap();
    up.iter().count() + up.file_count()
}

/// `multipart` 0.18 saving every entry to a temporary directory.
pub fn multipart_018_to_disk(content_type: &str, body: &[u8]) -> usize {
    let boundary = multer::parse_boundary(content_type).unwrap();
    let mut mp = multipart::server::Multipart::with_body(Cursor::new(body), boundary);
    match mp.save().size_limit(b_limit()).temp() {
        multipart::server::save::SaveResult::Full(entries) => entries.fields.len(),
        _ => panic!("save did not finish"),
    }
}

/// The per-file limit every crate gets in the disk family.
fn b_limit() -> u64 {
    MP_LIMITS.max_file_bytes
}
