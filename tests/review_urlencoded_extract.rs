//! The 0.9.x adversarial review of the urlencoded, extraction, writer, and
//! error code: one test per finding. Each test failed on 0.9.0; the comment
//! on it describes the defect as found, and the fix is in the decision log
//! (D-136 to D-138, D-146).

#![cfg(feature = "alloc")]

use form_net::FromForm;

const SECRET: &str = "hunter2-secret";

/// API.md: "No `Debug`, `Display`, or error output of form-net ever
/// contains a field value". `Pairs` derives `Debug`, and since D-104 it
/// holds the whole body as `whole: Option<&str>`, so `{:?}` prints every
/// value in the body.
#[test]
fn test_pairs_debug_never_shows_the_body() {
    let body = format!("email=a%40b.example&password={SECRET}");
    let it = form_net::pairs(body.as_bytes());
    let shown = format!("{it:?}");
    assert!(!shown.contains(SECRET), "Pairs Debug printed: {shown}");
}

/// The same promise, for the client side: `Serializer` derives `Debug`
/// over the `String` it is building, so `{:?}` prints every value written
/// so far.
#[test]
fn test_serializer_debug_never_shows_values() {
    let mut out = String::new();
    let mut ser = form_net::Serializer::new(&mut out);
    ser.append_pair("password", SECRET);
    let shown = format!("{ser:?}");
    assert!(!shown.contains(SECRET), "Serializer Debug printed: {shown}");
}

form_net::form! {
    pub struct Avatar {
        pub image: form_net::FileData,
    }
}

/// API.md value rules: "A scalar given twice is `Duplicate` for every type
/// except `Vec`." A `FileData` (and `StoredFile`) field given a real file
/// and then an empty file input under the same name is accepted with the
/// first file: the empty-input shortcut returns before the duplicate check.
/// A last-wins reader (PHP's `$_FILES`) sees no file for this body.
#[test]
fn test_file_field_given_twice_with_an_empty_file_is_duplicate() {
    let body =
        b"--X\r\nContent-Disposition: form-data; name=image; filename=\"a.png\"\r\n\r\nREAL\r\n\
--X\r\nContent-Disposition: form-data; name=image; filename=\"\"\r\n\r\n\r\n--X--";
    let r = Avatar::from_multipart(b"multipart/form-data; boundary=X", body);
    match r {
        Err(e) => assert_eq!(e.code(), "extract.duplicate"),
        Ok(a) => panic!(
            "two `image` parts accepted, read as {} bytes",
            a.image.len()
        ),
    }
}

/// D-114 refuses an unquoted `name` or `filename` that starts or ends with
/// `'`, because readers that take single quotes as quoting strip them. The
/// request boundary has the same ambiguity and is not refused:
/// `boundary='abc'` is the boundary `'abc'` to form-net and `abc` to such a
/// reader, so a body framed with `--abc` for one and `--'abc'` for the
/// other reads as two different forms.
#[test]
fn test_unquoted_boundary_in_single_quotes_is_refused() {
    for ct in [
        &b"multipart/form-data; boundary='abc'"[..],
        b"multipart/form-data; boundary='abc",
        b"multipart/form-data; boundary=abc'",
    ] {
        let r = form_net::FormKind::detect(ct);
        assert!(
            r.is_err(),
            "{:?} accepted as {r:?}",
            String::from_utf8_lossy(ct)
        );
    }
}

/// The writer side of the same ambiguity: a caller boundary `'abc'` is
/// accepted and `content_type()` sends it unquoted, as `boundary='abc'`.
#[test]
fn test_writer_never_sends_a_boundary_token_in_single_quotes() {
    let Ok(b) = form_net::Boundary::new(b"'abc'") else {
        return;
    };
    let w = form_net::MultipartWriter::with_boundary(Vec::new(), b);
    let ct = w.content_type();
    assert!(!ct.ends_with("boundary='abc'"), "writer sent {ct}");
}

/// The compile-time check behind `form!`'s refusal of a repeated wire name
/// (the `compile_fail` examples on `form!` show the macro side).
#[test]
fn test_repeated_wire_name_check() {
    assert!(form_net::__repeated_wire_name(&["a", "b", "a"]));
    assert!(form_net::__repeated_wire_name(&[
        "csrf_token",
        "csrf_token"
    ]));
    assert!(!form_net::__repeated_wire_name(&["a", "b", "ab", "ba"]));
    assert!(!form_net::__repeated_wire_name(&[]));
}

/// Found by `fuzz_writer_roundtrip` after D-136: the writer sent a part
/// content type with an empty parameter, and a name holding `name=`, which
/// the parser now refuses. The writer refuses both, so it never sends a body
/// form-net would not read.
#[test]
fn test_writer_refuses_what_the_parser_refuses() {
    // A refused head poisons the writer, so each case gets its own.
    let writer = || {
        form_net::MultipartWriter::with_boundary(Vec::new(), form_net::Boundary::new(b"B").unwrap())
    };
    let code = |r: Result<(), form_net::FormError>| r.map_err(|e| e.code());
    assert_eq!(
        code(writer().file("f", "", Some("%/G;;"), &b""[..]).map(|_| ())),
        Err("syntax.invalid_content_type")
    );
    assert_eq!(
        code(writer().text("name=x", "v")),
        Err("syntax.malformed_parameter")
    );
    assert_eq!(
        code(
            writer()
                .file("f", "a; name=role", None, &b""[..])
                .map(|_| ())
        ),
        Err("syntax.malformed_parameter")
    );
    // In a value it is data, not a parameter.
    let mut w = writer();
    assert_eq!(code(w.text("plain", "name=x is fine in a value")), Ok(()));
    let ct = w.content_type();
    let body = w.finish().unwrap();
    let mp = form_net::parse_multipart(ct.as_bytes(), &body).unwrap();
    assert_eq!(mp.field("plain"), Some("name=x is fine in a value"));
}
