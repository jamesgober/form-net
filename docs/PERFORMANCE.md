# form-net performance

Measured for 0.9.0 on 2026-11-08. Every number on this page is a measurement; the raw tables, each of the five runs, and their medians are committed in [`benches/baseline/2026-11-08/`](../benches/baseline/2026-11-08/), the 0.7.0 baseline in [`benches/baseline/2026-10-30/`](../benches/baseline/2026-10-30/), and the code that produced them is the [`benches/`](../benches/) crate.

---

## The result

The plan's bar (PLAN 11.2): at least 20% faster than the best other Rust crate in each family, at equal or stronger limits, with form-net's checks on.

| Family | Competitor | Result |
|---|---|---|
| urlencoded to pairs | form_urlencoded 1.2.2 | 1.49x to 10.01x on login, sign-up, 1,000 fields, and 1 MiB bodies; **level (0.94x collecting, 1.05x iterating) on a 25-byte two-field search query**, 1.55x there with `raw_pairs` + `decode_into` |
| urlencoded to a struct | serde_urlencoded 0.7.1, serde_qs 1.1.3 | 1.82x |
| urlencoded serialize | form_urlencoded 1.2.2 | 1.93x |
| multipart streaming | multer 3.1.0 | 1.39x to 16.50x on every input and chunk size, adversarial shapes included |
| multipart buffered | multer 3.1.0, multipart 0.18.0 | 1.37x to 2.53x |
| multipart to disk | multipart 0.18.0 | 2.37x |

The one input below the bar is the search query `q=rust+form+parser&page=2` through `pairs`. Both crates spend most of its time allocating the one owned string that `+` decoding needs; form-net checks more on top (strict UTF-8, every limit). It is accepted as level rather than closed by weakening a check (`dev/PLAN.md` D-111). The 0.9 polish pass tried it again (D-112): closing it needs a different return type, so it is listed for James in `dev/READINESS-1.0.md`. Since 0.9 the allocation-free path, `raw_pairs` + `decode_into`, passes on it at 1.55x (D-126).

---

## Machine and method

| | |
|---|---|
| CPU | AMD Ryzen 9 9950X3D, 16 cores, 32 threads |
| Memory | 128 GB |
| OS | Windows 11 Pro, power plan Balanced |
| Rust | 1.99.0 (b940084d7 2026-09-28), `--release` |
| Load | shared with other sessions; CPU load 100% throughout the 0.9.0 runs (about 20% for 0.7.0) |

Because the machine is shared, the bar is judged on the interleaved ratio harness (`benches/src/bin/ratios.rs`, D-106). For each pair it alternates form-net and the competitor in batches of about 4 ms, for 40 rounds, and each side keeps its fastest batch. Load from other work falls on both sides alike, and the minimum is the best estimate of the unloaded cost. The machine never went quiet for the 0.9.0 pass, so the harness ran five times and each table is the median of each cell over the five runs; every run is kept beside the medians. The ratios held: across the five runs one row crossed the bar once (near-miss in 1,500-byte chunks, 1.10x to 1.61x, median 1.46x). Absolute times under full load read about 1.5x their quiet values for both sides (multer on the login form: 2.24 µs in 0.7.0, 3.73 µs here), so the targets table below keeps the quiet 0.7.0 numbers beside the 0.9.0 ones, and the quiet numbers are the reference for absolute times. The criterion medians (`criterion.md`, one run) point the same way with more spread. Hardware counters were not measured: `perf stat` needs a Linux host with hardware events, which this host and its WSL kernel do not provide.

Fairness rules, all enforced in `benches/src/lib.rs` and checked by `benches/tests/agree.rs` (every driver returns the same total of value bytes):

- Every driver parses the whole body and touches every byte of every value.
- Competitors get the same limits where they take limits (multer `whole_stream` and `per_field`, multipart's `size_limit`); form-net never runs with its checks off.
- Every buffered driver parses the `Content-Type` header: form-net through `parse_multipart`, the others through `multer::parse_boundary` (the `mime` crate), as their framework integrations do.
- multer receives zero-copy `Bytes` slices of the body; nothing allocated by the harness is charged to it.
- In the disk family each side creates its temporary directory per request.

---

## Every comparison

form-net's fastest batch, the competitor's, and the ratio (competitor time over form-net time), each the median of five runs on the loaded machine. For urlencoded pairs there are three equal-work pairings per input: collecting the decoded pairs, iterating them without keeping any, and form-net's allocation-free `raw_pairs` + `decode_into` (which copies every byte into a caller buffer, more work than iterating borrowed pairs) against the same iteration.

| Family | Input | form-net | min | Competitor | min | Ratio | Bar |
|---|---|---|---|---|---|---|---|
| urlencoded pairs | login | pairs (collect) | 242 ns | form_urlencoded (collect) | 393 ns | 1.63x | pass |
| urlencoded pairs | login | pairs (no collect) | 197 ns | form_urlencoded (no collect) | 369 ns | 1.83x | pass |
| urlencoded pairs | login | raw_pairs+decode_into (no collect) | 126 ns | form_urlencoded (no collect) | 349 ns | 2.79x | pass |
| urlencoded pairs | search | pairs (collect) | 164 ns | form_urlencoded (collect) | 157 ns | 0.94x | below |
| urlencoded pairs | search | pairs (no collect) | 113 ns | form_urlencoded (no collect) | 119 ns | 1.05x | below |
| urlencoded pairs | search | raw_pairs+decode_into (no collect) | 80 ns | form_urlencoded (no collect) | 121 ns | 1.55x | pass |
| urlencoded pairs | signup | pairs (collect) | 602 ns | form_urlencoded (collect) | 1.09 µs | 1.88x | pass |
| urlencoded pairs | signup | pairs (no collect) | 504 ns | form_urlencoded (no collect) | 1.02 µs | 2.04x | pass |
| urlencoded pairs | signup | raw_pairs+decode_into (no collect) | 346 ns | form_urlencoded (no collect) | 1.01 µs | 2.93x | pass |
| urlencoded pairs | 1000 fields | pairs (collect) | 27.31 µs | form_urlencoded (collect) | 51.33 µs | 1.86x | pass |
| urlencoded pairs | 1000 fields | pairs (no collect) | 23.44 µs | form_urlencoded (no collect) | 47.49 µs | 2.06x | pass |
| urlencoded pairs | 1000 fields | raw_pairs+decode_into (no collect) | 32.76 µs | form_urlencoded (no collect) | 48.50 µs | 1.49x | pass |
| urlencoded pairs | 1 MiB plain | pairs (collect) | 146.12 µs | form_urlencoded (collect) | 1.49 ms | 10.01x | pass |
| urlencoded pairs | 1 MiB plain | pairs (no collect) | 143.24 µs | form_urlencoded (no collect) | 1.40 ms | 9.81x | pass |
| urlencoded pairs | 1 MiB plain | raw_pairs+decode_into (no collect) | 145.04 µs | form_urlencoded (no collect) | 1.39 ms | 9.68x | pass |
| urlencoded pairs | 1 MiB escaped | pairs (collect) | 800.85 µs | form_urlencoded (collect) | 1.71 ms | 2.13x | pass |
| urlencoded pairs | 1 MiB escaped | pairs (no collect) | 641.60 µs | form_urlencoded (no collect) | 1.56 ms | 2.41x | pass |
| urlencoded pairs | 1 MiB escaped | raw_pairs+decode_into (no collect) | 702.65 µs | form_urlencoded (no collect) | 1.49 ms | 2.13x | pass |
| urlencoded struct | signup | form! | 641 ns | serde_urlencoded | 1.17 µs | 1.82x | pass |
| urlencoded serialize | 10 pairs | serialize_urlencoded | 407 ns | form_urlencoded | 794 ns | 1.93x | pass |
| multipart stream | login, whole | MultipartParser | 1.11 µs | multer | 3.73 µs | 3.40x | pass |
| multipart stream | avatar 64 KiB, whole | MultipartParser | 3.64 µs | multer | 6.87 µs | 1.90x | pass |
| multipart stream | avatar 64 KiB, 65536 B chunks | MultipartParser | 3.73 µs | multer | 6.41 µs | 1.79x | pass |
| multipart stream | avatar 64 KiB, 1500 B chunks | MultipartParser | 5.15 µs | multer | 10.69 µs | 1.92x | pass |
| multipart stream | 100 parts, whole | MultipartParser | 25.61 µs | multer | 96.08 µs | 3.72x | pass |
| multipart stream | 100 parts, 1500 B chunks | MultipartParser | 26.05 µs | multer | 97.92 µs | 3.75x | pass |
| multipart stream | document 10 MiB, whole | MultipartParser | 863.98 µs | multer | 3.27 ms | 3.79x | pass |
| multipart stream | document 10 MiB, 65536 B chunks | MultipartParser | 909.70 µs | multer | 7.95 ms | 8.74x | pass |
| multipart stream | document 10 MiB, 1500 B chunks | MultipartParser | 1.22 ms | multer | 6.36 ms | 5.35x | pass |
| multipart stream | all CR 1 MiB, whole | MultipartParser | 31.08 µs | multer | 282.21 µs | 8.63x | pass |
| multipart stream | all CR 1 MiB, 65536 B chunks | MultipartParser | 34.60 µs | multer | 561.75 µs | 16.50x | pass |
| multipart stream | all CR 1 MiB, 1500 B chunks | MultipartParser | 75.70 µs | multer | 333.35 µs | 4.62x | pass |
| multipart stream | near-miss 1 MiB, whole | MultipartParser | 195.79 µs | multer | 266.65 µs | 1.39x | pass |
| multipart stream | near-miss 1 MiB, 65536 B chunks | MultipartParser | 202.56 µs | multer | 565.21 µs | 2.49x | pass |
| multipart stream | near-miss 1 MiB, 1500 B chunks | MultipartParser | 248.12 µs | multer | 353.07 µs | 1.46x | pass |
| multipart stream | near-miss in the middle 1 MiB, whole | MultipartParser | 174.19 µs | multer | 675.55 µs | 3.90x | pass |
| multipart stream | near-miss in the middle 1 MiB, 65536 B chunks | MultipartParser | 178.88 µs | multer | 927.20 µs | 4.94x | pass |
| multipart stream | near-miss in the middle 1 MiB, 1500 B chunks | MultipartParser | 218.82 µs | multer | 745.09 µs | 3.37x | pass |
| multipart buffered | login | parse_multipart | 1.36 µs | multipart 0.18 | 1.86 µs | 1.37x | pass |
| multipart buffered | avatar 64 KiB | parse_multipart | 4.04 µs | multipart 0.18 | 6.70 µs | 1.72x | pass |
| multipart buffered | batch 20 x 1 MiB | parse_multipart | 1.61 ms | multipart 0.18 | 4.11 ms | 2.53x | pass |
| multipart to disk | document 10 MiB | UploadSession + Store::temp | 7.19 ms | multipart 0.18 SaveBuilder | 17.08 ms | 2.37x | pass |

---

## The plan's targets (PLAN 11.1)

| Operation | Input | Target | 0.7.0, quiet | 0.9.0, loaded |
|---|---|---|---|---|
| `FormKind::detect` | browser `Content-Type` | under 100 ns | 114 ns | 185 ns |
| `raw_pairs` scan | 1 MiB, no escapes | at least 1.5 GB/s | 12.6 GB/s | 8.9 GB/s |
| `decode_into` | 1 MiB, every byte escaped | at least 700 MB/s | 3.9 GB/s | 1.6 GB/s |
| `parse_urlencoded` | 10 fields, 179 bytes | under 400 ns | 405 ns | 666 ns |
| `Signup::from_urlencoded` | 8 fields, 182 bytes | under 600 ns | 429 ns | 641 ns |
| `serialize_urlencoded` | 10 pairs | under 300 ns | 132 ns | 294 ns |
| `MultipartParser` | one 100 MiB file part, random bytes, 64 KiB chunks | at least 3 GB/s | 13.7 GB/s | 10.5 GB/s |
| `MultipartParser` | adversarial, 1 MiB all CR | at least 400 MB/s, linear | 52.9 GB/s | 31.3 GB/s |
| `MultipartParser` | adversarial, 1 MiB near-miss delimiters | at least 400 MB/s, linear | 8.7 GB/s | 5.0 GB/s |
| `MultipartParser` | 100 parts of 10 bytes | under 30 µs | 15.88 µs | 26.20 µs |
| `MultipartParser` | 360 B body, 3 text fields | under 1 µs | 669 ns | 1.06 µs |
| `parse_multipart` | 1 MiB, 2 files | within 10% of the raw parser | 21.66 µs against 21.46 µs raw (+1%) | 35.42 µs against 35.30 µs raw (+0%) |
| split overhead | 65921 B body in 1-byte chunks | linear, under 50 ns per call | 13.3 ns per byte | 24.0 ns per byte |
| idea 0.2: work factor | all CR against random data | add `max_work_factor` if 4x or more | 0.27x | 0.34x |
| idea 0.2: work factor | near-miss against random data | add `max_work_factor` if 4x or more | 1.64x | 2.23x |
| idea 0.2: work factor | near-miss in the middle against random data | add `max_work_factor` if 4x or more | 2.04x | 1.85x |
| idea 0.6: sniff window | 64 KiB upload, `sniff_bytes` 0 | measure | 17.12 µs | 28.35 µs |
| idea 0.6: sniff window | 64 KiB upload, `sniff_bytes` 64 | measure | 16.33 µs | 22.36 µs |
| idea 0.6: sniff window | 64 KiB upload, `sniff_bytes` 512 | measure | 16.40 µs | 25.67 µs |
| idea 0.8: refusal digest | `record`, form-net's own code | about 1 ns | 5 ns (0.8.0, 2026-11-05) | 5 ns |
| idea 0.8: refusal digest | `record`, the eighth named inspector code | about 1 ns | 6 ns (0.8.0, 2026-11-05) | 10 ns |
| idea 0.8: refusal digest | `record`, an unnamed inspector code | about 1 ns | 5 ns (0.8.0, 2026-11-05) | 6 ns |
| `TokenCheck` | login form, no inspector | measure | 257 ns (0.8.0, 2026-11-05) | 396 ns |
| `TokenCheck` | login form, the check (`verify` excluded) | within 5% of none | 268 ns (0.8.0, 2026-11-05) | 414 ns |

Every 0.9.0 absolute time reads high by the load, on both sides of the comparisons above; none moved against the competitors. Against the quiet 0.7.0 numbers, two targets were estimates that the measurement does not quite meet:

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

## What 0.9 changes

The polish pass (D-112) reran every family against the competitors at their current versions (unchanged since 0.7: multer 3.1.0, multipart 0.18.0, form_urlencoded 1.2.2, serde_urlencoded 0.7.1, serde_qs 1.1.3). Every family passes; allocation counts are the same, call for call. Six ratios read lower than the quiet 0.7.0 baseline. Four are within the spread of the five runs. The other two were measured against 0.8.0 on the same loaded machine, alternating the two builds four times (D-133):

- `serialize_urlencoded` of 10 pairs, 2.20x to 1.93x: 0.8.0 reads 1.62x to 1.98x under the same load, and the serializer has not changed since 0.6. Load, not code.
- `raw_pairs` + `decode_into` on 1 MiB of escapes, 2.88x to 2.13x: under the default bench profile 0.8.0 reads 2.34x to 2.50x and 0.9.0 2.03x to 2.24x, while `pairs` on the same body moves the other way (0.8.0 1.74x to 2.01x, 0.9.0 2.39x to 2.69x). Under the crate's release profile (fat LTO, one codegen unit) 0.9.0 reads 2.25x to 2.42x and 0.8.0 1.91x to 2.25x. This is code placement, the same effect as D-117. The escaped path does the same work as before; D-126 adds one flag test per component.

| Input | 0.7.0 | 0.9.0 | Why |
|---|---|---|---|
| 1,000 fields, `raw_pairs` + `decode_into` | 0.72x | 1.49x | a component with nothing to decode is borrowed, not copied (D-126) |
| search query, `raw_pairs` + `decode_into` | 0.91x | 1.55x | the same |
| search query, `pairs` iterating | 0.94x | 1.05x | within the spread of a loaded run; still level, see above |
| near-miss 1 MiB, whole | 1.23x | 1.39x | the margin that was thin; range over five runs 1.35x to 1.85x |
| `Form::get`, the last of 10 fields | 7 ns per field | 3.4 ns per field | names compared as bytes (D-127), from the `estimates` binary |

The 1 MiB escaped body still depends on code placement under the default bench profile (D-117, D-133); the crate's own release profile reads it steady. The text-check move (D-128) and the bounded arena (D-130) cost nothing measurable: the buffered family's login and avatar rows moved from 1.31x and 1.44x to 1.37x and 1.72x.

## What 0.8 changes

`Limits::DEFAULT.sniff_bytes` is 512 (D-122), so the buffered and to-disk drivers hold back the first 512 bytes of each file for the inspector. Measured again on 2026-11-05 on a loaded machine, both families still pass: buffered 1.69x to 2.82x, to disk 2.17x. The streaming families drive the parser directly and are unchanged. The refusal digest's `record` is 5 ns, over the 1 ns estimate: one locked add costs about that here. The 0.9 pass below takes over as the reference for ratios.

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
cargo run --release --bin estimates       # the API's cost estimates
cargo bench --bench urlencoded --bench multipart
python report.py "$CARGO_TARGET_DIR/criterion"   # criterion medians as a table
```

`FORM_NET_BENCH_BIG=1` adds the 100 MiB body to the criterion multipart run. A change that makes a ratio or a median more than 5% worse than the committed baseline needs its reason recorded in `dev/PLAN.md` before it merges; CI keeps the crate building and the drivers agreeing, and the numbers come from this machine, not from shared CI runners: ratios from the interleaved harness, which holds under load, and absolute times from a quiet run.
