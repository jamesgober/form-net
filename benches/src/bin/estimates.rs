//! The costs `docs/API.md` still marked as estimates, measured (0.9 polish),
//! as a markdown table: the fastest of 40 batches for each line. Run with
//! `cargo run --release --bin estimates`.

use std::hint::black_box;
use std::time::Instant;

use form_net::{
    Boundary, FromForm, Inspector, Limits, NoInspector, PartHead, Policy, SpillBudget, SpillQuota,
    Store, UploadSession, Verdict,
};
use form_net_bench as b;

fn best(mut f: impl FnMut() -> usize) -> f64 {
    let mut n = 1u32;
    loop {
        let t = Instant::now();
        for _ in 0..n {
            black_box(f());
        }
        if t.elapsed().as_millis() >= 4 || n >= 1 << 24 {
            break;
        }
        n *= 2;
    }
    let mut min = f64::MAX;
    for _ in 0..40 {
        let t = Instant::now();
        for _ in 0..n {
            black_box(f());
        }
        min = min.min(t.elapsed().as_nanos() as f64 / f64::from(n));
    }
    min
}

fn fmt(ns: f64) -> String {
    if ns >= 1e6 {
        format!("{:.2} ms", ns / 1e6)
    } else if ns >= 1e3 {
        format!("{:.2} µs", ns / 1e3)
    } else {
        format!("{ns:.1} ns")
    }
}

fn row(item: &str, what: &str, estimate: &str, measured: String) {
    println!("| {item} | {what} | {estimate} | {measured} |");
}

/// Counts events, so every call happens.
struct Count(usize);

impl Inspector for Count {
    fn part_start(&mut self, _: &PartHead<'_>) -> Verdict {
        self.0 += 1;
        Verdict::Continue
    }
    fn data(&mut self, _: u32, chunk: &[u8]) -> Verdict {
        self.0 += chunk.len() & 1;
        Verdict::Continue
    }
    fn field(&mut self, _: &str, _: &[u8]) -> Verdict {
        self.0 += 1;
        Verdict::Continue
    }
}

fn main() {
    println!("| Item | Input | Estimate | Measured |");
    println!("|---|---|---|---|");

    // Form::field: the last of ten fields, so ten names are compared.
    let body = b"a0=1&a1=2&a2=3&a3=4&a4=5&a5=6&a6=7&a7=8&a8=9&a9=10";
    let form = form_net::parse_urlencoded(body).unwrap();
    let t = best(|| form.field(black_box("a9")).map_or(0, str::len));
    row(
        "`Form::field`",
        "the last of 10 fields",
        "under 5 ns per field",
        format!("{} ({:.1} ns per field)", fmt(t), t / 10.0),
    );

    // Serializer throughput on long plain values.
    let value = "v".repeat(1 << 20);
    let t = best(|| form_net::serialize_urlencoded([("k", value.as_str())]).len());
    row(
        "`serialize_urlencoded`",
        "one 1 MiB plain value",
        "at least 1 GB/s",
        format!("{} ({:.1} GB/s)", fmt(t), (1u64 << 20) as f64 / t),
    );
    let escaped = "é".repeat(1 << 19);
    let t = best(|| form_net::serialize_urlencoded([("k", escaped.as_str())]).len());
    row(
        "`serialize_urlencoded`",
        "1 MiB that all needs escaping",
        "measure",
        format!("{} ({:.1} GB/s)", fmt(t), (1u64 << 20) as f64 / t),
    );

    // Boundary::new and Boundary::generate.
    let t = best(|| {
        Boundary::new(black_box(b"----WebKitFormBoundaryAbCdEfGhIjKlMnOp"))
            .map_or(0, |b| b.as_bytes().len())
    });
    row(
        "`Boundary::new`",
        "a 38-byte browser boundary",
        "under 100 ns",
        fmt(t),
    );
    let t = best(|| Boundary::generate().map_or(0, |b| b.as_bytes().len()));
    row(
        "`Boundary::generate`",
        "OS randomness",
        "under 1 µs",
        fmt(t),
    );

    // FileName::sanitized on a short name.
    let mp_body = b"--X\r\nContent-Disposition: form-data; name=f; filename=\"My Holiday Photo (2).jpg\"\r\n\r\nx\r\n--X--";
    let mp = form_net::parse_multipart(b"multipart/form-data; boundary=X", mp_body).unwrap();
    let name = mp.file("f").unwrap().filename().unwrap();
    let t = best(|| name.sanitized().map_or(0, |s| s.len()));
    row(
        "`FileName::sanitized`",
        "a 24-byte name",
        "about 100 ns",
        fmt(t),
    );

    // Inspector calls: a counting inspector against none, on 100 small parts.
    let parts = b::multipart_body(
        &(0..100)
            .map(|i| (format!("f{i}"), "0123456789".to_string()))
            .collect::<Vec<_>>()
            .iter()
            .map(|(n, v)| (n.as_str(), v.as_str()))
            .collect::<Vec<_>>(),
        &[],
    );
    let ct = format!("multipart/form-data; boundary={}", b::BOUNDARY);
    let none = best(|| {
        form_net::parse_multipart_inspected(
            ct.as_bytes(),
            &parts,
            &b::MP_LIMITS,
            &Policy::DEFAULT,
            &mut NoInspector,
        )
        .unwrap()
        .len()
    });
    let counted = best(|| {
        let mut c = Count(0);
        form_net::parse_multipart_inspected(
            ct.as_bytes(),
            &parts,
            &b::MP_LIMITS,
            &Policy::DEFAULT,
            &mut c,
        )
        .unwrap()
        .len()
            + c.0
    });
    row(
        "inspector calls",
        "100 text parts, no inspector",
        "nothing",
        fmt(none),
    );
    row(
        "inspector calls",
        "100 text parts, three calls each",
        "one monomorphized call per event",
        format!(
            "{} ({:.1} ns per call)",
            fmt(counted),
            (counted - none).max(0.0) / 300.0
        ),
    );

    // SpillQuota: one reserve and its release, uncontended.
    let quota = SpillQuota::new(1 << 30);
    let t = best(|| {
        let ok = quota.reserve(black_box(4096));
        quota.release(4096);
        usize::from(ok)
    });
    row(
        "`SpillQuota::reserve`",
        "reserve and release, one thread",
        "5 to 20 ns",
        fmt(t),
    );

    // Typed extraction: Signup through form! against the same parse alone.
    let signup = b::signup();
    let parse = best(|| form_net::parse_urlencoded(&signup).unwrap().len());
    let typed = best(|| usize::from(b::Signup::from_urlencoded(&signup).unwrap().age));
    row(
        "typed extraction",
        "sign-up, 8 fields",
        "10 to 30 ns per field over the parse",
        format!(
            "{} against {} parsed ({:.1} ns per field)",
            fmt(typed),
            fmt(parse),
            (typed - parse) / 8.0
        ),
    );

    // The wipe of an owned value: a 2 KiB `Secret` dropped, against the same
    // `String` dropped unwiped.
    let template = "s".repeat(2048);
    let plain = best(|| {
        let v = black_box(template.clone());
        v.len()
    });
    let wiped = best(|| {
        let v = form_net::Secret::from(black_box(template.clone()));
        v.expose().len()
    });
    row(
        "wipe on drop",
        "a 2 KiB `Secret` against a plain `String`",
        "50 to 100 ns for a 2 KiB form",
        format!(
            "{} against {} ({} for the wipe)",
            fmt(wiped),
            fmt(plain),
            fmt(wiped - plain)
        ),
    );

    // The writer: one part head, and the boundary scan per byte.
    let boundary = Boundary::new(b"----formnetBoundary0123456789").unwrap();
    let mut out = [0u8; 512];
    let t = best(|| {
        form_net::encode_part_head(
            &boundary,
            false,
            "avatar",
            Some("me.png"),
            Some("image/png"),
            &mut out,
        )
        .unwrap()
    });
    row("`encode_part_head`", "a file part", "under 100 ns", fmt(t));
    let text = "x".repeat(1 << 20);
    for (on, what) in [(false, "off"), (true, "on")] {
        let t = best(|| {
            let mut w =
                form_net::MultipartWriter::with_boundary(Vec::with_capacity(1 << 21), boundary);
            w.verify_boundary(on);
            w.text("t", &text).unwrap();
            w.finish().unwrap().len()
        });
        row(
            "`MultipartWriter`",
            &format!("1 MiB text, boundary scan {what}"),
            "scan 0.1 to 0.3 ns per byte",
            format!("{} ({:.3} ns per byte)", fmt(t), t / (1u64 << 20) as f64),
        );
    }

    // The store: creating one, persisting a file, and a 100 MiB upload.
    let dir = std::env::temp_dir().join(format!("form-net-estimates-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let t = best(|| Store::in_dir(&dir).map_or(0, |s| s.stats().files_on_disk as usize));
    row(
        "`Store::in_dir`",
        "an existing private directory",
        "10 to 50 µs (`Store::temp`)",
        fmt(t),
    );
    let t = best(|| Store::temp().map_or(0, |s| s.stats().files_on_disk as usize));
    row(
        "`Store::temp`",
        "the per-user temporary directory",
        "10 to 50 µs",
        fmt(t),
    );

    let store = Store::in_dir(&dir).unwrap().memory_threshold(0);
    let one = b::multipart_body(&[], &[("f", b"persist me".as_slice())]);
    let mut n = 0u64;
    let t = best(|| {
        let mut s =
            UploadSession::new(ct.as_bytes(), &Limits::DEFAULT, &Policy::DEFAULT, &store).unwrap();
        s.push(&one, &mut NoInspector).unwrap();
        let mut up = s.finish(&mut NoInspector).unwrap();
        let f = up.take_file("f").unwrap();
        n += 1;
        let dest = dir.join(format!("kept-{n}"));
        f.persist(&dest).unwrap();
        std::fs::remove_file(&dest).unwrap();
        1
    });
    row(
        "`StoredFile::persist`",
        "upload, persist, delete (whole cycle)",
        "persist under 50 µs",
        fmt(t),
    );

    let big = {
        let data = vec![0x5au8; 100 << 20];
        b::multipart_body(&[], &[("f", &data)])
    };
    let limits = b::MP_LIMITS
        .with_max_body_bytes(200 << 20)
        .with_max_file_bytes(200 << 20);
    let store = Store::in_dir(&dir).unwrap();
    let mut min = f64::MAX;
    for _ in 0..5 {
        let t = Instant::now();
        let mut s = UploadSession::new(ct.as_bytes(), &limits, &Policy::DEFAULT, &store).unwrap();
        for chunk in big.chunks(64 << 10) {
            s.push(chunk, &mut NoInspector).unwrap();
        }
        black_box(s.finish(&mut NoInspector).unwrap().file_count());
        min = min.min(t.elapsed().as_nanos() as f64);
    }
    row(
        "`UploadSession` + store",
        "100 MiB file in 64 KiB chunks, spilled",
        "under 5% of one core at 1 GB/s",
        format!(
            "{} ({:.1} GB/s; {:.0}% of one core at 1 GB/s)",
            fmt(min),
            (100u64 << 20) as f64 / min,
            min / ((100u64 << 20) as f64) * 100.0
        ),
    );
    // The CPU side of the same upload: parser and driver into a sink that
    // keeps nothing, so no disk time is counted.
    let mut min = f64::MAX;
    for _ in 0..5 {
        let t = Instant::now();
        let parser = form_net::MultipartParser::new(
            Boundary::new(b::BOUNDARY.as_bytes()).unwrap(),
            &limits,
            &Policy::DEFAULT,
        )
        .unwrap();
        let mut d = form_net::Driver::new(parser, form_net::DiscardSink::default());
        let mut files = 0usize;
        for chunk in big.chunks(64 << 10) {
            let mut rest = chunk;
            loop {
                let (n, p) = d.push(rest, false, &mut NoInspector).unwrap();
                if matches!(p, form_net::Progress::File(_)) {
                    files += 1;
                }
                rest = &rest[n..];
                if rest.is_empty() {
                    break;
                }
            }
        }
        loop {
            let (_, p) = d.push(&[], true, &mut NoInspector).unwrap();
            match p {
                form_net::Progress::File(_) => files += 1,
                form_net::Progress::End => break,
                _ => {}
            }
        }
        black_box(files);
        min = min.min(t.elapsed().as_nanos() as f64);
    }
    row(
        "`Driver` + `DiscardSink`",
        "the same 100 MiB, CPU only",
        "under 5% of one core at 1 GB/s",
        format!(
            "{} ({:.1} GB/s; {:.1}% of one core at 1 GB/s)",
            fmt(min),
            (100u64 << 20) as f64 / min,
            min / ((100u64 << 20) as f64) * 100.0
        ),
    );
    let _ = std::fs::remove_dir_all(&dir);
}
