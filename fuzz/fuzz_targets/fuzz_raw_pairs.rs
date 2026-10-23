#![no_main]
//! urlencoded scanning: no panic; the scanner and the owned parser agree;
//! budgets hold.

use form_net::{Limits, PercentPolicy, Policy};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let limits = Limits::DEFAULT
        .with_max_fields(64)
        .with_max_name_bytes(32)
        .with_max_value_bytes(128)
        .with_max_text_bytes(512);
    let mut raw = Vec::new();
    let mut raw_err = None;
    for pair in form_net::raw_pairs(data, &limits) {
        match pair {
            Ok(p) => {
                assert!(p.name_raw().len() <= 32 && p.value_raw().len() <= 128);
                let mut n = vec![0u8; p.name_raw().len()];
                let mut v = vec![0u8; p.value_raw().len()];
                let name = p.decode_name(&mut n, PercentPolicy::Refuse).map(str::to_owned);
                let value = p.decode_value(&mut v, PercentPolicy::Refuse).map(str::to_owned);
                match (name, value) {
                    (Ok(a), Ok(b)) => raw.push((a, b)),
                    (Err(e), _) | (_, Err(e)) => {
                        raw_err = Some(e);
                        break;
                    }
                }
            }
            Err(e) => {
                raw_err = Some(e);
                break;
            }
        }
    }
    match form_net::parse_urlencoded_with(data, &limits, &Policy::DEFAULT) {
        Ok(form) => {
            assert!(raw_err.is_none());
            let owned: Vec<(String, String)> =
                form.iter().map(|(a, b)| (a.to_owned(), b.to_owned())).collect();
            assert_eq!(owned, raw);
            let _ = format!("{form:?}");
        }
        Err(e) => assert_eq!(Some(e), raw_err),
    }
});
