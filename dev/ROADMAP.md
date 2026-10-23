# form-net - Roadmap

> Path from scaffold to a stable 1.0. Hard parts are front-loaded; each version has hard exit criteria, and every listed item is required. Item numbers (`#7`) are defined in [`../docs/API.md`](../docs/API.md#item-index); requirement ids (FR, NFR) and threat ids (T) in [`PLAN.md`](./PLAN.md).
>
> **Anti-deferral rule:** no listed item moves to a later version unless this file records the move and the reason.
>
> **Release routine after every version:** finish the version (every item done, version bumped everywhere it appears, `CHANGELOG.md` rolled, `docs/release/vX.Y.Z.md` written, README and `docs/API.md` checked against the build); local gate on Windows and WSL, stable and 1.85 (fmt, clippy in three feature modes, tests in three modes, docs with default and all features, `cargo package` in a fresh target directory); commit and push on a `feature/` branch with a pull request; wait for green CI on every platform; squash-merge; tag `vX.Y.Z`; GitHub release `vX.Y.Z - Name` from the release notes (pre-release for 0.x); `cargo doc` clean in both feature modes; `cargo publish`; verify crates.io and docs.rs.

## v0.1.0 - Scaffold (DONE, 2026-10-13)

Exit: CI green on every platform.

- [x] Repository in the portfolio layout; `Cargo.toml` (edition 2024, MSRV 1.85, no dependencies); `VERSION` (#1).
- [x] CI: Linux, macOS, Windows on stable and 1.85; audit and deny.
- [x] `dev/PLAN.md`, `dev/DIRECTIVES.md`, this file; README, CHANGELOG, `docs/API.md`, `docs/release/v0.1.0.md`.
- Not published to crates.io; the name is claimed by 0.2.0 (D-005).

## v0.2.0 - Design lock (DONE, 2026-10-23)

Exit: James approves `docs/API.md`; every open decision is decided or dated.

- [x] `docs/API.md` written for every item #1 to #73: signature, behaviour, errors, cost, and examples (not compiled until each item ships).
- [x] Independent review of the documents; its 22 findings fixed (D-060 to D-073).
- [x] `dev/PLAN.md` rewritten from the specification: requirements, enterprise and distributed (4.3), architecture, engines, budgets, storage, threats T-1 to T-38, performance targets, testing, ideas, and decisions D-004 to D-073 (O-1 to O-32 decided except O-19, dated after 1.0).
- [x] This file rewritten to the full 0.1 to 1.0 series (D-035).
- [x] parse-net's API checked against form-net's needs: media types, parameters, `Quoted::raw`, `Content-Disposition`, RFC 8187 values are all in parse-net 0.6.1 (D-006).
- [x] README updated: scope, planned API, status; the `&mdash;` entities replaced.
- [x] James approves `docs/API.md` (2026-10-23, with the go-ahead to release).

## v0.3.0 - Core (DONE, 2026-10-23)

Exit: everything below green on the full CI matrix, including the bare-metal `no_std` build; clippy's panic lints on (`unwrap_used`, `expect_used`, `panic`, `indexing_slicing`, `arithmetic_side_effects`) with no exception in `src/`.

- [x] Wire parse-net (`version = "0.6"`, `default-features = false`); features `alloc`, `std` (default); `#![no_std]` crate root.
- [x] Errors and configuration: #18, #24, #26 to #31, #57, #58, #65, #72, #73.
- [x] Detection: #2.
- [x] urlencoded: #3 to #7, #32, #41, #44 to #46.
- [x] multipart: #8, #9, #33, #48 to #51, #69; file name views #19, #20.
- [x] Tests: unit tests on every path; WHATWG urlencoded cases and RFC 7578 / RFC 2046 examples as data, whole and split at every byte; property tests (no panic, split invariance, budgets hold, serializer round trip); counting-allocator tests for every no-allocation claim; the reference-parser differential; attack tests for T-1 to T-6, T-12 to T-14, T-16, T-17, T-19, T-28, T-30, T-32, T-36, T-37; parser size assertion (NFR-18); static `Send`/`Sync` assertions.
- [x] Fuzz targets `fuzz_detect`, `fuzz_raw_pairs`, `fuzz_decode`, `fuzz_boundary`, `fuzz_multipart`, `fuzz_multipart_differential`, `fuzz_part_headers`, `fuzz_filename`, each run in CI for 60 seconds.
- [x] CI: `no_std` job (`thumbv7em-none-eabihf`, `--no-default-features`), fuzz smoke job, docs in both feature modes.
- [x] Defence idea recorded and measured: state-2 priming (PLAN 14).
- [x] Examples: `urlencoded_basic`, `query_string`, `multipart_buffered`, `multipart_sans_io`, `no_std_core`.

## v0.4.0 - Extended (DONE 2026-10-23)

Exit: storage, crash, and abort tests green on Linux, macOS, and Windows; the hook order test green at every split; the token tests green; customs-sec and web-net maintainers have reviewed the `Inspector` and `FromForm` shapes against their plans.

- [x] Wire path-lib and mod-rand; features `random`, `fs`. Wire crypt-io for the volatile wipe (D-083).
- [x] Hooks: #52 to #54, #63, #64, #66.
- [x] Storage: #10 to #13, #21, #35 to #38, #68, #70.
- [x] Blocking readers: #34.
- [x] Typed extraction: #14 to #17, #56, #61, #62.
- [x] File names: #22 (default-ignorable characters replaced, D-084).
- [x] Tests: `store.rs` (one file with per-platform cases: permissions, flags, anonymous and named modes, persist, files and links refused as directories); `crash.rs` (kill a child process mid-upload); `abort.rs` (abort at every byte of a 3-file body, then count files, and handles on Linux); `inspector_order.rs`; token tests in `attacks.rs` and `attacks_upload.rs`; `extract.rs` (value and arity rules, 0 and 64 fields); attack tests T-7 to T-11 (the shared budget part of T-7 and `sweep` part of T-11 with their 0.5 items), T-20 to T-27, T-29 (fuzz), T-31 (D-085), T-38. T-33 moves to 0.5 with `memory_bound` (D-087).
- [x] Fuzz targets `fuzz_extract` and `fuzz_upload_session`.
- [x] Defence idea: token before the first file (PLAN 14).
- [x] Examples: `upload_to_disk`, `upload_session_push`, `typed_form`, `content_inspector`, `token_first`.
- [x] Exit review of the `Inspector` and `FromForm` shapes against the customs-sec and web-net plans (both are scaffolds without form-net code yet): no blocker; inspector chaining (D-088) and lazy sink open (D-089) done in 0.4, Send and Sync asserted for every storage type, three additive shapes scheduled for 0.5 (D-090).

## v0.5.0 - Feature complete

Exit: every P0, P1, and P2 item exists, is documented with runnable examples, and is tested; the writer round-trip property is green.

- [ ] Wire unicode-lang; feature `unicode`.
- [ ] #23, #25, #39, #40, #42, #43, #47, #55, #67, #71.
- [ ] `loom` model of `SpillQuota` (three threads: never over the limit, never negative, no lost release) and a CI job for it.
- [ ] Tests: `writer.rs`, `memory_bound.rs` (peak-tracking allocator), `sweep` cases, attack tests T-7 (shared budget, parallel uploads), T-11 (named mode plus `sweep`), T-21 (NFD names with `unicode`), T-33, T-34, T-35.
- [ ] Fuzz targets `fuzz_urlencoded_stream`, `fuzz_writer_roundtrip`.
- [ ] `docs/ARCHITECTURE.md`, `docs/PLATFORM-NOTES.md`; `docs/API.md` complete with runnable examples.
- [ ] Defence idea: a budget per tenant (PLAN 14).
- [ ] Shapes from the 0.4 consumer review (D-090): `FromForm::from_upload_mut`, all-errors extraction, one owned accessor type for both encodings.
- [ ] Example: `build_multipart_client`.

## v0.6.0 - Optimisation

Exit: numbers published; no hot-path allocation; adversarial benchmarks linear; each family at least 20% faster than its best Rust competitor at equal or stronger limits, or this file records which family missed, why, and James's decision.

- [ ] Benchmark crate with every family of PLAN 11.2 against the competitors; baselines committed; a CI job keeps it building.
- [ ] The parity matrix's unverified cells checked against competitor source code before any comparison is published.
- [ ] Optimise against the measurements; record each change with its numbers.
- [ ] `docs/PERFORMANCE.md`: results, method, machine, commands.
- [ ] The work-budget idea measured (PLAN 14, 0.2) and the first-bytes fingerprint idea (0.6).

## v0.7.0 - Hardening

Exit: every threat has a passing attack test; the soak shows no growth; the differential log has no open item.

- [ ] Every fuzz target run 24 hours with no crash and no budget violation.
- [ ] WAFFLED-style mutation corpus and generator (PLAN 12); multer differential triaged.
- [ ] Attack tests T-1 to T-38 complete.
- [ ] 4-hour soak (NFR-15).
- [ ] Browser capture corpus (Chrome, Firefox, Safari) refreshed and green (NFR-14); client captures (curl, Python `requests`, Go).
- [ ] `docs/SECURITY.md` complete.

## v0.8.0 - Alpha

Exit: the first site's form routes run on form-net in a test deployment; findings fixed test-first.

- [ ] web-net uses `FormKind`, `FromForm::from_body*`, `UploadSession`, and `form!` end to end on the login and upload routes; customs-sec runs as an `Inspector`; csrf-sec verifies the captured token.
- [ ] API changes from that use are made now (the last chance before beta), each with a decision-log entry.
- [ ] Defence idea: refusal digest (PLAN 14).

## v0.9.0 - Beta and release candidates

Exit: one release candidate with no change for two weeks of use by web-net and customs-sec.

- [ ] API freeze candidates `0.9.x`; fixes only.
- [ ] Adversarial review (DIRECTIVES definition of done, item 10) with every finding fixed test-first.
- [ ] `docs/STABILITY-1.0.md` drafted.
- [ ] Defence idea: high-byte refusal outside `filename` checked against the browser corpus (PLAN 14).

## v1.0.0 - Stable

Exit: the release routine, after James's explicit go.

- [ ] Full audit recorded in `docs/STABILITY-1.0.md`: API, docs, tests, benchmarks, security.
- [ ] web-net and customs-sec have used the API end to end.
- [ ] API frozen until 2.0.

## After 1.0

- #59 `NamePath` (P3).
- #60 serde adapter (P3), only if a real user needs it (O-19).
- A `nonblocking` example driving `UploadSession` from a reactor.
