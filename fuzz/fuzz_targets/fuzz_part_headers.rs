#![no_main]
//! Part header blocks: no panic; the result does not depend on the split;
//! an accepted head has a non-empty name and the part keeps its data.

use form_net::{ExtendedFilenamePolicy, HeaderPolicy, Limits, Policy};
use libfuzzer_sys::fuzz_target;

#[path = "common.rs"]
mod common;

fuzz_target!(|data: &[u8]| {
    let mut body = b"--B\r\n".to_vec();
    body.extend_from_slice(data);
    body.extend_from_slice(b"\r\n\r\nvalue\r\n--B--");
    let strict = Policy::DEFAULT
        .with_extended_filename(ExtendedFilenamePolicy::AcceptAlone)
        .with_unknown_headers(HeaderPolicy::Refuse);
    for policy in [Policy::DEFAULT, strict] {
        let whole = common::drive(b"B", &[&body], &Limits::DEFAULT, &policy);
        let (a, b) = body.split_at(body.len() / 2);
        assert_eq!(whole, common::drive(b"B", &[a, b], &Limits::DEFAULT, &policy));
        if let Ok(parts) = whole {
            for p in parts {
                assert!(!p.name.is_empty());
            }
        }
    }
});
