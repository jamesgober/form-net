# Benchmark baseline, 2026-11-08

form-net 0.9.0 against multer 3.1.0, multipart 0.18.0, form_urlencoded 1.2.2, serde_urlencoded 0.7.1, and serde_qs 1.1.3, on an AMD Ryzen 9 9950X3D (16 cores, 128 GB), Windows 11 Pro, Rust 1.99.0, shared with other sessions at 100% CPU load throughout.

The machine did not go quiet in a day of waiting, so the timing binaries ran five times and each table here is the median of each cell over the five runs; `run-1` to `run-5` hold every run.

- `ratios.md`: the bar, from `cargo run --release --bin ratios` (interleaved minimums), median of five; the bar is recomputed from the median ratio.
- `targets.md`: PLAN 11.1, from `--bin targets`, median of five.
- `estimates.md`: the API's cost estimates, from `--bin estimates`, median of five.
- `allocs.md`: allocation counts, from `--bin allocs`, one run (the counts are exact; they match 2026-10-30).
- `criterion.md`: criterion medians, from `cargo bench` and `report.py`, one run.

Absolute times read about 1.5x their quiet values under this load, for form-net and the competitors alike; the ratios hold. `benches/baseline/2026-10-30` stays the reference for absolute times. See `docs/PERFORMANCE.md` for the method.
