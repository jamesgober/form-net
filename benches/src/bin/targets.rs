//! The targets of PLAN 11.1 and the two measured ideas of PLAN 14, as a
//! markdown table: the fastest of 40 batches for each line (the machine is
//! shared; the minimum is the unloaded cost). Run with
//! `cargo run --release --bin targets`.

use std::hint::black_box;
use std::time::Instant;

use form_net::{
    Boundary, FormKind, Inspector, Limits, NoInspector, PartHead, Policy, Store, UploadSession,
    Verdict,
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
        min = min.min(t.elapsed().as_secs_f64() * 1e9 / f64::from(n));
    }
    min
}

fn fmt(ns: f64) -> String {
    if ns >= 1e6 {
        format!("{:.2} ms", ns / 1e6)
    } else if ns >= 1e3 {
        format!("{:.2} µs", ns / 1e3)
    } else {
        format!("{ns:.0} ns")
    }
}

fn gbs(bytes: usize, ns: f64) -> String {
    format!("{:.1} GB/s", bytes as f64 / ns)
}

fn row(op: &str, input: &str, target: &str, measured: String) {
    println!("| {op} | {input} | {target} | {measured} |");
}

/// An inspector that looks at the sniff window, as customs-sec would.
struct Sniffer(usize);
impl Inspector for Sniffer {
    fn sniff(&mut self, _: &PartHead<'_>, first: &[u8]) -> Verdict {
        self.0 += usize::from(first.starts_with(b"\x89PNG"));
        Verdict::Continue
    }
}

fn main() {
    println!("| Operation | Input | Target | Measured |");
    println!("|---|---|---|---|");
    let chrome = b"multipart/form-data; boundary=----WebKitFormBoundary7MA4YWxkTrZu0gW";
    let t = best(|| matches!(FormKind::detect(chrome), Ok(FormKind::Multipart(_))) as usize);
    row(
        "`FormKind::detect`",
        "browser `Content-Type`",
        "under 100 ns",
        fmt(t),
    );

    let plain = b::one_mib_plain();
    let t = best(|| {
        form_net::raw_pairs(&plain, &b::URL_LIMITS)
            .map(|p| p.unwrap().value_raw().len())
            .sum()
    });
    row(
        "`raw_pairs` scan",
        "1 MiB, no escapes",
        "at least 1.5 GB/s",
        gbs(plain.len(), t),
    );

    let mut escaped = Vec::new();
    for _ in 0..(1 << 20) / 3 {
        escaped.extend_from_slice(b"%41");
    }
    let mut out = vec![0u8; escaped.len()];
    let t = best(|| {
        form_net::decode_into(&escaped, &mut out, form_net::PercentPolicy::Refuse)
            .unwrap()
            .len()
    });
    row(
        "`decode_into`",
        "1 MiB, every byte escaped",
        "at least 700 MB/s",
        gbs(escaped.len(), t),
    );

    let ten: Vec<u8> = (0..10)
        .map(|i| format!("field{i}=value+{i}%21"))
        .collect::<Vec<_>>()
        .join("&")
        .into_bytes();
    let t = best(|| form_net::parse_urlencoded(&ten).unwrap().len());
    row(
        "`parse_urlencoded`",
        &format!("10 fields, {} bytes", ten.len()),
        "under 400 ns",
        fmt(t),
    );

    let signup = b::signup();
    let t = best(|| usize::from(b::fnet_signup(&signup)));
    row(
        "`Signup::from_urlencoded`",
        &format!("8 fields, {} bytes", signup.len()),
        "under 600 ns",
        fmt(t),
    );

    let t = best(b::fnet_serialize);
    row("`serialize_urlencoded`", "10 pairs", "under 300 ns", fmt(t));

    let big = b::mp_hundred_mib();
    let t = best(|| b::fnet_stream(&big, 64 * 1024));
    row(
        "`MultipartParser`",
        "one 100 MiB file part, random bytes, 64 KiB chunks",
        "at least 3 GB/s",
        gbs(big.len(), t),
    );
    drop(big);

    for (name, body) in [
        ("all CR", b::mp_all_cr()),
        ("near-miss delimiters", b::mp_near_miss()),
    ] {
        let t = best(|| b::fnet_stream(&body, 64 * 1024));
        row(
            "`MultipartParser`",
            &format!("adversarial, 1 MiB {name}"),
            "at least 400 MB/s, linear",
            gbs(body.len(), t),
        );
    }
    let parts = b::mp_many_parts();
    let t = best(|| b::fnet_stream(&parts, parts.len()));
    row(
        "`MultipartParser`",
        "100 parts of 10 bytes",
        "under 30 µs",
        fmt(t),
    );
    let login = b::mp_login();
    let t = best(|| b::fnet_stream(&login, login.len()));
    row(
        "`MultipartParser`",
        &format!("{} B body, 3 text fields", login.len()),
        "under 1 µs",
        fmt(t),
    );

    let ct = b::content_type();
    let mut files = Vec::new();
    files.push(b::random_bytes(1 << 19, 21));
    files.push(b::random_bytes(1 << 19, 22));
    let doc = b::multipart_body(&[], &[("a", &files[0]), ("b", &files[1])]);
    let raw = best(|| b::fnet_stream(&doc, doc.len()));
    let buffered = best(|| b::fnet_buffered(&ct, &doc));
    row(
        "`parse_multipart`",
        "1 MiB, 2 files",
        "within 10% of the raw parser",
        format!(
            "{} against {} raw ({:+.0}%)",
            fmt(buffered),
            fmt(raw),
            (buffered / raw - 1.0) * 100.0
        ),
    );

    let avatar = b::mp_avatar();
    let t = best(|| b::fnet_stream(&avatar, 1));
    row(
        "split overhead",
        &format!("{} B body in 1-byte chunks", avatar.len()),
        "linear, under 50 ns per call",
        format!("{:.1} ns per byte", t / avatar.len() as f64),
    );

    // PLAN 14, 0.2: the work-budget idea. Cost per body byte of the hostile
    // shapes against random file data, at the same chunking.
    let doc10 = b::mp_document();
    let base = best(|| b::fnet_stream(&doc10, 64 * 1024)) / doc10.len() as f64;
    for (name, body) in [
        ("all CR", b::mp_all_cr()),
        ("near-miss", b::mp_near_miss()),
        ("near-miss in the middle", b::mp_near_miss_middle()),
    ] {
        let per = best(|| b::fnet_stream(&body, 64 * 1024)) / body.len() as f64;
        row(
            "idea 0.2: work factor",
            &format!("{name} against random data"),
            "add `max_work_factor` if 4x or more",
            format!("{:.2}x", per / base),
        );
    }

    // PLAN 14, 0.6: what the sniff window costs, with an inspector reading
    // it, on a 64 KiB upload into memory.
    let png = {
        let mut d = b"\x89PNG\r\n\x1a\n".to_vec();
        d.extend(b::random_bytes(64 * 1024, 5));
        b::multipart_body(&[("user", "42")], &[("avatar", &d)])
    };
    let store = Store::memory_only();
    for sniff in [0u32, 64, 512] {
        let limits = b::MP_LIMITS.with_sniff_bytes(sniff);
        let t = best(|| {
            let mut s =
                UploadSession::new(ct.as_bytes(), &limits, &Policy::DEFAULT, &store).unwrap();
            let mut i = Sniffer(0);
            s.push(&png, &mut i).unwrap();
            s.finish(&mut NoInspector).unwrap().file_count() + i.0
        });
        row(
            "idea 0.6: sniff window",
            &format!("64 KiB upload, `sniff_bytes` {sniff}"),
            "measure",
            fmt(t),
        );
    }
    let _ = (Boundary::new(b"X"), Limits::DEFAULT);
}
