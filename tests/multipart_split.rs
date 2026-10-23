//! Split invariance: the events are identical for every split of the same
//! body (FR-42, T-32), and agree with a reference parser.

#![cfg(feature = "alloc")]

mod common;

use common::{Record, assert_split_invariant, body, reference, whole};
use form_net::{Limits, PartKind, Policy};

const CD: &str = "Content-Disposition: form-data";

fn cases() -> Vec<(&'static str, Vec<u8>)> {
    vec![
        ("X", body("X", &[(&format!("{CD}; name=\"a\""), b"1")])),
        ("X", body("X", &[(&format!("{CD}; name=\"a\""), b"")])),
        ("X", b"--X--".to_vec()),
        ("X", b"--X--\r\n".to_vec()),
        ("X", b"\r\n--X\r\nContent-Disposition: form-data; name=\"a\"\r\n\r\nv\r\n--X--".to_vec()),
        ("X", b"preamble\r\n--X\r\nContent-Disposition: form-data; name=\"a\"\r\n\r\nv\r\n--X--epilogue".to_vec()),
        (
            "----WebKitFormBoundary7MA4YWxkTrZu0gW",
            body(
                "----WebKitFormBoundary7MA4YWxkTrZu0gW",
                &[
                    (&format!("{CD}; name=\"title\""), b"Holiday"),
                    (
                        &format!("{CD}; name=\"avatar\"; filename=\"me.png\"\r\nContent-Type: image/png"),
                        b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR",
                    ),
                    (&format!("{CD}; name=\"notes\""), b"line one\r\nline two\r\n"),
                ],
            ),
        ),
        // Near misses of the delimiter inside data.
        ("ab", body("ab", &[(&format!("{CD}; name=\"a\""), b"\r\n--a\r\n--ac\r\r\n-\r\n--a\r")])),
        ("--", body("--", &[(&format!("{CD}; name=\"a\""), b"\r\n---\r\n---x\r\n-")])),
        ("aaa", body("aaa", &[(&format!("{CD}; name=\"a\""), b"\r\n--aa\r\n--aab\r\n--a")])),
        // Data that is all CR, all dashes.
        ("B", body("B", &[(&format!("{CD}; name=\"a\""), &[b'\r'; 40][..])])),
        ("B", body("B", &[(&format!("{CD}; name=\"a\""), &[b'-'; 40][..])])),
        // Padding after delimiters.
        ("X", b"--X  \r\nContent-Disposition: form-data; name=\"a\"\r\n\r\n1\r\n--X\t\r\nContent-Disposition: form-data; name=\"b\"\r\n\r\n2\r\n--X--".to_vec()),
        // Escaped names, file names with backslashes and raw UTF-8.
        ("X", body("X", &[(&format!("{CD}; name=\"a%22b\"; filename=\"C:\\x\\caf\u{e9}.txt\""), b"z")])),
    ]
}

#[test]
fn test_split_every_byte_gives_identical_events() {
    for (boundary, b) in cases() {
        assert_split_invariant(boundary.as_bytes(), &b, &Limits::DEFAULT, &Policy::DEFAULT);
        assert!(
            whole(boundary.as_bytes(), &b).is_ok(),
            "{boundary}: {:?}",
            whole(boundary.as_bytes(), &b)
        );
    }
}

#[test]
fn test_split_errors_are_split_invariant_too() {
    let bad: Vec<(&str, Vec<u8>)> = vec![
        (
            "X",
            b"--X\r\nContent-Disposition: form-data; name=\"a\"\r\n\r\n1\r\n--Xjunk".to_vec(),
        ),
        (
            "X",
            b"--X\r\nContent-Disposition: form-data; name=\"a\"\r\n\r\n1".to_vec(),
        ),
        (
            "X",
            b"--X\r\nContent-Disposition: form-data; name=\"a\"\n\n1\r\n--X--".to_vec(),
        ),
        ("X", b"no delimiter at all".to_vec()),
        (
            "X",
            b"--X\r\nContent-Type: text/plain\r\n\r\n1\r\n--X--".to_vec(),
        ),
    ];
    for (boundary, b) in bad {
        assert!(whole(boundary.as_bytes(), &b).is_err());
        assert_split_invariant(boundary.as_bytes(), &b, &Limits::DEFAULT, &Policy::DEFAULT);
    }
}

#[test]
fn test_streaming_agrees_with_reference_parser() {
    for (boundary, b) in cases() {
        if let Some(expected) = reference(boundary.as_bytes(), &b) {
            assert_eq!(
                whole(boundary.as_bytes(), &b).unwrap(),
                expected,
                "{boundary}"
            );
        }
    }
}

#[test]
fn test_cr_at_chunk_end_then_delimiter() {
    let b = body("B", &[(&format!("{CD}; name=\"a\""), b"data\r")]);
    let parts = whole(b"B", &b).unwrap();
    assert_eq!(parts[0].data, b"data\r");
    assert_eq!(
        parts,
        vec![Record {
            name: "a".into(),
            kind: PartKind::Field,
            filename: None,
            data: b"data\r".to_vec()
        }]
    );
}
