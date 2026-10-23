#![no_main]
//! The component decoder: no panic, never longer than its input, and the
//! two policies agree whenever the strict one accepts.

use form_net::PercentPolicy;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let mut a = vec![0u8; data.len()];
    let mut b = vec![0u8; data.len()];
    let strict = form_net::decode_into(data, &mut a, PercentPolicy::Refuse);
    let literal = form_net::decode_into(data, &mut b, PercentPolicy::Literal);
    if let Ok(s) = strict {
        assert!(s.len() <= data.len());
        assert_eq!(Ok(s), literal);
    }
    if let Ok(s) = literal {
        assert!(s.len() <= data.len());
    }
    if !data.is_empty() {
        let mut short = vec![0u8; data.len() - 1];
        assert!(form_net::decode_into(data, &mut short, PercentPolicy::Literal).is_err());
    }
});
