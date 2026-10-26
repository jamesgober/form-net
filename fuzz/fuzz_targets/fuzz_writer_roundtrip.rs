#![no_main]
//! The writer: whatever names, file names, content types, and data it is
//! given, it either refuses or writes a body that form-net parses back to
//! the same fields (FR-66).

use form_net::{Boundary, FormError, MultipartWriter};
use libfuzzer_sys::fuzz_target;

/// Splits fuzz input into short strings at 0xFF bytes.
fn pieces(data: &[u8]) -> Vec<&[u8]> {
    data.split(|&b| b == 0xFF).collect()
}

fn normalized(v: &str) -> String {
    v.replace("\r\n", "\n").replace('\r', "\n").replace('\n', "\r\n")
}

fuzz_target!(|data: &[u8]| {
    let p = pieces(data);
    let mut body = Vec::new();
    let mut w = MultipartWriter::with_boundary(&mut body, Boundary::new(b"B").unwrap());
    let mut expected: Vec<(String, Option<String>, Vec<u8>)> = Vec::new();
    let mut ok = true;
    for group in p.chunks(4) {
        let text = |i: usize| group.get(i).and_then(|b| std::str::from_utf8(b).ok());
        let (Some(name), Some(second)) = (text(0), group.get(1)) else { break };
        let r = match text(2) {
            Some(filename) if group.len() == 4 => {
                let ct = text(3).filter(|c| !c.is_empty());
                let r = w.file(name, filename, ct, &second[..]).map(|_| ());
                expected.push((name.to_owned(), Some(filename.to_owned()), second.to_vec()));
                r
            }
            _ => match std::str::from_utf8(second) {
                Ok(value) => {
                    let r = w.text(name, value);
                    expected.push((name.to_owned(), None, normalized(value).into_bytes()));
                    r
                }
                Err(_) => break,
            },
        };
        if let Err(e) = r {
            assert!(matches!(e, FormError::Syntax { .. } | FormError::Encoding { .. }), "{e:?}");
            ok = false;
            break;
        }
    }
    if !ok {
        return;
    }
    let ct = w.content_type();
    w.finish().unwrap();
    // Limits that admit every size the input can produce: escaping triples
    // CR, LF, and quotes, and the parser counts the escaped bytes.
    let limits = form_net::Limits::DEFAULT
        .with_max_part_header_bytes(64 * 1024)
        .with_max_name_bytes(64 * 1024)
        .with_max_filename_bytes(64 * 1024)
        .with_max_parts(10_000)
        .with_max_fields(10_000)
        .with_max_files(10_000);
    let mp = form_net::parse_multipart_with(ct.as_bytes(), &body, &limits, &form_net::Policy::DEFAULT)
        .unwrap_or_else(|e| panic!("refused its own body: {e:?}"));
    assert_eq!(mp.len(), expected.len());
    for (got, (name, filename, data)) in mp.parts().iter().zip(&expected) {
        assert_eq!(got.name(), name);
        match filename {
            Some(f) => assert_eq!(got.filename().unwrap().html_unescaped().unwrap().as_ref(), f.as_str()),
            None => assert!(got.filename().is_none()),
        }
        assert_eq!(got.bytes(), &data[..]);
    }
});
