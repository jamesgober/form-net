//! Every multipart refusal with a minimal body, and the bodies browsers send
//! that must be accepted.

#![cfg(feature = "alloc")]

mod common;

use common::{body, whole};
use form_net::{
    EncodingError, ExtendedFilenamePolicy, FormError, HeaderPolicy, Limits, PartKind, Policy,
    PolicyRefusal, SyntaxError,
};

fn syntax(b: &[u8]) -> Option<SyntaxError> {
    match whole(b"X", b) {
        Err(FormError::Syntax { reason, .. }) => Some(reason),
        other => panic!("expected a syntax error, got {other:?}"),
    }
}

fn policy_refusal(b: &[u8], policy: &Policy) -> Option<PolicyRefusal> {
    match common::drive(b"X", &[b], &Limits::DEFAULT, policy) {
        Err(FormError::Policy { reason, .. }) => Some(reason),
        other => panic!("expected a policy refusal, got {other:?}"),
    }
}

fn one(headers: &str, data: &[u8]) -> Vec<u8> {
    body("X", &[(headers, data)])
}

#[test]
fn test_framing_refusals() {
    assert_eq!(syntax(b"nothing"), Some(SyntaxError::NoInitialDelimiter));
    assert_eq!(syntax(b""), Some(SyntaxError::NoInitialDelimiter));
    assert_eq!(syntax(b"--X"), Some(SyntaxError::Truncated));
    assert_eq!(syntax(b"--X\r\n"), Some(SyntaxError::Truncated));
    assert_eq!(syntax(b"--Xy\r\n"), Some(SyntaxError::BoundaryInContent));
    assert_eq!(syntax(b"--X-x"), Some(SyntaxError::BoundaryInContent));
    assert_eq!(syntax(b"--X\n"), Some(SyntaxError::BareLineFeed));
    assert_eq!(syntax(b"--X \n"), Some(SyntaxError::BareLineFeed));
    let truncated = b"--X\r\nContent-Disposition: form-data; name=a\r\n\r\ndata";
    assert_eq!(syntax(truncated), Some(SyntaxError::Truncated));
    let in_delimiter = b"--X\r\nContent-Disposition: form-data; name=a\r\n\r\ndata\r\n-";
    assert_eq!(syntax(in_delimiter), Some(SyntaxError::Truncated));
    let delimiter_prefix_in_data =
        b"--X\r\nContent-Disposition: form-data; name=a\r\n\r\ndata\r\n--Xs\r\n--X--";
    assert_eq!(
        syntax(delimiter_prefix_in_data),
        Some(SyntaxError::BoundaryInContent)
    );
}

#[test]
fn test_header_refusals() {
    assert_eq!(
        syntax(&one("X-Only: 1", b"")),
        Some(SyntaxError::MissingContentDisposition)
    );
    assert_eq!(
        syntax(b"--X\r\n\r\ndata\r\n--X--"),
        Some(SyntaxError::MissingContentDisposition)
    );
    assert_eq!(
        syntax(&one(
            "Content-Disposition: form-data; name=a\r\nContent-Disposition: form-data; name=b",
            b""
        )),
        Some(SyntaxError::DuplicateHeader)
    );
    assert_eq!(
        syntax(&one(
            "Content-Disposition: form-data; name=a\r\nContent-Type: a/b\r\ncontent-type: a/b",
            b""
        )),
        Some(SyntaxError::DuplicateHeader)
    );
    assert_eq!(
        syntax(&one("Content-Disposition: inline; name=a", b"")),
        Some(SyntaxError::NotFormData)
    );
    assert_eq!(
        syntax(&one("Content-Disposition: form-data", b"")),
        Some(SyntaxError::MissingName)
    );
    assert_eq!(
        syntax(&one("Content-Disposition: form-data; name=\"\"", b"")),
        Some(SyntaxError::EmptyName)
    );
    assert_eq!(
        syntax(&one("Content-Disposition: form-data; name=a; Name=b", b"")),
        Some(SyntaxError::DuplicateParameter)
    );
    assert_eq!(
        syntax(&one(
            "Content-Disposition: form-data; name=a; filename=x; filename=y",
            b""
        )),
        Some(SyntaxError::DuplicateParameter)
    );
    assert_eq!(
        syntax(&one("Content-Disposition: form-data; name=\"a\\\"b\"", b"")),
        Some(SyntaxError::AmbiguousQuote)
    );
    assert_eq!(
        syntax(&one(
            "Content-Disposition: form-data; filename=\"a\\\"; name=\"x\"",
            b""
        )),
        Some(SyntaxError::AmbiguousQuote)
    );
    assert_eq!(
        syntax(&one(
            "Content-Disposition: form-data; name=\"unterminated",
            b""
        )),
        Some(SyntaxError::MalformedParameter)
    );
    assert_eq!(
        syntax(&one(
            "Content-Disposition: form-data; name=a\r\n Folded: x",
            b""
        )),
        Some(SyntaxError::HeaderFolding)
    );
    assert_eq!(
        syntax(&one("Content-Disposition : form-data; name=a", b"")),
        Some(SyntaxError::InvalidHeaderName)
    );
    assert_eq!(
        syntax(&one(
            "Content-Disposition: form-data; name=a\r\nNo colon here",
            b""
        )),
        Some(SyntaxError::MalformedHeaderLine)
    );
    assert_eq!(
        syntax(&one(
            "Content-Disposition: form-data; name=a\r\nX: a\x7Fb",
            b""
        )),
        Some(SyntaxError::ControlInHeader)
    );
    assert_eq!(
        syntax(&one(
            "Content-Disposition: form-data; name=a\r\nContent-Type: not a type",
            b""
        )),
        Some(SyntaxError::InvalidContentType)
    );
}

#[test]
fn test_policy_refusals() {
    let d = Policy::DEFAULT;
    assert_eq!(
        policy_refusal(
            &one(
                "Content-Disposition: form-data; name=a\r\nContent-Transfer-Encoding: base64",
                b"aGk="
            ),
            &d
        ),
        Some(PolicyRefusal::TransferEncoding)
    );
    assert_eq!(
        policy_refusal(
            &one(
                "Content-Disposition: form-data; name=a\r\nContent-Type: multipart/mixed; boundary=y",
                b""
            ),
            &d
        ),
        Some(PolicyRefusal::NestedMultipart)
    );
    let strict = Policy::DEFAULT.with_unknown_headers(HeaderPolicy::Refuse);
    assert_eq!(
        policy_refusal(
            &one(
                "Content-Disposition: form-data; name=a\r\nContent-Length: 1",
                b"x"
            ),
            &strict
        ),
        Some(PolicyRefusal::UnknownHeader)
    );
    assert_eq!(
        policy_refusal(
            &one(
                "Content-Disposition: form-data; name=a; filename*=UTF-8''x",
                b""
            ),
            &d
        ),
        Some(PolicyRefusal::ExtendedFilename)
    );
    let accept = Policy::DEFAULT.with_extended_filename(ExtendedFilenamePolicy::AcceptAlone);
    assert_eq!(
        policy_refusal(
            &one(
                "Content-Disposition: form-data; name=a; filename*=UTF-8''x; filename=y",
                b""
            ),
            &accept
        ),
        Some(PolicyRefusal::BothFilenames)
    );
}

#[test]
fn test_accepted_shapes() {
    // Every transfer encoding that is identity, in any case.
    for te in ["7bit", "8BIT", "binary", "Binary"] {
        let b = one(
            &format!("Content-Disposition: form-data; name=a\r\nContent-Transfer-Encoding: {te}"),
            b"1",
        );
        assert!(whole(b"X", &b).is_ok(), "{te}");
    }
    // Unknown headers are ignored by default.
    let b = one(
        "Content-Disposition: form-data; name=a\r\nX-Anything: yes",
        b"1",
    );
    assert_eq!(whole(b"X", &b).unwrap()[0].data, b"1");
    // Header names in any case; whitespace around values.
    let b = one("content-disposition:   form-data;name=\"a\"  ", b"1");
    assert!(whole(b"X", &b).is_ok());
    // A file input left empty.
    let b = one(
        "Content-Disposition: form-data; name=\"f\"; filename=\"\"\r\nContent-Type: application/octet-stream",
        b"",
    );
    let parts = whole(b"X", &b).unwrap();
    assert_eq!(parts[0].kind, PartKind::File);
    assert_eq!(parts[0].filename.as_deref(), Some(&b""[..]));
    // filename* accepted alone under AcceptAlone, decoded.
    let accept = Policy::DEFAULT.with_extended_filename(ExtendedFilenamePolicy::AcceptAlone);
    let b = one(
        "Content-Disposition: form-data; name=f; filename*=UTF-8''%E2%82%AC.txt",
        b"z",
    );
    let parts = common::drive(b"X", &[&b], &Limits::DEFAULT, &accept).unwrap();
    assert_eq!(
        parts[0].filename.as_deref(),
        Some("\u{20ac}.txt".as_bytes())
    );
    // A %00 in a file name stays literal.
    let b = one(
        "Content-Disposition: form-data; name=f; filename=\"a%00b\"",
        b"",
    );
    assert_eq!(
        whole(b"X", &b).unwrap()[0].filename.as_deref(),
        Some(&b"a%00b"[..])
    );
}

#[test]
fn test_buffered_text_must_be_text() {
    let b = one("Content-Disposition: form-data; name=a", b"\xff");
    assert!(matches!(
        form_net::parse_multipart(b"multipart/form-data; boundary=X", &b),
        Err(FormError::Encoding {
            reason: EncodingError::InvalidUtf8,
            ..
        })
    ));
    let b = one(
        "Content-Disposition: form-data; name=a\r\nContent-Type: text/plain; charset=shift_jis",
        b"x",
    );
    assert!(matches!(
        form_net::parse_multipart(b"multipart/form-data; boundary=X", &b),
        Err(FormError::Encoding {
            reason: EncodingError::UnsupportedCharset,
            ..
        })
    ));
    // _charset_ governs the fields after it.
    let b = body(
        "X",
        &[
            ("Content-Disposition: form-data; name=before", b"ok"),
            (
                "Content-Disposition: form-data; name=_charset_",
                b"windows-1252",
            ),
            ("Content-Disposition: form-data; name=after", b"x"),
        ],
    );
    assert!(matches!(
        form_net::parse_multipart(b"multipart/form-data; boundary=X", &b),
        Err(FormError::Encoding {
            reason: EncodingError::UnsupportedCharset,
            ..
        })
    ));
    let b = body(
        "X",
        &[
            ("Content-Disposition: form-data; name=_charset_", b"UTF-8"),
            ("Content-Disposition: form-data; name=after", b"x"),
        ],
    );
    let mp = form_net::parse_multipart(b"multipart/form-data; boundary=X", &b).unwrap();
    assert_eq!(mp.field("after"), Some("x"));
    // A file part's bytes are never checked as text.
    let b = one(
        "Content-Disposition: form-data; name=f; filename=a",
        b"\xff\xfe",
    );
    let mp = form_net::parse_multipart(b"multipart/form-data; boundary=X", &b).unwrap();
    assert!(mp.file("f").unwrap().text().is_err());
}

#[test]
fn test_poisoned_after_error_until_reset() {
    use form_net::{Boundary, MultipartParser};
    let mut p = MultipartParser::new(
        Boundary::new(b"X").unwrap(),
        &Limits::DEFAULT,
        &Policy::DEFAULT,
    )
    .unwrap();
    assert!(p.parse(b"--Xjunk", false).is_err());
    assert_eq!(p.parse(b"", false).unwrap_err(), FormError::Poisoned);
    p.reset(Boundary::new(b"X").unwrap());
    let step = p.parse(b"--X--", true).unwrap();
    assert!(matches!(step.event, form_net::Event::End));
}
