//! The 20% bar, measured on a shared machine: each pair (form-net against
//! the best competitor) runs interleaved, A B A B, in short batches over
//! many rounds, and each side keeps its fastest batch. Load from other work
//! falls on both sides alike, and the minimum is the best estimate of the
//! unloaded cost. Run with `cargo run --release --bin ratios`.
//!
//! Prints a markdown table: per-iteration minimum for each side and the
//! ratio (competitor / form-net; above 1.20 passes the bar).

use std::hint::black_box;
use std::time::{Duration, Instant};

use bytes::Bytes;
use form_net_bench as b;

const ROUNDS: usize = 40;
const BATCH: Duration = Duration::from_millis(4);

/// Iterations that take about one batch.
fn calibrate(f: &mut dyn FnMut() -> usize) -> u32 {
    let mut n = 1u32;
    loop {
        let t = Instant::now();
        for _ in 0..n {
            black_box(f());
        }
        if t.elapsed() >= BATCH || n >= 1 << 24 {
            return n;
        }
        n = n.saturating_mul(2);
    }
}

fn batch(f: &mut dyn FnMut() -> usize, n: u32) -> f64 {
    let t = Instant::now();
    for _ in 0..n {
        black_box(f());
    }
    t.elapsed().as_secs_f64() * 1e9 / f64::from(n)
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

fn pair(
    family: &str,
    input: &str,
    ours: &str,
    mut a: impl FnMut() -> usize,
    theirs: &str,
    mut c: impl FnMut() -> usize,
) {
    assert_eq!(
        a(),
        c(),
        "{family} {input}: the two sides do different work"
    );
    let na = calibrate(&mut a);
    let nc = calibrate(&mut c);
    let (mut best_a, mut best_c) = (f64::MAX, f64::MAX);
    for _ in 0..ROUNDS {
        best_a = best_a.min(batch(&mut a, na));
        best_c = best_c.min(batch(&mut c, nc));
    }
    let ratio = best_c / best_a;
    let verdict = if ratio >= 1.2 { "pass" } else { "below" };
    println!(
        "| {family} | {input} | {ours} | {} | {theirs} | {} | {ratio:.2}x | {verdict} |",
        fmt(best_a),
        fmt(best_c)
    );
}

fn main() {
    let ct = b::content_type();
    let only = std::env::args().nth(1);
    let want = |name: &str| only.as_deref().is_none_or(|o| name.contains(o));
    println!("| Family | Input | form-net | min | Competitor | min | Ratio | Bar |");
    println!("|---|---|---|---|---|---|---|---|");

    let mut scratch = vec![0u8; 1 << 20];
    for (input, body) in [
        ("login", b::login()),
        ("search", b::search()),
        ("signup", b::signup()),
        ("1000 fields", b::thousand_fields()),
        ("1 MiB plain", b::one_mib_plain()),
        ("1 MiB escaped", b::one_mib_escaped()),
    ] {
        if !want("pairs") {
            break;
        }
        // Pairings doing the same work: collecting the decoded pairs;
        // iterating them without keeping any; and form-net's allocation-free
        // decode into a caller buffer (which copies every byte: more work
        // than iterating borrowed pairs) against the same iteration.
        pair(
            "urlencoded pairs",
            input,
            "pairs (collect)",
            || b::fnet_lazy_pairs(&body),
            "form_urlencoded (collect)",
            || b::form_urlencoded_pairs(&body),
        );
        pair(
            "urlencoded pairs",
            input,
            "pairs (no collect)",
            || b::fnet_lazy_sum(&body),
            "form_urlencoded (no collect)",
            || b::form_urlencoded_sum(&body),
        );
        pair(
            "urlencoded pairs",
            input,
            "raw_pairs+decode_into (no collect)",
            || b::fnet_raw(&body, &mut scratch),
            "form_urlencoded (no collect)",
            || b::form_urlencoded_sum(&body),
        );
    }
    if want("struct") {
        let body = b::signup();
        pair(
            "urlencoded struct",
            "signup",
            "form!",
            || usize::from(b::fnet_signup(&body)),
            "serde_urlencoded",
            || usize::from(b::serde_urlencoded_signup(&body)),
        );
    }
    if want("serialize") {
        pair(
            "urlencoded serialize",
            "10 pairs",
            "serialize_urlencoded",
            b::fnet_serialize,
            "form_urlencoded",
            b::form_urlencoded_serialize,
        );
    }
    if want("stream") {
        for (input, body) in [
            ("login", b::mp_login()),
            ("avatar 64 KiB", b::mp_avatar()),
            ("100 parts", b::mp_many_parts()),
            ("document 10 MiB", b::mp_document()),
            ("all CR 1 MiB", b::mp_all_cr()),
            ("near-miss 1 MiB", b::mp_near_miss()),
            ("near-miss in the middle 1 MiB", b::mp_near_miss_middle()),
        ] {
            let shared = Bytes::from(body.clone());
            for chunk in [usize::MAX, 64 * 1024, 1500] {
                let chunk = chunk.min(body.len());
                let label = if chunk == body.len() {
                    format!("{input}, whole")
                } else {
                    format!("{input}, {chunk} B chunks")
                };
                pair(
                    "multipart stream",
                    &label,
                    "MultipartParser",
                    || b::fnet_stream(&body, chunk),
                    "multer",
                    || b::multer_stream(&shared, chunk),
                );
            }
        }
    }
    if want("buffered") {
        for (input, body) in [
            ("login", b::mp_login()),
            ("avatar 64 KiB", b::mp_avatar()),
            ("batch 20 x 1 MiB", b::mp_batch()),
        ] {
            let shared = Bytes::from(body.clone());
            // The best competitor for each input.
            let m = {
                let t = Instant::now();
                for _ in 0..20 {
                    black_box(b::multer_buffered(&ct, &shared));
                }
                t.elapsed()
            };
            let p = {
                let t = Instant::now();
                for _ in 0..20 {
                    black_box(b::multipart_018_buffered(&ct, &body));
                }
                t.elapsed()
            };
            if m <= p {
                pair(
                    "multipart buffered",
                    input,
                    "parse_multipart",
                    || b::fnet_buffered(&ct, &body),
                    "multer",
                    || b::multer_buffered(&ct, &shared),
                );
            } else {
                pair(
                    "multipart buffered",
                    input,
                    "parse_multipart",
                    || b::fnet_buffered(&ct, &body),
                    "multipart 0.18",
                    || b::multipart_018_buffered(&ct, &body),
                );
            }
        }
    }
    if want("disk") {
        let body = b::mp_document();
        // Each side creates its temporary directory per request, as
        // multipart's `SaveBuilder::temp` does.
        pair(
            "multipart to disk",
            "document 10 MiB",
            "UploadSession + Store::temp",
            || b::fnet_to_disk(&form_net::Store::temp().unwrap(), &ct, &body),
            "multipart 0.18 SaveBuilder",
            || b::multipart_018_to_disk(&ct, &body),
        );
    }
}
