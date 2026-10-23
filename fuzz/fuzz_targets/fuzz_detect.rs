#![no_main]
//! Content-Type detection: no panic; a multipart result always carries a
//! valid boundary.

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    match form_net::FormKind::detect(data) {
        Ok(form_net::FormKind::Multipart(b)) => {
            assert!(!b.as_bytes().is_empty() && b.as_bytes().len() <= 70);
            assert_eq!(form_net::Boundary::new(b.as_bytes()), Ok(b));
        }
        Ok(_) => {}
        Err(e) => {
            let _ = (e.code(), e.status_hint(), e.to_string());
        }
    }
});
