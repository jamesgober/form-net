#![no_main]
//! Typed extraction: no panic on any body in either encoding, and a
//! successful extraction agrees with the plain parsers field by field.

use form_net::{FileData, FromForm, Secret};
use libfuzzer_sys::fuzz_target;

form_net::form! {
    pub struct Target {
        pub a: String,
        pub n: Option<u32>,
        pub flag: bool,
        pub pw: Option<Secret>,
        pub many: Vec<i16> = "many[]",
        pub file: Option<FileData>,
    }
    ignore = ["_charset_"];
}

fuzz_target!(|data: &[u8]| {
    let Some((&mode, body)) = data.split_first() else { return };
    if mode & 1 == 0 {
        let typed = Target::from_body(b"application/x-www-form-urlencoded", body);
        if let Ok(t) = typed {
            let form = form_net::parse_urlencoded(body).expect("extraction accepted what parsing refused");
            assert_eq!(form.get("a"), Some(t.a.as_str()));
            assert_eq!(t.many.len(), form.get_all("many[]").count());
            assert!(t.file.is_none());
        }
    } else {
        let typed = Target::from_body(b"multipart/form-data; boundary=B", body);
        if let Ok(t) = typed {
            let mp = form_net::parse_multipart(b"multipart/form-data; boundary=B", body)
                .expect("extraction accepted what parsing refused");
            assert_eq!(mp.field("a"), Some(t.a.as_str()));
            if let Some(f) = &t.file {
                assert_eq!(mp.file("file").map(|p| p.bytes()), Some(f.bytes()));
            }
        }
    }
});
