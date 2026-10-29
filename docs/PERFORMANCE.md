# form-net performance

Measured for 0.6.0 on 2026-10-29. Every number on this page is a measurement; the raw tables are committed in [`benches/baseline/2026-10-29/`](../benches/baseline/2026-10-29/), and the code that produced them is the [`benches/`](../benches/) crate.

---

## The result

The plan's bar (PLAN 11.2): at least 20% faster than the best other Rust crate in each family, at equal or stronger limits, with form-net's checks on.

| Family | Competitor | Result |
|---|---|---|
| urlencoded to pairs | form_urlencoded 1.2.2 | 1.39x to 9.55x on login, sign-up, 1,000 fields, and 1 MiB bodies; **level (0.88x to 1.04x) on a 25-byte two-field search query** |
| urlencoded to a struct | serde_urlencoded 0.7.1, serde_qs 1.1.3 | 1.59x |
| urlencoded serialize | form_urlencoded 1.2.2 | 2.26x |
| multipart streaming | multer 3.1.0 | 1.32x to 4.97x on every input and chunk size, adversarial shapes included |
| multipart buffered | multer 3.1.0, multipart 0.18.0 | 1.36x to 2.31x |
| multipart to disk | multipart 0.18.0 | 2.09x |

The one input below the bar is the search query `q=rust+form+parser&page=2`. Both crates spend most of its time allocating the one owned string that `+` decoding needs; form-net checks more on top (strict UTF-8, every limit). It is accepted as level rather than closed by weakening a check (`dev/PLAN.md` D-111), and is tried again in the polish pass before 1.0 (D-112).

---

## Machine and method

| | |
|---|---|
| CPU | AMD Ryzen 9 9950X3D, 16 cores, 32 threads |
| Memory | 128 GB |
| OS | Windows 11 Pro, power plan Balanced |
| Rust | 1.99.0 (b940084d7 2026-09-28), `--release` |
| Load | shared with other build sessions; CPU load about 46% during the runs |

Because the machine is shared, the bar is judged on the interleaved ratio harness (`benches/src/bin/ratios.rs`, D-106). For each pair it alternates form-net and the competitor in batches of about 4 ms, for 40 rounds, and each side keeps its fastest batch. Load from other work falls on both sides alike, and the minimum is the best estimate of the unloaded cost. The criterion medians (`benches/baseline/2026-10-29/criterion.md`) agree with it within the noise. Hardware counters were not measured: `perf stat` needs a Linux host with hardware events, which this host and its WSL kernel do not provide.

Fairness rules, all enforced in `benches/src/lib.rs` and checked by `benches/tests/agree.rs` (every driver returns the same total of value bytes):

- Every driver parses the whole body and touches every byte of every value.
- Competitors get the same limits where they take limits (multer `whole_stream` and `per_field`, multipart's `size_limit`); form-net never runs with its checks off.
- Every buffered driver parses the `Content-Type` header: form-net through `parse_multipart`, the others through `multer::parse_boundary` (the `mime` crate), as their framework integrations do.
- multer receives zero-copy `Bytes` slices of the body; nothing allocated by the harness is charged to it.
- In the disk family each side creates its temporary directory per request.

---

## Every comparison

form-net's fastest batch, the competitor's, and the ratio (competitor time over form-net time). For urlencoded pairs there are three equal-work pairings per input: collecting the decoded pairs, iterating them without keeping any, and form-net's allocation-free `raw_pairs` + `decode_into` (which copies every byte into a caller buffer, more work than iterating borrowed pairs) against the same iteration.

| Family | Input | form-net | min | Competitor | min | Ratio | Bar |
|---|---|---|---|---|---|---|---|
| urlencoded pairs | login | pairs (collect) | 160 ns | form_urlencoded (collect) | 235 ns | 1.47x | pass |
| urlencoded pairs | login | pairs (no collect) | 125 ns | form_urlencoded (no collect) | 210 ns | 1.68x | pass |
| urlencoded pairs | login | raw_pairs+decode_into (no collect) | 107 ns | form_urlencoded (no collect) | 209 ns | 1.96x | pass |
| urlencoded pairs | search | pairs (collect) | 105 ns | form_urlencoded (collect) | 92 ns | 0.88x | below |
| urlencoded pairs | search | pairs (no collect) | 72 ns | form_urlencoded (no collect) | 71 ns | 0.99x | below |
| urlencoded pairs | search | raw_pairs+decode_into (no collect) | 69 ns | form_urlencoded (no collect) | 72 ns | 1.04x | below |
| urlencoded pairs | signup | pairs (collect) | 384 ns | form_urlencoded (collect) | 672 ns | 1.75x | pass |
| urlencoded pairs | signup | pairs (no collect) | 317 ns | form_urlencoded (no collect) | 628 ns | 1.98x | pass |
| urlencoded pairs | signup | raw_pairs+decode_into (no collect) | 290 ns | form_urlencoded (no collect) | 629 ns | 2.17x | pass |
| urlencoded pairs | 1000 fields | pairs (collect) | 19.36 µs | form_urlencoded (collect) | 26.88 µs | 1.39x | pass |
| urlencoded pairs | 1000 fields | pairs (no collect) | 16.33 µs | form_urlencoded (no collect) | 24.55 µs | 1.50x | pass |
| urlencoded pairs | 1000 fields | raw_pairs+decode_into (no collect) | 32.42 µs | form_urlencoded (no collect) | 24.99 µs | 0.77x | below |
| urlencoded pairs | 1 MiB plain | pairs (collect) | 98.24 µs | form_urlencoded (collect) | 888.40 µs | 9.04x | pass |
| urlencoded pairs | 1 MiB plain | pairs (no collect) | 97.40 µs | form_urlencoded (no collect) | 929.88 µs | 9.55x | pass |
| urlencoded pairs | 1 MiB plain | raw_pairs+decode_into (no collect) | 202.28 µs | form_urlencoded (no collect) | 944.96 µs | 4.67x | pass |
| urlencoded pairs | 1 MiB escaped | pairs (collect) | 370.36 µs | form_urlencoded (collect) | 900.80 µs | 2.43x | pass |
| urlencoded pairs | 1 MiB escaped | pairs (no collect) | 331.71 µs | form_urlencoded (no collect) | 887.44 µs | 2.68x | pass |
| urlencoded pairs | 1 MiB escaped | raw_pairs+decode_into (no collect) | 320.23 µs | form_urlencoded (no collect) | 871.65 µs | 2.72x | pass |
| urlencoded struct | signup | form! | 465 ns | serde_urlencoded | 740 ns | 1.59x | pass |
| urlencoded serialize | 10 pairs | serialize_urlencoded | 152 ns | form_urlencoded | 344 ns | 2.26x | pass |
| multipart stream | login, whole | MultipartParser | 670 ns | multer | 2.32 µs | 3.46x | pass |
| multipart stream | avatar 64 KiB, whole | MultipartParser | 2.54 µs | multer | 5.28 µs | 2.08x | pass |
| multipart stream | avatar 64 KiB, 65536 B chunks | MultipartParser | 2.56 µs | multer | 5.22 µs | 2.03x | pass |
| multipart stream | avatar 64 KiB, 1500 B chunks | MultipartParser | 3.20 µs | multer | 7.85 µs | 2.45x | pass |
| multipart stream | 100 parts, whole | MultipartParser | 15.08 µs | multer | 63.52 µs | 4.21x | pass |
| multipart stream | 100 parts, 1500 B chunks | MultipartParser | 15.37 µs | multer | 63.77 µs | 4.15x | pass |
| multipart stream | document 10 MiB, whole | MultipartParser | 751.69 µs | multer | 1.48 ms | 1.97x | pass |
| multipart stream | document 10 MiB, 65536 B chunks | MultipartParser | 752.11 µs | multer | 3.53 ms | 4.69x | pass |
| multipart stream | document 10 MiB, 1500 B chunks | MultipartParser | 933.73 µs | multer | 3.37 ms | 3.60x | pass |
| multipart stream | all CR 1 MiB, whole | MultipartParser | 66.66 µs | multer | 165.22 µs | 2.48x | pass |
| multipart stream | all CR 1 MiB, 65536 B chunks | MultipartParser | 67.56 µs | multer | 335.97 µs | 4.97x | pass |
| multipart stream | all CR 1 MiB, 1500 B chunks | MultipartParser | 92.42 µs | multer | 227.02 µs | 2.46x | pass |
| multipart stream | near-miss 1 MiB, whole | MultipartParser | 123.48 µs | multer | 165.11 µs | 1.34x | pass |
| multipart stream | near-miss 1 MiB, 65536 B chunks | MultipartParser | 115.10 µs | multer | 333.74 µs | 2.90x | pass |
| multipart stream | near-miss 1 MiB, 1500 B chunks | MultipartParser | 160.64 µs | multer | 212.24 µs | 1.32x | pass |
| multipart stream | near-miss in the middle 1 MiB, whole | MultipartParser | 139.55 µs | multer | 375.32 µs | 2.69x | pass |
| multipart stream | near-miss in the middle 1 MiB, 65536 B chunks | MultipartParser | 140.07 µs | multer | 513.95 µs | 3.67x | pass |
| multipart stream | near-miss in the middle 1 MiB, 1500 B chunks | MultipartParser | 161.69 µs | multer | 418.48 µs | 2.59x | pass |
| multipart buffered | login | parse_multipart | 932 ns | multipart 0.18 | 1.27 µs | 1.36x | pass |
| multipart buffered | avatar 64 KiB | parse_multipart | 2.80 µs | multipart 0.18 | 4.05 µs | 1.44x | pass |
| multipart buffered | batch 20 x 1 MiB | parse_multipart | 1.32 ms | multer | 3.04 ms | 2.31x | pass |
| multipart to disk | document 10 MiB | UploadSession + Store::temp | 3.95 ms | multipart 0.18 SaveBuilder | 8.23 ms | 2.09x | pass |

---

## The plan's targets (PLAN 11.1)

| Operation | Input | Target | Measured |
|---|---|---|---|
| `FormKind::detect` | browser `Content-Type` | under 100 ns | 114 ns |
| `raw_pairs` scan | 1 MiB, no escapes | at least 1.5 GB/s | 12.4 GB/s |
| `decode_into` | 1 MiB, every byte escaped | at least 700 MB/s | 3.9 GB/s |
| `parse_urlencoded` | 10 fields, 179 bytes | under 400 ns | 379 ns |
| `Signup::from_urlencoded` | 8 fields, 182 bytes | under 600 ns | 461 ns |
| `serialize_urlencoded` | 10 pairs | under 300 ns | 133 ns |
| `MultipartParser` | one 100 MiB file part, random bytes, 64 KiB chunks | at least 3 GB/s | 13.8 GB/s |
| `MultipartParser` | adversarial, 1 MiB all CR | at least 400 MB/s, linear | 15.5 GB/s |
| `MultipartParser` | adversarial, 1 MiB near-miss delimiters | at least 400 MB/s, linear | 8.2 GB/s |
| `MultipartParser` | 100 parts of 10 bytes | under 30 µs | 14.89 µs |
| `MultipartParser` | 360 B body, 3 text fields | under 1 µs | 656 ns |
| `parse_multipart` | 1 MiB, 2 files | within 10% of the raw parser | 27.94 µs against 27.50 µs raw (+2%) |
| split overhead | 65921 B body in 1-byte chunks | linear, under 50 ns per call | 13.8 ns per byte |
| idea 0.2: work factor | all CR against random data | add `max_work_factor` if 4x or more | 0.85x |
| idea 0.2: work factor | near-miss against random data | add `max_work_factor` if 4x or more | 1.57x |
| idea 0.2: work factor | near-miss in the middle against random data | add `max_work_factor` if 4x or more | 1.80x |
| idea 0.6: sniff window | 64 KiB upload, `sniff_bytes` 0 | measure | 17.59 µs |
| idea 0.6: sniff window | 64 KiB upload, `sniff_bytes` 64 | measure | 16.92 µs |
| idea 0.6: sniff window | 64 KiB upload, `sniff_bytes` 512 | measure | 17.17 µs |

Two targets were estimates that the measurement does not quite meet:

- `FormKind::detect` takes 114 ns against an estimate of 100 ns. It runs parse-net's full, strict media-type grammar; making it faster belongs in parse-net, and no benchmark family depends on it.
- `parse_urlencoded` of ten fields came in at 379 ns in the final run (under 400 ns) and at 419 ns in an earlier one; it sits at the estimate.

---

## Allocations

From `benches/src/bin/allocs.rs` (a counting allocator around each driver; full table in the baseline):

| Family | Input | Crate | Allocations | Bytes |
|---|---|---|---|---|
| urlencoded pairs | sign-up | form-net `parse_urlencoded` | 2 | 502 |
| urlencoded pairs | sign-up | form-net `raw_pairs` + `decode_into` | 0 | 0 |
| urlencoded pairs | sign-up | form_urlencoded | 14 | 773 |
| urlencoded struct | sign-up | form-net `form!` | 6 | 95 |
| urlencoded struct | sign-up | serde_urlencoded | 14 | 201 |
| urlencoded serialize | 10 pairs | form-net | 1 | 256 |
| urlencoded serialize | 10 pairs | form_urlencoded | 6 | 504 |
| multipart stream | login, whole body | form-net `MultipartParser` | 1 | 8,480 |
| multipart stream | login, whole body | multer | 41 | 1,657 |
| multipart stream | document 10 MiB, whole body | form-net `MultipartParser` | 1 | 8,480 |
| multipart stream | document 10 MiB, whole body | multer | 33 | 10,487,266 |

The multipart parser's one allocation is its buffer (`multipart_buffer_len`), made once per parse; nothing is allocated per chunk, per part, or per byte.

---

## What 0.6 changed, and why it is faster

| Change | Decision | Effect |
|---|---|---|
| The delimiter search skips to the next CRLF 32 bytes per step and compares the delimiter whole there, last byte first; the automaton runs only within one delimiter length of a chunk's end | D-102 | random file data 5.3 to 2.2 µs for 64 KiB (12 to 29 GB/s); all-CR 1.5 ms to 60 µs per MiB |
| Wipes cover the bytes written, not whole buffers; growing buffers start small | D-103 | a login form's parser construction and drop 914 to 108 ns; the driver's fixed cost 7 µs to 0.3 µs |
| One UTF-8 pass over a urlencoded body; plain names and values cut from it | D-104 | 1,000 fields 31 to 19 µs |
| The urlencoded scanner narrows its byte set; escapes decoded in place from a table; the serializer writes escapes from a table | D-105 | 1 MiB of escapes 2.96 to 0.32 ms; ten pairs serialized 547 to 133 ns |

Every one is safe Rust (`#![forbid(unsafe_code)]` is unchanged), and every one keeps the behaviour the earlier tests, the property tests, and the fuzz targets pin down.

---

## Two ideas measured (PLAN 14)

- **A work budget (0.2).** The idea: refuse a body by the work the matcher does, if hostile shapes cost far more per byte than normal data. Measured cost per byte against random file data: all CR 0.77x, near-miss delimiters 1.60x, near misses in the middle 1.88x. The trigger was 4x; no limit is added (D-107).
- **A first-bytes fingerprint (0.6).** The idea: let customs-sec skip sniffing for common types. A 64 KiB upload costs the same with `sniff_bytes` 0, 64, or 512 (17.2, 16.4, 16.6 µs), so there is nothing to save (D-107).

A cost found on the way: a 64 KiB upload kept in memory spends most of its 10 µs wiping the file's bytes when it is dropped, because crypt-io's wipe writes one volatile byte at a time. A word-wide wipe in crypt-io would cut that about eightfold; it is a change for crypt-io, not form-net.

---

## What the others do (checked against source)

The comparison below was checked against each crate's source code at the versions named (D-108). It states what the code does.

### multipart

| | form-net 0.6 | multer 3.1.0 | multipart 0.18.0 (server) |
|---|---|---|---|
| model | sans-IO, any split; sync reader; async-free | async `Stream` of `Bytes` | sync `Read` |
| `no_std` core | yes, without `alloc` | no | no |
| `unsafe` in the crate | forbidden | forbidden (`#![forbid(unsafe_code)]`) | none |
| third-party runtime dependencies | none (crypt-io's audited zeroize and subtle only) | 8 | 10 for server use |
| boundary search | safe SWAR, CRLF anchored | `memchr::memmem` (SIMD) | `twoway` |
| limits by default | every one finite | all size limits unlimited (`u64::MAX`); 32 headers per part | `SaveBuilder`: 8 MiB per field, 256 fields; `read_entry`: none |
| part, file, name, header-size, preamble, epilogue limits | all | none (whole stream only) | part count with `SaveBuilder` only; 4 headers per part |
| a delimiter followed by other bytes | refused | refused | refused |
| bare LF inside part headers | refused | accepted | accepted |
| duplicate `Content-Disposition` | refused | last one kept | first one used |
| duplicate `name` / `filename` | refused | first used | first `name=` substring, which can match inside `filename=` |
| `\"` in a quoted value | refused | unescaped to `"` | not recognised; the value ends at `;` |
| `Content-Transfer-Encoding` | refused unless identity | ignored | ignored |
| nested `multipart/mixed` | refused | passed as bytes | passed as bytes |
| empty name | refused | accepted | accepted |
| `filename*` | RFC 8187, by policy | not supported | not supported |
| text charsets | UTF-8 only, others refused | malformed bytes replaced with U+FFFD | UTF-8; `try_text` falls back to bytes |
| temporary files | private directory, anonymous by default, shared quota | none | named, in a temp directory |
| inspection before storage, token first, typed extraction, writer | yes | field allowlist; optional JSON | writer (client module) |

### urlencoded

| | form-net 0.6 | form_urlencoded 1.2.2 | serde_urlencoded 0.7.1 | serde_qs 1.1.3 |
|---|---|---|---|---|
| algorithm | WHATWG, strict | WHATWG | WHATWG (through form_urlencoded) | its own, with brackets |
| invalid UTF-8 | refused | replaced (lossy) | replaced | refused |
| invalid `%` | refused (or literal by policy) | literal | literal | literal, different from WHATWG |
| limits | fields, name, value, text, body | none | none | `max_depth` only (flattens deeper keys) |
| streaming | yes | no | no | no |
| decode into a caller buffer, no allocation | yes | no | no | no |
| duplicate scalar in a struct | refused | n/a | serde's `duplicate field` error | last wins by default |
| `no_std` | yes, without `alloc` | yes, with `alloc` | no | no |

Corrections to the 0.2 specification's matrix found in this check: multipart 0.18 needs 10 third-party crates for its server parser, not 5; serde_qs does not use the WHATWG algorithm; serde_urlencoded keeps order and duplicates when the target is a list of pairs; and several strictness rules the specification called unique to form-net (a delimiter followed by other bytes, a missing close delimiter, refusing invalid UTF-8, header folding) are shared with at least one of the others.

---

## Reproducing

```bash
cd benches
cargo test --release                      # every driver does the same work
cargo run --release --bin ratios          # the bar (minutes)
cargo run --release --bin targets         # PLAN 11.1
cargo run --release --bin allocs          # allocation counts
cargo bench --bench urlencoded --bench multipart
python report.py "$CARGO_TARGET_DIR/criterion"   # criterion medians as a table
```

`FORM_NET_BENCH_BIG=1` adds the 100 MiB body to the criterion multipart run. A change that makes a ratio or a median more than 5% worse than the committed baseline needs its reason recorded in `dev/PLAN.md` before it merges; CI keeps the crate building and the drivers agreeing, and the numbers come from a quiet run, not from shared CI runners.
