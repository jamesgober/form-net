//! WAFFLED-style mutations of multipart bodies (PLAN 12, T-14): duplicated,
//! reordered, re-cased, re-spaced, and re-quoted headers and parameters,
//! line-ending and folding changes, and framing changes. Each mutation keeps
//! the fields a reader should see, so a parser must either refuse the result
//! or give exactly the original fields; anything else is a reading two
//! parsers can disagree on. Shared by `tests/waffled.rs` and the multer
//! comparison in `benches/`.

#![allow(dead_code)]

/// One part: its header lines (without CR LF) and its data.
#[derive(Clone, Debug)]
pub struct Part {
    pub headers: Vec<Vec<u8>>,
    pub data: Vec<u8>,
}

/// A body split into parts, with the boundary that frames them.
#[derive(Clone, Debug)]
pub struct Body {
    pub boundary: Vec<u8>,
    pub parts: Vec<Part>,
    /// What follows the close delimiter: CR LF, or nothing.
    pub tail: Vec<u8>,
}

fn find(hay: &[u8], needle: &[u8], from: usize) -> Option<usize> {
    hay.get(from..)?
        .windows(needle.len())
        .position(|w| w == needle)
        .map(|p| p + from)
}

impl Body {
    /// Splits a well-formed body (initial delimiter at byte 0, a close
    /// delimiter, at most CR LF past it).
    pub fn split(boundary: &[u8], body: &[u8]) -> Option<Body> {
        let mut first = b"--".to_vec();
        first.extend_from_slice(boundary);
        let mut delim = b"\r\n--".to_vec();
        delim.extend_from_slice(boundary);
        let mut pos = first.len();
        if !body.starts_with(&first) {
            return None;
        }
        let mut parts = Vec::new();
        loop {
            match body.get(pos..pos + 2)? {
                b"--" => {
                    let tail = body[pos + 2..].to_vec();
                    if !tail.is_empty() && tail != b"\r\n" {
                        return None;
                    }
                    return Some(Body {
                        boundary: boundary.to_vec(),
                        parts,
                        tail,
                    });
                }
                b"\r\n" => pos += 2,
                _ => return None,
            }
            let head_end = find(body, b"\r\n\r\n", pos)?;
            let headers = body[pos..head_end]
                .split(|&b| b == b'\n')
                .map(|l| l.strip_suffix(b"\r").unwrap_or(l).to_vec())
                .collect();
            let data_start = head_end + 4;
            let data_end = find(body, &delim, data_start)?;
            parts.push(Part {
                headers,
                data: body[data_start..data_end].to_vec(),
            });
            pos = data_end + delim.len();
        }
    }

    /// The body as bytes.
    pub fn render(&self) -> Vec<u8> {
        self.render_with(b"\r\n", b"", &self.tail)
    }

    /// The body with `eol` ending header lines, `padding` after each
    /// delimiter, and `tail` after the close delimiter.
    fn render_with(&self, eol: &[u8], padding: &[u8], tail: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        for (i, p) in self.parts.iter().enumerate() {
            if i > 0 {
                out.extend_from_slice(b"\r\n");
            }
            out.extend_from_slice(b"--");
            out.extend_from_slice(&self.boundary);
            out.extend_from_slice(padding);
            out.extend_from_slice(b"\r\n");
            for h in &p.headers {
                out.extend_from_slice(h);
                out.extend_from_slice(eol);
            }
            out.extend_from_slice(eol);
            out.extend_from_slice(&p.data);
        }
        out.extend_from_slice(b"\r\n--");
        out.extend_from_slice(&self.boundary);
        out.extend_from_slice(b"--");
        out.extend_from_slice(tail);
        out
    }
}

/// What a mutation must do to a strict parser.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Expect {
    /// The same fields; refusing it would refuse what real clients send.
    Same,
    /// Refused, or the same fields; nothing else.
    SameOrRefused,
}

/// One mutated body.
pub struct Mutant {
    pub category: &'static str,
    pub part: Option<usize>,
    pub expect: Expect,
    pub body: Vec<u8>,
}

fn is_disposition(h: &[u8]) -> bool {
    h.len() > 20 && h[..20].eq_ignore_ascii_case(b"content-disposition:")
}

fn replace(h: &[u8], from: &[u8], to: &[u8]) -> Option<Vec<u8>> {
    let at = find(h, from, 0)?;
    let mut out = h[..at].to_vec();
    out.extend_from_slice(to);
    out.extend_from_slice(&h[at + from.len()..]);
    Some(out)
}

type PartMutation = fn(&Part) -> Option<(Part, Expect)>;

fn with_disposition(p: &Part, f: impl Fn(&[u8]) -> Option<Vec<u8>>) -> Option<Part> {
    let i = p.headers.iter().position(|h| is_disposition(h))?;
    let mut q = p.clone();
    q.headers[i] = f(&p.headers[i])?;
    Some(q)
}

/// The part mutations, by WAFFLED category.
pub const PART_MUTATIONS: &[(&str, PartMutation)] = &[
    ("duplicate Content-Disposition", |p| {
        let i = p.headers.iter().position(|h| is_disposition(h))?;
        let mut q = p.clone();
        q.headers.insert(i, p.headers[i].clone());
        Some((q, Expect::SameOrRefused))
    }),
    ("duplicate Content-Disposition, other name", |p| {
        p.headers.iter().find(|h| is_disposition(h))?;
        let mut q = p.clone();
        q.headers
            .push(b"Content-Disposition: form-data; name=\"smuggled\"".to_vec());
        Some((q, Expect::SameOrRefused))
    }),
    ("header names lower case", |p| {
        let mut q = p.clone();
        for h in &mut q.headers {
            let colon = h.iter().position(|&b| b == b':')?;
            h[..colon].make_ascii_lowercase();
        }
        Some((q, Expect::Same))
    }),
    ("header names upper case", |p| {
        let mut q = p.clone();
        for h in &mut q.headers {
            let colon = h.iter().position(|&b| b == b':')?;
            h[..colon].make_ascii_uppercase();
        }
        Some((q, Expect::Same))
    }),
    ("headers reordered", |p| {
        if p.headers.len() < 2 {
            return None;
        }
        let mut q = p.clone();
        q.headers.reverse();
        Some((q, Expect::Same))
    }),
    ("disposition type case", |p| {
        Some((
            with_disposition(p, |h| replace(h, b"form-data", b"Form-Data"))?,
            Expect::Same,
        ))
    }),
    ("whitespace before colon", |p| {
        Some((
            with_disposition(p, |h| replace(h, b":", b" :"))?,
            Expect::SameOrRefused,
        ))
    }),
    ("no space after semicolon", |p| {
        Some((
            with_disposition(p, |h| replace(h, b"; name=", b";name="))?,
            Expect::Same,
        ))
    }),
    ("extra spaces after semicolon", |p| {
        Some((
            with_disposition(p, |h| replace(h, b"; name=", b";   name="))?,
            Expect::Same,
        ))
    }),
    ("tab after semicolon", |p| {
        Some((
            with_disposition(p, |h| replace(h, b"; name=", b";\tname="))?,
            Expect::SameOrRefused,
        ))
    }),
    ("space around equals", |p| {
        Some((
            with_disposition(p, |h| replace(h, b"name=", b"name = "))?,
            Expect::SameOrRefused,
        ))
    }),
    ("parameter name case", |p| {
        Some((
            with_disposition(p, |h| replace(h, b"; name=", b"; NAME="))?,
            Expect::SameOrRefused,
        ))
    }),
    ("folded disposition", |p| {
        Some((
            with_disposition(p, |h| replace(h, b"; name=", b";\r\n name="))?,
            Expect::SameOrRefused,
        ))
    }),
    ("second name parameter", |p| {
        let q = with_disposition(p, |h| {
            let mut h = h.to_vec();
            h.extend_from_slice(b"; name=\"smuggled\"");
            Some(h)
        })?;
        Some((q, Expect::SameOrRefused))
    }),
    ("second filename parameter", |p| {
        find(
            p.headers.iter().find(|h| is_disposition(h))?,
            b"filename=",
            0,
        )?;
        let q = with_disposition(p, |h| {
            let mut h = h.to_vec();
            h.extend_from_slice(b"; filename=\"smuggled.exe\"");
            Some(h)
        })?;
        Some((q, Expect::SameOrRefused))
    }),
    ("filename* beside filename", |p| {
        find(
            p.headers.iter().find(|h| is_disposition(h))?,
            b"filename=",
            0,
        )?;
        let q = with_disposition(p, |h| {
            let mut h = h.to_vec();
            h.extend_from_slice(b"; filename*=UTF-8''smuggled.exe");
            Some(h)
        })?;
        Some((q, Expect::SameOrRefused))
    }),
    ("name* beside name", |p| {
        let q = with_disposition(p, |h| {
            let mut h = h.to_vec();
            h.extend_from_slice(b"; name*=UTF-8''smuggled");
            Some(h)
        })?;
        Some((q, Expect::SameOrRefused))
    }),
    ("unknown parameter", |p| {
        let q = with_disposition(p, |h| {
            let mut h = h.to_vec();
            h.extend_from_slice(b"; x-extra=\"1\"");
            Some(h)
        })?;
        Some((q, Expect::Same))
    }),
    ("filename before name", |p| {
        let q = with_disposition(p, |h| {
            let n = find(h, b"; name=\"", 0)?;
            let n_end = find(h, b"\"", n + 8)? + 1;
            let f = find(h, b"; filename=\"", n_end)?;
            let f_end = find(h, b"\"", f + 12)? + 1;
            if f != n_end || f_end != h.len() {
                return None;
            }
            let mut out = h[..n].to_vec();
            out.extend_from_slice(&h[f..f_end]);
            out.extend_from_slice(&h[n..n_end]);
            Some(out)
        })?;
        Some((q, Expect::Same))
    }),
    ("name unquoted", |p| {
        let q = with_disposition(p, |h| {
            let n = find(h, b"; name=\"", 0)? + 8;
            let end = find(h, b"\"", n)?;
            let token = &h[n..end];
            if token.is_empty()
                || !token
                    .iter()
                    .all(|b| b.is_ascii_alphanumeric() || *b == b'_')
            {
                return None;
            }
            let mut out = h[..n - 1].to_vec();
            out.extend_from_slice(token);
            out.extend_from_slice(&h[end + 1..]);
            Some(out)
        })?;
        Some((q, Expect::Same))
    }),
    ("name single-quoted", |p| {
        let q = with_disposition(p, |h| {
            let n = find(h, b"; name=\"", 0)? + 8;
            let end = find(h, b"\"", n)?;
            let mut out = h[..n - 1].to_vec();
            out.push(b'\'');
            out.extend_from_slice(&h[n..end]);
            out.push(b'\'');
            out.extend_from_slice(&h[end + 1..]);
            Some(out)
        })?;
        Some((q, Expect::SameOrRefused))
    }),
    ("escaped quote in name", |p| {
        Some((
            with_disposition(p, |h| replace(h, b"; name=\"", b"; name=\"\\\""))?,
            Expect::SameOrRefused,
        ))
    }),
    ("Content-Transfer-Encoding", |p| {
        let mut q = p.clone();
        q.headers
            .push(b"Content-Transfer-Encoding: base64".to_vec());
        Some((q, Expect::SameOrRefused))
    }),
    ("nested multipart type", |p| {
        let mut q = p.clone();
        q.headers
            .retain(|h| !h.to_ascii_lowercase().starts_with(b"content-type:"));
        q.headers
            .push(b"Content-Type: multipart/mixed; boundary=inner".to_vec());
        Some((q, Expect::SameOrRefused))
    }),
    ("NUL in header", |p| {
        Some((
            with_disposition(p, |h| replace(h, b"form-data", b"form-data\0"))?,
            Expect::SameOrRefused,
        ))
    }),
];

/// Every mutant of `body`: each part mutation on each part, then the
/// framing mutations.
pub fn mutants(body: &Body) -> Vec<Mutant> {
    let mut out = Vec::new();
    for (i, part) in body.parts.iter().enumerate() {
        for (category, f) in PART_MUTATIONS {
            if let Some((q, expect)) = f(part) {
                let mut b = body.clone();
                b.parts[i] = q;
                out.push(Mutant {
                    category,
                    part: Some(i),
                    expect,
                    body: b.render(),
                });
            }
        }
    }
    let frame = |category, expect, bytes| Mutant {
        category,
        part: None,
        expect,
        body: bytes,
    };
    out.push(frame(
        "bare LF header endings",
        Expect::SameOrRefused,
        body.render_with(b"\n", b"", &body.tail),
    ));
    out.push(frame(
        "transport padding",
        Expect::Same,
        body.render_with(b"\r\n", b" \t ", &body.tail),
    ));
    out.push(frame(
        "padding past the budget",
        Expect::SameOrRefused,
        body.render_with(b"\r\n", &[b' '; 40], &body.tail),
    ));
    out.push(frame(
        "close delimiter then CR LF",
        Expect::Same,
        body.render_with(b"\r\n", b"", b"\r\n"),
    ));
    out.push(frame(
        "close delimiter alone",
        Expect::Same,
        body.render_with(b"\r\n", b"", b""),
    ));
    out.push(frame(
        "short epilogue",
        Expect::Same,
        body.render_with(b"\r\n", b"", b"\r\nepilogue\r\n"),
    ));
    out.push(frame(
        "epilogue past the budget",
        Expect::SameOrRefused,
        body.render_with(b"\r\n", b"", &[b'e'; 2048]),
    ));
    let mut pre = b"a preamble line\r\n".to_vec();
    pre.extend_from_slice(&body.render());
    out.push(frame("short preamble", Expect::Same, pre));
    let mut cut = body.render_with(b"\r\n", b"", b"");
    cut.truncate(cut.len() - 2);
    out.push(frame(
        "close delimiter cut short",
        Expect::SameOrRefused,
        cut,
    ));
    let mut lf = body.render();
    let mut delim = b"\r\n--".to_vec();
    delim.extend_from_slice(&body.boundary);
    if let Some(at) = find(&lf, &delim, 0) {
        lf.remove(at);
        out.push(frame(
            "bare LF before a delimiter",
            Expect::SameOrRefused,
            lf,
        ));
    }
    out
}
