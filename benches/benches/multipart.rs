//! multipart families (PLAN 11.2): streaming, buffered, to disk, and the
//! adversarial shapes of T-3 and T-4.
//!
//! `FORM_NET_BENCH_BIG=1` adds the 100 MiB body.

use std::hint::black_box;

use bytes::Bytes;
use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use form_net_bench as b;

fn streaming(c: &mut Criterion) {
    let mut inputs = vec![
        ("login", b::mp_login()),
        ("avatar-64KiB", b::mp_avatar()),
        ("100-parts", b::mp_many_parts()),
        ("document-10MiB", b::mp_document()),
        ("adversarial-all-CR-1MiB", b::mp_all_cr()),
        ("adversarial-near-miss-1MiB", b::mp_near_miss()),
        (
            "adversarial-near-miss-middle-1MiB",
            b::mp_near_miss_middle(),
        ),
    ];
    if std::env::var_os("FORM_NET_BENCH_BIG").is_some() {
        inputs.push(("one-file-100MiB", b::mp_hundred_mib()));
    }
    for (name, body) in &inputs {
        let shared = Bytes::from(body.clone());
        for chunk in [usize::MAX, 64 * 1024, 8 * 1024, 1500] {
            let chunk = chunk.min(body.len());
            let label = if chunk == body.len() {
                "whole".to_string()
            } else {
                format!("{chunk}B")
            };
            let mut g = c.benchmark_group(format!("multipart-stream/{name}/{label}"));
            g.throughput(Throughput::Bytes(body.len() as u64));
            if body.len() > (1 << 20) {
                g.sample_size(20);
            }
            g.bench_function("form-net MultipartParser", |bn| {
                bn.iter(|| b::fnet_stream(black_box(body), chunk))
            });
            g.bench_function("multer", |bn| {
                bn.iter(|| b::multer_stream(black_box(&shared), chunk))
            });
            g.finish();
        }
    }
    // Bytewise: the split overhead (PLAN 11.1), form-net only.
    let body = b::mp_avatar();
    let mut g = c.benchmark_group("multipart-stream/avatar-64KiB/1B");
    g.throughput(Throughput::Bytes(body.len() as u64));
    g.sample_size(20);
    g.bench_function("form-net MultipartParser", |bn| {
        bn.iter(|| b::fnet_stream(black_box(&body), 1))
    });
    g.finish();
}

fn buffered(c: &mut Criterion) {
    let ct = b::content_type();
    for (name, body) in [
        ("login", b::mp_login()),
        ("avatar-64KiB", b::mp_avatar()),
        ("batch-20x1MiB", b::mp_batch()),
    ] {
        let shared = Bytes::from(body.clone());
        let mut g = c.benchmark_group(format!("multipart-buffered/{name}"));
        g.throughput(Throughput::Bytes(body.len() as u64));
        if body.len() > (1 << 20) {
            g.sample_size(20);
        }
        g.bench_function("form-net parse_multipart", |bn| {
            bn.iter(|| b::fnet_buffered(&ct, black_box(&body)))
        });
        g.bench_function("multer one chunk", |bn| {
            bn.iter(|| b::multer_buffered(&ct, black_box(&shared)))
        });
        g.bench_function("multipart 0.18", |bn| {
            bn.iter(|| b::multipart_018_buffered(&ct, black_box(&body)))
        });
        g.finish();
    }
}

fn to_disk(c: &mut Criterion) {
    let ct = b::content_type();
    let body = b::mp_document();
    let mut g = c.benchmark_group("multipart-disk/document-10MiB");
    g.throughput(Throughput::Bytes(body.len() as u64));
    g.sample_size(10);
    g.bench_function("form-net UploadSession + Store::temp", |bn| {
        bn.iter(|| b::fnet_to_disk(&form_net::Store::temp().unwrap(), &ct, black_box(&body)))
    });
    g.bench_function("multipart 0.18 SaveBuilder::temp", |bn| {
        bn.iter(|| b::multipart_018_to_disk(&ct, black_box(&body)))
    });
    g.finish();
}

criterion_group!(benches, streaming, buffered, to_disk);
criterion_main!(benches);
