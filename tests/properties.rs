//! Property tests: no panic on any bytes, split invariance on random
//! bodies, budgets that hold, and round trips.

#![cfg(feature = "alloc")]

mod common;

use form_net::{Boundary, Event, FormKind, Limits, MultipartParser, PercentPolicy, Policy};
use proptest::prelude::*;

/// Bytes biased toward the ones the parsers care about.
fn form_bytes() -> impl Strategy<Value = Vec<u8>> {
    prop::collection::vec(
        prop_oneof![
            4 => any::<u8>(),
            3 => prop::sample::select(b"\r\n-X=&%+;\"\\: \tab".to_vec()),
        ],
        0..400,
    )
}

/// Multipart bodies built from random parts, so most of them are valid.
fn multipart_body() -> impl Strategy<Value = Vec<u8>> {
    let part = (
        "[a-z]{1,6}",
        prop::option::of("[a-z.]{0,6}"),
        prop::collection::vec(
            prop_oneof![3 => any::<u8>(), 2 => prop::sample::select(b"\r\n-B".to_vec())],
            0..64,
        ),
    );
    prop::collection::vec(part, 0..5).prop_map(|parts| {
        let mut out = Vec::new();
        for (name, filename, data) in parts {
            let mut head = format!("Content-Disposition: form-data; name=\"{name}\"");
            if let Some(f) = filename {
                head.push_str(&format!("; filename=\"{f}\""));
            }
            out.extend_from_slice(format!("--BB\r\n{head}\r\n\r\n").as_bytes());
            out.extend_from_slice(&data);
            out.extend_from_slice(b"\r\n");
        }
        out.extend_from_slice(b"--BB--\r\n");
        out
    })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    #[test]
    fn prop_urlencoded_never_panics(input in form_bytes()) {
        let _ = form_net::parse_urlencoded(&input);
        let _ = form_net::pairs(&input).count();
        for pair in form_net::raw_pairs(&input, &Limits::DEFAULT) {
            let Ok(pair) = pair else { break };
            let mut out = vec![0u8; input.len()];
            let _ = pair.decode_value(&mut out, PercentPolicy::Literal);
        }
    }

    #[test]
    fn prop_decode_never_grows(input in form_bytes()) {
        let mut out = vec![0u8; input.len()];
        if let Ok(s) = form_net::decode_into(&input, &mut out, PercentPolicy::Literal) {
            prop_assert!(s.len() <= input.len());
        }
    }

    #[test]
    fn prop_detect_never_panics(input in form_bytes()) {
        let _ = FormKind::detect(&input);
    }

    #[test]
    fn prop_multipart_any_bytes_any_split_never_panics(input in form_bytes(), split in 0usize..400) {
        let at = split.min(input.len());
        let _ = common::split_at(b"X", &input, at, &Limits::DEFAULT, &Policy::DEFAULT);
        let _ = form_net::parse_multipart(b"multipart/form-data; boundary=X", &input);
    }

    #[test]
    fn test_attack_t32_split_invariant(body in multipart_body(), a in 0usize..2000, b in 0usize..2000) {
        let expected = common::drive(b"BB", &[&body], &Limits::DEFAULT, &Policy::DEFAULT);
        let (x, y) = (a.min(body.len()), b.min(body.len()));
        let (lo, hi) = (x.min(y), x.max(y));
        let three = common::drive(b"BB", &[&body[..lo], &body[lo..hi], &body[hi..]], &Limits::DEFAULT, &Policy::DEFAULT);
        prop_assert_eq!(&three, &expected);
        let bytewise = common::bytewise(b"BB", &body, &Limits::DEFAULT, &Policy::DEFAULT);
        prop_assert_eq!(&bytewise, &expected);
        if let Some(reference) = common::reference(b"BB", &body) {
            prop_assert_eq!(expected.unwrap(), reference);
        }
    }

    #[test]
    fn prop_multipart_budgets_hold(body in multipart_body(), value in 0u32..64, file in 0u64..64) {
        let limits = Limits::DEFAULT.with_max_value_bytes(value).with_max_file_bytes(file);
        if let Ok(parts) = common::drive(b"BB", &[&body], &limits, &Policy::DEFAULT) {
            for p in parts {
                match p.kind {
                    form_net::PartKind::Field => prop_assert!(p.data.len() as u64 <= u64::from(value)),
                    _ => prop_assert!(p.data.len() as u64 <= file),
                }
            }
        }
    }

    #[test]
    fn prop_serialize_round_trips(pairs in prop::collection::vec((".{0,12}", ".{0,12}"), 0..8)) {
        let s = form_net::serialize_urlencoded(pairs.iter().map(|(a, b)| (a.as_str(), b.as_str())));
        let back = form_net::parse_urlencoded(s.as_bytes()).unwrap();
        let got: Vec<(String, String)> = back.iter().map(|(a, b)| (a.to_owned(), b.to_owned())).collect();
        // Empty sequences are skipped, so a pair of two empty strings
        // comes back as `=`, which is a pair, and stays.
        prop_assert_eq!(got, pairs);
    }

    #[test]
    fn prop_data_events_never_empty(body in multipart_body(), chunk in 1usize..17) {
        let mut parser = MultipartParser::new(Boundary::new(b"BB").unwrap(), &Limits::DEFAULT, &Policy::DEFAULT).unwrap();
        let chunks: Vec<&[u8]> = body.chunks(chunk).collect();
        'outer: for (i, c) in chunks.iter().enumerate() {
            let mut rest: &[u8] = c;
            loop {
                let Ok(step) = parser.parse(rest, i + 1 == chunks.len()) else { break 'outer };
                rest = &rest[step.consumed..];
                match step.event {
                    Event::Data(d) => prop_assert!(!d.is_empty()),
                    Event::NeedInput | Event::End => break,
                    _ => {}
                }
            }
        }
    }
}
