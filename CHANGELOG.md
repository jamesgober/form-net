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

[Unreleased]: https://github.com/jamesgober/form-net/compare/v0.4.0...HEAD
[0.4.0]: https://github.com/jamesgober/form-net/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/jamesgober/form-net/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/jamesgober/form-net/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/jamesgober/form-net/releases/tag/v0.1.0
