//! urlencoded families (PLAN 11.2): to pairs, to a struct, serialize.

use std::hint::black_box;

use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use form_net_bench as b;

fn to_pairs(c: &mut Criterion) {
    let inputs = [
        ("login", b::login()),
        ("search", b::search()),
        ("signup", b::signup()),
        ("1000-fields", b::thousand_fields()),
        ("1MiB-plain", b::one_mib_plain()),
        ("1MiB-escaped", b::one_mib_escaped()),
    ];
    let mut scratch = vec![0u8; 1 << 20];
    for (name, body) in &inputs {
        let mut g = c.benchmark_group(format!("urlencoded-pairs/{name}"));
        g.throughput(Throughput::Bytes(body.len() as u64));
        g.bench_function("form-net parse_urlencoded", |bn| {
            bn.iter(|| b::fnet_pairs(black_box(body)))
        });
        g.bench_function("form-net pairs", |bn| {
            bn.iter(|| b::fnet_lazy_pairs(black_box(body)))
        });
        g.bench_function("form-net raw_pairs+decode_into", |bn| {
            bn.iter(|| b::fnet_raw(black_box(body), &mut scratch))
        });
        g.bench_function("form_urlencoded parse", |bn| {
            bn.iter(|| b::form_urlencoded_pairs(black_box(body)))
        });
        g.finish();
    }
}

fn to_struct(c: &mut Criterion) {
    let body = b::signup();
    let mut g = c.benchmark_group("urlencoded-struct/signup");
    g.throughput(Throughput::Bytes(body.len() as u64));
    g.bench_function("form-net form!", |bn| {
        bn.iter(|| b::fnet_signup(black_box(&body)))
    });
    g.bench_function("serde_urlencoded", |bn| {
        bn.iter(|| b::serde_urlencoded_signup(black_box(&body)))
    });
    g.bench_function("serde_qs", |bn| {
        bn.iter(|| b::serde_qs_signup(black_box(&body)))
    });
    g.finish();
}

fn serialize(c: &mut Criterion) {
    let mut g = c.benchmark_group("urlencoded-serialize/10-pairs");
    g.bench_function("form-net serialize_urlencoded", |bn| {
        bn.iter(b::fnet_serialize)
    });
    g.bench_function("form_urlencoded Serializer", |bn| {
        bn.iter(b::form_urlencoded_serialize)
    });
    g.bench_function("serde_urlencoded to_string", |bn| {
        bn.iter(b::serde_urlencoded_serialize)
    });
    g.finish();
}

criterion_group!(benches, to_pairs, to_struct, serialize);
criterion_main!(benches);
