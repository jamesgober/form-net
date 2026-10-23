#![no_main]
//! The streaming parser: no panic, the event contract holds, and the result
//! is identical whole, in the chunk size the first byte picks, bytewise, and
//! through the buffered entry point.

use form_net::{Limits, Policy};
use libfuzzer_sys::fuzz_target;

#[path = "common.rs"]
mod common;

fuzz_target!(|data: &[u8]| {
    let (_, chunks) = common::chunks(data);
    let body = data.get(1..).unwrap_or(&[]);
    let limits = Limits::DEFAULT.with_max_parts(16).with_max_part_header_bytes(256);
    let whole = common::drive(b"B", &[body], &limits, &Policy::DEFAULT);
    assert_eq!(whole, common::drive(b"B", &chunks, &limits, &Policy::DEFAULT));
    if body.len() < 512 {
        let bytes: Vec<&[u8]> = body.chunks(1).collect();
        assert_eq!(whole, common::drive(b"B", &bytes, &limits, &Policy::DEFAULT));
    }
    let buffered = form_net::parse_multipart_with(
        b"multipart/form-data; boundary=B",
        body,
        &limits,
        &Policy::DEFAULT,
    );
    match (&whole, &buffered) {
        (Ok(parts), Ok(mp)) => {
            assert_eq!(parts.len(), mp.len());
            for (r, p) in parts.iter().zip(mp.parts()) {
                assert_eq!((r.name.as_str(), r.data.as_slice()), (p.name(), p.bytes()));
            }
        }
        // The buffered parser also checks text fields as text.
        (Ok(_), Err(_)) | (Err(_), Err(_)) => {}
        (Err(e), Ok(_)) => panic!("buffered accepted what streaming refused: {e:?}"),
    }
});
