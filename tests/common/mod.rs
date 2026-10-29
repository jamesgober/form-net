//! Shared test helpers: drive the streaming parser over any split, and a
//! small, obviously correct reference parser for well-formed bodies.

#![allow(dead_code)]

use form_net::{Boundary, Event, FormError, Limits, MultipartParser, PartKind, Policy};

/// What a part was, with its data joined.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Record {
    pub name: String,
    pub kind: PartKind,
    pub filename: Option<Vec<u8>>,
    pub data: Vec<u8>,
}

/// Feeds `chunks` to a fresh parser and records every part, or the first
/// error.
pub fn drive(
    boundary: &[u8],
    chunks: &[&[u8]],
    limits: &Limits,
    policy: &Policy,
) -> Result<Vec<Record>, FormError> {
    let mut parser = MultipartParser::new(Boundary::new(boundary)?, limits, policy)?;
    let mut out = Vec::new();
    let mut current: Option<Record> = None;
    let mut done = false;
    for (i, chunk) in chunks.iter().enumerate() {
        let last = i + 1 == chunks.len();
        let mut rest: &[u8] = chunk;
        loop {
            let step = parser.parse(rest, last)?;
            rest = &rest[step.consumed..];
            match step.event {
                Event::PartStart(head) => {
                    assert!(current.is_none(), "part started inside a part");
                    current = Some(Record {
                        name: head.name().to_owned(),
                        kind: head.kind(),
                        filename: head.filename().map(|f| f.raw().to_vec()),
                        data: Vec::new(),
                    });
                }
                Event::Data(bytes) => {
                    assert!(!bytes.is_empty(), "empty Data event");
                    current
                        .as_mut()
                        .expect("data outside a part")
                        .data
                        .extend_from_slice(bytes);
                }
                Event::PartEnd(summary) => {
                    let rec = current.take().expect("end outside a part");
                    assert_eq!(summary.bytes, rec.data.len() as u64);
                    out.push(rec);
                }
                Event::End => {
                    done = true;
                    if !rest.is_empty() {
                        let step = parser.parse(rest, last)?;
                        assert!(matches!(step.event, Event::End));
                        assert_eq!(step.consumed, rest.len());
                    }
                    break;
                }
                Event::NeedInput => {
                    assert!(rest.is_empty(), "NeedInput with bytes left");
                    break;
                }
                _ => unreachable!(),
            }
        }
        if done {
            // Remaining chunks are epilogue: feed them too.
            for (j, more) in chunks.iter().enumerate().skip(i + 1) {
                let step = parser.parse(more, j + 1 == chunks.len())?;
                assert!(matches!(step.event, Event::End));
            }
            break;
        }
    }
    assert!(done || chunks.is_empty(), "parser finished without End");
    Ok(out)
}

/// The whole body as one chunk.
pub fn whole(boundary: &[u8], body: &[u8]) -> Result<Vec<Record>, FormError> {
    drive(boundary, &[body], &Limits::DEFAULT, &Policy::DEFAULT)
}

/// The body split at one point.
pub fn split_at(
    boundary: &[u8],
    body: &[u8],
    at: usize,
    limits: &Limits,
    policy: &Policy,
) -> Result<Vec<Record>, FormError> {
    let (a, b) = body.split_at(at);
    drive(boundary, &[a, b], limits, policy)
}

/// The body one byte at a time.
pub fn bytewise(
    boundary: &[u8],
    body: &[u8],
    limits: &Limits,
    policy: &Policy,
) -> Result<Vec<Record>, FormError> {
    let chunks: Vec<&[u8]> = body.chunks(1).collect();
    drive(boundary, &chunks, limits, policy)
}

/// Asserts that every split of `body` gives the same result as the whole.
pub fn assert_split_invariant(boundary: &[u8], body: &[u8], limits: &Limits, policy: &Policy) {
    let expected = drive(boundary, &[body], limits, policy);
    for at in 0..=body.len() {
        let got = split_at(boundary, body, at, limits, policy);
        assert_eq!(got, expected, "split at {at}");
    }
    assert_eq!(
        bytewise(boundary, body, limits, policy),
        expected,
        "bytewise"
    );
    for size in [2, 3, 7, 64] {
        let chunks: Vec<&[u8]> = body.chunks(size).collect();
        assert_eq!(
            drive(boundary, &chunks, limits, policy),
            expected,
            "chunks of {size}"
        );
    }
}

/// Builds a body from `(headers, data)` parts.
pub fn body(boundary: &str, parts: &[(&str, &[u8])]) -> Vec<u8> {
    let mut out = Vec::new();
    for (headers, data) in parts {
        out.extend_from_slice(format!("--{boundary}\r\n{headers}\r\n\r\n").as_bytes());
        out.extend_from_slice(data);
        out.extend_from_slice(b"\r\n");
    }
    out.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());
    out
}

/// A reference parser: whole body, simple loops, for well-formed bodies
/// whose parts have exactly one `Content-Disposition` line with a quoted
/// `name` and optional quoted `filename`, and nothing else. Returns `None`
/// for anything it does not understand.
pub fn reference(boundary: &[u8], body: &[u8]) -> Option<Vec<Record>> {
    let mut first = b"--".to_vec();
    first.extend_from_slice(boundary);
    let mut delim = b"\r\n--".to_vec();
    delim.extend_from_slice(boundary);
    let find = |hay: &[u8], needle: &[u8], from: usize| -> Option<usize> {
        hay.get(from..)?
            .windows(needle.len())
            .position(|w| w == needle)
            .map(|p| p + from)
    };
    let mut pos = if body.starts_with(&first) {
        first.len()
    } else {
        find(body, &delim, 0)? + delim.len()
    };
    let mut out = Vec::new();
    loop {
        if body.get(pos..pos + 2)? == b"--" {
            return Some(out);
        }
        if body.get(pos..pos + 2)? != b"\r\n" {
            return None;
        }
        pos += 2;
        let head_end = find(body, b"\r\n\r\n", pos)?;
        let head = std::str::from_utf8(&body[pos..head_end]).ok()?;
        let data_start = head_end + 4;
        let data_end = find(body, &delim, data_start)?;
        // `--boundary` at the start of the data or after a lone CR or LF
        // is refused (D-113).
        let dashes = &delim[2..];
        let data = &body[data_start..data_end];
        if data.starts_with(dashes)
            || data
                .windows(dashes.len() + 1)
                .any(|w| (w[0] == b'\r' || w[0] == b'\n') && &w[1..] == dashes)
        {
            return None;
        }
        let line = head.strip_prefix("Content-Disposition: form-data; ")?;
        let mut name = None;
        let mut filename = None;
        for param in line.split("; ") {
            let (k, v) = param.split_once('=')?;
            let v = v.strip_prefix('"')?.strip_suffix('"')?;
            match k {
                "name" => {
                    name = Some(
                        v.replace("%22", "\"")
                            .replace("%0D", "\r")
                            .replace("%0A", "\n"),
                    )
                }
                "filename" => filename = Some(v.as_bytes().to_vec()),
                _ => return None,
            }
        }
        out.push(Record {
            name: name?,
            kind: if filename.is_some() {
                PartKind::File
            } else {
                PartKind::Field
            },
            filename,
            data: body[data_start..data_end].to_vec(),
        });
        pos = data_end + delim.len();
    }
}
