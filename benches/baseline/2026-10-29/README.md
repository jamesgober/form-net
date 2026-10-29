# Benchmark baseline, 2026-10-29

form-net 0.6.0 against multer 3.1.0, multipart 0.18.0, form_urlencoded 1.2.2, serde_urlencoded 0.7.1, and serde_qs 1.1.3, on an AMD Ryzen 9 9950X3D (16 cores, 128 GB), Windows 11 Pro, Rust 1.99.0, shared with other build sessions (about 46% CPU load).

- `ratios.md`: the bar, from `cargo run --release --bin ratios` (interleaved minimums).
- `targets.md`: PLAN 11.1, from `--bin targets`.
- `allocs.md`: allocation counts, from `--bin allocs`.
- `criterion.md`: criterion medians, from `cargo bench` and `report.py`.

See `docs/PERFORMANCE.md` for the method.
