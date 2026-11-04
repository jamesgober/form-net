//! The multer differential (PLAN 12, T-14): every WAFFLED-style mutant of
//! every captured body (`tests/corpus`) through form-net and multer, as a
//! table by mutation category for the differential log in
//! `docs/SECURITY.md`.
//!
//! "Same" means the mutation did not change that parser's own reading of
//! the original body; "different" means it did. The last column counts the
//! mutants both accepted where the mutation changed one reading and not the
//! other: the shape a reader in front of the other could be bypassed with.
//!
//! `cargo run --release --bin differential`

#[path = "../../../tests/common/waffled.rs"]
mod waffled;

use std::collections::BTreeMap;
use std::convert::Infallible;
use std::path::Path;

use bytes::Bytes;
use form_net::FormKind;

type Record = (String, Option<Vec<u8>>, Vec<u8>);

fn fnet(content_type: &[u8], body: &[u8]) -> Option<Vec<Record>> {
    let mp = form_net::parse_multipart(content_type, body).ok()?;
    Some(
        mp.parts()
            .iter()
            .map(|p| {
                (
                    p.name().to_owned(),
                    p.filename().map(|f| f.raw().to_vec()),
                    p.bytes().to_vec(),
                )
            })
            .collect(),
    )
}

fn multer(boundary: &str, body: &[u8]) -> Option<Vec<Record>> {
    let once = futures_util::stream::iter([Ok::<Bytes, Infallible>(Bytes::copy_from_slice(body))]);
    let mut m = multer::Multipart::new(once, boundary);
    futures_executor::block_on(async {
        let mut out = Vec::new();
        while let Some(mut field) = m.next_field().await.ok()? {
            let name = field.name().unwrap_or_default().to_owned();
            let filename = field.file_name().map(|f| f.as_bytes().to_vec());
            let mut data = Vec::new();
            while let Some(chunk) = field.chunk().await.ok()? {
                data.extend_from_slice(&chunk);
            }
            out.push((name, filename, data));
        }
        Some(out)
    })
}

#[derive(Default)]
struct Row {
    mutants: usize,
    fnet: [usize; 3],
    multer: [usize; 3],
    disagree: usize,
}

fn class(got: &Option<Vec<Record>>, original: &[Record]) -> usize {
    match got {
        Some(r) if r == original => 0,
        None => 1,
        Some(_) => 2,
    }
}

fn main() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../tests/corpus");
    let mut rows: BTreeMap<&'static str, Row> = BTreeMap::new();
    let mut dirs = Vec::new();
    for group in ["browsers", "clients"] {
        for entry in std::fs::read_dir(root.join(group)).unwrap() {
            dirs.push(entry.unwrap().path());
        }
    }
    dirs.sort();
    for dir in dirs {
        let ct = std::fs::read_to_string(dir.join("multipart.content-type")).unwrap();
        let ct = ct.trim();
        let FormKind::Multipart(b) = FormKind::detect(ct.as_bytes()).unwrap() else {
            panic!("{}", dir.display());
        };
        let boundary = std::str::from_utf8(b.as_bytes()).unwrap().to_owned();
        let raw = std::fs::read(dir.join("multipart.body")).unwrap();
        let body = waffled::Body::split(b.as_bytes(), &raw).unwrap();
        let fnet_original = fnet(ct.as_bytes(), &raw).unwrap();
        let multer_original = multer(&boundary, &raw).unwrap();
        for m in waffled::mutants(&body) {
            let f = fnet(ct.as_bytes(), &m.body);
            let u = multer(&boundary, &m.body);
            let row = rows.entry(m.category).or_default();
            row.mutants += 1;
            let (cf, cu) = (class(&f, &fnet_original), class(&u, &multer_original));
            row.fnet[cf] += 1;
            row.multer[cu] += 1;
            // Both accepted, and the mutation changed one reading only.
            if cf != 1 && cu != 1 && cf != cu {
                row.disagree += 1;
            }
        }
    }
    println!(
        "| Mutation | Mutants | form-net same / refused / different | multer same / refused / different | Both accepted, read differently |"
    );
    println!("|---|---|---|---|---|");
    for (category, r) in &rows {
        println!(
            "| {category} | {} | {} / {} / {} | {} / {} / {} | {} |",
            r.mutants,
            r.fnet[0],
            r.fnet[1],
            r.fnet[2],
            r.multer[0],
            r.multer[1],
            r.multer[2],
            r.disagree
        );
    }
}
