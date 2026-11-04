# form-net performance

Measured for 0.7.0 on 2026-10-30. Every number on this page is a measurement; the raw tables are committed in [`benches/baseline/2026-10-30/`](../benches/baseline/2026-10-30/), and the code that produced them is the [`benches/`](../benches/) crate.

---

## The result

The plan's bar (PLAN 11.2): at least 20% faster than the best other Rust crate in each family, at equal or stronger limits, with form-net's checks on.

| Family | Competitor | Result |
|---|---|---|
| urlencoded to pairs | form_urlencoded 1.2.2 | 1.34x to 8.00x on login, sign-up, 1,000 fields, and 1 MiB bodies; **level (0.88x to 0.94x) on a 25-byte two-field search query** |
| urlencoded to a struct | serde_urlencoded 0.7.1, serde_qs 1.1.3 | 1.75x |
| urlencoded serialize | form_urlencoded 1.2.2 | 2.20x |
| multipart streaming | multer 3.1.0 | 1.23x to 12.32x on every input and chunk size, adversarial shapes included |
| multipart buffered | multer 3.1.0, multipart 0.18.0 | 1.31x to 2.27x |
| multipart to disk | multipart 0.18.0 | 2.06x |

The one input below the bar is the search query `q=rust+form+parser&page=2`. Both crates spend most of its time allocating the one owned string that `+` decoding needs; form-net checks more on top (strict UTF-8, every limit). It is accepted as level rather than closed by weakening a check (`dev/PLAN.md` D-111), and is tried again in the polish pass before 1.0 (D-112).

---

## Machine and method

| | |
|---|---|
| CPU | AMD Ryzen 9 9950X3D, 16 cores, 32 threads |
| Memory | 128 GB |
| OS | Windows 11 Pro, power plan Balanced |
| Rust | 1.99.0 (b940084d7 2026-09-28), `--release` |
| Load | shared with other build sessions; CPU load about 20% during the runs |

Because the machine is shared, the bar is judged on the interleaved ratio harness (`benches/src/bin/ratios.rs`, D-106). For each pair it alternates form-net and the competitor in batches of about 4 ms, for 40 rounds, and each side keeps its fastest batch. Load from other work falls on both sides alike, and the minimum is the best estimate of the unloaded cost. The criterion medians (`benches/baseline/2026-10-30/criterion.md`) agree with it within the noise. Hardware counters were not measured: `perf stat` needs a Linux host with hardware events, which this host and its WSL kernel do not provide.

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
| urlencoded pairs | login | pairs (collect) | 156 ns | form_urlencoded (collect) | 226 ns | 1.45x | pass |
| urlencoded pairs | login | pairs (no collect) | 126 ns | form_urlencoded (no collect) | 218 ns | 1.74x | pass |
| urlencoded pairs | login | raw_pairs+decode_into (no collect) | 105 ns | form_urlencoded (no collect) | 219 ns | 2.08x | pass |
| urlencoded pairs | search | pairs (collect) | 104 ns | form_urlencoded (collect) | 91 ns | 0.88x | below |
| urlencoded pairs | search | pairs (no collect) | 75 ns | form_urlencoded (no collect) | 70 ns | 0.94x | below |
| urlencoded pairs | search | raw_pairs+decode_into (no collect) | 77 ns | form_urlencoded (no collect) | 70 ns | 0.91x | below |
| urlencoded pairs | signup | pairs (collect) | 380 ns | form_urlencoded (collect) | 668 ns | 1.76x | pass |
| urlencoded pairs | signup | pairs (no collect) | 316 ns | form_urlencoded (no collect) | 624 ns | 1.97x | pass |
| urlencoded pairs | signup | raw_pairs+decode_into (no collect) | 273 ns | form_urlencoded (no collect) | 634 ns | 2.32x | pass |
| urlencoded pairs | 1000 fields | pairs (collect) | 19.02 µs | form_urlencoded (collect) | 25.51 µs | 1.34x | pass |
| urlencoded pairs | 1000 fields | pairs (no collect) | 16.16 µs | form_urlencoded (no collect) | 23.35 µs | 1.44x | pass |
| urlencoded pairs | 1000 fields | raw_pairs+decode_into (no collect) | 32.10 µs | form_urlencoded (no collect) | 22.99 µs | 0.72x | below |
| urlencoded pairs | 1 MiB plain | pairs (collect) | 96.33 µs | form_urlencoded (collect) | 767.89 µs | 7.97x | pass |
| urlencoded pairs | 1 MiB plain | pairs (no collect) | 96.77 µs | form_urlencoded (no collect) | 774.39 µs | 8.00x | pass |
| urlencoded pairs | 1 MiB plain | raw_pairs+decode_into (no collect) | 170.02 µs | form_urlencoded (no collect) | 769.99 µs | 4.53x | pass |
| urlencoded pairs | 1 MiB escaped | pairs (collect) | 565.90 µs | form_urlencoded (collect) | 982.40 µs | 1.74x | pass |
| urlencoded pairs | 1 MiB escaped | pairs (no collect) | 451.69 µs | form_urlencoded (no collect) | 903.10 µs | 2.00x | pass |
| urlencoded pairs | 1 MiB escaped | raw_pairs+decode_into (no collect) | 313.15 µs | form_urlencoded (no collect) | 902.54 µs | 2.88x | pass |
| urlencoded struct | signup | form! | 434 ns | serde_urlencoded | 758 ns | 1.75x | pass |
| urlencoded serialize | 10 pairs | serialize_urlencoded | 147 ns | form_urlencoded | 322 ns | 2.20x | pass |
| multipart stream | login, whole | MultipartParser | 663 ns | multer | 2.24 µs | 3.38x | pass |
| multipart stream | avatar 64 KiB, whole | MultipartParser | 2.51 µs | multer | 4.16 µs | 1.66x | pass |
| multipart stream | avatar 64 KiB, 65536 B chunks | MultipartParser | 2.52 µs | multer | 4.94 µs | 1.96x | pass |
| multipart stream | avatar 64 KiB, 1500 B chunks | MultipartParser | 3.31 µs | multer | 5.92 µs | 1.79x | pass |
| multipart stream | 100 parts, whole | MultipartParser | 15.74 µs | multer | 61.91 µs | 3.93x | pass |
| multipart stream | 100 parts, 1500 B chunks | MultipartParser | 15.97 µs | multer | 61.72 µs | 3.87x | pass |
| multipart stream | document 10 MiB, whole | MultipartParser | 739.92 µs | multer | 1.44 ms | 1.95x | pass |
| multipart stream | document 10 MiB, 65536 B chunks | MultipartParser | 738.98 µs | multer | 3.60 ms | 4.87x | pass |
| multipart stream | document 10 MiB, 1500 B chunks | MultipartParser | 959.24 µs | multer | 3.31 ms | 3.45x | pass |
| multipart stream | all CR 1 MiB, whole | MultipartParser | 24.94 µs | multer | 147.50 µs | 5.91x | pass |
| multipart stream | all CR 1 MiB, 65536 B chunks | MultipartParser | 25.50 µs | multer | 314.25 µs | 12.32x | pass |
| multipart stream | all CR 1 MiB, 1500 B chunks | MultipartParser | 44.72 µs | multer | 199.44 µs | 4.46x | pass |
| multipart stream | near-miss 1 MiB, whole | MultipartParser | 119.94 µs | multer | 147.69 µs | 1.23x | pass |
| multipart stream | near-miss 1 MiB, 65536 B chunks | MultipartParser | 120.66 µs | multer | 318.14 µs | 2.64x | pass |
| multipart stream | near-miss 1 MiB, 1500 B chunks | MultipartParser | 147.01 µs | multer | 198.62 µs | 1.35x | pass |
| multipart stream | near-miss in the middle 1 MiB, whole | MultipartParser | 150.21 µs | multer | 367.84 µs | 2.45x | pass |
| multipart stream | near-miss in the middle 1 MiB, 65536 B chunks | MultipartParser | 151.49 µs | multer | 506.46 µs | 3.34x | pass |
| multipart stream | near-miss in the middle 1 MiB, 1500 B chunks | MultipartParser | 174.09 µs | multer | 395.53 µs | 2.27x | pass |
| multipart buffered | login | parse_multipart | 917 ns | multipart 0.18 | 1.20 µs | 1.31x | pass |
| multipart buffered | avatar 64 KiB | parse_multipart | 2.70 µs | multipart 0.18 | 3.90 µs | 1.44x | pass |
| multipart buffered | batch 20 x 1 MiB | parse_multipart | 1.31 ms | multer | 2.98 ms | 2.27x | pass |
| multipart to disk | document 10 MiB | UploadSession + Store::temp | 4.17 ms | multipart 0.18 SaveBuilder | 8.59 ms | 2.06x | pass |

---

## The plan's targets (PLAN 11.1)

| Operation | Input | Target | Measured |
|---|---|---|---|
| `FormKind::detect` | browser `Content-Type` | under 100 ns | 114 ns |
| `raw_pairs` scan | 1 MiB, no escapes | at least 1.5 GB/s | 12.6 GB/s |
| `decode_into` | 1 MiB, every byte escaped | at least 700 MB/s | 3.9 GB/s |
| `parse_urlencoded` | 10 fields, 179 bytes | under 400 ns | 405 ns |
| `Signup::from_urlencoded` | 8 fields, 182 bytes | under 600 ns | 429 ns |
| `serialize_urlencoded` | 10 pairs | under 300 ns | 132 ns |
| `MultipartParser` | one 100 MiB file part, random bytes, 64 KiB chunks | at least 3 GB/s | 13.7 GB/s |
| `MultipartParser` | adversarial, 1 MiB all CR | at least 400 MB/s, linear | 52.9 GB/s |
| `MultipartParser` | adversarial, 1 MiB near-miss delimiters | at least 400 MB/s, linear | 8.7 GB/s |
| `MultipartParser` | 100 parts of 10 bytes | under 30 µs | 15.88 µs |
| `MultipartParser` | 360 B body, 3 text fields | under 1 µs | 669 ns |
| `parse_multipart` | 1 MiB, 2 files | within 10% of the raw parser | 21.66 µs against 21.46 µs raw (+1%) |
| split overhead | 65921 B body in 1-byte chunks | linear, under 50 ns per call | 13.3 ns per byte |
| idea 0.2: work factor | all CR against random data | add `max_work_factor` if 4x or more | 0.27x |
| idea 0.2: work factor | near-miss against random data | add `max_work_factor` if 4x or more | 1.64x |
| idea 0.2: work factor | near-miss in the middle against random data | add `max_work_factor` if 4x or more | 2.04x |
| idea 0.6: sniff window | 64 KiB upload, `sniff_bytes` 0 | measure | 17.12 µs |
| idea 0.6: sniff window | 64 KiB upload, `sniff_bytes` 64 | measure | 16.33 µs |
| idea 0.6: sniff window | 64 KiB upload, `sniff_bytes` 512 | measure | 16.40 µs |

Two targets were estimates that the measurement does not quite meet:

- `FormKind::detect` takes 114 ns against an estimate of 100 ns. It runs parse-net's full, strict media-type grammar; making it faster belongs in parse-net, and no benchmark family depends on it.
- `parse_urlencoded` of ten fields came in at 405 ns in this run, and at 379 ns and 419 ns in earlier ones; it sits at the estimate.

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

## What 0.7 changed

0.7 added checks, and the matcher they need keeps the speed. The search anchors on a CR or LF followed by `-`, so one pass finds delimiters and bare line breaks (D-113). Around a chunk's edges and after a miss it compares a word at a time where the automata would step a byte at a time (D-117).

| Input | 0.6.0 | 0.7.0 | Why |
|---|---|---|---|
| all CR, 1 MiB | 66.66 µs | 24.94 µs | a line break alone is no longer a stop; the `-` after it is |
| near-miss, 1 MiB, whole | 123.48 µs | 119.94 µs | a miss resumes at the byte that differed |
| near-miss, 1 MiB, 1,500-byte chunks | 160.64 µs | 147.01 µs | a compare per chunk edge |
| near-miss in the middle, 1 MiB | 139.55 µs | 150.21 µs | slower by 8%: a word-wise compare against the 0.6 whole compare; still 2.45x multer |
| 100 parts of 10 bytes | 15.08 µs | 15.76 µs | slower by 4%: the bare-break state per part |

Some ratios fell with form-net's time unchanged, because the competitor's minimum fell on a quieter machine: the avatar against multer (form-net 2.54 to 2.51 µs, multer 5.28 to 4.16 µs), 1 MiB plain against form_urlencoded (97 to 97 µs, 930 to 774 µs).

One number depends on the build. `pairs` on 1 MiB of escapes reads 332 µs in 0.6.0 and 452 µs here, with no urlencoded code changed: built with this crate's release profile (fat LTO, one codegen unit) the 0.7.0 code and the code before it both read 326 to 334 µs, so the move is code placement under the default 16 codegen units. It stays well past the bar (2.00x) and is on the polish list (`dev/ROADMAP.md`).

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

- **A work budget (0.2).** The idea: refuse a body by the work the matcher does, if hostile shapes cost far more per byte than normal data. Measured cost per byte against random file data: all CR 0.27x, near-miss delimiters 1.64x, near misses in the middle 2.04x. The trigger was 4x; no limit is added (D-107).
- **A first-bytes fingerprint (0.6).** The idea: let customs-sec skip sniffing for common types. A 64 KiB upload costs the same with `sniff_bytes` 0, 64, or 512 (17.2, 16.4, 16.6 µs), so there is nothing to save (D-107).

A cost found on the way: a 64 KiB upload kept in memory spends most of its 10 µs wiping the file's bytes when it is dropped, because crypt-io's wipe writes one volatile byte at a time. A word-wide wipe in crypt-io would cut that about eightfold; it is a change for crypt-io, not form-net.

---

## What the others do (checked against source)

The comparison below was checked against each crate's source code at the versions named (D-108). It states what the code does.

### multipart

| | form-net 0.7 | multer 3.1.0 | multipart 0.18.0 (server) |
|---|---|---|---|
| model | sans-IO, any split; sync reader; async-free | async `Stream` of `Bytes` | sync `Read` |
| `no_std` core | yes, without `alloc` | no | no |
| `unsafe` in the crate | forbidden | forbidden (`#![forbid(unsafe_code)]`) | none |
| third-party runtime dependencies | none (crypt-io's audited zeroize and subtle only) | 8 | 10 for server use |
| boundary search | safe SWAR, anchored on a line break and `-` | `memchr::memmem` (SIMD) | `twoway` |
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

| | form-net 0.7 | form_urlencoded 1.2.2 | serde_urlencoded 0.7.1 | serde_qs 1.1.3 |
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
