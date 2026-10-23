//! One attack test per threat in `dev/PLAN.md` section 10 that the core
//! covers. Each must fail to get through.

#![cfg(feature = "alloc")]

mod common;

use std::time::{Duration, Instant};

use form_net::{EncodingError, FormError, FormKind, Limit, Limits, PolicyRefusal, SyntaxError};

const CT: &[u8] = b"multipart/form-data; boundary=X";

fn quick<R>(f: impl FnOnce() -> R) -> R {
    let start = Instant::now();
    let r = f();
    // Generous: an unoptimized build on a shared machine. The point is that
    // nothing here scales with the attacker's input.
    assert!(
        start.elapsed() < Duration::from_secs(2),
        "took {:?}",
        start.elapsed()
    );
    r
}

fn limit(r: Result<impl Sized, FormError>) -> Limit {
    match r {
        Err(FormError::Limit { limit, .. }) => limit,
        Err(e) => panic!("expected a limit, got {e:?}"),
        Ok(_) => panic!("expected a limit, got Ok"),
    }
}

#[test]
fn test_attack_t1_many_tiny_parts_refused() {
    let mut body = Vec::new();
    for _ in 0..100_000 {
        body.extend_from_slice(b"--X\r\nContent-Disposition: form-data; name=a\r\n\r\n\r\n");
    }
    body.truncate(9 << 20);
    let err = quick(|| form_net::parse_multipart(CT, &body));
    match err {
        Err(FormError::Limit {
            limit: Limit::Parts,
            at,
        }) => assert_eq!(at.part, Some(100)),
        other => panic!("{other:?}"),
    }
}

#[test]
fn test_attack_t2_many_fields_refused() {
    let body = b"a=&".repeat(1_000_001);
    let l = Limits::DEFAULT.with_max_body_bytes(4 << 20);
    let r = quick(|| form_net::parse_urlencoded_with(&body, &l, &form_net::Policy::DEFAULT));
    assert_eq!(limit(r), Limit::Fields);
}

#[test]
fn test_attack_t3_huge_preamble_and_epilogue_refused() {
    let mut body = b"\r\n".repeat(4 << 20);
    body.extend_from_slice(b"\r\n--X--");
    assert_eq!(
        limit(quick(|| form_net::parse_multipart(CT, &body))),
        Limit::Preamble
    );
    let mut body = b"--X--".to_vec();
    body.extend(b"\r\n".repeat(4 << 20));
    assert_eq!(
        limit(quick(|| form_net::parse_multipart(CT, &body))),
        Limit::Epilogue
    );
}

#[test]
fn test_attack_t4_cve_2023_46136_shape_is_linear_at_every_chunk_size() {
    // A part that starts with CR LF and then megabytes without CR LF.
    let mut body =
        b"--X\r\nContent-Disposition: form-data; name=f; filename=a\r\n\r\n\r\n".to_vec();
    body.extend(std::iter::repeat_n(b'a', 256 << 10));
    body.extend_from_slice(b"\r\n--X--");
    for chunk in [1usize, 7, 64 * 1024] {
        let chunks: Vec<&[u8]> = body.chunks(chunk).collect();
        let parts =
            quick(|| common::drive(b"X", &chunks, &Limits::DEFAULT, &form_net::Policy::DEFAULT))
                .unwrap();
        assert_eq!(parts[0].data.len(), 2 + (256 << 10));
    }
}

#[test]
fn test_attack_t5_header_bomb_refused() {
    let mut body = b"--X\r\nContent-Disposition: form-data; name=a\r\nX: ".to_vec();
    body.extend(std::iter::repeat_n(b'a', 1 << 20));
    body.extend_from_slice(b"\r\n\r\n\r\n--X--");
    assert_eq!(
        limit(quick(|| form_net::parse_multipart(CT, &body))),
        Limit::PartHeaderBytes
    );
    let mut body = b"--X\r\nContent-Disposition: form-data; name=a\r\n".to_vec();
    body.extend(b"X: 1\r\n".repeat(10_000));
    body.extend_from_slice(b"\r\n\r\n--X--");
    assert_eq!(
        limit(quick(|| form_net::parse_multipart(CT, &body))),
        Limit::PartHeaderCount
    );
}

#[test]
fn test_attack_t6_huge_text_value_refused_at_the_limit() {
    let mut body = b"--X\r\nContent-Disposition: form-data; name=a\r\n\r\n".to_vec();
    body.extend(std::iter::repeat_n(b'a', 9 << 20));
    body.extend_from_slice(b"\r\n--X--");
    match quick(|| form_net::parse_multipart(CT, &body)) {
        Err(FormError::Limit {
            limit: Limit::ValueLength,
            at,
        }) => {
            // The first byte past 1 MiB of the value.
            assert_eq!(at.offset, 47 + (1 << 20));
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn test_attack_t12_delimiter_prefix_inside_data_refused() {
    let body =
        b"--X\r\nContent-Disposition: form-data; name=f; filename=a\r\n\r\nabc\r\n--Xx\r\n--X--";
    assert!(matches!(
        form_net::parse_multipart(CT, body),
        Err(FormError::Syntax {
            reason: SyntaxError::BoundaryInContent,
            ..
        })
    ));
}

#[test]
fn test_attack_t13_two_boundaries_refused() {
    assert!(matches!(
        FormKind::detect(b"multipart/form-data; boundary=a; boundary=b"),
        Err(FormError::Syntax {
            reason: SyntaxError::DuplicateBoundary,
            ..
        })
    ));
    assert!(matches!(
        FormKind::detect(b"multipart/form-data; boundary=a; BOUNDARY=\"a\""),
        Err(FormError::Syntax {
            reason: SyntaxError::DuplicateBoundary,
            ..
        })
    ));
}

#[test]
fn test_attack_t14_waf_differential_shapes_refused() {
    let ok = b"--X\r\nContent-Disposition: form-data; name=\"a\"\r\n\r\n1\r\n--X--";
    assert!(form_net::parse_multipart(CT, ok).is_ok());
    let mutants: &[&[u8]] = &[
        b"--X\r\nContent-Disposition: form-data; name=\"a\"\r\nContent-Disposition: form-data; name=\"b\"\r\n\r\n1\r\n--X--",
        b"--X\r\nContent-Disposition: form-data; name=\"a\"; name=\"b\"\r\n\r\n1\r\n--X--",
        b"--X\r\nContent-Disposition: form-data; name=\"a\"; filename=\"x\"; filename*=UTF-8''y\r\n\r\n1\r\n--X--",
        b"--X\r\nContent-Disposition: form-data; name=\"a\"\r\nContent-Transfer-Encoding: base64\r\n\r\nMQ==\r\n--X--",
        b"--X\nContent-Disposition: form-data; name=\"a\"\n\n1\n--X--",
        b"--X\r\nContent-Disposition: form-data;\r\n name=\"a\"\r\n\r\n1\r\n--X--",
        b"--X\r\nContent-Disposition : form-data; name=\"a\"\r\n\r\n1\r\n--X--",
        b"--X\r\nContent-Disposition: form-data; name=\"a\\\"; x=\"b\"\r\n\r\n1\r\n--X--",
        b"--X\r\nContent-Disposition: form-data; name=\"a\"\r\n\r\n1\r\n--X",
        b"--X\r\nContent-Disposition: form-data; name=\"a\"\r\n\r\n1\r\n--X\r\n",
    ];
    for m in mutants {
        assert!(
            form_net::parse_multipart(CT, m).is_err(),
            "{}",
            String::from_utf8_lossy(m)
        );
    }
    let mut data_after_epilogue = ok.to_vec();
    data_after_epilogue.extend(
        b"\r\n--X\r\nContent-Disposition: form-data; name=\"evil\"\r\n\r\n2\r\n--X--".repeat(20),
    );
    assert!(form_net::parse_multipart(CT, &data_after_epilogue).is_err());
}

#[test]
fn test_attack_t16_semicolon_is_not_a_separator() {
    let form = form_net::parse_urlencoded(b"a=1;b=2").unwrap();
    assert_eq!(form.iter().collect::<Vec<_>>(), [("a", "1;b=2")]);
}

#[test]
fn test_attack_t17_parameter_pollution_refused_by_strict_accessors() {
    let form = form_net::parse_urlencoded(b"role=user&role=admin").unwrap();
    assert!(form.get_one("role").is_err());
    let body = b"--X\r\nContent-Disposition: form-data; name=role\r\n\r\nuser\r\n--X\r\nContent-Disposition: form-data; name=role\r\n\r\nadmin\r\n--X--";
    let mp = form_net::parse_multipart(CT, body).unwrap();
    assert!(mp.field_one("role").is_err());
}

#[test]
fn test_attack_t19_invalid_encodings_refused() {
    for v in [&b"%C0%AF"[..], b"%ED%A0%80", b"%", b"%4", b"%zz"] {
        let mut input = b"a=".to_vec();
        input.extend_from_slice(v);
        assert!(
            matches!(
                form_net::parse_urlencoded(&input),
                Err(FormError::Encoding {
                    reason: EncodingError::InvalidUtf8 | EncodingError::InvalidPercent,
                    ..
                })
            ),
            "{v:?}"
        );
    }
}

#[test]
fn test_attack_t30_errors_and_debug_never_show_values() {
    let secret = "hunter2-secret";
    let form = form_net::parse_urlencoded(format!("password={secret}").as_bytes()).unwrap();
    assert!(!format!("{form:?}").contains(secret));
    let body = format!(
        "--X\r\nContent-Disposition: form-data; name=password; filename=\"{secret}\"\r\n\r\n{secret}\r\n--X--"
    );
    let mp = form_net::parse_multipart(CT, body.as_bytes()).unwrap();
    assert!(!format!("{mp:?}").contains(secret));
    let errors = [
        form_net::parse_urlencoded(format!("{secret}=%zz").as_bytes()).unwrap_err(),
        form_net::parse_multipart(
            CT,
            format!("--X\r\nContent-Disposition: form-data; name={secret}\r\n\r\n{secret}")
                .as_bytes(),
        )
        .unwrap_err(),
    ];
    for e in errors {
        assert!(!format!("{e} {e:?}").contains(secret));
    }
}

#[test]
fn test_attack_t36_quoted_pair_ambiguity_refused() {
    let body = b"--X\r\nContent-Disposition: form-data; name=\"f\"; filename=\"a\\\"; name=\"x\"\r\n\r\n1\r\n--X--";
    assert!(matches!(
        form_net::parse_multipart(CT, body),
        Err(FormError::Syntax {
            reason: SyntaxError::AmbiguousQuote,
            ..
        })
    ));
}

#[test]
fn test_attack_t37_empty_and_missing_names_refused() {
    for head in [
        "Content-Disposition: form-data; name=\"\"",
        "Content-Disposition: form-data; filename=a",
    ] {
        let body = format!("--X\r\n{head}\r\n\r\n1\r\n--X--");
        assert!(matches!(
            form_net::parse_multipart(CT, body.as_bytes()),
            Err(FormError::Syntax {
                reason: SyntaxError::EmptyName | SyntaxError::MissingName,
                ..
            })
        ));
    }
}

#[test]
fn test_attack_t23_t24_token_rules() {
    use form_net::{Policy, TokenField};
    let policy = Policy::DEFAULT.with_token_field(TokenField::new("csrf_token"));
    let late = b"--X\r\nContent-Disposition: form-data; name=f; filename=a\r\n\r\n1\r\n--X\r\nContent-Disposition: form-data; name=csrf_token\r\n\r\nt\r\n--X--";
    assert!(matches!(
        form_net::parse_multipart_with(CT, late, &Limits::DEFAULT, &policy),
        Err(FormError::Policy {
            reason: PolicyRefusal::TokenFieldLate,
            ..
        })
    ));
    let anywhere = Policy::DEFAULT.with_token_field(TokenField::new("csrf_token").anywhere());
    assert!(form_net::parse_multipart_with(CT, late, &Limits::DEFAULT, &anywhere).is_ok());
    let twice = b"--X\r\nContent-Disposition: form-data; name=csrf_token\r\n\r\na\r\n--X\r\nContent-Disposition: form-data; name=csrf_token\r\n\r\nb\r\n--X--";
    assert!(matches!(
        form_net::parse_multipart_with(CT, twice, &Limits::DEFAULT, &policy),
        Err(FormError::Policy {
            reason: PolicyRefusal::DuplicateToken,
            ..
        })
    ));
    let first = b"--X\r\nContent-Disposition: form-data; name=csrf_token\r\n\r\ntok\r\n--X\r\nContent-Disposition: form-data; name=f; filename=a\r\n\r\n1\r\n--X--";
    let mp = form_net::parse_multipart_with(CT, first, &Limits::DEFAULT, &policy).unwrap();
    assert_eq!(mp.field("csrf_token"), Some("tok"));
}
