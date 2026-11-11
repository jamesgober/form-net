//! The 0.9.x adversarial review of the multipart core (parser, headers,
//! boundary, kind detection): one test per finding. Each test failed on
//! 0.9.0; the comment on it describes the defect as found, and the fix is in
//! the decision log (D-136, D-142, D-145).

#![cfg(feature = "alloc")]

mod common;

use std::io::Read;

use common::drive;
use form_net::{FormError, FormKind, Limits, MultipartReader, Policy, parse_multipart};

const CT: &[u8] = b"multipart/form-data; boundary=X";

fn one_part(disposition: &str) -> Vec<u8> {
    format!("--X\r\nContent-Disposition: {disposition}\r\n\r\nadmin\r\n--X--\r\n").into_bytes()
}

/// What form-net read: `(name, filename)` per part, or the error.
fn read(ct: &[u8], body: &[u8]) -> Result<Vec<(String, Option<String>)>, FormError> {
    parse_multipart(ct, body).map(|mp| {
        mp.parts()
            .iter()
            .map(|p| {
                (
                    p.name().to_owned(),
                    p.filename()
                        .map(|f| String::from_utf8_lossy(f.raw()).into_owned()),
                )
            })
            .collect()
    })
}

/// High. `boundary*` (RFC 8187 / RFC 2231) beside `boundary` is accepted and
/// ignored. Go's `mime.ParseMediaType` (used by `net/http`'s
/// `ParseMultipartForm` and Go WAFs) lets the starred value win, so the two
/// readers frame the body with different boundaries. D-081 refuses starred
/// parameters in a part's Content-Disposition for this reason; the request
/// Content-Type was left out.
#[test]
fn test_starred_boundary_parameter_is_accepted_and_ignored() {
    for ct in [
        &b"multipart/form-data; boundary=X; boundary*=UTF-8''Y"[..],
        b"multipart/form-data; boundary*=UTF-8''Y; boundary=X",
        b"multipart/form-data; boundary=X; boundary*0=Y",
    ] {
        assert!(
            FormKind::detect(ct).is_err(),
            "{}: accepted with boundary X; Go frames this body with boundary Y",
            String::from_utf8_lossy(ct)
        );
    }
    // The smuggle: Go 1.27 net/http reads one field `note` holding the
    // whole inner form as text; form-net reads `role=admin`.
    let body = b"--Y\r\nContent-Disposition: form-data; name=\"note\"\r\n\r\nhello\r\n\
--X\r\nContent-Disposition: form-data; name=\"role\"\r\n\r\nadmin\r\n--X--\r\n--Y--\r\n";
    let got = read(b"multipart/form-data; boundary=X; boundary*=UTF-8''Y", body);
    assert!(got.is_err(), "smuggled form read as {got:?}");
}

/// Medium. An empty parameter (`;;`, `; ;`) in a part's Content-Disposition
/// is accepted. RFC 6266's grammar has no empty parameter; Go's
/// `mime.ParseMediaType` refuses the whole list, so `Part::FormName` is ""
/// and `net/http`'s `ReadForm` skips the part. A field, or a file, that
/// form-net delivers is invisible to a Go reader in front.
#[test]
fn test_empty_disposition_parameter_hides_part_from_go_readers() {
    for disposition in [
        "form-data;; name=\"role\"",
        "form-data; name=\"role\";;",
        "form-data; name=\"avatar\"; ; filename=\"shell.php\"",
    ] {
        let got = read(CT, &one_part(disposition));
        assert!(
            got.is_err(),
            "{disposition}: read as {got:?}; Go reads no name and skips the part"
        );
    }
}

/// Medium (same root, request header). `multipart/form-data;; boundary=X` is
/// accepted; Go refuses it as not multipart (`mime: invalid media
/// parameter`), so a Go reader in front does not parse the body at all.
#[test]
fn test_empty_content_type_parameter_accepted() {
    let ct = b"multipart/form-data;; boundary=X";
    assert!(
        FormKind::detect(ct).is_err(),
        "accepted: {:?}",
        FormKind::detect(ct)
    );
}

/// Low. A parameter value that contains `name=` or `filename=` after `;` or
/// a space is accepted. multer 3.1.0 (axum's reader, and the reader of
/// form-net's own differential, `benches/src/bin/differential.rs`) finds
/// those by substring search and reads another name, or a file where
/// form-net reads a field. Checked by running multer 3.1.0 on each body:
/// `evil`; file `safe.jpg`; name `role`; file `evil.php`. SECURITY.md says
/// no accepted body is read differently by the two.
#[test]
fn test_parameter_value_holding_name_or_filename_reads_differently_in_multer() {
    for (disposition, form_net_reads) in [
        ("form-data; x=\"y;name=evil;\"; name=\"good\"", "field good"),
        (
            "form-data; name=\"f\"; x=\"y;filename=safe.jpg;\"; filename=\"shell.php\"",
            "file shell.php",
        ),
        (
            "form-data; filename=\"x; name=role;\"; name=\"avatar\"",
            "file in avatar",
        ),
        ("form-data; name=\"x;filename=evil.php;\"", "a field"),
    ] {
        let got = read(CT, &one_part(disposition));
        assert!(
            got.is_err(),
            "{disposition}: form-net reads {form_net_reads} ({got:?}), multer another"
        );
    }
}

/// Low. The error depends on the split when the input passes
/// `max_body_bytes`: `run` refuses the whole input before parsing it
/// (parser.rs:860), so the whole body reports `Limit { Body }` while a split
/// at the end of the header block reports the header's syntax error. D-080:
/// the same body must not fail differently on different networks.
#[test]
fn test_body_limit_error_depends_on_split() {
    let limits = Limits::DEFAULT.with_max_body_bytes(60);
    let mut body = b"--X\r\nbad\r\n\r\n".to_vec();
    body.extend_from_slice(&[b'a'; 100]);
    let whole = drive(b"X", &[&body], &limits, &Policy::DEFAULT);
    let (a, b) = body.split_at(12);
    let split = drive(b"X", &[a, b], &limits, &Policy::DEFAULT);
    assert_eq!(whole, split);
}

/// Low. The same: the delimiter search runs over the whole chunk before the
/// data before a match is charged to its budgets, so a framing refusal found
/// ahead (`Truncated` at eof, a bare break) wins over the budget that a split
/// input meets first (`ValueLength` here).
#[test]
fn test_framing_error_found_ahead_beats_budget_depending_on_split() {
    let limits = Limits::DEFAULT.with_max_value_bytes(4);
    let head = b"--X\r\nContent-Disposition: form-data; name=a\r\n\r\n";
    for tail in [&b"0123456789"[..], b"0123456789\n--X--"] {
        let mut body = head.to_vec();
        body.extend_from_slice(tail);
        let whole = drive(b"X", &[&body], &limits, &Policy::DEFAULT);
        let (a, b) = body.split_at(head.len() + 6);
        let split = drive(b"X", &[a, b], &limits, &Policy::DEFAULT);
        assert_eq!(whole, split, "{:?}", String::from_utf8_lossy(&body));
    }
}

/// Low, where the parser hands off to `MultipartReader` (src/io.rs): the
/// reader stops at `End` and never feeds the epilogue, so
/// `max_epilogue_bytes` (FR-21) is not enforced; `parse_multipart` refuses
/// the same body.
#[test]
fn test_multipart_reader_skips_epilogue_budget() {
    let mut body = b"--X\r\nContent-Disposition: form-data; name=a\r\n\r\n1\r\n--X--".to_vec();
    body.extend_from_slice(&[b'e'; 4096]);
    assert!(parse_multipart(CT, &body).is_err(), "buffered refuses");
    let mut reader = MultipartReader::new(CT, &body[..]).unwrap();
    let result = (|| -> Result<(), FormError> {
        while let Some(mut part) = reader.next_part()? {
            let mut sink = Vec::new();
            let _ = part.read_to_end(&mut sink);
        }
        Ok(())
    })();
    assert!(
        result.is_err(),
        "MultipartReader accepted a 4 KiB epilogue against a 1 KiB budget"
    );
}

/// Reported by the review, decided after it (D-147): under
/// `ExtendedFilenamePolicy::Ignore` a part with only `filename*` was a text
/// field, where an RFC 2231 reader (Go's `mime/multipart`, an inspection
/// layer) sees a file. 0.9.2 refused it; 0.10 removed `Ignore`, which then
/// did what `Refuse` does (D-148). Under the default the part is refused,
/// and a plain `filename` still makes a file.
#[test]
fn test_a_part_with_only_extended_filename_is_refused_by_default() {
    let policy = Policy::DEFAULT;
    let only_ext =
        b"--X\r\nContent-Disposition: form-data; name=\"f\"; filename*=UTF-8''a.php\r\n\r\nx\r\n--X--";
    let got = form_net::parse_multipart_with(CT, only_ext, &Limits::DEFAULT, &policy);
    assert_eq!(
        got.map(|mp| mp.parts().len()).map_err(|e| e.code()),
        Err("policy.extended_filename"),
        "a file to RFC 2231 readers, a field to form-net"
    );
    let plain =
        b"--X\r\nContent-Disposition: form-data; name=\"f\"; filename=\"a.png\"\r\n\r\nx\r\n--X--";
    let mp = form_net::parse_multipart_with(CT, plain, &Limits::DEFAULT, &policy).unwrap();
    assert!(mp.parts()[0].is_file());
}
