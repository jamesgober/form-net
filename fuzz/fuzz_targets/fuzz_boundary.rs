#![no_main]
//! Boundary validation: no panic; a valid boundary round-trips through a
//! Content-Type value.

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(b) = form_net::Boundary::new(data) {
        assert_eq!(b.as_bytes(), data);
        assert_eq!(b.delimiter().len(), data.len() + 4);
        let mut ct = br#"multipart/form-data; boundary=""#.to_vec();
        ct.extend_from_slice(data);
        ct.push(b'"');
        assert_eq!(form_net::Boundary::parse(&ct), Ok(b));
    }
});
