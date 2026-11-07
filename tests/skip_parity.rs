//! The buffered parser and `UploadSession` agree on fields an inspector
//! skips (0.9 review): a text field is checked as text exactly when its
//! whole value was read, before `Inspector::field`, and a `_charset_` field
//! counts whether or not it was skipped.

#![cfg(feature = "fs")]

use form_net::{FormError, Inspector, Limits, PartHead, Policy, Store, UploadSession, Verdict};

const CT: &[u8] = b"multipart/form-data; boundary=X";

/// Where an inspector skips the field named `name`.
#[derive(Clone, Copy, Debug)]
enum At {
    PartStart,
    Data,
    Field,
}

struct SkipOne {
    name: &'static str,
    at: At,
    current: String,
    fields_seen: Vec<String>,
}

impl SkipOne {
    fn new(name: &'static str, at: At) -> Self {
        SkipOne {
            name,
            at,
            current: String::new(),
            fields_seen: Vec::new(),
        }
    }
}

impl Inspector for SkipOne {
    fn part_start(&mut self, head: &PartHead<'_>) -> Verdict {
        self.current = head.name().to_owned();
        if matches!(self.at, At::PartStart) && head.name() == self.name {
            return Verdict::Skip;
        }
        Verdict::Continue
    }
    fn data(&mut self, _: u32, _: &[u8]) -> Verdict {
        if matches!(self.at, At::Data) && self.current == self.name {
            return Verdict::Skip;
        }
        Verdict::Continue
    }
    fn field(&mut self, name: &str, value: &[u8]) -> Verdict {
        // Every value the inspector sees as a field is text.
        assert!(
            std::str::from_utf8(value).is_ok(),
            "{name}: field() saw bytes that are not UTF-8"
        );
        self.fields_seen.push(name.to_owned());
        if matches!(self.at, At::Field) && name == self.name {
            return Verdict::Skip;
        }
        Verdict::Continue
    }
}

fn body(parts: &[(&str, &[u8])]) -> Vec<u8> {
    let mut out = Vec::new();
    for (name, value) in parts {
        out.extend_from_slice(
            format!("--X\r\nContent-Disposition: form-data; name={name}\r\n\r\n").as_bytes(),
        );
        out.extend_from_slice(value);
        out.extend_from_slice(b"\r\n");
    }
    out.extend_from_slice(b"--X--");
    out
}

/// The result as comparable data: the fields kept, or the error code.
fn buffered(b: &[u8], skip: &mut SkipOne) -> Result<Vec<(String, String)>, &'static str> {
    form_net::parse_multipart_inspected(CT, b, &Limits::DEFAULT, &Policy::DEFAULT, skip)
        .map(|mp| {
            mp.parts()
                .iter()
                .map(|p| (p.name().to_owned(), p.text().unwrap_or("").to_owned()))
                .collect()
        })
        .map_err(|e: FormError| e.code())
}

fn session(
    b: &[u8],
    chunk: usize,
    skip: &mut SkipOne,
) -> Result<Vec<(String, String)>, &'static str> {
    let store = Store::memory_only();
    let r = (|| {
        let mut s = UploadSession::new(CT, &Limits::DEFAULT, &Policy::DEFAULT, &store)?;
        for c in b.chunks(chunk) {
            s.push(c, &mut *skip)?;
        }
        s.finish(&mut *skip)
    })();
    r.map(|up| {
        up.iter()
            .map(|(n, v)| (n.to_owned(), v.to_owned()))
            .collect()
    })
    .map_err(|e| e.code())
}

#[test]
fn test_buffered_and_session_agree_on_skipped_fields() {
    let cases: Vec<(&str, Vec<u8>, &'static str, At, Result<usize, &str>)> = vec![
        // A skipped `_charset_` still declares the charset of what follows.
        (
            "skipped charset",
            body(&[("_charset_", b"iso-8859-1"), ("a", b"hi")]),
            "_charset_",
            At::PartStart,
            Err("encoding.unsupported_charset"),
        ),
        (
            "skipped charset, data",
            body(&[("_charset_", b"iso-8859-1"), ("a", b"hi")]),
            "_charset_",
            At::Data,
            Err("encoding.unsupported_charset"),
        ),
        (
            "skipped charset, field",
            body(&[("_charset_", b"iso-8859-1"), ("a", b"hi")]),
            "_charset_",
            At::Field,
            Err("encoding.unsupported_charset"),
        ),
        // A declared charset with nothing after it reads nothing in it.
        (
            "charset last",
            body(&[("a", b"hi"), ("_charset_", b"iso-8859-1")]),
            "none",
            At::Field,
            Ok(2),
        ),
        // Bad UTF-8 is refused when the whole value was read...
        (
            "bad utf-8, field skip",
            body(&[("a", b"\xff"), ("b", b"ok")]),
            "a",
            At::Field,
            Err("encoding.invalid_utf8"),
        ),
        (
            "bad utf-8, kept",
            body(&[("a", b"\xff"), ("b", b"ok")]),
            "none",
            At::Field,
            Err("encoding.invalid_utf8"),
        ),
        // ...and not when the inspector discarded it unread.
        (
            "bad utf-8, part_start skip",
            body(&[("a", b"\xff"), ("b", b"ok")]),
            "a",
            At::PartStart,
            Ok(1),
        ),
        (
            "bad utf-8, data skip",
            body(&[("a", b"\xff"), ("b", b"ok")]),
            "a",
            At::Data,
            Ok(1),
        ),
        (
            "utf-8 declared",
            body(&[("_charset_", b"UTF-8"), ("a", "é".as_bytes())]),
            "_charset_",
            At::PartStart,
            Ok(1),
        ),
    ];
    for (what, b, name, at, want) in &cases {
        let mut s1 = SkipOne::new(name, *at);
        let got = buffered(b, &mut s1);
        match want {
            Ok(n) => assert_eq!(got.as_ref().map(Vec::len), Ok(*n), "{what}: buffered"),
            Err(code) => assert_eq!(got.as_ref().map(|_| ()), Err(code), "{what}: buffered"),
        }
        for chunk in [1, 3, 7, b.len()] {
            let mut s2 = SkipOne::new(name, *at);
            let streamed = session(b, chunk, &mut s2);
            assert_eq!(
                streamed, got,
                "{what}: session in chunks of {chunk} differs from buffered"
            );
            assert_eq!(
                s2.fields_seen, s1.fields_seen,
                "{what}: field() calls differ (chunks of {chunk})"
            );
        }
    }
}
