//! Typed extraction (FR-58 to FR-60): `form!` with every field type,
//! unknown, missing, duplicate, and invalid values, both encodings.

#![cfg(feature = "alloc")]

use form_net::{ExtractError, FileData, FormError, FromForm, Secret};

form_net::form! {
    /// Every shipped field type.
    #[derive(Debug)]
    pub struct All {
        pub s: String,
        pub secret: Secret,
        pub n8: u8,
        pub n64: u64,
        pub i: i32,
        pub f: f64,
        pub b: bool,
        pub c: char,
        pub maybe: Option<String>,
        pub many: Vec<u16> = "many[]",
        pub renamed: String = "display-name",
    }
    ignore = ["csrf_token", "_charset_"];
}

const OK: &[u8] = b"s=x&secret=pw&n8=255&n64=18446744073709551615&i=-7&f=1.5&b=on&c=%C3%A9&many[]=1&many[]=2&display-name=Ada&csrf_token=t&csrf_token=u";

fn reason(r: Result<All, FormError>) -> (ExtractError, Option<&'static str>) {
    match r {
        Err(FormError::Extract { reason, field }) => (reason, field),
        other => panic!("expected an extraction error, got {other:?}"),
    }
}

#[test]
fn test_every_type_extracts() {
    let a = All::from_urlencoded(OK).unwrap();
    assert!(!format!("{a:?}").contains("pw"), "Secret must not print");
    assert_eq!(
        (a.s.as_str(), a.secret.expose(), a.n8, a.n64),
        ("x", "pw", 255, u64::MAX)
    );
    assert_eq!((a.i, a.f, a.b, a.c), (-7, 1.5, true, 'é'));
    assert_eq!(
        (a.maybe, a.many, a.renamed.as_str()),
        (None, vec![1, 2], "Ada")
    );
}

#[test]
fn test_unknown_missing_duplicate_invalid() {
    let mut extra = OK.to_vec();
    extra.extend_from_slice(b"&admin=1");
    assert_eq!(
        reason(All::from_urlencoded(&extra)),
        (ExtractError::Unknown, None)
    );
    let missing = b"secret=pw&n8=1&n64=1&i=1&f=1&c=a&display-name=x";
    assert_eq!(
        reason(All::from_urlencoded(missing)),
        (ExtractError::Missing, Some("s"))
    );
    let mut dup = OK.to_vec();
    dup.extend_from_slice(b"&s=y");
    assert_eq!(
        reason(All::from_urlencoded(&dup)),
        (ExtractError::Duplicate, Some("s"))
    );
    let bad = String::from_utf8(OK.to_vec())
        .unwrap()
        .replace("n8=255", "n8=256");
    assert_eq!(
        reason(All::from_urlencoded(bad.as_bytes())),
        (ExtractError::Invalid("out of range"), Some("n8"))
    );
    let bad = String::from_utf8(OK.to_vec())
        .unwrap()
        .replace("b=on", "b=yes");
    assert!(matches!(
        reason(All::from_urlencoded(bad.as_bytes())),
        (ExtractError::Invalid(_), Some("b"))
    ));
    let bad = String::from_utf8(OK.to_vec())
        .unwrap()
        .replace("i=-7", "i=+7");
    assert!(matches!(
        reason(All::from_urlencoded(bad.as_bytes())),
        (ExtractError::Invalid(_), Some("i"))
    ));
}

#[test]
fn test_errors_never_echo_input() {
    let bad = String::from_utf8(OK.to_vec())
        .unwrap()
        .replace("n8=255", "n8=sneaky-input");
    let e = All::from_urlencoded(bad.as_bytes()).unwrap_err();
    assert!(!format!("{e} {e:?}").contains("sneaky"));
}

form_net::form! {
    pub struct Upload {
        pub title: String,
        pub photo: FileData,
        pub extra: Option<FileData>,
        pub tags: Vec<String>,
    }
}

#[test]
fn test_multipart_and_from_body_agree() {
    let ct = b"multipart/form-data; boundary=X";
    let body = b"--X\r\nContent-Disposition: form-data; name=title\r\n\r\nHi\r\n--X\r\nContent-Disposition: form-data; name=photo; filename=\"a.png\"\r\nContent-Type: image/png\r\n\r\nPNG\r\n--X\r\nContent-Disposition: form-data; name=extra; filename=\"\"\r\n\r\n\r\n--X\r\nContent-Disposition: form-data; name=tags\r\n\r\nt1\r\n--X--";
    let u = Upload::from_multipart(ct, body).unwrap();
    assert_eq!(
        (u.title.as_str(), u.photo.bytes(), u.photo.filename()),
        ("Hi", &b"PNG"[..], Some("a.png"))
    );
    assert!(u.extra.is_none(), "an empty file input maps to None");
    assert_eq!(u.tags, ["t1"]);
    let u2 = Upload::from_body(ct, body).unwrap();
    assert_eq!(u2.photo.content_type(), Some("image/png"));
    // A file where text is expected, and the reverse.
    let wrong = b"--X\r\nContent-Disposition: form-data; name=title; filename=a\r\n\r\nx\r\n--X--";
    assert!(matches!(
        Upload::from_multipart(ct, wrong),
        Err(FormError::Extract {
            reason: ExtractError::WrongKind,
            field: Some("title")
        })
    ));
    let wrong = b"title=a&photo=x";
    assert!(matches!(
        Upload::from_urlencoded(wrong),
        Err(FormError::Extract {
            reason: ExtractError::WrongKind,
            field: Some("photo")
        })
    ));
}

form_net::form! {
    pub struct Empty {}
}

form_net::form! {
    #[derive(Debug)]
    pub struct Search {
        pub q: String,
        pub page: Option<u32>,
    }
}

#[test]
fn test_query_empty_struct_and_encodings() {
    assert!(Empty::from_urlencoded(b"").is_ok());
    assert!(Empty::from_urlencoded(b"a=1").is_err());
    let s = Search::from_query(b"?q=rust+forms&page=2").unwrap();
    assert_eq!((s.q.as_str(), s.page), ("rust forms", Some(2)));
    let s = Search::from_body(b"application/x-www-form-urlencoded", b"q=a").unwrap();
    assert_eq!(s.page, None);
    assert_eq!(
        Search::from_body(b"text/plain", b"q=a").err(),
        Some(FormError::UnsupportedMediaType)
    );
    // A long value decodes through the heap buffer, not the stack.
    let long = format!("q={}", "x".repeat(4000));
    assert_eq!(
        Search::from_urlencoded(long.as_bytes()).unwrap().q.len(),
        4000
    );
}

form_net::form! {
    pub struct Wide {
        pub f00: u8, pub f01: u8, pub f02: u8, pub f03: u8, pub f04: u8, pub f05: u8, pub f06: u8, pub f07: u8,
        pub f08: u8, pub f09: u8, pub f10: u8, pub f11: u8, pub f12: u8, pub f13: u8, pub f14: u8, pub f15: u8,
        pub f16: u8, pub f17: u8, pub f18: u8, pub f19: u8, pub f20: u8, pub f21: u8, pub f22: u8, pub f23: u8,
        pub f24: u8, pub f25: u8, pub f26: u8, pub f27: u8, pub f28: u8, pub f29: u8, pub f30: u8, pub f31: u8,
        pub f32: u8, pub f33: u8, pub f34: u8, pub f35: u8, pub f36: u8, pub f37: u8, pub f38: u8, pub f39: u8,
        pub f40: u8, pub f41: u8, pub f42: u8, pub f43: u8, pub f44: u8, pub f45: u8, pub f46: u8, pub f47: u8,
        pub f48: u8, pub f49: u8, pub f50: u8, pub f51: u8, pub f52: u8, pub f53: u8, pub f54: u8, pub f55: u8,
        pub f56: u8, pub f57: u8, pub f58: u8, pub f59: u8, pub f60: u8, pub f61: u8, pub f62: u8, pub f63: u8,
    }
}

#[test]
fn test_sixty_four_fields() {
    let body: Vec<String> = (0..64).map(|i| format!("f{i:02}={i}")).collect();
    let w = Wide::from_urlencoded(body.join("&").as_bytes()).unwrap();
    assert_eq!((w.f00, w.f31, w.f63), (0, 31, 63));
}

#[test]
fn test_inspected_extraction_sees_the_same_bytes() {
    use form_net::{Inspector, Limits, Policy, Refusal, Verdict};
    struct NoAdmin(Vec<String>);
    impl Inspector for NoAdmin {
        fn field(&mut self, name: &str, value: &[u8]) -> Verdict {
            self.0
                .push(format!("{name}={}", String::from_utf8_lossy(value)));
            if value == b"admin" {
                Verdict::Refuse(Refusal::new(403, "test.admin"))
            } else {
                Verdict::Continue
            }
        }
    }
    let mut i = NoAdmin(Vec::new());
    let s = Search::from_body_inspected(
        b"application/x-www-form-urlencoded",
        b"q=a+b",
        &Limits::DEFAULT,
        &Policy::DEFAULT,
        &mut i,
    )
    .unwrap();
    assert_eq!(
        (s.q.as_str(), i.0.as_slice()),
        ("a b", &["q=a b".to_string()][..])
    );
    let e = Search::from_body_inspected(
        b"application/x-www-form-urlencoded",
        b"q=admin",
        &Limits::DEFAULT,
        &Policy::DEFAULT,
        &mut i,
    )
    .unwrap_err();
    assert_eq!(e.status_hint(), 403);
}

#[cfg(feature = "fs")]
mod stored {
    use form_net::{FromForm, StoredFile};

    form_net::form! {
        pub struct Avatar {
            pub user: u64,
            pub picture: StoredFile,
            pub thumbs: Vec<StoredFile>,
        }
    }

    #[test]
    fn test_from_upload_moves_files_into_the_struct() {
        let ct = b"multipart/form-data; boundary=X";
        let mut body = Vec::new();
        body.extend_from_slice(b"--X\r\nContent-Disposition: form-data; name=user\r\n\r\n42\r\n");
        for (name, data) in [
            ("picture", &b"big"[..]),
            ("thumbs", b"t1"),
            ("thumbs", b"t2"),
        ] {
            body.extend_from_slice(
                format!("--X\r\nContent-Disposition: form-data; name={name}; filename=x\r\n\r\n")
                    .as_bytes(),
            );
            body.extend_from_slice(data);
            body.extend_from_slice(b"\r\n");
        }
        body.extend_from_slice(b"--X--");
        let up = form_net::read_multipart(ct, &body[..]).unwrap();
        let a = Avatar::from_upload(up).unwrap();
        assert_eq!(
            (a.user, a.picture.read_to_vec(16).unwrap()),
            (42, b"big".to_vec())
        );
        assert_eq!(a.thumbs.len(), 2);
        assert_eq!(a.thumbs[1].read_to_vec(16).unwrap(), b"t2");
        let up = form_net::read_multipart(
            ct,
            &b"--X\r\nContent-Disposition: form-data; name=user\r\n\r\n42\r\n--X--"[..],
        )
        .unwrap();
        assert!(matches!(
            Avatar::from_upload(up),
            Err(form_net::FormError::Extract {
                reason: form_net::ExtractError::Missing,
                field: Some("picture")
            })
        ));
    }
}
