#![no_main]
//! The streaming urlencoded parser: no panic, the same pairs and errors in
//! the chunk size the first byte picks as whole, and the same as
//! `raw_pairs` whenever the body fits its budget.

use form_net::{FormError, Limits, UrlencodedEvent, UrlencodedParser};
use libfuzzer_sys::fuzz_target;

#[path = "common.rs"]
mod common;

type Seen = (Vec<(Vec<u8>, Vec<u8>, bool, u64)>, Option<FormError>);

fn stream(chunks: &[&[u8]], limits: &Limits) -> Seen {
    let mut parser = UrlencodedParser::new(limits).expect("valid limits");
    let mut out = Vec::new();
    let empty: [&[u8]; 1] = [&[]];
    let chunks = if chunks.is_empty() { &empty[..] } else { chunks };
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
                        UrlencodedEvent::End => return (out, None),
                        _ => unreachable!(),
                    }
                }
                Err(e) => return (out, Some(e)),
            }
        }
    }
    panic!("no End after eof");
}

fuzz_target!(|data: &[u8]| {
    let Some((&knobs, body)) = data.split_first() else { return };
    let limits = Limits::DEFAULT
        .with_max_name_bytes(u32::from(knobs & 7))
        .with_max_value_bytes(u32::from(knobs >> 3 & 15))
        .with_max_fields(u32::from(knobs >> 5) + 1)
        .with_max_body_bytes(512);
    let (_, chunks) = common::chunks(data);
    let whole = stream(&[body], &limits);
    assert_eq!(stream(&chunks, &limits), whole);
    if body.len() < 256 {
        let bytes: Vec<&[u8]> = body.chunks(1).collect();
        assert_eq!(stream(&bytes, &limits), whole);
    }
    if body.len() <= 512 {
        let mut reference = Vec::new();
        let mut err = None;
        for p in form_net::raw_pairs(body, &limits) {
            match p {
                Ok(p) => reference.push((p.name_raw().to_vec(), p.value_raw().to_vec(), p.has_equals(), p.offset())),
                Err(e) => {
                    err = Some(e);
                    break;
                }
            }
        }
        assert_eq!(whole, (reference, err));
    }
});
