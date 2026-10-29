//! The streaming urlencoded parser (FR-11): the same pairs and the same
//! errors as `raw_pairs` for every split, and refusal before the scratch
//! could overflow.

use form_net::{FormError, Limit, Limits, UrlencodedEvent, UrlencodedParser};

type Seen = (Vec<(Vec<u8>, Vec<u8>, bool, u64)>, Option<FormError>);

fn stream(chunks: &[&[u8]], limits: &Limits) -> Seen {
    let mut scratch = vec![0u8; limits.urlencoded_buffer_len()];
    let mut parser = UrlencodedParser::with_buffer(limits, &mut scratch[..]).unwrap();
    let mut out = Vec::new();
    let empty: [&[u8]; 1] = [&[]];
    let chunks = if chunks.is_empty() {
        &empty[..]
    } else {
        chunks
    };
    for (i, chunk) in chunks.iter().enumerate() {
        let last = i + 1 == chunks.len();
        let mut rest: &[u8] = chunk;
        loop {
            match parser.parse(rest, last) {
                Ok(step) => {
                    assert!(step.consumed <= rest.len());
                    rest = &rest[step.consumed..];
                    match step.event {
                        UrlencodedEvent::Pair(p) => out.push((
                            p.name_raw().to_vec(),
                            p.value_raw().to_vec(),
                            p.has_equals(),
                            p.offset(),
                        )),
                        UrlencodedEvent::NeedInput => {
                            assert!(rest.is_empty() && !last);
                            break;
                        }
                        UrlencodedEvent::End => {
                            assert!(last);
                            return (out, None);
                        }
                        _ => unreachable!(),
                    }
                }
                Err(e) => {
                    assert_eq!(parser.parse(b"", true).unwrap_err(), FormError::Poisoned);
                    return (out, Some(e));
                }
            }
        }
    }
    unreachable!("no End after eof")
}

fn reference(body: &[u8], limits: &Limits) -> Seen {
    let mut out = Vec::new();
    for p in form_net::raw_pairs(body, limits) {
        match p {
            Ok(p) => out.push((
                p.name_raw().to_vec(),
                p.value_raw().to_vec(),
                p.has_equals(),
                p.offset(),
            )),
            Err(e) => return (out, Some(e)),
        }
    }
    (out, None)
}

const BODIES: &[&[u8]] = &[
    b"",
    b"&&&",
    b"a=1",
    b"a=1&b=2&",
    b"=&a&b=&==&c=d=e",
    b"name=Al&city=Paris+France&q=%41%42",
    b"x&&y=1&&",
];

fn check_all_splits(body: &[u8], limits: &Limits, whole_matches_reference: bool) {
    let whole = stream(&[body], limits);
    if whole_matches_reference {
        assert_eq!(
            whole,
            reference(body, limits),
            "{:?}",
            String::from_utf8_lossy(body)
        );
    }
    for split in 0..=body.len() {
        let (a, b) = body.split_at(split);
        assert_eq!(
            stream(&[a, b], limits),
            whole,
            "split {split} of {:?}",
            String::from_utf8_lossy(body)
        );
    }
    let bytes: Vec<&[u8]> = body.chunks(1).collect();
    assert_eq!(stream(&bytes, limits), whole, "bytewise");
}

#[test]
fn test_stream_matches_raw_pairs_at_every_split() {
    for body in BODIES {
        check_all_splits(body, &Limits::DEFAULT, true);
    }
}

#[test]
fn test_stream_limits_are_split_invariant() {
    let tight = Limits::DEFAULT
        .with_max_name_bytes(3)
        .with_max_value_bytes(4)
        .with_max_text_bytes(6)
        .with_max_fields(3);
    let cases: &[&[u8]] = &[
        b"abc=defg&x=1",                                 // at the limits
        b"abcd=1",                                       // name too long
        b"abcd",                                         // name too long, no equals
        b"a=12345",                                      // value too long
        b"a=1234&b=123",                                 // text total
        b"a&b&c&d",                                      // fields
        b"a&b&c&dddd=1",                                 // fields beats name length
        b"a=1&bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb", // name far past the scratch
    ];
    for body in cases {
        check_all_splits(body, &tight, true);
    }
}

#[test]
fn test_stream_body_limit() {
    let l = Limits::DEFAULT.with_max_body_bytes(8);
    // Within the budget: as raw_pairs.
    check_all_splits(b"a=1&b=22", &l, true);
    // Past it: the pairs whose `&` lies inside the budget, then `Body`, for
    // every split (`b=22` ends at byte 8, outside the budget).
    let body = b"a=1&b=22&c=333";
    check_all_splits(body, &l, false);
    let (pairs, err) = stream(&[body], &l);
    assert_eq!(pairs.len(), 1);
    assert!(matches!(err, Some(FormError::Limit { limit: Limit::Body, at }) if at.offset == 8));
}

#[test]
fn test_stream_scratch_never_overflows() {
    // The smallest scratch the limits allow, and sequences that straddle
    // every chunk: no panic, and refusals are limits.
    let l = Limits::DEFAULT
        .with_max_name_bytes(2)
        .with_max_value_bytes(2)
        .with_max_text_bytes(64);
    let mut scratch = vec![0u8; l.urlencoded_buffer_len()];
    assert_eq!(scratch.len(), 5);
    for body in [&b"ab=cd&ef=gh"[..], b"ab=cde", b"abc", b"a=b=c"] {
        let bytes: Vec<&[u8]> = body.chunks(1).collect();
        let (_, err) = stream(&bytes, &l);
        if let Some(e) = err {
            assert!(matches!(e, FormError::Limit { .. }), "{e:?}");
        }
    }
    assert!(UrlencodedParser::with_buffer(&l, &mut scratch[..4]).is_err());
}

#[test]
fn test_stream_reset_reuses_the_parser() {
    let limits = Limits::DEFAULT.with_max_fields(1);
    let mut scratch = vec![0u8; limits.urlencoded_buffer_len()];
    let mut parser = UrlencodedParser::with_buffer(&limits, &mut scratch[..]).unwrap();
    assert!(matches!(
        parser.parse(b"a=1", false).unwrap().event,
        UrlencodedEvent::NeedInput
    ));
    let step = parser.parse(b"&b=2", true).unwrap();
    assert_eq!(step.consumed, 1, "a=1 completes from the scratch");
    assert!(parser.parse(b"b=2", true).is_err(), "a second field");
    assert_eq!(parser.parse(b"", true).unwrap_err(), FormError::Poisoned);
    parser.reset();
    let step = parser.parse(b"z=9", true).unwrap();
    assert!(
        matches!(step.event, UrlencodedEvent::Pair(p) if p.offset() == 0 && p.value_raw() == b"9")
    );
}

mod props {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(256))]
        #[test]
        fn prop_stream_is_split_invariant(
            body in proptest::collection::vec(prop_oneof![Just(b'&'), Just(b'='), Just(b'%'), Just(b'+'), b'a'..=b'c'], 0..40),
            cuts in proptest::collection::vec(0usize..40, 0..6),
            name in 0u32..5, value in 0u32..5, fields in 0u32..6,
        ) {
            let l = Limits::DEFAULT.with_max_name_bytes(name).with_max_value_bytes(value).with_max_fields(fields);
            let mut cuts: Vec<usize> = cuts.into_iter().map(|c| c.min(body.len())).collect();
            cuts.sort_unstable();
            let mut chunks = Vec::new();
            let mut last = 0;
            for c in cuts { chunks.push(&body[last..c]); last = c; }
            chunks.push(&body[last..]);
            prop_assert_eq!(stream(&chunks, &l), reference(&body, &l));
        }
    }
}

#[test]
fn test_scratch_is_wiped_where_it_was_used() {
    // D-103: only the held bytes are wiped, and every held byte is.
    let limits = Limits::DEFAULT
        .with_max_name_bytes(16)
        .with_max_value_bytes(64);
    let mut scratch = vec![0u8; limits.urlencoded_buffer_len()];
    {
        let body = b"password=hunter2hunter2&token=abcdefghijklmnopqrstuvwxyz";
        let bytes: Vec<&[u8]> = body.chunks(3).collect();
        let mut parser = UrlencodedParser::with_buffer(&limits, &mut scratch[..]).unwrap();
        for (i, c) in bytes.iter().enumerate() {
            let last = i + 1 == bytes.len();
            let mut rest: &[u8] = c;
            loop {
                let step = parser.parse(rest, last).unwrap();
                rest = &rest[step.consumed..];
                if matches!(
                    step.event,
                    UrlencodedEvent::NeedInput | UrlencodedEvent::End
                ) {
                    break;
                }
            }
        }
    }
    assert!(
        scratch.iter().all(|&b| b == 0),
        "held bytes left in the scratch"
    );
}
