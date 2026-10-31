//! The capture corpus (NFR-14, T-15): every body a browser or HTTP client
//! sent for the capture page (`tools/capture`) parses to its expected
//! fields, whole and at every split, and every body form-net refuses by
//! design is refused with its reason. A browser added to
//! `tests/corpus/browsers` is checked by the same rows.

#![cfg(feature = "alloc")]

mod common;

use std::fs;
use std::path::{Path, PathBuf};

use common::{Record, assert_split_invariant, whole};
use form_net::{FormError, FormKind, Limits, PartKind, Policy, SyntaxError};

/// The capture directories under `tests/corpus/<group>`, sorted.
fn captures(group: &str) -> Vec<PathBuf> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/corpus")
        .join(group);
    let mut dirs: Vec<PathBuf> = fs::read_dir(&root)
        .unwrap_or_else(|e| panic!("{}: {e}", root.display()))
        .map(|entry| entry.unwrap().path())
        .filter(|p| p.is_dir())
        .collect();
    dirs.sort();
    dirs
}

/// The family a capture directory names: `chrome-155` is `chrome`.
fn family(dir: &Path) -> String {
    let name = dir.file_name().unwrap().to_str().unwrap();
    name.split('-').next().unwrap().to_owned()
}

fn read(dir: &Path, file: &str) -> Vec<u8> {
    fs::read(dir.join(file)).unwrap_or_else(|e| panic!("{}/{file}: {e}", dir.display()))
}

/// The `Content-Type` a capture was sent with, as `FormKind::detect` reads it.
fn kind(dir: &Path, form: &str) -> (Vec<u8>, FormKind) {
    let mut ct = read(dir, &format!("{form}.content-type"));
    while ct.last().is_some_and(|b| b.is_ascii_whitespace()) {
        ct.pop();
    }
    let kind = FormKind::detect(&ct).unwrap_or_else(|e| panic!("{}: {e:?}", dir.display()));
    (ct, kind)
}

fn boundary(dir: &Path, form: &str) -> Vec<u8> {
    match kind(dir, form).1 {
        FormKind::Multipart(b) => b.as_bytes().to_vec(),
        other => panic!("{}: {form} is {other:?}", dir.display()),
    }
}

fn field(name: &str, value: &str) -> Record {
    Record {
        name: name.to_owned(),
        kind: PartKind::Field,
        filename: None,
        data: value.as_bytes().to_vec(),
    }
}

fn file(name: &str, filename: &str, data: &[u8]) -> Record {
    Record {
        name: name.to_owned(),
        kind: PartKind::File,
        filename: Some(filename.as_bytes().to_vec()),
        data: data.to_vec(),
    }
}

fn all_bytes() -> Vec<u8> {
    (0..=255).collect()
}

/// Checks one multipart capture against `expected`, whole, at every split,
/// and through the buffered API.
fn check_multipart(dir: &Path, form: &str, expected: &Result<Vec<Record>, SyntaxError>) {
    let body = read(dir, &format!("{form}.body"));
    let b = boundary(dir, form);
    let got = whole(&b, &body);
    match (expected, &got) {
        (Ok(want), Ok(have)) => assert_eq!(have, want, "{} {form}", dir.display()),
        (Err(want), Err(FormError::Syntax { reason, .. })) => {
            assert_eq!(reason, want, "{} {form}", dir.display())
        }
        _ => panic!(
            "{} {form}: expected {expected:?}, got {got:?}",
            dir.display()
        ),
    }
    assert_split_invariant(&b, &body, &Limits::DEFAULT, &Policy::DEFAULT);
    let (ct, _) = kind(dir, form);
    let buffered = form_net::parse_multipart(&ct, &body);
    match expected {
        Ok(want) => assert_eq!(buffered.unwrap().len(), want.len(), "{}", dir.display()),
        Err(_) => assert!(buffered.is_err(), "{}", dir.display()),
    }
}

fn browser_multipart() -> Vec<Record> {
    vec![
        field("ascii", "hello world"),
        field("unicode", "h\u{e9}llo w\u{f6}rld \u{2713} \u{65e5}\u{672c}"),
        // Every line break normalized to CR LF.
        field("newline", "line1\r\nline2\r\nline3\r\nline4"),
        field("quotes", "say \"hi\" and 'bye'"),
        // Browsers send `"` in a name as `%22`; `name()` decodes the HTML
        // escapes, while file names stay raw (`html_unescaped` decodes them).
        field("na\"me", "quote in the name"),
        field("per%cent", "percent in the name"),
        field("\u{fc}n\u{ef}", "non-ASCII name"),
        field("_charset_", "UTF-8"),
        // An empty file input: an empty file name and no data.
        file("empty", "", b""),
        file("ascii_file", "hello.txt", b"plain text\n"),
        file(
            "unicode_file",
            "r\u{e9}sum\u{e9} \u{2713}.txt",
            b"unicode name\n",
        ),
        file("quote_file", "na%22me.txt", b"quote\n"),
        file("percent_file", "100%.txt", b"percent\n"),
        // A backslash in a file name is literal, as browsers send it (D-082).
        file("backslash_file", "back\\slash.txt", b"backslash\n"),
        file("newline_file", "line%0Abreak.txt", b"newline\n"),
        file("multi", "a.txt", b"first\r\n--not a boundary\r\n"),
        file("multi", "b.bin", &all_bytes()),
    ]
}

#[test]
fn test_attack_t15_browser_captures_parse_to_their_fields() {
    let dirs = captures("browsers");
    let families: Vec<String> = dirs.iter().map(|d| family(d)).collect();
    for required in ["chrome", "edge"] {
        assert!(
            families.iter().any(|f| f == required),
            "no {required} capture"
        );
    }
    for dir in &dirs {
        check_multipart(dir, "multipart", &Ok(browser_multipart()));
        let (ct, _) = kind(dir, "multipart");
        let body = read(dir, "multipart.body");
        let mp = form_net::parse_multipart(&ct, &body).unwrap();
        for (name, chosen) in [
            ("quote_file", "na\"me.txt"),
            (
                "newline_file",
                "line
break.txt",
            ),
            ("percent_file", "100%.txt"),
        ] {
            let filename = mp.file(name).unwrap().filename().unwrap();
            assert_eq!(
                filename.html_unescaped().unwrap(),
                chosen,
                "{}",
                dir.display()
            );
        }
        // A backslash in a part name is refused by design (D-082).
        check_multipart(dir, "refused", &Err(SyntaxError::MalformedParameter));

        let (_, k) = kind(dir, "urlencoded");
        assert!(matches!(k, FormKind::Urlencoded), "{}", dir.display());
        let form = form_net::parse_urlencoded(&read(dir, "urlencoded.body")).unwrap();
        let pairs: Vec<(&str, &str)> = form.iter().collect();
        assert_eq!(
            pairs,
            [
                ("ascii", "hello world"),
                ("unicode", "h\u{e9}llo w\u{f6}rld \u{2713} \u{65e5}\u{672c}"),
                ("newline", "line1\r\nline2\r\nline3\r\nline4"),
                ("symbols", "a+b=c&d%e;f/g?h#i"),
                ("na\"me", "quote in the name"),
                ("_charset_", "UTF-8"),
            ],
            "{}",
            dir.display()
        );
    }
}

#[test]
fn test_client_captures_parse_to_their_fields() {
    let dirs = captures("clients");
    let families: Vec<String> = dirs.iter().map(|d| family(d)).collect();
    for required in ["curl", "requests", "go"] {
        assert!(
            families.iter().any(|f| f == required),
            "no {required} capture"
        );
    }
    for dir in &dirs {
        let fam = family(dir);
        let multipart = vec![
            field("ascii", "hello world"),
            field("unicode", "h\u{e9}llo w\u{f6}rld \u{2713} \u{65e5}\u{672c}"),
            field("newline", "line1\r\nline2"),
            file("ascii_file", "hello.txt", b"plain text\n"),
            file(
                "unicode_file",
                "r\u{e9}sum\u{e9} \u{2713}.txt",
                b"unicode name\n",
            ),
            file("multi", "a.txt", b"first\r\n--not a boundary\r\n"),
            file("multi", "b.bin", &all_bytes()),
        ];
        check_multipart(dir, "multipart", &Ok(multipart));
        // curl and requests escape `"` in a file name as browsers do; Go's
        // mime/multipart writes `\"`, which form-net refuses because RFC 9110
        // and HTML readers disagree on it.
        let quote = if fam == "go" {
            Err(SyntaxError::AmbiguousQuote)
        } else {
            Ok(vec![file("quote_file", "na%22me.txt", b"quote\n")])
        };
        check_multipart(dir, "quote", &quote);
    }
}
