#![no_main]
//! File name views: no panic, and every view is consistent with the raw
//! bytes.

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if data.iter().any(|&b| b == b'"' || b == b'\\' || b < 0x20 || b == 0x7F) {
        return;
    }
    let mut body = b"--B\r\nContent-Disposition: form-data; name=f; filename=\"".to_vec();
    body.extend_from_slice(data);
    body.extend_from_slice(b"\"\r\n\r\n\r\n--B--");
    let Ok(mp) = form_net::parse_multipart(b"multipart/form-data; boundary=B", &body) else {
        return;
    };
    let name = mp.file("f").expect("a file part").filename().expect("a file name");
    assert_eq!(name.raw(), data);
    assert!(data.ends_with(name.basename()));
    let mut out = vec![0u8; data.len()];
    let into = name.html_unescape_into(&mut out).map(str::to_owned).ok();
    let owned = name.html_unescaped().ok().map(|c| c.into_owned());
    assert_eq!(into, owned);
    if let Ok(s) = name.as_str() {
        assert_eq!(s.as_bytes(), data);
    }
});
