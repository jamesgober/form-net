<h1 align="center">
    <img width="90px" height="auto" src="https://raw.githubusercontent.com/jamesgober/jamesgober/main/media/icons/hexagon-3.svg" alt="Triple Hexagon">
    <br><b>CHANGELOG</b>
</h1>
<p>
  All notable changes to <code>form-net</code> will be documented in this file. The format is based on <a href="https://keepachangelog.com/en/1.1.0/">Keep a Changelog</a>,
  and this project adheres to <a href="https://semver.org/spec/v2.0.0.html/">Semantic Versioning</a>.
</p>

---

## [Unreleased]

Hardening (0.7).

### Added

- A capture corpus (`tests/corpus`): the bodies Chrome 155, Edge 155,
  curl 8.5, Python requests 2.31, and Go 1.27 send for a fixed test page,
  with the tools that capture them (`tools/capture`), checked whole and at
  every split.
- A WAFFLED-style mutation generator: every mutation of every captured body
  is refused or reads exactly as the original.
- The 4-hour upload soak (`tests/soak.rs`, NFR-15).
- An attack test for every threat T-1 to T-38 (T-29 is the fuzzing itself).
- A multer differential (`benches`, `differential`).

### Changed

- `--boundary` after a lone CR or LF, or as the first bytes of a part's
  data, is refused as `BoundaryInContent`; the writer refuses such data
  too. Readers that accept lone line breaks before a delimiter saw more
  parts than form-net (D-113).
- An unquoted `name` or `filename` that starts or ends with `'` is refused
  as `MalformedParameter` (D-114).
- `name`, `filename`, and `filename*` written other than in lower case are
  refused as `MalformedParameter`: multer reads `NAME=` as no name (D-116).
- The multipart delimiter search anchors on a CR or LF followed by `-`, so
  one pass finds delimiters and bare breaks; matches across chunks are
  compared whole, and a miss resumes at the byte that differed (D-117).
- A new benchmark baseline (`benches/baseline/2026-10-30`), and
  `docs/PERFORMANCE.md` measured for 0.7.

---

## [0.6.0] - 2026-10-29

Optimisation: measured against the other Rust form parsers, and at least 20%
faster than the best of them on every benchmark of the plan but one input, a
25-byte search query where form-net and form_urlencoded are level.

### Added

- `benches/`, a benchmark crate in its own workspace (the competitors never
  enter form-net's dependency tree): criterion benchmarks of every family,
  an interleaved ratio harness for a shared machine, the PLAN 11.1 targets,
  allocation counts, and tests that every driver does the same work.
- `docs/PERFORMANCE.md`: results, method, machine, commands, and a
  capability comparison checked against each competitor's source.

### Changed

- The multipart delimiter search skips to the next CRLF 32 bytes at a time
  and compares the delimiter whole there; the automaton runs only near the
  end of a chunk. Random file data parses at about 13 to 29 GB/s and the
  adversarial shapes faster than random data.
- urlencoded: the scanner narrows its byte set as it learns, the decoder
  decodes escapes in place, plain names and values are cut from one UTF-8
  pass over the body, and the serializer writes escapes from a
  table.
- Wipes cover the bytes that were written rather than whole buffers, and
  growing buffers start small: a parser no longer spends most of a small
  form's time wiping 9.6 KiB it never used. Every written byte is still
  wiped; tests over caller buffers prove it.
- CI: a job keeps the benchmark crate building and its drivers agreeing.
- The multipart differential fuzz target's reference parser skips bodies
  over the default name, filename, part, and file limits; it had flagged
  form-net's correct refusal of a 263-byte name as a disagreement.

---

## [0.5.0] - 2026-10-26

Feature complete: every P0, P1, and P2 item of the design.

### Added

- `UrlencodedParser`, `UrlencodedStep`, `UrlencodedEvent`: streaming
  urlencoded over a caller scratch buffer; the same pairs and errors for
  every split of a body.
- The client side: `MultipartWriter` (feature `std`; `new` with a generated
  boundary under `random`), `encode_part_head` (no allocation), and
  `Boundary::generate`. Every body the writer produces parses back with
  form-net to the same fields; inputs that could not are refused before a
  byte is written.
- `SpillBudget` and `SpillQuota`, with `Store::quota`: a disk budget shared
  across requests and stores; the request that would cross it is refused
  with a 503 hint. `sweep` removes named-mode spill files left by a crash.
- `Limits::memory_bound` and `Store::memory_bound` for admission control,
  measured against hostile bodies.
- Feature `unicode`: `FileName::nfc`; `FileName::sanitized` normalizes to
  NFC first.
- Extraction: `FromForm::from_body_all` and `FormErrors` (every field's
  error, for showing a form again), `FromForm::from_upload_mut` (the upload
  stays usable), and `Upload::from_form` (one owned type for both
  encodings, with the anti-forgery token captured as for multipart).
- `docs/ARCHITECTURE.md` and `docs/PLATFORM-NOTES.md`; every example in
  `docs/API.md` now runs as a doctest.
- Example `build_multipart_client`.

### Changed

- A memory-held file's buffer is capped at the most it can hold, and the
  upload text arena at its text and name budgets.
- The CI security job audits the newest-version dependency resolution.

### Security

- Tests for T-7 (parallel uploads against a shared budget), T-11 (named
  mode plus `sweep` after a killed process), T-21 (NFD names), T-33 (1,000
  concurrent uploads within 1,000 times `memory_bound`), T-34 (header
  injection through the writer), and T-35 (delimiter in client data). A
  `loom` model of `SpillQuota` runs in CI.

---

## [0.4.0] - 2026-10-23

Upload storage, inspection hooks, blocking readers, and typed extraction.

### Added

- Inspection: the `Inspector` trait (`part_start`, `sniff`, `data`,
  `field`, `part_end`, `end`, `failed`) with `Verdict` and `Refusal`, called
  in a fixed order at every split; `parse_multipart_inspected` and
  `parse_urlencoded_inspected`; `NoInspector`.
- Storage hooks: the `Sink` trait, `DiscardSink`, `MemorySink`, and the
  `Driver`, which runs a `MultipartParser`, an inspector, and a sink and
  guarantees that `part_start` and `sniff` come before any sink write.
  `OwnedPartHead` keeps a part head past the parser's next call. The
  driver opens the sink only after the inspector has accepted the part, so
  a refused part never reaches storage.
- Inspector chains: `(A, B)` runs two inspectors in one pass (the first
  refusal wins) and `Box<I>` holds one chosen at run time.
- Storage (feature `fs`): `Store` (`temp`, `memory_only`, `in_dir`,
  `memory_threshold`, `max_disk_bytes`, `mode`, `stats`), `StoredFile`
  (positional `reader`, `read_to_vec`, `persist` that never overwrites),
  `Upload`, `UploadSession` for pushed chunks, and `read_multipart` for any
  `Read`. Spill files live in a private directory (`form-net-` plus 128
  random bits, 0700) with random names, `create_new`, and 0600; in the default
  anonymous mode they have no name (unlinked on Unix, delete-on-close on
  Windows) and cannot outlive the process. Every file is deleted on drop, on
  abort, and on any parse error.
- `TokenField` with `BeforeFirstFile` enforced through the driver: a file
  before the token is refused at its first header, before the sink opens;
  `UploadSession::captured_token` gives the token as soon as its part ends.
- Blocking readers (feature `std`): `MultipartReader` and `PartReader`
  (`text`, `skip`, `save`, `Read`).
- Typed extraction: `form!`, `FromForm` (`from_urlencoded`, `from_query`,
  `from_multipart`, `from_body`, `from_upload`, and the `_with` and
  `_inspected` forms), `FromField` for strings, integers (strict), finite
  floats, `bool` (checkbox), `char`, `Option`, `Vec`, `Secret`, `FileData`,
  and `StoredFile`. Unknown, repeated, missing, and invalid fields are
  refused; errors name the field and never echo input.
- `FileName::sanitized`: one portable name segment through
  `path_lib::sanitize`, with Unicode default-ignorable characters replaced.
- Examples `upload_to_disk`, `upload_session_push`, `typed_form`,
  `content_inspector`, `token_first`.

### Changed

- Owned values are wiped with crypt-io's volatile `wipe` (whole allocation)
  instead of `fill(0)` with `black_box`; growing buffers wipe the old
  allocation after each move. This brings `zeroize` and `subtle` under
  `alloc`, the audited-cryptography exception.

### Security

- Each 0.4 threat in `dev/PLAN.md` has its test: disk limits (T-7), abort at
  every byte of a 3-file body leaves no file or handle (T-8), planted files
  and links never opened (T-9), owner-only permissions (T-10), a killed
  process leaves no anonymous file (T-11), file names never paths (T-20,
  T-21), sniffing sees the real bytes (T-22), the token rules (T-23, T-24),
  hidden content refused (T-25 to T-27), wiping on drop (T-31), and the hook
  order at every split (T-38).

---

## [0.3.0] - 2026-10-23

The core: urlencoded and multipart parsing, on parse-net's grammar.

### Added

- `FormKind::detect`: form kind and boundary from a request's
  `Content-Type`; `text/plain` and every other type are refused.
- urlencoded: `parse_urlencoded`, `parse_query`, `parse_urlencoded_with`,
  the owned `Form` (two allocations, wiped on drop, values never in `Debug`),
  lazy `pairs`, and the allocation-free `raw_pairs` and `decode_into`. `&` is
  the only separator; a malformed `%` is refused by default
  (`PercentPolicy::Literal` keeps it, as WHATWG does); invalid UTF-8 is
  refused under every policy.
- `serialize_urlencoded`, `Serializer`, and `encode_into`, byte-identical to
  the WHATWG serializer for UTF-8.
- `MultipartParser`: a sans-IO parser that takes chunks of any size, never
  allocates after construction (`with_buffer` never at all), never rescans
  a byte (streaming Knuth-Morris-Pratt with an eight-byte skip to the next
  CR), and gives identical events for every split. `skip_part`, `reset`,
  token capture (`TokenField`, placed before the first file by default),
  `_charset_` capture.
- `parse_multipart` and `parse_multipart_with` for buffered bodies: parts
  borrow the body; text fields are checked as text at parse time.
- `Boundary`, `FileName` views (`raw`, `as_str`, `basename`,
  `html_unescape_into`, `html_unescaped`), `PartHead`, `ContentType`,
  `Headers`, `PartSummary`.
- `Limits` with every budget of the design and buffer sizes as `const fn`s
  (`multipart_buffer_len`, `urlencoded_buffer_len`); `Policy` with
  `PercentPolicy`, `ExtendedFilenamePolicy`, `HeaderPolicy`,
  `EmptyFilePolicy`, `TokenField`.
- `FormError` with `status_hint`, stable `code`s, and `location`; `Refusal`.
- Tests: unit tests on every path, WHATWG urlencoded cases, every
  multipart refusal, every limit at n and n + 1, split invariance at every
  byte against a reference parser, property tests, counting-allocator tests,
  attack tests for the threats the core covers, a matcher visit count that
  proves the delimiter search linear on adversarial bodies.
- Eight fuzz targets (detection, scanning, decoding, boundaries, multipart
  whole and split, a differential against the reference parser, part
  headers, file names) and a CI job that runs each for 60 seconds.
- CI: a bare-metal `thumbv7em-none-eabihf` build without `std` or `alloc`;
  a check that no third-party crate enters the runtime tree; docs in both
  feature modes.
- Examples: `urlencoded_basic`, `query_string`, `multipart_buffered`,
  `multipart_sans_io`, `no_std_core`.

### Changed

- `max_name_bytes` is capped by `max_body_bytes` (D-074); the parser is 384
  bytes plus its buffer, measured (D-075); at most 16 parameters per header
  (D-076); RFC 2231 starred parameters other than `filename*`, and a
  backslash in a part name, are refused (D-081, D-082). Decisions D-074 to
  D-082 in `dev/PLAN.md`.
- Default features are `["std"]`; `alloc` and `std` are features.

---

## [0.2.0] - 2026-10-23

The design lock: the public API from `0.3.0` to `1.0.0`, written down before
the code. No parsing code yet.

### Added

- `docs/API.md`: every public item (#1 to #73) with its grade, tier, version,
  feature, signature, behaviour, errors, cost target, and examples:
  detection, urlencoded parsing and serializing (buffered, borrowed, owned,
  streaming), buffered and streaming multipart, file name views, `Limits` and
  `Policy`, inspection and storage hooks, temporary-file storage with a
  pluggable disk budget, blocking readers, typed extraction without serde
  (`form!`, `FromForm`, `FromField`, `Secret`), the multipart writer, and the
  error type with its status hints and stable codes. It closes with checklists
  for web-net, customs-sec, csrf-sec, and outbound clients.
- `dev/PLAN.md` rewritten from the specification: requirements FR-1 to FR-67
  and NFR-1 to NFR-18, the enterprise and distributed section, architecture,
  the urlencoded and multipart engines, budgets and the worst-case memory
  formula, storage per platform, threats T-1 to T-38 with defences, tests, and
  costs, performance targets and competitors, the testing plan, and one
  defence idea per version.
- Decisions D-004 to D-073 in the decision log: every open question of the
  specification is decided (the serde adapter stays open until after 1.0),
  and the changes made at the lock are recorded with their reasons.

### Changed

- `dev/ROADMAP.md` covers every version from 0.1 to 1.0 with its items and
  exit criteria.
- `filename*` in multipart is refused by default, with an opt-in to accept it
  when it is the only file name; the plan said RFC 5987, which RFC 8187
  replaced.
- The Tier 3 core is planned to need no `alloc` at all, stronger than the
  0.1 plan's `no_std` with `alloc`.
- README: scope, planned features, quick start, status; plain hyphens instead
  of `&mdash;` entities, also in `dev/DIRECTIVES.md`.
- The crate description names upload spilling and typed extraction and says
  "no third-party dependencies" (first-party crates are used from 0.3).

---

## [0.1.0] - 2026-10-13

The scaffold and the plan. No functional code yet.

### Added

- Crate manifest (edition 2024, MSRV 1.85, no dependencies), `Apache-2.0 OR MIT`
  license, README, REPS, CI (Linux, macOS, Windows on stable and 1.85; audit
  and deny), `deny.toml`, `clippy.toml`, `rustfmt.toml`, `.gitattributes`.
- `dev/PLAN.md` (purpose, scope, architecture, threats, ecosystem, open
  decisions, decision log), `dev/DIRECTIVES.md`, and `dev/ROADMAP.md`.

[Unreleased]: https://github.com/jamesgober/form-net/compare/v0.6.0...HEAD
[0.6.0]: https://github.com/jamesgober/form-net/compare/v0.5.0...v0.6.0
[0.5.0]: https://github.com/jamesgober/form-net/compare/v0.4.0...v0.5.0
[0.4.0]: https://github.com/jamesgober/form-net/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/jamesgober/form-net/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/jamesgober/form-net/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/jamesgober/form-net/releases/tag/v0.1.0
