//! Every limit at n and n + 1, and 0 (FR-37, FR-38), whole and split.

#![cfg(feature = "alloc")]

mod common;

use common::{body, drive};
use form_net::{FormError, Limit, Limits, Policy, TokenField};

fn limit_of(r: Result<Vec<common::Record>, FormError>) -> Option<Limit> {
    match r {
        Err(FormError::Limit { limit, .. }) => Some(limit),
        Ok(_) => None,
        Err(other) => panic!("expected a limit error, got {other:?}"),
    }
}

/// The limit hit by `b` under `limits`, checked whole, bytewise, and at
/// every split.
fn hit(b: &[u8], limits: &Limits, policy: &Policy) -> Option<Limit> {
    let whole = limit_of(drive(b"X", &[b], limits, policy));
    common::assert_split_invariant(b"X", b, limits, policy);
    whole
}

fn field(name: &str, data: &[u8]) -> (String, Vec<u8>) {
    (
        format!("Content-Disposition: form-data; name=\"{name}\""),
        data.to_vec(),
    )
}

fn file(name: &str, data: &[u8]) -> (String, Vec<u8>) {
    (
        format!("Content-Disposition: form-data; name=\"{name}\"; filename=\"f\""),
        data.to_vec(),
    )
}

fn build(parts: &[(String, Vec<u8>)]) -> Vec<u8> {
    let refs: Vec<(&str, &[u8])> = parts
        .iter()
        .map(|(h, d)| (h.as_str(), d.as_slice()))
        .collect();
    body("X", &refs)
}

const P: Policy = Policy::DEFAULT;

#[test]
fn test_max_parts() {
    let b = build(&[field("a", b"1"), field("b", b"2")]);
    assert_eq!(hit(&b, &Limits::DEFAULT.with_max_parts(2), &P), None);
    assert_eq!(
        hit(&b, &Limits::DEFAULT.with_max_parts(1), &P),
        Some(Limit::Parts)
    );
    assert_eq!(
        hit(&b, &Limits::DEFAULT.with_max_parts(0), &P),
        Some(Limit::Parts)
    );
}

#[test]
fn test_max_fields_and_files() {
    let b = build(&[field("a", b"1"), file("f", b"2"), field("b", b"3")]);
    assert_eq!(hit(&b, &Limits::DEFAULT.with_max_fields(2), &P), None);
    assert_eq!(
        hit(&b, &Limits::DEFAULT.with_max_fields(1), &P),
        Some(Limit::Fields)
    );
    assert_eq!(hit(&b, &Limits::DEFAULT.with_max_files(1), &P), None);
    assert_eq!(
        hit(&b, &Limits::DEFAULT.with_max_files(0), &P),
        Some(Limit::Files)
    );
}

#[test]
fn test_value_text_and_file_sizes() {
    let b = build(&[
        field("a", b"12345"),
        field("b", b"678"),
        file("f", b"abcdef"),
    ]);
    assert_eq!(hit(&b, &Limits::DEFAULT.with_max_value_bytes(5), &P), None);
    assert_eq!(
        hit(&b, &Limits::DEFAULT.with_max_value_bytes(4), &P),
        Some(Limit::ValueLength)
    );
    assert_eq!(hit(&b, &Limits::DEFAULT.with_max_text_bytes(8), &P), None);
    assert_eq!(
        hit(&b, &Limits::DEFAULT.with_max_text_bytes(7), &P),
        Some(Limit::TextTotal)
    );
    assert_eq!(hit(&b, &Limits::DEFAULT.with_max_file_bytes(6), &P), None);
    assert_eq!(
        hit(&b, &Limits::DEFAULT.with_max_file_bytes(5), &P),
        Some(Limit::FileSize)
    );
}

#[test]
fn test_body_limit_counts_every_byte() {
    let b = build(&[field("a", b"1")]);
    let n = b.len() as u64;
    assert_eq!(hit(&b, &Limits::DEFAULT.with_max_body_bytes(n), &P), None);
    assert_eq!(
        hit(&b, &Limits::DEFAULT.with_max_body_bytes(n - 1), &P),
        Some(Limit::Body)
    );
}

#[test]
fn test_name_and_filename_lengths() {
    let b = build(&[field("abcd", b"1")]);
    assert_eq!(hit(&b, &Limits::DEFAULT.with_max_name_bytes(4), &P), None);
    assert_eq!(
        hit(&b, &Limits::DEFAULT.with_max_name_bytes(3), &P),
        Some(Limit::NameLength)
    );
    let b = body(
        "X",
        &[(
            "Content-Disposition: form-data; name=a; filename=\"abc\"",
            b"",
        )],
    );
    assert_eq!(
        hit(&b, &Limits::DEFAULT.with_max_filename_bytes(3), &P),
        None
    );
    assert_eq!(
        hit(&b, &Limits::DEFAULT.with_max_filename_bytes(2), &P),
        Some(Limit::FilenameLength)
    );
}

#[test]
fn test_header_bytes_and_count() {
    let headers =
        "Content-Disposition: form-data; name=a\r\nX-One: 1111111111\r\nX-Two: 2222222222";
    let b = body("X", &[(headers, b"")]);
    // The block is the header lines plus the final CRLF CRLF.
    let block = headers.len() as u32 + 4;
    assert_eq!(
        hit(&b, &Limits::DEFAULT.with_max_part_header_bytes(block), &P),
        None
    );
    assert_eq!(
        hit(
            &b,
            &Limits::DEFAULT.with_max_part_header_bytes(block - 1),
            &P
        ),
        Some(Limit::PartHeaderBytes)
    );
    assert_eq!(hit(&b, &Limits::DEFAULT.with_max_part_headers(3), &P), None);
    assert_eq!(
        hit(&b, &Limits::DEFAULT.with_max_part_headers(2), &P),
        Some(Limit::PartHeaderCount)
    );
}

#[test]
fn test_preamble_epilogue_padding() {
    let mut b = b"12345\r\n".to_vec();
    b.extend(build(&[field("a", b"1")]));
    assert_eq!(
        hit(&b, &Limits::DEFAULT.with_max_preamble_bytes(5), &P),
        None
    );
    assert_eq!(
        hit(&b, &Limits::DEFAULT.with_max_preamble_bytes(4), &P),
        Some(Limit::Preamble)
    );
    let mut b = build(&[field("a", b"1")]);
    b.extend_from_slice(b"abc");
    // "--X--" is followed by "\r\n" and "abc": five epilogue bytes.
    assert_eq!(
        hit(&b, &Limits::DEFAULT.with_max_epilogue_bytes(5), &P),
        None
    );
    assert_eq!(
        hit(&b, &Limits::DEFAULT.with_max_epilogue_bytes(4), &P),
        Some(Limit::Epilogue)
    );
    let b = b"--X   \r\nContent-Disposition: form-data; name=a\r\n\r\n1\r\n--X--".to_vec();
    assert_eq!(
        hit(&b, &Limits::DEFAULT.with_max_padding_bytes(3), &P),
        None
    );
    assert_eq!(
        hit(&b, &Limits::DEFAULT.with_max_padding_bytes(2), &P),
        Some(Limit::Padding)
    );
    let one = b"--X \r\nContent-Disposition: form-data; name=a\r\n\r\n1\r\n--X--".to_vec();
    assert_eq!(
        hit(&one, &Limits::DEFAULT.with_max_padding_bytes(1), &P),
        None
    );
    assert_eq!(
        hit(&one, &Limits::DEFAULT.with_max_padding_bytes(0), &P),
        Some(Limit::Padding)
    );
}

#[test]
fn test_empty_file_inputs_skipped_do_not_count_against_max_files() {
    use form_net::EmptyFilePolicy;
    let mut parts = Vec::new();
    for _ in 0..50 {
        parts.push((
            String::from("Content-Disposition: form-data; name=f; filename=\"\""),
            Vec::new(),
        ));
    }
    parts.push(file("g", b"real"));
    let b = build(&parts);
    let skip = Policy::DEFAULT.with_empty_files(EmptyFilePolicy::Skip);
    let l = Limits::DEFAULT.with_max_files(1);
    assert_eq!(limit_of(drive(b"X", &[&b], &l, &skip)), None);
    let mp =
        form_net::parse_multipart_with(b"multipart/form-data; boundary=X", &b, &l, &skip).unwrap();
    assert_eq!(mp.len(), 1);
    assert_eq!(mp.file("g").map(|p| p.bytes()), Some(&b"real"[..]));
    // Under the default Report policy they are files, and they count.
    assert_eq!(limit_of(drive(b"X", &[&b], &l, &P)), Some(Limit::Files));
}

#[test]
fn test_token_length() {
    let policy = Policy::DEFAULT.with_token_field(TokenField::new("t"));
    let b = build(&[field("t", b"abcd")]);
    assert_eq!(
        hit(&b, &Limits::DEFAULT.with_max_token_bytes(4), &policy),
        None
    );
    assert_eq!(
        hit(&b, &Limits::DEFAULT.with_max_token_bytes(3), &policy),
        Some(Limit::TokenLength)
    );
}

#[test]
fn test_urlencoded_limits_at_the_edge() {
    let ok = |l: &Limits, input: &[u8]| form_net::parse_urlencoded_with(input, l, &P).is_ok();
    let lim = |l: &Limits, input: &[u8]| match form_net::parse_urlencoded_with(input, l, &P) {
        Err(FormError::Limit { limit, .. }) => Some(limit),
        _ => None,
    };
    let input = b"ab=cd&e=f";
    assert!(ok(&Limits::DEFAULT.with_max_fields(2), input));
    assert_eq!(
        lim(&Limits::DEFAULT.with_max_fields(1), input),
        Some(Limit::Fields)
    );
    assert_eq!(
        lim(&Limits::DEFAULT.with_max_fields(0), b"a"),
        Some(Limit::Fields)
    );
    assert!(ok(&Limits::DEFAULT.with_max_name_bytes(2), input));
    assert_eq!(
        lim(&Limits::DEFAULT.with_max_name_bytes(1), input),
        Some(Limit::NameLength)
    );
    assert!(ok(&Limits::DEFAULT.with_max_value_bytes(2), input));
    assert_eq!(
        lim(&Limits::DEFAULT.with_max_value_bytes(1), input),
        Some(Limit::ValueLength)
    );
    assert!(ok(&Limits::DEFAULT.with_max_text_bytes(3), input));
    assert_eq!(
        lim(&Limits::DEFAULT.with_max_text_bytes(2), input),
        Some(Limit::TextTotal)
    );
    assert!(ok(&Limits::DEFAULT.with_max_body_bytes(9), input));
    assert_eq!(
        lim(&Limits::DEFAULT.with_max_body_bytes(8), input),
        Some(Limit::Body)
    );
    // The budget is on raw length, before decoding.
    assert_eq!(
        lim(&Limits::DEFAULT.with_max_value_bytes(2), b"a=%41"),
        Some(Limit::ValueLength)
    );
}
