//! Shared helpers for the fuzz targets.

#![allow(dead_code)]

use form_net::{Boundary, Event, FormError, Limits, MultipartParser, PartKind, Policy};

#[derive(Debug, PartialEq, Eq)]
pub struct Record {
    pub name: String,
    pub kind: PartKind,
    pub filename: Option<Vec<u8>>,
    pub data: Vec<u8>,
}

/// Drives a parser over `chunks`, checking the event contract as it goes.
pub fn drive(boundary: &[u8], chunks: &[&[u8]], limits: &Limits, policy: &Policy) -> Result<Vec<Record>, FormError> {
    let mut parser = MultipartParser::new(Boundary::new(boundary)?, limits, policy)?;
    // An empty body is still one (empty) final chunk: the parser must hear
    // that the body ended.
    let empty: [&[u8]; 1] = [&[]];
    let chunks = if chunks.is_empty() { &empty[..] } else { chunks };
    let mut out = Vec::new();
    let mut current: Option<Record> = None;
    let mut done = false;
    for (i, chunk) in chunks.iter().enumerate() {
        let last = i + 1 == chunks.len();
        let mut rest: &[u8] = chunk;
        loop {
            if done {
                let step = parser.parse(rest, last)?;
                assert!(matches!(step.event, Event::End));
                assert_eq!(step.consumed, rest.len());
                break;
            }
            let step = parser.parse(rest, last)?;
            assert!(step.consumed <= rest.len());
            rest = &rest[step.consumed..];
            match step.event {
                Event::PartStart(head) => {
                    assert!(current.is_none());
                    assert!(!head.name().is_empty());
                    current = Some(Record {
                        name: head.name().to_owned(),
                        kind: head.kind(),
                        filename: head.filename().map(|f| f.raw().to_vec()),
                        data: Vec::new(),
                    });
                }
                Event::Data(bytes) => {
                    assert!(!bytes.is_empty());
                    current.as_mut().expect("data inside a part").data.extend_from_slice(bytes);
                }
                Event::PartEnd(summary) => {
                    let rec = current.take().expect("end inside a part");
                    assert_eq!(summary.bytes, rec.data.len() as u64);
                    out.push(rec);
                }
                Event::End => {
                    done = true;
                    if rest.is_empty() {
                        break;
                    }
                }
                Event::NeedInput => {
                    assert!(rest.is_empty());
                    break;
                }
                _ => unreachable!(),
            }
        }
    }
    if !done {
        // The last call had eof = true, so the parser must have refused.
        unreachable!("no End and no error");
    }
    Ok(out)
}

/// Splits `data` into chunks whose sizes come from the first byte.
pub fn chunks(data: &[u8]) -> (usize, Vec<&[u8]>) {
    let size = usize::from(data.first().copied().unwrap_or(1)).max(1);
    let body = data.get(1..).unwrap_or(&[]);
    (size, body.chunks(size).collect())
}

/// A whole-body reference parser for simple well-formed bodies; `None`
/// for anything outside that shape.
pub fn reference(boundary: &[u8], body: &[u8]) -> Option<Vec<Record>> {
    let mut first = b"--".to_vec();
    first.extend_from_slice(boundary);
    let mut delim = b"\r\n--".to_vec();
    delim.extend_from_slice(boundary);
    let find = |hay: &[u8], needle: &[u8], from: usize| -> Option<usize> {
        hay.get(from..)?.windows(needle.len()).position(|w| w == needle).map(|p| p + from)
    };
    let mut pos = if body.starts_with(&first) { first.len() } else { return None };
    let mut out = Vec::new();
    loop {
        if body.get(pos..pos + 2)? == b"--" {
            // Short epilogues only, so the budget never applies.
            return if body.len() - pos <= 64 { Some(out) } else { None };
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
            || data.windows(dashes.len() + 1).any(|w| (w[0] == b'\r' || w[0] == b'\n') && &w[1..] == dashes)
        {
            return None;
        }
        let line = head.strip_prefix("Content-Disposition: form-data; ")?;
        let mut name = None;
        let mut filename = None;
        for param in line.split("; ") {
            let (k, v) = param.split_once('=')?;
            let v = v.strip_prefix('"')?.strip_suffix('"')?;
            if v.contains(['"', '\\', '%', '\r', '\n']) || !v.bytes().all(|b| b >= 0x20 && b != 0x7F) {
                return None;
            }
            match k {
                "name" if name.is_none() && !v.is_empty() => name = Some(v.to_owned()),
                "filename" if filename.is_none() => filename = Some(v.as_bytes().to_vec()),
                _ => return None,
            }
        }
        // Inside the default limits only, so a refusal is always a
        // disagreement. Value, file, text, and header sizes cannot be
        // reached by a fuzz input.
        let name = name?;
        let limits = Limits::DEFAULT;
        if name.len() > limits.max_name_bytes as usize
            || filename.as_ref().is_some_and(|f| f.len() > limits.max_filename_bytes as usize)
        {
            return None;
        }
        out.push(Record {
            name,
            kind: if filename.is_some() { PartKind::File } else { PartKind::Field },
            filename,
            data: body[data_start..data_end].to_vec(),
        });
        let files = out.iter().filter(|r| r.kind == PartKind::File).count();
        if out.len() > limits.max_parts as usize || files > limits.max_files as usize {
            return None;
        }
        pos = data_end + delim.len();
    }
}
