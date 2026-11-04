//! The multipart writer (FR-64 to FR-66, T-34): every body it produces
//! parses back to the same fields, and every input that could not is
//! refused before a byte of it is written.

#![cfg(feature = "random")]

use form_net::{Boundary, FormError, MultipartWriter, SyntaxError};

#[derive(Clone, Debug)]
enum Part {
    Text(String, String),
    File(String, String, Option<String>, Vec<u8>),
}

/// Browsers send text line breaks as CRLF.
fn normalized(v: &str) -> String {
    v.replace("\r\n", "\n")
        .replace('\r', "\n")
        .replace('\n', "\r\n")
}

fn write(parts: &[Part], boundary: Option<&[u8]>) -> Result<(String, Vec<u8>), FormError> {
    let mut body = Vec::new();
    let mut w = match boundary {
        Some(b) => MultipartWriter::with_boundary(&mut body, Boundary::new(b)?),
        None => MultipartWriter::new(&mut body)?,
    };
    for p in parts {
        match p {
            Part::Text(n, v) => w.text(n, v)?,
            Part::File(n, f, ct, data) => {
                w.file(n, f, ct.as_deref(), &data[..])?;
            }
        }
    }
    let ct = w.content_type();
    w.finish()?;
    Ok((ct, body))
}

fn check_round_trip(parts: &[Part], boundary: Option<&[u8]>) -> Result<(), FormError> {
    let (ct, body) = write(parts, boundary)?;
    let mp = form_net::parse_multipart(ct.as_bytes(), &body).unwrap_or_else(|e| {
        panic!(
            "the writer produced a body form-net refuses: {e:?}\n{}",
            String::from_utf8_lossy(&body)
        )
    });
    assert_eq!(mp.len(), parts.len());
    for (p, got) in parts.iter().zip(mp.parts()) {
        match p {
            Part::Text(n, v) => {
                assert_eq!(got.name(), n);
                assert!(got.filename().is_none());
                assert_eq!(got.text().unwrap(), normalized(v));
            }
            Part::File(n, f, ct, data) => {
                assert_eq!(got.name(), n);
                assert_eq!(
                    got.filename().unwrap().html_unescaped().unwrap().as_ref(),
                    f.as_str()
                );
                assert_eq!(
                    got.content_type().map(|c| c.raw().to_vec()),
                    ct.as_ref().map(|c| c.as_bytes().to_vec())
                );
                assert_eq!(got.bytes(), &data[..]);
            }
        }
    }
    Ok(())
}

#[test]
fn test_browser_shapes_round_trip() {
    let parts = [
        Part::Text("title".into(), "Holiday\nphotos\r\nand\rmore".into()),
        Part::Text("quote\"d name\r\n".into(), String::new()),
        Part::Text("emoji \u{1F600}".into(), "caf\u{e9}".into()),
        Part::File(
            "photo".into(),
            "C:\\Users\\me\\beach \"1\".png".into(),
            Some("image/png".into()),
            b"\x89PNG\r\n\x1a\n\r\n--".to_vec(),
        ),
        Part::File("empty".into(), String::new(), None, Vec::new()),
        Part::Text("_charset_".into(), "UTF-8".into()),
    ];
    check_round_trip(&parts, None).unwrap();
    check_round_trip(&parts, Some(b"----WebKitFormBoundaryx7Y")).unwrap();
    check_round_trip(&[], Some(b"X")).unwrap();
    check_round_trip(&[], None).unwrap();
}

#[test]
fn test_content_type_quotes_boundaries_that_need_it() {
    let w = MultipartWriter::with_boundary(Vec::new(), Boundary::new(b"a b:c").unwrap());
    assert_eq!(w.content_type(), "multipart/form-data; boundary=\"a b:c\"");
    check_round_trip(&[Part::Text("a".into(), "1".into())], Some(b"a b:c")).unwrap();
}

#[test]
fn test_attack_t34_header_injection_refused() {
    let refused = |name: &str, filename: Option<&str>, ct: Option<&str>| {
        let mut body = Vec::new();
        let mut w = MultipartWriter::with_boundary(&mut body, Boundary::new(b"X").unwrap());
        let r = match filename {
            Some(f) => w.file_start(name, f, ct),
            None => w.text(name, "v"),
        };
        let e = r.unwrap_err();
        // A refused call writes nothing and poisons the writer.
        assert_eq!(w.text("b", "1").unwrap_err(), FormError::Poisoned);
        let _ = w;
        assert!(
            body.is_empty(),
            "{name:?}: {:?}",
            String::from_utf8_lossy(&body)
        );
        e
    };
    assert!(matches!(
        refused("a\0b", None, None),
        FormError::Syntax {
            reason: SyntaxError::ControlInHeader,
            ..
        }
    ));
    assert!(matches!(
        refused("a\x1bb", None, None),
        FormError::Syntax {
            reason: SyntaxError::ControlInHeader,
            ..
        }
    ));
    assert!(matches!(
        refused("a\\", None, None),
        FormError::Syntax {
            reason: SyntaxError::AmbiguousQuote,
            ..
        }
    ));
    assert!(matches!(
        refused("a%0Ab", None, None),
        FormError::Syntax {
            reason: SyntaxError::AmbiguousQuote,
            ..
        }
    ));
    assert!(matches!(
        refused("f", Some("x.txt\\"), None),
        FormError::Syntax {
            reason: SyntaxError::AmbiguousQuote,
            ..
        }
    ));
    assert!(matches!(
        refused("f", Some("x"), Some("text/html\r\nSet-Cookie: a=b")),
        FormError::Syntax {
            reason: SyntaxError::ControlInHeader,
            ..
        }
    ));
    assert!(matches!(
        refused("f", Some("x"), Some("text/html; x=\"\\\"\"")),
        FormError::Syntax {
            reason: SyntaxError::AmbiguousQuote,
            ..
        }
    ));
    assert!(matches!(
        refused("f", Some("x"), Some("multipart/mixed; boundary=Y")),
        FormError::Syntax {
            reason: SyntaxError::InvalidContentType,
            ..
        }
    ));
    assert!(matches!(
        refused("", None, None),
        FormError::Syntax {
            reason: SyntaxError::EmptyName,
            ..
        }
    ));
    // CR, LF, and quotes are escaped, never refused, and read back intact.
    check_round_trip(
        &[Part::File(
            "n\r\n\"".into(),
            "f\r\n\".txt".into(),
            None,
            b"x".to_vec(),
        )],
        Some(b"X"),
    )
    .unwrap();
}

#[test]
fn test_attack_t35_data_containing_the_delimiter_is_refused_across_splits() {
    // A lone CR or LF before `--X` is refused too: the parser refuses it
    // (D-113).
    for data in [
        &b"--X"[..],
        b"a\r\n--X",
        b"a\r\n--X--",
        b"a\n--X",
        b"a\r--X",
    ] {
        for split in 0..=data.len() {
            let mut w = MultipartWriter::with_boundary(Vec::new(), Boundary::new(b"X").unwrap());
            w.file_start("f", "a", None).unwrap();
            let (a, b) = data.split_at(split);
            let r = w.file_data(a).and_then(|()| w.file_data(b));
            assert!(
                matches!(
                    r,
                    Err(FormError::Syntax {
                        reason: SyntaxError::BoundaryInContent,
                        ..
                    })
                ),
                "{data:?} at {split}"
            );
        }
    }
    // A near miss is data.
    check_round_trip(
        &[Part::File(
            "f".into(),
            "a".into(),
            None,
            b"\r\n--Y\r\n-X\r\n--".to_vec(),
        )],
        Some(b"X"),
    )
    .unwrap();
    let mut w = MultipartWriter::with_boundary(Vec::new(), Boundary::new(b"X").unwrap());
    assert!(matches!(
        w.text("a", "x\n--X"),
        Err(FormError::Syntax {
            reason: SyntaxError::BoundaryInContent,
            ..
        })
    ));
}

#[test]
fn test_file_data_outside_a_file_is_poisoned() {
    let mut w = MultipartWriter::with_boundary(Vec::new(), Boundary::new(b"X").unwrap());
    assert_eq!(w.file_data(b"x").unwrap_err(), FormError::Poisoned);
    assert_eq!(w.finish().unwrap_err(), FormError::Poisoned);
    let mut w = MultipartWriter::with_boundary(Vec::new(), Boundary::new(b"X").unwrap());
    w.text("a", "1").unwrap();
    assert!(w.text("_charset_", "shift_jis").is_err());
}

mod props {
    use super::*;
    use proptest::prelude::*;

    fn text() -> impl Strategy<Value = String> {
        proptest::collection::vec(
            prop_oneof![
                Just('a'),
                Just('Z'),
                Just(' '),
                Just('"'),
                Just('\r'),
                Just('\n'),
                Just('\t'),
                Just('%'),
                Just('2'),
                Just('0'),
                Just('A'),
                Just('\\'),
                Just('-'),
                Just('X'),
                Just('\u{e9}'),
                Just('\0'),
                Just(';'),
                Just('='),
            ],
            0..12,
        )
        .prop_map(|v| v.into_iter().collect())
    }

    fn part() -> impl Strategy<Value = Part> {
        prop_oneof![
            (text(), text()).prop_map(|(n, v)| Part::Text(n, v)),
            (
                text(),
                text(),
                proptest::option::of(prop_oneof![
                    Just("image/png".to_string()),
                    Just("text/plain; charset=utf-8".to_string())
                ]),
                proptest::collection::vec(
                    prop_oneof![
                        Just(b'\r'),
                        Just(b'\n'),
                        Just(b'-'),
                        Just(b'X'),
                        any::<u8>()
                    ],
                    0..24
                )
            )
                .prop_map(|(n, f, ct, d)| Part::File(n, f, ct, d)),
        ]
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(512))]
        #[test]
        fn prop_writer_round_trips_or_refuses(parts in proptest::collection::vec(part(), 0..5), generated in any::<bool>()) {
            let boundary: Option<&[u8]> = if generated { None } else { Some(b"X") };
            match check_round_trip(&parts, boundary) {
                Ok(()) => {}
                Err(FormError::Syntax { .. } | FormError::Encoding { .. }) => {}
                Err(e) => prop_assert!(false, "unexpected {e:?}"),
            }
        }
    }
}
