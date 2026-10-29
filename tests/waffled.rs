//! T-14 with the WAFFLED-style generator (`tests/common/waffled.rs`): every
//! mutation of every captured browser and client body is refused or parses
//! to exactly the original fields, whole and in chunks; the mutations real
//! clients produce (case, order, spacing, padding, preamble, epilogue) are
//! accepted.

#![cfg(feature = "alloc")]

mod common;

#[path = "common/waffled.rs"]
mod waffled;

use std::fs;
use std::path::{Path, PathBuf};

use common::{drive, whole};
use form_net::{FormKind, Limits, Policy};
use waffled::{Body, Expect, mutants};

fn corpus() -> Vec<PathBuf> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/corpus");
    let mut dirs = Vec::new();
    for group in ["browsers", "clients"] {
        for entry in fs::read_dir(root.join(group)).unwrap() {
            let p = entry.unwrap().path();
            if p.is_dir() {
                dirs.push(p);
            }
        }
    }
    dirs.sort();
    dirs
}

fn boundary(dir: &Path) -> Vec<u8> {
    let mut ct = fs::read(dir.join("multipart.content-type")).unwrap();
    while ct.last().is_some_and(|b| b.is_ascii_whitespace()) {
        ct.pop();
    }
    match FormKind::detect(&ct).unwrap() {
        FormKind::Multipart(b) => b.as_bytes().to_vec(),
        other => panic!("{other:?}"),
    }
}

#[test]
fn test_attack_t14_waffled_mutations_are_refused_or_read_the_same() {
    let mut failures = Vec::new();
    let mut counts = (0usize, 0usize, 0usize);
    for dir in corpus() {
        let b = boundary(&dir);
        let raw = fs::read(dir.join("multipart.body")).unwrap();
        let body = Body::split(&b, &raw).unwrap_or_else(|| panic!("{}", dir.display()));
        assert_eq!(
            body.render(),
            raw,
            "{}: the splitter must round-trip",
            dir.display()
        );
        let original = whole(&b, &raw).unwrap();
        for m in mutants(&body) {
            let got = whole(&b, &m.body);
            let name = dir.file_name().unwrap().to_string_lossy();
            let at = format!("{name} {} (part {:?})", m.category, m.part);
            match &got {
                Ok(records) if *records == original => counts.0 += 1,
                Ok(records) => failures.push(format!("{at}: read differently: {records:?}")),
                Err(e) if m.expect == Expect::Same => {
                    failures.push(format!("{at}: refused a shape clients send: {e:?}"))
                }
                Err(_) => counts.1 += 1,
            }
            for size in [1, 7, 64] {
                let chunks: Vec<&[u8]> = m.body.chunks(size).collect();
                let split = drive(&b, &chunks, &Limits::DEFAULT, &Policy::DEFAULT);
                if split != got {
                    failures.push(format!(
                        "{at}: chunks of {size} differ: {:?} against {:?}",
                        split.as_ref().err(),
                        got.as_ref().err()
                    ));
                }
            }
            counts.2 += 1;
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
    println!(
        "{} mutants: {} read the same, {} refused",
        counts.2, counts.0, counts.1
    );
    assert!(counts.0 > 0 && counts.1 > 0);
}
