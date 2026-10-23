#![no_main]
//! The streaming parser against a whole-body reference parser: for every
//! body the reference understands, both give the same parts, at any split.

use form_net::{Limits, Policy};
use libfuzzer_sys::fuzz_target;

#[path = "common.rs"]
mod common;

fuzz_target!(|data: &[u8]| {
    let (_, chunks) = common::chunks(data);
    let body = data.get(1..).unwrap_or(&[]);
    if let Some(expected) = common::reference(b"B", body) {
        let got = common::drive(b"B", &chunks, &Limits::DEFAULT, &Policy::DEFAULT);
        assert_eq!(got.as_ref().ok(), Some(&expected), "{got:?}");
    }
});
