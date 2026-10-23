#![no_main]
//! The upload session: no panic, the same result in any chunking as the
//! buffered parser, files spilled past a small threshold read back intact,
//! and nothing is left on disk once the upload is dropped.

use std::sync::OnceLock;

use form_net::{Limits, NoInspector, Policy, SpillMode, Store, UploadSession};
use libfuzzer_sys::fuzz_target;

#[path = "common.rs"]
mod common;

const CT: &[u8] = b"multipart/form-data; boundary=B";

fn store() -> &'static Store {
    static STORE: OnceLock<Store> = OnceLock::new();
    STORE.get_or_init(|| {
        Store::temp()
            .expect("temporary directory")
            .memory_threshold(16)
            .mode(SpillMode::Named)
    })
}

fuzz_target!(|data: &[u8]| {
    let (_, chunks) = common::chunks(data);
    let body = data.get(1..).unwrap_or(&[]);
    let limits = Limits::DEFAULT.with_max_parts(16).with_max_part_header_bytes(256);
    let store = store();
    let session = (|| {
        let mut s = UploadSession::new(CT, &limits, &Policy::DEFAULT, store)?;
        for c in &chunks {
            s.push(c, &mut NoInspector)?;
        }
        s.finish(&mut NoInspector)
    })();
    let buffered = form_net::parse_multipart_with(CT, body, &limits, &Policy::DEFAULT);
    match (&session, &buffered) {
        (Ok(up), Ok(mp)) => {
            let fields: Vec<(&str, &str)> = up.iter().collect();
            let (files, texts): (Vec<_>, Vec<_>) =
                mp.parts().iter().partition(|p| p.filename().is_some());
            let expected: Vec<(&str, &str)> = texts
                .iter()
                .map(|p| (p.name(), p.text().unwrap_or_default()))
                .collect();
            assert_eq!(fields, expected);
            assert_eq!(up.file_count(), files.len());
        }
        (Err(_), Err(_)) => {}
        // The buffered parser checks text fields as text; the session
        // keeps them as text too, so both refuse the same bodies.
        (s, b) => panic!("session {:?} and buffered {:?} disagree", s.as_ref().err(), b.as_ref().err()),
    }
    if let Ok(up) = session {
        for f in up.into_files() {
            let bytes = f.read_to_vec(u64::MAX).expect("read back");
            assert_eq!(bytes.len() as u64, f.len());
        }
    }
    assert_eq!(store.stats().files_on_disk, 0, "a dropped upload left files");
});
