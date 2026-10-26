# form-net - Project Plan

> The design, strategy, and decision record for `form-net`. A living document: every design decision, idea, or open question about form-net is written here or in [`ROADMAP.md`](./ROADMAP.md) before it is acted on. The public API is specified item by item in [`../docs/API.md`](../docs/API.md); this file says why it has that shape.
>
> **Read order:** [`../REPS.md`](../REPS.md) &rarr; [`DIRECTIVES.md`](./DIRECTIVES.md) &rarr; **this file** &rarr; [`ROADMAP.md`](./ROADMAP.md) &rarr; [`../docs/API.md`](../docs/API.md).
>
> **Status:** design lock, `0.2.0`. The API is fixed for `0.3.0` to `1.0.0`; a change needs an entry in the [decision log](#16-decision-log).

---

## Contents

1. [Purpose](#1-purpose)
2. [Where it sits](#2-where-it-sits)
3. [Scope](#3-scope)
4. [Requirements](#4-requirements)
5. [Standards and exceptions](#5-standards-and-exceptions)
6. [Architecture](#6-architecture)
7. [The engines](#7-the-engines)
8. [Budgets and memory](#8-budgets-and-memory)
9. [Storage](#9-storage)
10. [Threats](#10-threats)
11. [Performance](#11-performance)
12. [Testing](#12-testing)
13. [Documentation](#13-documentation)
14. [Ideas](#14-ideas)
15. [Open decisions](#15-open-decisions)
16. [Decision log](#16-decision-log)

---

## 1. Purpose

form-net parses what a client sends when an HTML form is submitted, and builds the same formats for clients:

- `application/x-www-form-urlencoded` bodies and query strings, decoded by the WHATWG URL standard's algorithm, with strict refusal where the standard would silently repair;
- `multipart/form-data` (RFC 7578, framed by RFC 2046), parsed incrementally: bytes go in as they arrive, in chunks of any size split at any byte, and fields and file parts come out as they complete;
- uploads kept in memory or spilled to private, randomly named temporary files that are removed on drop, on abort, and (in the default mode) by the operating system on a crash;
- typed extraction into plain Rust structs without serde, through two small traits and one declarative macro;
- hooks so a content inspector (customs-sec) can check every part before it is stored, and so an anti-forgery verifier (csrf-sec) can check the token before any file reaches the disk.

Every budget (bytes, counts, lengths) is checked before the bytes it governs are buffered. Input that two reasonable parsers could read differently is refused with a precise error, never repaired.

### 1.1 What it is not

| Not this | Where it lives |
|---|---|
| an HTTP parser | `http-net` parses the request head and frames the body; form-net receives body bytes and the `Content-Type` value |
| a socket reader or async adapter | sans-IO by design (D-001); transport is net-io and serve-net |
| a validator of field meaning (email shape, password rules, allowed file types) | customs-sec contracts and the application |
| a file-type detector | customs-sec; form-net hands it the first bytes of each file |
| a decompressor | `Content-Encoding` is undone before form-net sees the body; archives inside uploads are never opened |
| a generic MIME library | email multipart belongs to smtp-net and parse-net; form-net handles `multipart/form-data` only |
| a URL or general percent-encoding library | url-lib (planned) parses URL structure and hands form-net the raw query (D-007) |
| a charset converter | UTF-8 only (D-026) |

---

## 2. Where it sits

```
bytes ─▶ net-io ─▶ http-net (head, body framing) ─▶ server request context
       ─▶ customs-sec C1 (content type allowed, declared length within bound)
       ─▶ authentication, routing, authorization
       ─▶ csrf-sec (header token, or the form field form-net captures)
       ─▶ handler: body read on first access
             ├─ urlencoded (small, buffered): form-net parses zero-copy ─▶ customs-sec C2 as an Inspector
             │      ─▶ typed struct (FromForm) ─▶ handler
             └─ multipart (streamed): MultipartParser ─▶ Inspector (customs-sec C2, token capture)
                    ─▶ Sink (memory or spill) ─▶ Upload or typed struct ─▶ handler
       ─▶ ok: 303 See Other      invalid: 422 with the same form re-rendered
```

form-net decides whether the bytes are a well-formed form. customs-sec decides whether the form is acceptable for the route. csrf-sec decides whether the request came from the site's own page. The handler decides what the form means.

### 2.1 Who uses it

| User | What it uses | From |
|---|---|---|
| `web-net` (framework) | `FormKind::detect`, `FromForm::from_body*`, `form!`, `UploadSession`, `Store`, `status_hint` | 0.4 |
| `customs-sec` (body inspection) | `Inspector`, `PartHead`, `Limits` as the shared budget model, `FormError::code` | 0.4 |
| `csrf-sec` integration | `TokenField` capture: the token is available before the first file part starts | 0.4 |
| application handlers | `Signup::from_body(content_type, body)` | 0.4 |
| outbound clients, webhooks | `serialize_urlencoded`, `MultipartWriter` | 0.5 |
| `url-lib` (planned) | possibly `raw_pairs` for query strings (D-007) | 0.5 |

### 2.2 What it uses

| Crate | Version | For | Feature |
|---|---|---|---|
| `parse-net` | 0.6 (published) | media type and parameter grammar, quoted strings (read with `Quoted::raw`), `Content-Disposition`, RFC 8187 values | always, `default-features = false` (`no_std`, no `alloc`) |
| `path-lib` | 1.0 (published) | `path_lib::sanitize` for `FileName::sanitized` | `fs` |
| `mod-rand` | 1.0 (published) | `tier3::fill_bytes` (OS CSPRNG) for temporary names and generated boundaries | `random` (implied by `fs`) |
| `crypt-io` | 1.2 (published) | `wipe` and `wipe_vec`, volatile zeroing of owned values (D-083); brings `zeroize` and `subtle`, the audited-cryptography exception | `alloc` |
| `unicode-lang` | 1.0 (published) | `normalize(.., Form::Nfc)` for `FileName::nfc` | `unicode` |

Each is first-party, has no third-party runtime dependency in the features form-net enables (crypt-io's `zeroize` and `subtle` are the audited-cryptography exception, D-083), and is wired in the version whose code first uses it (parse-net at 0.3, path-lib and mod-rand at 0.4, unicode-lang at 0.5).

### 2.3 Load and capacity

Parsing is CPU-light and bounded. Spilling to disk is blocking file I/O: an event-driven server runs a handler that stores uploads where blocking is allowed, or gives the driver a custom `Sink` that hands chunks to a worker pool. `Limits::memory_bound` gives admission control a worst-case memory figure per request. The spill budget (`SpillBudget`) refuses cleanly with a 503-class hint instead of filling the disk; one budget per tenant gives each tenant its share.

---

## 3. Scope

### 3.1 In scope

1. Form kind detection from the `Content-Type` value.
2. urlencoded parsing: buffered (zero-copy, borrowed), owned, and streaming; query strings with the same grammar.
3. urlencoded serialization, byte-identical to the WHATWG serializer for UTF-8.
4. `multipart/form-data` boundary extraction and validation.
5. Streaming `multipart/form-data` parsing: delimiters, transport padding, preamble, epilogue, part headers, `Content-Disposition`, part `Content-Type`, `charset`, `_charset_`, `Content-Transfer-Encoding` refusal.
6. Budgets for every dimension, checked before buffering, with a worst-case memory formula.
7. File names: raw access, UTF-8 view, base name, HTML-escape decoding, sanitized portable name (path-lib), NFC (unicode-lang).
8. Storage: memory-only mode, spill to temporary files above a threshold, secure creation, cleanup on drop, abort, and crash; per-request disk limit; shared disk budget; sweep for named mode.
9. Inspection hooks (`Inspector`) and storage hooks (`Sink`).
10. Anti-forgery token field capture, with a "token before first file" policy.
11. Typed extraction without serde (`FromForm`, `FromField`, `form!`).
12. Client side: urlencoded serializer and a multipart body writer.
13. `std::io::Read` adapters for blocking callers.

### 3.2 Out of scope

| Out of scope | Lives in |
|---|---|
| HTTP head parsing, chunked decoding, `Content-Length` framing | http-net |
| socket reads, timeouts, slow-upload defence | net-io, serve-net (form-net has no clock) |
| request `Content-Encoding`, decompression bombs | the HTTP layer and customs-sec |
| magic-byte file type detection, allowed types per route | customs-sec (form-net supplies the sniff window) |
| field schemas at route level | customs-sec contracts (`FromForm` is the handler-local typed view) |
| joining paths for saving uploads | path-lib `join` (`persist` takes a full destination the caller built) |
| CSRF token format, verification, keys | csrf-sec (form-net only finds the field) |
| confusable and mixed-script detection | customs-sec |
| virus scanning, image re-encoding | the application, as a background job |
| resumable uploads (tus) | a later crate |
| `text/plain` form bodies | not parsed (D-013) |
| nested `multipart/mixed` inside a part | refused (D-012) |
| charsets other than UTF-8 and ASCII | not supported (D-026) |
| bracket-notation nesting into trees | not by default (D-011); opt-in `NamePath` after 1.0 |
| serde | optional adapter after 1.0 only if a real user needs it (O-19) |
| base64 and quoted-printable part bodies | refused (D-017) |

---

## 4. Requirements

"Refuse" means: return an error, emit no further events, leave the parser poisoned until `reset`, and abort every open sink. Each requirement is proved by the test family named in section 12.

### 4.1 Functional

**Detection.** FR-1 `FormKind::detect` accepts `application/x-www-form-urlencoded` (a `charset` other than UTF-8 or US-ASCII refused) and `multipart/form-data` (boundary required), ASCII case-insensitively; everything else is `UnsupportedMediaType`. FR-2 Detection is a pure function of the header bytes.

**urlencoded.** FR-3 WHATWG URL section 5.1: split on `&` only, skip empty sequences, split at the first `=`, no `=` means an empty value, `+` is space, percent-decode, UTF-8 decode. FR-4 `;` is an ordinary byte. FR-5 A malformed `%` is refused under `PercentPolicy::Refuse` and kept under `Literal`. FR-6 Decoded bytes must be valid UTF-8, or the pair is refused; never replaced. FR-7 Order and duplicates are preserved. FR-8 A component with no `%` and no `+` is borrowed. FR-9 The Tier 3 decoder writes into a caller buffer, never allocates, never grows the text. FR-10 The raw view tells `a` from `a=`. FR-11 The streaming parser gives the same pairs for any split. FR-12 The serializer matches WHATWG for UTF-8 (alphanumerics and `*-._` kept, space as `+`, uppercase hex). FR-13 `parse_query` strips exactly one leading `?`.

**Boundary.** FR-14 Exactly one `boundary` parameter, token or quoted. FR-15 1 to 70 `bchars`, not ending in a space. FR-16 Body comparison is exact and case-sensitive.

**Framing.** FR-17 The first delimiter is `--boundary` at offset 0 or `CRLF--boundary` after a preamble within `max_preamble_bytes`. FR-18 After `--boundary`: `--`, or padding (SP, HTAB, at most `max_padding_bytes`) then CRLF; any other byte after a full delimiter is `BoundaryInContent`. FR-19 The CRLF before a delimiter belongs to the delimiter. FR-20 Bare LF where CRLF is required is refused. FR-21 At most `max_epilogue_bytes` after the close delimiter, counted with constant work per chunk. FR-22 End of input before the close delimiter is `Truncated`, and every sink is aborted. FR-23 The delimiter search does constant amortized work per byte and never rescans, however the input is split.

**Part headers.** FR-24 The header block ends at CRLF CRLF, may span chunks, is bounded by `max_part_header_bytes` and `max_part_headers`. FR-25 `name ":" OWS value OWS CRLF`; token names; no whitespace before the colon; no folding; no control bytes except HTAB; 0x80 to 0xFF allowed in values. FR-26 `Content-Disposition` exactly once, type `form-data`, `name` exactly once and not empty, `filename` at most once, no repeated parameter, unknown parameters ignored. FR-27 `filename*` follows `ExtendedFilenamePolicy` (default `Refuse`, opt-in `AcceptAlone`); both file name parameters are refused under every policy. FR-28 `Content-Type` at most once, parsed, `multipart/*` refused, `charset` recorded. FR-29 `Content-Transfer-Encoding` absent or `7bit`, `8bit`, `binary`; anything else refused. FR-30 Other headers ignored under `HeaderPolicy::Ignore`, refused under `Refuse`, counted either way, visible through `PartHead::headers`. FR-31 Names have `%0A`, `%0D`, `%22` decoded and nothing else; file names are raw with an unescaped view.

**Parts.** FR-32 A part with `filename` (or an accepted `filename*`) is a file part, even with an empty name; otherwise a text field; text is bound by `max_value_bytes` and `max_text_bytes`, files by `max_file_bytes`. FR-33 An empty file input is a file part with an empty file name; under `EmptyFilePolicy::Skip` drivers produce nothing for it and it does not count against `max_files`. FR-34 Text values are bytes always and `&str` only when valid UTF-8 under a UTF-8 or US-ASCII charset (the part's, else `_charset_`, else UTF-8). FR-35 A `_charset_` field governs the text fields after it. FR-36 The declared part `Content-Type` is reported as a hint and never acted on, except the nested-multipart refusal.

**Budgets.** FR-37 Every limit is checked before the bytes it governs are buffered, stored, or passed to a sink. FR-38 A limit of 0 means none allowed. FR-39 Containing limits cap contained ones, and `Limits::validate` refuses at startup the values no body could satisfy (D-060). FR-40 `Limits::memory_bound` is never exceeded by the observed peak.

**Sans-IO parser.** FR-41 `parse(input, eof)` consumes a prefix and returns one event and the bytes consumed; the caller never re-sends consumed bytes. FR-42 Events (with `Data` concatenated per part) are identical for every split. FR-43 No allocation after construction; `with_buffer` never allocates. FR-44 Poisoned after any error until `reset`.

**Storage.** FR-45 `Store::memory_only` never touches the file system. FR-46 `Store::temp` spills a file when it would pass `memory_threshold`. FR-47 Temporary files live in a private directory the store created (0700), have 128 random bits in their names, are created with `create_new`, 0600. FR-48 `SpillMode::Anonymous` files have no name after creation on Unix and delete-on-close on Windows. FR-49 `SpillMode::Named` files are deleted on drop without `persist`, and `sweep` removes leftovers. FR-50 On any refusal every file of the request is closed and then deleted before the error is returned. FR-51 `persist` never overwrites. FR-52 The per-request disk limit and the shared budget are reserved before bytes are written.

**File names.** FR-53 A client file name is never used as a path or part of one. FR-54 `FileName` offers raw bytes, UTF-8, base name, HTML-unescaped, sanitized, NFC, and an extension hint.

**Hooks.** FR-55 Drivers call `part_start` before any byte reaches a sink, `sniff` with the first `min(sniff_bytes, length)` bytes before any data, `data` for every byte, `field` once per text field, `part_end` and `end` in order, and `failed` once on any error. FR-56 `Refuse` stops parsing, aborts sinks, returns `Refused`; `Skip` discards the rest of the part, still counted. FR-57 With a token field set, its value (at most `max_token_bytes`) is captured; under `BeforeFirstFile` a file part before it is refused.

**Typed extraction.** FR-58 `form!` refuses unknown names unless ignored, a scalar twice, a missing required field; `Option<T>` may be missing; `Vec<T>` collects; `bool` is false when missing. FR-59 Values are strict: integers refuse `+`, whitespace, overflow; floats refuse NaN and infinities; `bool` accepts exactly `on`, `true`, `1`, `off`, `false`, `0`. FR-60 Errors name the field by its Rust-side name, never echo input.

**Secrets and errors.** FR-61 `Secret` wipes on drop and prints `Secret(..)`; owned containers wipe their values and print names and lengths only. FR-62 Errors carry a location and a reason, never values or file names. FR-63 `status_hint` maps every error to 400, 413, 415, 422, 500, or 503 (or the inspector's code).

**Client side.** FR-64 `MultipartWriter` writes browser-shaped bodies and escapes LF, CR, `"` only; every body it writes parses back to the same fields. FR-65 A generated boundary has 128 random bits; with a caller boundary every chunk is scanned and data containing the delimiter is refused. FR-66 `content_type` returns the exact header value.

**After 1.0.** FR-67 `NamePath::parse` checks depth and index while scanning; never applied automatically.

### 4.2 Non-functional

| Id | Requirement |
|---|---|
| NFR-1 | No third-party runtime dependency in any feature combination, except the audited cryptography crypt-io brings (`zeroize`, `subtle`; D-083); first-party crates by published version only |
| NFR-2 | `#![forbid(unsafe_code)]`, no exception (SWAR in safe Rust, no SIMD intrinsics) |
| NFR-3 | The Tier 3 core builds with `--no-default-features` on a `no_std` target without `alloc` |
| NFR-4 | No panic on any input: no `unwrap`, `expect`, failing index, or overflowing arithmetic on input-derived values |
| NFR-5 | Every parser is linear in its input; the worst case is within a stated factor of the typical case |
| NFR-6 | At least 20% faster than the best Rust competitor per benchmark family, at equal or stronger limits, measured and published |
| NFR-7 | No allocation on hot paths |
| NFR-8 | Memory bounded by configuration, never by input; `memory_bound` holds |
| NFR-9 | Zero CPU when idle: no threads, timers, or polling |
| NFR-10 | Linux, macOS, Windows on stable and 1.85; temporary-file behaviour tested on all three |
| NFR-11 | Parsers, `Limits`, `Policy`, `Boundary` are `Send`; `Store` and `SpillQuota` are `Send + Sync`; checked at compile time |
| NFR-12 | Deterministic output on every platform and for every split |
| NFR-13 | Strict by default; every leniency opt-in, named after what it allows, with its differential documented |
| NFR-14 | Accepts every body current Chrome, Firefox, and Safari produce for forms |
| NFR-15 | A 4-hour upload soak (mixed sizes, 20% aborted) shows no growth in memory, files, or handles |
| NFR-16 | Every error variant documented with what the caller should do |
| NFR-17 | Every security measure documents its cost |
| NFR-18 | The multipart parser is at most 384 bytes plus its buffer on 64-bit targets (measured at 0.3, D-075; the specification estimated 256) |

### 4.3 Enterprise and distributed

form-net is in the **stateless** row of the standard's table: a parser and codec with no state between requests. Distributed-ready means deterministic and free of process-local assumptions, which it is: the same bytes give the same result on every node and platform, for every split. The parts that touch shared resources meet the enterprise points as follows.

| Point | How it is met |
|---|---|
| correct under concurrency and load | parsers are owned by one request; `Store` and `SpillQuota` are `Send + Sync`, lock-free counters, `loom`-tested; no global state anywhere (D-048) |
| bounded everywhere | every dimension has a limit; `memory_bound` for admission; the spill budget for disk |
| observable | stable error codes (`FormError::code`, D-042); `Inspector::failed` reports every refusal in the same pass (D-043); `Store::stats` (D-045); all free when unused |
| operable | every limit configurable with a safe default and validated at startup; overload refused cleanly (413, 503) |
| secure | section 10; no value in any error or `Debug` output; wiping |
| stateful pieces behind a trait | the disk budget is `SpillBudget` (D-044): in-process by default, a host-wide or fleet-wide implementation can be plugged in; storage is a `Sink`, so uploads can go to any backend |
| crash and retry | anonymous spill files cannot survive a crash; named ones are swept at startup; a failed parse leaves nothing behind, so a retried request starts clean |
| partition | nothing to partition: a node keeps protecting itself with its own limits and budget; a `SpillBudget` that cannot reach a coordinator must answer `false` (fail closed) |
| long-lived systems | semantic versioning, `#[non_exhaustive]` everywhere it can grow, codes stable after 1.0, MSRV 1.85 with raises only in a major release, `docs/STABILITY-1.0.md` at 1.0 |

---

## 5. Standards and exceptions

The crate follows `REPS.md` and the portfolio's library standard, with these recorded exceptions (D-059):

- **No async runtime.** The REPS clause requiring tokio does not apply: the engine is sans-IO and works under any driver.
- **No `tracing`.** It is a third-party crate. Observability is provided through hooks and counters that cost nothing when unused: `FormError::code`, `Inspector::failed`, `Store::stats`. The caller logs with whatever it uses.
- **Disk capacity.** REPS asks components to stop writing before a disk is full. Querying free space needs `unsafe` or a third-party crate, so the protection is a configured budget instead: `max_disk_bytes` per request and `SpillBudget` across requests, both checked before every write.
- **Secrets compared in constant time.** form-net compares no secret (the token is compared by csrf-sec); the wiping and redaction rules apply to every owned value.

---

## 6. Architecture

### 6.1 Modules

```
src/
  lib.rs           crate docs, Tier 1 free functions, VERSION, feature wiring
  kind.rs          FormKind detection (parse-net)
  limits.rs        Limits, Policy and its enums, memory_bound, validate
  error.rs         FormError, reason enums, Location, status_hint, code
  urlencoded/
    mod.rs         parse_urlencoded*, parse_query, pairs, Form
    raw.rs         raw_pairs, RawPair, the SWAR scan for & = % +
    decode.rs      decode_into: + and %XX in one pass, UTF-8 check
    stream.rs      UrlencodedParser
    encode.rs      serialize_urlencoded, Serializer, encode_into
  multipart/
    mod.rs         parse_multipart*, Multipart, Part
    boundary.rs    Boundary: validation, delimiter, failure table
    scan.rs        delimiter matcher: SWAR CR skip and streaming KMP
    parser.rs      MultipartParser state machine, Step, Event
    header.rs      header lines, PartHead, header budgets
    disposition.rs Content-Disposition and filename* through parse-net
    filename.rs    FileName views
    writer.rs      MultipartWriter, encode_part_head
  hook.rs          Inspector, Verdict, Refusal, Sink, DiscardSink, MemorySink, Driver
  extract/
    mod.rs         FromForm, FormBuilder, FromField, FileValue
    value.rs       FromField for String, integers, floats, bool, char, Option, Vec
    secret.rs      Secret
    file.rs        FileData
    macros.rs      form!
  io.rs            (std) MultipartReader, PartReader, OwnedPartHead
  store/           (fs)
    mod.rs         Store, SpillMode, StoreStats, StoreSink, StoredFile, Upload, UploadSession, read_multipart
    temp.rs        private directory, random names, platform open flags
    budget.rs      SpillBudget, SpillQuota
    sweep.rs       sweep
  wipe.rs          the best-effort wipe
```

### 6.2 Data flow

```
                      Content-Type bytes
                             │
                     FormKind::detect ──────────────▶ UnsupportedMediaType (415)
                             │
          ┌──────────────────┴─────────────────────┐
          ▼                                        ▼
   Urlencoded                               Multipart(Boundary)
          │                                        │
   buffered body?                           MultipartParser::parse(chunk, eof)
     yes ─▶ raw_pairs (zero copy)                  │ Step { consumed, event }
            ─▶ decode_into / Cow                   ▼
            ─▶ Form | FromForm               Driver (Tier 1 and 2 helpers, or the caller)
     no  ─▶ UrlencodedParser                       │
                                                   ├─ PartStart ─▶ Inspector::part_start ─▶ Sink::open
                                                   │               (token placement check)
                                                   ├─ first bytes ─▶ Inspector::sniff
                                                   ├─ Data ───────▶ Inspector::data ─▶ Sink::write
                                                   │               (text: buffered for Inspector::field)
                                                   ├─ PartEnd ────▶ Inspector::part_end ─▶ Sink::close
                                                   └─ End ────────▶ Inspector::end ─▶ Upload / FromForm
                     any error or Refuse ─▶ Inspector::failed ─▶ Sink::abort, drop finished files ─▶ FormError
```

### 6.3 Feature flags

| Feature | Implies | Adds | Dependencies |
|---|---|---|---|
| (none) | | Tier 3 core, no allocation | parse-net |
| `alloc` | | owned results, Tier 1 parsing, extraction, `Driver`, `MemorySink` | |
| `std` (default) | `alloc` | `MultipartReader`, `MultipartWriter`, `FormError::Io` | |
| `random` | `std` | `Boundary::generate`, `MultipartWriter::new` | mod-rand |
| `fs` | `std`, `random` | storage, `FileName::sanitized` | path-lib |
| `unicode` | `alloc` | `FileName::nfc` | unicode-lang |

The crate root is `#![no_std]` with `extern crate alloc` under `alloc` and `extern crate std` under `std`.

---

## 7. The engines

### 7.1 urlencoded

**Scan.** One pass. A SWAR loop (eight bytes per step as a `u64` from `u64::from_le_bytes` over `chunks_exact(8)`, safe Rust) finds the next byte that is `&`, `=`, `%`, or `+`, using the "has zero byte" test on `word ^ splat(b)` for each of the four, combined with OR. Nothing between special bytes is copied. Per sequence the scan records the start, the first `=`, and whether the name and value contain `%` or `+`.

**Decode.** Only when the flags say so. One loop copies runs of ordinary bytes, writes space for `+`, decodes `%XX` with a 256-entry table (0xFF for "not hex"), and refuses or keeps a bad `%` by policy. Output is never longer than input. UTF-8 is validated with `core::str::from_utf8` on the output.

**Owned `Form`.** Two allocations whatever the field count: one arena of decoded names and values back to back, and one table of `u32` spans, sized from the input length and the `&` count of the first scan, capped by limits, so neither reallocates. The arena is wiped on drop.

**Streaming.** `UrlencodedParser` keeps a scratch buffer of `max_name_bytes + max_value_bytes + 1` bytes for a sequence that straddles chunks; a sequence inside one chunk is returned borrowed. A sequence longer than the scratch is refused before it is copied.

### 7.2 Delimiter matching

The delimiter is `CRLF "--" boundary`, 5 to 74 bytes. The matcher is a streaming Knuth-Morris-Pratt automaton with a failure table built once in `Boundary::new`, plus a skip:

```
state k = delimiter bytes matched so far (0..=len)

loop over input:
  if k == 0: jump to the next CR with the SWAR scan; the bytes skipped are data
  else:      compare one byte; on a match k += 1; on a mismatch k = fail[k] and retry the byte
  if k == len: delimiter found
```

- **Linear, never rescanning.** Each byte is examined a bounded number of times whatever the split; `k` carries across chunks, so a split delimiter costs nothing extra (the CVE-2023-46136 failure, re-searching a growing buffer, cannot happen).
- **No held-back buffer.** When a partial match of `k` bytes ends a chunk, nothing is copied. If the match later drops from `k` to `j`, the bytes that turn out to be data are exactly `delimiter[..k - j]`, so the parser emits them as a `Data` event borrowed from the boundary's own bytes.
- **Start of body.** The parser starts in state `k = 2`, as if CRLF had been seen, so a body that begins with `--boundary` needs no special case; if that fails it scans the preamble for the full delimiter.
- **Worst case.** A body of CR bytes or repeated near misses disables the skip and runs the automaton byte by byte: linear, about 4 to 8 times the typical constant [est.], measured at 0.6.

Rejected (D-020): Boyer-Moore-Horspool (O(n·m) worst case and a held-back copy), Two-Way (hard to make streaming across arbitrary splits), SIMD `memmem` (needs `unsafe` or a third-party crate).

### 7.3 State machine

```
Preamble ─(delimiter)─▶ AfterDelimiter
AfterDelimiter: "--" ─▶ Epilogue (End emitted)    LWSP* ─▶ Padding ─▶ CRLF ─▶ Headers
                any other byte ─▶ refuse BoundaryInContent
Padding: more than max_padding_bytes ─▶ refuse Limit(Padding)
Headers: accumulate up to max_part_header_bytes until CRLF CRLF ─▶ parse lines ─▶ classify ─▶ PartStart ─▶ Body
Body: matcher; Data events; skip_part discards; on delimiter ─▶ PartEnd ─▶ AfterDelimiter
Epilogue: count bytes in O(1) per chunk; over max_epilogue_bytes ─▶ refuse; every call returns End
Poisoned: any call ─▶ Poisoned (until reset)
```

`eof = true` in any state before `Epilogue` is `Truncated` (or `NoInitialDelimiter` in `Preamble`). `End` is emitted when the close delimiter is read (D-047).

### 7.4 Part headers

- Fast path: when the whole block up to CRLF CRLF is inside the current chunk, it is parsed in place and `PartHead` borrows the input. Slow path: the bytes are copied into the header buffer (capacity `max_part_header_bytes`) and `PartHead` borrows the buffer.
- The CRLF CRLF search uses the same SWAR CR skip.
- Each line is split at the first `:`; the name is checked against a 256-entry token table; the value is trimmed of SP and HTAB; bytes are checked against a control-byte table.
- Known headers are matched ASCII case-insensitively by length first, then bytes; a repeated known header is refused.
- `Content-Disposition` and `Content-Type` values go to parse-net. Parameters are read with parse-net's `DuplicatePolicy::First` and checked by form-net itself, so a repeated `boundary` is `DuplicateBoundary` and any other repeated parameter is refused with its own error (D-063). A quoted value is read with `Quoted::raw` (backslash literal, as browsers send it) and refused if its raw bytes contain `\"` (D-015). Names are HTML-unescaped into a name area of the parser buffer that follows the header block (`max_name_bytes`), and an accepted `filename*` is decoded into an area of `max_filename_bytes`, so the raw header bytes stay intact and `PartHead::name` is always borrowed (D-062).

### 7.5 Sniff window and hook order

Drivers with `sniff_bytes > 0` hold back the first `sniff_bytes` of each file part in a buffer allocated once, until it is full or the part ends, call `Inspector::sniff`, and only then pass those bytes to `Inspector::data` and the sink. The window is contiguous however the input was split. For buffered bodies it is a slice of the body. Order per part: `part_start`, `sniff` (file parts, when enabled), `data`*, `field` (text parts), `part_end`; then `end`; `failed` once on any error. The driver opens the sink on the first byte the inspector has accepted, or at the end of an empty file, so a refused or skipped part makes no sink call at all (D-089).

### 7.6 Typed extraction

No serde and no procedural macro (a procedural macro needs `syn` and `quote`). Two traits and one `macro_rules!` macro. `FromField` uses a slot per field, so every arity rule is in one place: scalars use `Option<T>` (a second value is `Duplicate`, none is `Missing`), `Option<T>` returns `None` when missing, `Vec<T>` pushes, `bool` finishes missing as `false`. The macro expands to the struct, a builder inside an unnameable `const _: () = { ... }` block (no identifier concatenation is needed, D-050), and name comparisons the compiler turns into a length switch. No hash map.

### 7.7 Client writer

`MultipartWriter` generates `form-net-` plus 32 hex characters unless given a boundary; with a caller boundary it runs the delimiter matcher over every data chunk and refuses a collision. Names and file names are escaped as browsers do (LF, CR, `"` only) and other control bytes are refused; part content types are checked by parse-net. Every body it writes parses back with form-net to the same fields (D-056).

### 7.8 Errors

One `Copy` error enum with `#[non_exhaustive]` reason enums; every input error carries a `Location` (part index, byte offset) and never input bytes. `Display` is one line: reason, location, action. `status_hint` maps to an HTTP status; `code` gives a stable name. `FormError` implements `core::error::Error` (stable since 1.81).

### 7.9 Wiping

Owned values that may hold secrets (the `Form` arena, `Upload` text and token, `Secret`, `FileData`, `OwnedPartHead`, memory-held `StoredFile` bytes and every file's name, file name, and content type, the parsers' header and token buffers on `reset` and drop) are overwritten with zeros. With `alloc` this is crypt-io's `wipe` and `wipe_vec`, volatile writes over the whole allocation including spare capacity (D-083); the allocation-free core, which owns only the caller's buffer, uses `fill(0)` plus `core::hint::black_box`, best effort (D-028). Growing buffers move through `append_wiping`, which wipes the old allocation after each move, so a reallocation leaves no copy. The cost is one write over the allocation, about 50 to 100 ns for a 2 KiB form [est.]. A test-only counter proves that every owned value sends all its bytes through the wipe on drop (D-085).

---

## 8. Budgets and memory

The limits, their defaults, and where each is checked are in [`docs/API.md`](../docs/API.md#limits). The defaults (D-030) are 10 MiB body, 1,000 fields, 256-byte names, 1 MiB values, 2 MiB of text, 100 parts, 20 files, 10 MiB per file, 8 KiB and 8 lines of part headers, 1,024-byte file names, 1 KiB preamble and epilogue, 32 bytes of padding, sniffing off, and a 128-byte token. Routes override them; customs-sec compiles each route contract into a `Limits`.

All counters are `u64` with checked arithmetic. Worst-case memory per request:

```
parser state                      ~256 B [est.]
+ parser buffer                   header block + max_name_bytes (+ max_filename_bytes under AcceptAlone)
                                  (+ max_token_bytes with a token field) + 32 for _charset_
+ driver head copy                the same size (D-064)
+ sniff_bytes                     sniff window
+ max_value_bytes                 the field being received
+ max_text_bytes                  all text values kept
+ max_files × min(threshold, max_file_bytes), capped by max_body_bytes   in-memory files
```

Every term uses the effective limits (D-060). With the defaults and `Store::temp()`: 4,473,664 bytes, about 4.3 MiB per request at worst against 10 MiB of input [est.]; a route with `max_body_bytes = 4 KiB` is under 32 KiB. The caller's read buffer is not included.

---

## 9. Storage

Behind `fs`. The API is in [`docs/API.md`](../docs/API.md#storage); the platform details go into `docs/PLATFORM-NOTES.md` at 0.5.

**Directory.** `Store::temp()` creates `<temp dir>/form-net-<32 hex>` (128 bits from mod-rand), failing if it exists, mode 0700 on Unix. `Store::in_dir` checks with `symlink_metadata` that the path is a directory and not a link. A store's own directory is removed, if empty, when the store and all its files are gone (files hold an `Arc` of the store's shared state, D-048).

**Files.** `upload-<32 hex>.part`, `OpenOptions::new().read(true).write(true).create_new(true)`, plus:

| Platform | Extra | Effect |
|---|---|---|
| Unix | `mode(0o600)` | owner only |
| Unix, anonymous | `remove_file` right after open | no name; the kernel frees it when the handle closes or the process dies |
| Windows | `attributes(FILE_ATTRIBUTE_TEMPORARY)` | kept in cache where possible |
| Windows, anonymous | `custom_flags(FILE_FLAG_DELETE_ON_CLOSE)`, `access_mode` with `DELETE`, `share_mode(0)` | deleted when the last handle closes, including on termination; no other process can open it |
| Windows, named | a share mode that lets the persist step link and remove it | |

The Windows constant values are checked in CI by behaviour tests, not by value. Windows ACLs cannot be set without `unsafe` or a third-party crate, so the private directory inherits the ACL of the per-user temporary directory; this is documented and tested.

**Reading back** uses positional reads, so readers do not share a cursor. **Persist** links (named, same file system) or copies into a `create_new` destination; never overwrites. **Cleanup** closes the handle, then removes the name; drop-time failures increment `cleanup_failures`. **Crash**: anonymous mode leaves nothing; named mode is swept at startup. **Budget**: the per-request disk limit and the `SpillBudget` are reserved before every write and released on delete or persist.

Linux `O_TMPFILE` with `linkat` would let anonymous files be persisted by link instead of copy, but needs `unsafe` or libc; it is not used (section 14).

---

## 10. Threats

Costs are per request unless stated; [est.] until measured at 0.6. Every threat gets an attack test named `test_attack_t<n>_<name>_refused` (or `_contained`), and `docs/SECURITY.md` (0.7) holds the full model.

| Id | Threat | Defence | Attack test | Cost |
|---|---|---|---|---|
| T-1 | many tiny parts (Werkzeug CVE-2023-25577) | `max_parts` at each delimiter, before headers | 100,000 empty parts: refused at part 101 | one compare per part |
| T-2 | many fields, hash flooding | `max_fields`; no hash map in the crate | 1,000,001 pairs: refused at 1,001 | one compare per pair |
| T-3 | per-byte work on preamble or epilogue (python-multipart CVE-2024-53981) | SWAR-scanned preamble; epilogue counted O(1) per chunk; both budgeted | 10 MiB of CR LF before and after: refused at 1 KiB, constant CPU per chunk | none on normal bodies |
| T-4 | quadratic boundary search (Werkzeug CVE-2023-46136) | streaming KMP, state across chunks, no growing buffer | the CVE shape at 1-byte, 7-byte, 64 KiB splits; operation counter proves at most c·n byte visits | none |
| T-5 | header bomb inside a part | `max_part_header_bytes`, `max_part_headers` before copying | 1 MiB header block; 10,000 header lines | one compare per line |
| T-6 | one huge text value | `max_value_bytes`, `max_text_bytes` before buffering | 100 MiB text field: refused within `memory_bound` | one compare per chunk |
| T-7 | disk filling | `max_file_bytes`, `max_files`, `max_disk_bytes`, `SpillBudget`, reserved before writes | parallel uploads beyond the budget: `QuotaExceeded`, disk never above it | one CAS per write, 5 to 20 ns |
| T-8 | aborted uploads leave files or handles (Node multer CVE-2026-5038, CVE-2026-77037) | abort closes then deletes every file; anonymous files have no name; `Truncated` at eof | abort at every byte of a 3-file body; count entries and open handles | none on success |
| T-9 | temporary file races: predictable names, planted files, symlinks | fresh private directory with 128 random bits, 0700; random file names; `create_new`; 0600 | pre-create the next name and a symlink through a test-only randomness seam: open fails, nothing written through the link | one random read per file |
| T-10 | disclosure to other local users | 0600 in 0700 (Unix); per-user temp ACL (Windows) | permission bits asserted; Windows directory under the user profile | none |
| T-11 | a crash leaves uploads on disk | anonymous mode; named mode plus `sweep` | kill a child process mid-upload; assert no file, or `sweep` removes it | none |
| T-12 | delimiter prefix inside data | a full delimiter followed by anything but `--`, padding, or CRLF is refused | `CRLF--BOUNDARYx` in a file part: refused | none |
| T-13 | two boundaries, or token and quoted forms that read differently | exactly one `boundary`; parse-net grammar | `boundary=a; boundary=b`: refused | none |
| T-14 | differential with a WAF or proxy in front (WAFFLED) | refuse every construct readers disagree on: duplicate disposition, duplicate `name`, both file names, transfer encodings, bare LF, folding, whitespace before colon, `\"`, missing close delimiter, data past the epilogue budget | WAFFLED-category mutations of the corpus: each is refused or parses to exactly the original fields | none on clean bodies |
| T-15 | differential with browsers (refusing real forms) | browser capture corpus (NFR-14) | every capture parses to its expected fields | none |
| T-16 | `;` as a separator (Python CVE-2021-23336) | `&` only | `a=1;b=2`: one pair, value `1;b=2` | none |
| T-17 | parameter pollution | order kept; `get_one`, `field_one`, typed extraction refuse duplicates; `get` documented as first-wins | `role=user&role=admin` into a scalar: refused | none |
| T-18 | nested bracket names (Node multer CVE-2026-5079, CVE-2026-82333, CVE-2026-77078) | names opaque; `NamePath` (after 1.0) checks depth and index while scanning | `a[1000000000][x]=1`: no work | none |
| T-19 | invalid encodings smuggling bytes | strict percent and UTF-8; never replace | `%C0%AF`, `%ED%A0%80`, `%`, `%4`, `%zz`: refused | none |
| T-20 | file name traversal, absolute, UNC, streams, device names | names never paths; random spill names; `sanitized`; `persist` never overwrites | `../../etc/passwd`, `C:\Windows\x`, `\\host\share\x`, `a.txt:stream`, `CON`, `NUL.txt` | about 100 ns when `sanitized` is called |
| T-21 | file name Unicode tricks (RLO, NFD, invisibles) | `sanitized` replaces every default-ignorable character (D-084), path-lib the rest; NFC with `unicode` (0.5) | `"gpj\u{202E}.exe"`, NFD input: neither in `sanitized` | only when called |
| T-22 | content type spoofing | declared type is a hint; sniff window for customs-sec | a `.png` part whose bytes are `<html>`: the inspector receives the real first bytes | one copy of up to `sniff_bytes` per file |
| T-23 | uploads stored before the token is checked | `TokenField` with `BeforeFirstFile` | token after the file: refused before `Sink::open` | about 10 ns per text field name |
| T-24 | two token fields | `DuplicateToken` | refused | none |
| T-25 | charset confusion (UTF-7 and friends) | UTF-8 only | `_charset_=utf-7` then `+ADw-script+AD4-`: refused | none |
| T-26 | base64 or quoted-printable parts hide content | transfer encodings refused | refused | none |
| T-27 | nested `multipart/mixed` hides files | refused | refused | none |
| T-28 | counter overflow | `u64`, checked arithmetic | lengths near `u64::MAX` through a test counter seam | none |
| T-29 | panic on crafted input (release builds abort) | no panicking constructs; fuzzing | every fuzz target 24 hours clean before 0.7 exits | none |
| T-30 | secrets in logs | errors hold no values; `Debug` redacts | format every error and container; no value bytes appear | none |
| T-31 | secret residue in freed memory | volatile wipe on drop (D-083) | a test-only counter shows every owned value wipes all its bytes on drop (D-085) | 50 to 100 ns per 2 KiB form |
| T-32 | split-dependent results | split invariance as a property | every split of every corpus body gives identical events | none |
| T-33 | memory exhaustion through many concurrent uploads | `memory_bound` for admission; bounded buffers | 1,000 concurrent parses: peak within 1,000 × `memory_bound` | none |
| T-34 | header injection in built requests | writer escapes LF, CR, `"`; refuses other controls; content types validated | names with CR LF and quotes re-parse to the same name; no extra header | about 10 ns per part |
| T-35 | boundary collision in client bodies | 128-bit generated boundaries; caller boundaries scanned | data with the delimiter under a caller boundary: refused | 0.1 to 0.3 ns per byte |
| T-36 | quoted-pair ambiguity in `filename` | backslash literal, `\"` refused | `filename="a\"; name="x"`: refused | none |
| T-37 | empty or missing names | `EmptyName`, `MissingName` | `name=""`: refused | none |
| T-38 | inspector bypass through ordering | drivers call `part_start`, `sniff`, and the first `data` before any sink call (D-089) | a recording sink asserts no write precedes them, at every split | none |

Out of scope, named so nobody assumes otherwise: decompression bombs (HTTP layer, customs-sec), slow uploads (serve-net, net-io), malware in files and image parser exploits (the application, in a sandbox), cross-site scripting through a reflected file name (the render layer's escaping).

---

## 11. Performance

### 11.1 Targets

All [est.] until measured at 0.6 on the machines named in `docs/PERFORMANCE.md`.

| Operation | Input | Target |
|---|---|---|
| `FormKind::detect` | browser `Content-Type` | under 100 ns, 0 allocations |
| `raw_pairs` scan | 1 MiB, no escapes | at least 1.5 GB/s, 0 allocations |
| `decode_into` | 1 MiB, every byte escaped | at least 700 MB/s, 0 allocations |
| `parse_urlencoded` | 10 fields, 300 bytes | under 400 ns, 2 allocations |
| `Signup::from_urlencoded` | 6 fields, 200 bytes | under 600 ns, allocations only for `String` fields |
| `serialize_urlencoded` | 10 pairs | under 300 ns, 1 allocation |
| `MultipartParser` | one 100 MiB file part, random bytes, 64 KiB chunks | at least 3 GB/s, 0 allocations |
| `MultipartParser` | adversarial: all CR, near-miss delimiters | at least 400 MB/s, linear, 0 allocations |
| `MultipartParser` | 100 parts of 10 bytes | under 30 µs total |
| `MultipartParser` | 1 KiB body, 3 text fields | under 1 µs |
| `parse_multipart` | 1 MiB, 2 files | within 10% of the raw parser |
| `UploadSession` + `Store::temp()` | 100 MiB file | disk-bound; under 5% of one core at 1 GB/s |
| split overhead | 1 MiB body in 1-byte chunks | linear, under 50 ns per call |

### 11.2 Competitive bar and families

At least 20% faster than the best Rust competitor in each family, at equal or stronger limits: competitors get the same limits where they support them, and form-net is never benchmarked with its checks off.

| Family | form-net | Competitors (dev-dependencies of the benchmark crate only) |
|---|---|---|
| urlencoded to pairs | `pairs`, `raw_pairs` + `decode_into` | `form_urlencoded::parse` |
| urlencoded to struct | `form!` + `from_urlencoded` | `serde_urlencoded::from_bytes`, `serde_qs::from_bytes` |
| urlencoded serialize | `serialize_urlencoded` | `form_urlencoded::Serializer`, `serde_urlencoded::to_string` |
| multipart streaming | `MultipartParser` | `multer` 3.x through `futures_executor::block_on` |
| multipart buffered | `parse_multipart` | `multer` (one chunk), `multipart` 0.18 over a `Cursor` |
| multipart to disk | `UploadSession` + `Store::temp()` | `multipart` 0.18 `SaveBuilder::temp` |
| reference only | `MultipartParser` | busboy and `@fastify/busboy` under Node, reported, never part of the bar |

Inputs: 1 KiB, 1 MiB, 100 MiB bodies; login (3 fields), sign-up (8), search (2), avatar (64 KiB), document (10 MiB), batch (20 files of 1 MiB), 1,000 fields; chunks of the whole body, 64 KiB, 8 KiB, 1,500 bytes, 1 byte; the adversarial shapes of T-3 and T-4; boundaries of browser shape, 1 byte, 70 bytes.

Method: criterion with committed baselines (a regression over 5% blocks a change); throughput in bytes per second, median and p99; allocation counts from a counting allocator; instructions and cache misses through `perf stat` for the headline numbers; hardware, OS, Rust version, and governor recorded with every number. The benchmarks live in their own crate so competitors never enter form-net's dependency tree.

Risk: multer finds boundaries with `memchr`, which uses SIMD; a safe SWAR loop can be slower on long data. form-net's margin has to come from zero allocation per chunk, no async machinery, no reference-counted buffers, and borrowed events. If 0.6 measures the 100 MiB family under the bar, the options are wider SWAR words and a better rare-byte choice; `unsafe` SIMD is not one of them without James's approval.

---

## 12. Testing

- **Unit tests** at the bottom of each file, one per `Ok` and `Err` path, named `test_<subject>_<condition>_<expected>`.
- **Integration tests** (`tests/`): `urlencoded_whatwg.rs`, `urlencoded_strict.rs`, `serialize.rs`, `boundary.rs`, `multipart_rfc.rs`, `multipart_browsers.rs`, `multipart_strict.rs`, `multipart_split.rs`, `budgets.rs`, `inspector_order.rs`, `token_field.rs`, `extract.rs`, `store_unix.rs`, `store_windows.rs`, `crash.rs`, `abort.rs`, `writer.rs`, `no_alloc.rs`, `memory_bound.rs`, `attacks.rs`.
- **Property tests** (proptest): no panic on any bytes into any parser; split invariance for arbitrary bodies and split lists; budgets hold; `parse(serialize(pairs)) == pairs`; `parse_multipart(write(parts)) == parts`; `decode_into` never grows text; a writer with a caller boundary never emits an extra delimiter.
- **Conformance corpora:** the web-platform-tests urlencoded parser cases (each with the WHATWG result and form-net's result: equal, or a refusal with its reason); RFC 7578 and RFC 2046 examples; browser captures from current Chrome, Firefox, and Safari for a fixed test page (text with ASCII, non-ASCII, CR LF, quotes; file inputs empty, ASCII and non-ASCII names, names with `"`, `%`, `\`; several files under one name; `_charset_`), committed with their expected parse and the capture date; client captures from curl `-F`, Python `requests`, Go `mime/multipart`, and form-net's own writer.
- **Differential tests:** a small, obviously correct, whole-body reference parser in the test suite, compared with the streaming parser on every corpus and fuzz input; multer as an oracle for well-formed bodies, with disagreements triaged into the differential log in `docs/SECURITY.md`; WAFFLED-style mutations (duplicated or reordered headers and parameters, case, whitespace and quoting changes, line endings, boundary variants), each refused or parsed to exactly the original fields.
- **Fuzz targets** (cargo-fuzz, corpus outside the repository): `fuzz_detect`, `fuzz_raw_pairs`, `fuzz_decode`, `fuzz_urlencoded_stream`, `fuzz_boundary`, `fuzz_multipart`, `fuzz_multipart_differential`, `fuzz_part_headers`, `fuzz_filename`, `fuzz_extract`, `fuzz_writer_roundtrip`. CI runs each for 60 seconds per push; before 0.7 exits each runs 24 hours with no crash and no budget violation.
- **Counting-allocator tests** for every "no allocation" claim; a peak-tracking allocator for `memory_bound`.
- **Concurrency, platform, soak:** `loom` model of `SpillQuota` with three threads (never over the limit, never negative, no lost release); platform tests on all three systems (permission bits, delete-on-close, persist across one and two file systems, `sweep`); a 4-hour soak with 64 concurrent sessions, mixed sizes, 20% aborted, sampled every minute.
- **Attack tests:** one per threat T-1 to T-38.
- **Edge cases** (each a data row run whole and split at every byte): every Content-Type and detection case (empty, case variants, charset values, missing, empty, and quoted-empty boundaries, 1, 70, and 71 bytes, trailing space, every `bchars` symbol, bytes outside `bchars`, duplicate boundaries, parameter case, whitespace, other parameters, unsupported types, trailing garbage); urlencoded (empty body, `&`, `&&&`, `=`, `==`, `=&=`, `a`, `a=`, `=a`, `a==b`, `+`, `%2B`, `%20`, every bad `%` form under both policies, hex case, `%00`, overlong, surrogate, truncated, 5- and 6-byte forms, `%FF`, raw non-ASCII valid and invalid, the BOM, `;`, duplicates 2 and 1,000 times, every length and count at max-1, max, max+1, body at the limit, `?` handling, a pair split inside `%XX`, long runs of `&`, serializer inputs); multipart framing (`--B--`, `--B--\r\n`, a CRLF preamble, preamble at the limit, `--B` mid-line, all-CR preamble, epilogue at and over the limit, padding 0, 1, 32, 33, bad bytes after a delimiter, data after the close, truncation at every state, a delimiter split across two and three chunks, a different boundary inside data, the boundary without CRLF inside data, CR at a chunk end, all-CR data, the CVE-2023-46136 shape, an empty part, bare LF, eof with no input, input after `End`, calls after an error and after `reset`); part headers (none, block and count at the limits, split at every byte, bad names, whitespace, folding, controls, raw and invalid UTF-8 in `filename`, duplicates, mixed case, unknown headers under both policies, every transfer encoding); `Content-Disposition` (type variants, `name` forms, escapes in names, `\"`, `\\`, `\x`, unterminated quotes, empty file names with and without data, path-shaped file names, `filename*` forms under every policy, both file names in either order, valueless and extra parameters, unknown parameters, file name length at the limits); part content types and charsets; parts and budgets at their limits and at 0; `validate` with every inconsistent pair; buffers one byte short; file name views (`basename`, `extension`, `sanitized`, `nfc` of the cases in API.md); storage (memory-only over the threshold, exactly at and one over the threshold, threshold 0, missing or read-only directories, persist to existing, missing, and other-filesystem destinations, concurrent readers, budget at the limit from many threads, `sweep` with links and fresh files, kill during spill and during persist); inspector and token (`Refuse` and `Skip` from each hook, refusal at `end` after files were stored, sniff windows smaller, equal, and larger than the file at every split, token placement and duplicates and lengths and case, urlencoded tokens); typed extraction (every scalar at its bounds and with `+`, leading zeros, whitespace, empty; `bool` spellings; float extremes, NaN, infinities, `-0`; `char` lengths; `Option` and `Vec` arities; unknown, ignored, and repeated ignored names; kind mismatches; structs of 0 and 64 fields); writer (escaping, line-break normalization, caller boundary in data split across calls, zero parts, misuse).

---

## 13. Documentation

| Deliverable | By |
|---|---|
| `README.md`: header image, badges with MSRV, pitch, features, install, quick start with Tier 1 first, status, links, license, footer | 0.2, updated every version |
| `docs/API.md`: every public item, metrics-lib format | 0.2 (lock), complete with runnable examples at 0.5 |
| `docs/ARCHITECTURE.md`: sections 6, 7, and 9 of this file as built | 0.5 |
| `docs/PLATFORM-NOTES.md`: temporary files per platform, ACLs, delete-on-close, persist across file systems, fsync guidance | 0.5 |
| `docs/PERFORMANCE.md`: measured numbers, method, hardware, competitor versions, adversarial results | 0.6 |
| `docs/SECURITY.md`: the threat model, strict deviations from WHATWG and RFC 7578, the differential log | 0.7 |
| `docs/STABILITY-1.0.md`: the 1.0 audit | 1.0 |
| `CHANGELOG.md` (Keep a Changelog 1.1.0), `docs/release/vX.Y.Z.md` | every change and version |
| `examples/`: `urlencoded_basic`, `query_string`, `multipart_buffered`, `multipart_sans_io`, `upload_to_disk`, `upload_session_push`, `typed_form`, `content_inspector`, `token_first`, `build_multipart_client`, `no_std_core` | as features land |

---

## 14. Ideas

At least one new defence idea per version (hypothesis, method, expected cost):

| Version | Hypothesis | Method | Expected cost |
|---|---|---|---|
| 0.2 | A work budget (bytes the matcher examines, from the same counter the T-4 test uses) could refuse a pathological body by work done rather than by size, so a body that is legal in size but expensive in shape is cut short. | At 0.6, measure examined bytes per body byte on the corpus and on the adversarial inputs; if adversarial bodies are at least 4 times the corpus and the corpus stays under 1.2, add a `max_work_factor` limit. | one add per matcher step, about 0.1 ns per byte [est.] |
| 0.3 | Priming the matcher at state 2 removes a whole branch of first-delimiter special cases, and their bugs. | Done at 0.3: the parser starts in state 2 as if CRLF had been seen, so a body that begins with `--boundary` and one with a preamble take the same path; there is no first-delimiter branch to cover, and the split, fuzz, and reference tests found no difference between the two shapes. | none |
| 0.4 | Verifying the token before the first file stops forged uploads from costing disk. | Done at 0.4: `TokenField` with `BeforeFirstFile` refuses a file part that comes before the token at its first header, before `Sink::open`, at every split; `UploadSession::captured_token` gives the token as soon as its part ends, so the caller verifies it before reading any file byte. | about 10 ns per text field name [est., benchmarked at 0.6] |
| 0.5 | A budget per tenant stops one tenant filling the shared temporary disk. | Done at 0.5: one `SpillQuota` per tenant; `tests/budget.rs` fills tenant A's share and shows tenant B's uploads unaffected. | one CAS per write [est., benchmarked at 0.6] |
| 0.6 | A cheap first-bytes fingerprint (no copy) could let customs-sec skip full sniffing for common types. | Measure sniff cost with and without. | about 10 ns per file |
| 0.7 | A WAFFLED-style mutation fuzzer finds strictness gaps faster than random fuzzing. | A structure-aware generator over the corpus. | CI minutes only |
| 0.8 | A refusal digest (counts per refusal code per route) helps the detection layer spot probing. | Counters fed from `Inspector::failed` and `FormError::code`, no allocation. | about 1 ns per refusal |
| 0.9 | Refusing any part header byte at or above 0x80 outside `filename` shrinks the differential surface without hurting browsers. | Check the browser corpus. | none |

Other ideas, not planned:

- Linux `O_TMPFILE` with `linkat` would persist anonymous files by link instead of copy. It needs `unsafe` or libc, which this crate forbids.
- A `nonblocking` example driving `UploadSession` from a reactor, after 1.0.

---

## 15. Open decisions

| Id | Question | Recommendation | When |
|---|---|---|---|
| O-19 | A serde adapter? | No, until a real user needs it; `form!` covers typed forms. If approved later, a non-default `serde` feature, the only third-party dependency. | after 1.0 |

Every other open decision of the specification is decided below (D-004 to D-035).

---

## 16. Decision log

| Id | Date | Decision | Reason |
|---|---|---|---|
| D-001 | 2026-10-13 | Sans-IO; the Tier 3 core is `no_std` without `alloc` (strengthened 2026-10-23 from "`no_std` with `alloc`", D-004) | the same parser serves any server and is tested without sockets; caller buffers make the core usable anywhere |
| D-002 | 2026-10-13 | Streaming multipart events, never the whole upload in memory | large uploads must not cost their size in RAM |
| D-003 | 2026-10-13 | Strict decoding: refuse, never replace | a parser that repairs input creates differentials |
| D-004 | 2026-10-23 | The form-net specification (2026-10-21) is adopted as this plan. It changes the 0.1 plan in five places: the roadmap covers every series from 0.1 to 1.0; `filename*` follows RFC 8187 (RFC 5987 is obsolete) and is refused by default; the Tier 3 core needs no `alloc`; new budgets (files, header lines, text total, file name, preamble, epilogue, padding, sniff, token); storage, typed extraction, inspection hooks, token capture, and the client writer join the scope | the specification is the complete contract; the 0.1 plan was a first sketch |
| D-005 | 2026-10-23 | O-1: the crates.io name is claimed by publishing 0.2.0 at the design lock; 0.1.0 is not published retroactively | 0.1.0 was never published (checked on crates.io 2026-10-23); publishing the scaffold now would add nothing that 0.2.0 does not |
| D-006 | 2026-10-23 | O-2: form-net uses parse-net for media types, parameters, quoted strings, `Content-Disposition`, and RFC 8187 values, never an in-house copy. parse-net 0.6.1 is published with that API (its `docs/API.md` has a form-net checklist), so 0.3 does not wait | the grammar is shared with http-net, cookie-net, and smtp-net; a copy would drift |
| D-007 | 2026-10-23 | O-3: form-net owns the urlencoded codec; url-lib parses URL structure and generic percent-encoding and hands form-net the raw query | the decode is form-net's hot path (plus, percent, budgets, and UTF-8 in one pass) with form semantics (`+` is space only here); no dependency either way |
| D-008 | 2026-10-23 | O-4: no encode-lib | transfer encodings are refused (D-017); generated boundaries are hex from a 16-entry table |
| D-009 | 2026-10-23 | O-5: a malformed `%` is refused by default; `PercentPolicy::Literal` is the opt-in WHATWG behaviour | refusing fails closed; the literal rule stays available for routes that must mirror `URLSearchParams` |
| D-010 | 2026-10-23 | O-6: invalid UTF-8 after decoding is refused, with no option | replacement merges distinct inputs into one value, the root of several differentials |
| D-011 | 2026-10-23 | O-7: names are opaque; an opt-in, bounded `NamePath` after 1.0 | automatic expansion is where the 2026 Node multer CVEs live (depth, index, crash); `form!` maps `tags[]` as a plain name |
| D-012 | 2026-10-23 | O-8: `multipart/mixed` inside a part is refused, a deliberate deviation from RFC 7578 section 4.3's SHOULD | browsers do not send it, RFC 7578 deprecates it, and supporting it adds a nested parser and a place to hide files |
| D-013 | 2026-10-23 | O-9: `text/plain` bodies are `UnsupportedMediaType` | the HTML standard says the format is not reliably machine-readable; it is also the classic cross-site content type |
| D-014 | 2026-10-21 | O-10: `filename*` is refused by default, `ExtendedFilenamePolicy::AcceptAlone` is the opt-in, `Ignore` drops it; both file name parameters are refused under every policy. **Decided by James** (cross-check X-5) | browsers never send it, so refusing costs real users nothing and removes a differential with inspection layers; RFC 7578 section 4.2 forbids senders from using it |
| D-015 | 2026-10-23 | O-11: backslash in quoted parameter values is literal and `\"` is refused, through parse-net's `Quoted::raw` (parse-net D-028); no second quoting mode | browsers escape only LF, CR, `"` (as `%0A`, `%0D`, `%22`); `\"` is the one sequence where the RFC 9110 and HTML readings disagree on where a value ends |
| D-016 | 2026-10-23 | O-12: names are HTML-unescaped (`%0A`, `%0D`, `%22` only); file names are raw with `html_unescaped` and `sanitized` views | form authors control names; users control file names, and a literal `%22` typed by a user cannot be told apart |
| D-017 | 2026-10-23 | O-13: `Content-Transfer-Encoding` other than `7bit`, `8bit`, `binary` is refused | RFC 7578 section 4.7 says senders should not use it; decoding would make form-net see bytes a WAF does not |
| D-018 | 2026-10-23 | O-14: up to 32 bytes of transport padding are accepted | RFC 2046 requires receivers to handle padding; a small cap keeps it from being a CPU or differential lever |
| D-019 | 2026-10-23 | O-15: bare LF is refused wherever CRLF is required | browsers send CRLF; accepting LF where a WAF requires CRLF, or the reverse, is a known bypass shape |
| D-020 | 2026-10-23 | O-16: the delimiter search is a SWAR CR skip plus streaming KMP | linear worst case, state across chunks without a held-back copy, safe Rust; Boyer-Moore-Horspool is O(n·m) at worst and SIMD needs `unsafe`; revisited only with 0.6 numbers |
| D-021 | 2026-10-23 | O-17: the default spill mode is anonymous | crash-safe by construction; routes that keep large uploads choose `Named` so `persist` is a link |
| D-022 | 2026-10-23 | O-18: spilling lives in form-net behind `fs` | temporary-file security is subtle and bound to the budgets and the abort path; every user of uploads needs it |
| D-023 | 2026-10-23 | O-20: a duplicate scalar in typed extraction is refused | parameter pollution defence; customs-sec refuses too |
| D-024 | 2026-10-23 | O-21: an empty file input is reported by default; `EmptyFilePolicy::Skip` is available; `Option<StoredFile>` maps it to `None` | reporting loses nothing; skipping is a convenience for routes that want it |
| D-025 | 2026-10-23 | O-22: unknown part headers are ignored by default (RFC 7578 section 4.8), counted against budgets, visible to inspectors; `HeaderPolicy::Refuse` for strict routes | the RFC says they must be ignored |
| D-026 | 2026-10-23 | O-23: UTF-8 and US-ASCII only | a charset table is a large new attack surface; other text stays available as bytes |
| D-027 | 2026-10-23 | O-24: default features are `["std"]` | `fs` pulls path-lib and mod-rand and touches the file system, so it is an explicit choice |
| D-028 | 2026-10-23 | O-25: wiping is `fill(0)` plus `black_box`, documented as best effort; if crypt-io exposes a vetted public wipe, form-net switches and records why | `forbid(unsafe_code)` rules out a volatile write; crypt-io's wipe is private today |
| D-029 | 2026-10-23 | O-26: magic-byte detection is customs-sec's; form-net provides the contiguous sniff window | customs-sec's plan already holds it |
| D-030 | 2026-10-23 | O-27: the default limits of section 8 | they fit every browser form and bound memory to about 4.3 MiB per request (section 8); routes override them |
| D-031 | 2026-10-23 | O-29: an empty `name=""` is refused | a nameless field is never meaningful and some frameworks treat it as a wildcard |
| D-032 | 2026-10-23 | O-30: `Upload` text storage grows in 64 KiB doubling steps, capped by `max_text_bytes` | small forms stay small; the cap is still checked before each growth |
| D-033 | 2026-10-23 | O-31: single-pass parsing everywhere: the inspector runs while form-net parses (`*_inspected`, `from_body_inspected`, `UploadSession`) | streams cannot be replayed, and a single pass means the inspector and the handler see the same bytes |
| D-034 | 2026-10-23 | O-32: storage errors carry the `io::ErrorKind` only, never a path | paths reveal layout in logs |
| D-035 | 2026-10-23 | O-28: `dev/ROADMAP.md` rewritten to the full 0.1 to 1.0 series | the standard forbids skipping a series |
| D-036 | 2026-10-23 | Names changed from the specification's sketch: `ExtendedFilenamePolicy` and `Policy::extended_filename` (was `FilenameExtPolicy`, `filename_ext`); `UrlencodedStep`, `UrlencodedEvent` (was `UrlStep`, `UrlEvent`) | plain words without abbreviations (standard 2.10); `Url` read as URL |
| D-037 | 2026-10-23 | `_with` means explicit limits and policy; `_inspected` means that plus an inspector. `parse_multipart_with` ships at 0.3 without an inspector (#33); `parse_multipart_inspected` (#64) and `parse_urlencoded_inspected` (#63) ship at 0.4 | the sketch had no inspected urlencoded entry although customs-sec inspects urlencoded bodies, and used `_with` for two different things |
| D-038 | 2026-10-23 | `FromForm` gains `from_query`, `from_body`, `from_body_with`, `from_body_inspected` (#61, #62) | the simplified API is a hard requirement: a handler reads its form in one line whichever encoding the browser used; `from_body_inspected` is the single-pass path of D-033 |
| D-039 | 2026-10-23 | `parse_multipart*` and `Upload` check text fields as text at parse time and refuse a field that is not valid UTF-8 under a UTF-8 or ASCII charset | `Multipart::field` and `Upload::field` return `Option<&str>` without hiding an encoding error; consistent with D-026; Tier 3 still hands out bytes |
| D-040 | 2026-10-23 | `FormError::InvalidLimits { limit }` and `BufferTooSmall { needed }` replace the sketch's reuse of `Limit { .. }` for configuration and caller-buffer mistakes; both hint 500 | a misconfiguration must not look like a client's 413, and caller bugs must be told apart from attacks in metrics |
| D-041 | 2026-10-23 | `FormError::Io { kind }` and `FormError::Random` (under `std`) are separate from `Store { reason }` (under `fs`); `StoreError` keeps `QuotaExceeded`, `NotADirectory`, `DestinationExists` | the reader, the writer, and boundary generation exist without `fs`, and their failures are not storage failures |
| D-042 | 2026-10-23 | `FormError::code` returns a stable code (#65); `FormError::location` returns the location when there is one | enterprise observability (standard 2.16): metrics and security signals need identifiers that never change |
| D-043 | 2026-10-23 | `Inspector::failed(&FormError)` is called once on every failed parse (#66) | refusals are security signals; the security layer hears about every one in the same pass, so no component judges a probe in isolation |
| D-044 | 2026-10-23 | The shared disk budget is the `SpillBudget` trait (#67); `SpillQuota` is its in-process implementation; `Store::quota` takes any `SpillBudget` | standard 2.16: state that may need to be shared lives behind a trait (host-wide or fleet-wide budgets); cost one indirect call per disk write, about 1 to 2 ns [est.], next to the write itself |
| D-045 | 2026-10-23 | `Store::stats` and `StoreStats` (#68); `Store::memory_bound` (#71) | metrics and health for stores; admission control without the caller tracking the threshold |
| D-046 | 2026-10-23 | `MultipartParser::skip_part` (#69); `PartSummary::skipped` is set by it and by drivers | the parser had no way to drop a part without emitting its data; `PartReader` and the drivers' `Skip` need it |
| D-047 | 2026-10-23 | `End` is emitted when the close delimiter is read, not at `eof`; later input is epilogue | drivers finish as soon as the form is complete; `eof` stays the truncation signal |
| D-048 | 2026-10-23 | `read_multipart` makes its own store per call, whose directory is created only when a file spills; files hold an `Arc` of their store's shared state, so a store can be dropped before its files | no global state (standard 2.6) instead of the sketch's process-wide `OnceLock`; nothing is created when nothing spills |
| D-049 | 2026-10-23 | Under `EmptyFilePolicy::Skip`, drivers delay `part_start` and `Sink::open` for a file part with an empty file name until its first data byte | the sketch opened the sink at the first byte but did not say when the inspector hears of the part; the hook order must hold for skipped parts too |
| D-050 | 2026-10-23 | `form!` takes its ignore list as `ignore = [ ... ];` after the struct, not as `#[form(ignore = ..)]`; the builder is a type inside an unnameable `const _` block | `macro_rules!` cannot pick one attribute out of a list of attributes without a local-ambiguity error, and cannot build a new identifier without a third-party crate |
| D-051 | 2026-10-23 | `FileValue::Buffered(&Part)` replaces `Bytes { head, bytes }`; `StoredFile` accepts buffered parts by copying them into memory; `FileData` reads a spilled `StoredFile` into memory (bounded by its length) | a buffered body has no `PartHead` after the parse; one field type then works with every entry point |
| D-052 | 2026-10-23 | `FormBuilder` methods return `FormError` | the builder knows which field failed; `ExtractError` alone cannot name it |
| D-053 | 2026-10-23 | `Refusal::new` (#73); every public struct with public fields is `#[non_exhaustive]` and gets a constructor where other crates build it; a refusal code outside 400 to 599 is reported as 400 | other crates must be able to build refusals; a refusal is never a success |
| D-054 | 2026-10-23 | `OwnedPartHead` is the type `PartReader::head` returns, with `view()` giving a `PartHead` | the sketch said the head is copied into an owned head without naming it |
| D-055 | 2026-10-23 | Small accessors added for symmetry: `UrlencodedParser::buffer_len_needed` (#72), `Upload::iter` (#70), `FileData::into_bytes` and `len`, `MultipartWriter::boundary`, `Driver::parser` and `into_parts`, `UploadSession::bytes_seen`, `TokenField::name` and `placement`, `ContentType::charset` | each answers a question a caller of the sketch could not ask without reimplementing it |
| D-056 | 2026-10-23 | The writer refuses control bytes other than CR and LF (which it escapes) and HTAB in names and file names | invariant: every body the writer produces parses back with form-net to the same fields |
| D-057 | 2026-10-23 | `max_token_bytes` defaults to 128 | csrf-sec's tokens are 59 characters and its one-time tokens 102 (checked in its source, `TOKEN_LEN` and `ONCE_TOKEN_LEN`) |
| D-058 | 2026-10-23 | `max_filename_bytes` is capped by `max_part_header_bytes` (D-060) | a file name lives inside the header block, so a larger limit can never apply |
| D-059 | 2026-10-23 | REPS exceptions: no tokio (sans-IO); no `tracing` (third-party; observability through codes, hooks, and counters); disk exhaustion guarded by configured budgets instead of free-space queries (which need `unsafe` or a third-party crate) | section 5 |
| D-060 | 2026-10-23 | Containing limits cap contained ones (effective value = the smaller): body caps file, text, preamble, epilogue; text caps value; value caps name and token; file caps sniff; part header caps file name. `validate` refuses only what no body could satisfy (a header budget under 64 bytes, buffer sizes over `usize`, a zero token budget with a token field). This replaces the specification's refusal of every inconsistent pair (FR-39) | the simplified API is a hard requirement: `Limits::DEFAULT.with_max_body_bytes(4096)` must be a valid login configuration, and a limit larger than the one containing it is harmless because the containing one is checked first; the review found that the strict rule refused the document's own examples |
| D-061 | 2026-10-23 | Buffer sizes are `const fn`s on `Limits`: `multipart_buffer_len(&policy)` and `urlencoded_buffer_len()` (#72), replacing the parsers' `buffer_len_needed` | an associated function of a generic type cannot be called without naming the buffer type, and a `const fn` sizes a stack buffer at compile time for `no_std` callers |
| D-062 | 2026-10-23 | The multipart parser buffer holds the header block, then the decoded name (`max_name_bytes`), the decoded `filename*` (`max_filename_bytes`, only under `AcceptAlone`), the token (with a token field), and 32 bytes for `_charset_` | decoding in place would corrupt `name_raw` and `headers`; every area has a fixed size known at construction |
| D-063 | 2026-10-23 | `FormKind::detect` and part headers read parameters with parse-net's `DuplicatePolicy::First` and apply form-net's own repeat checks: `boundary` twice is `DuplicateBoundary`, any other repeat `InvalidContentType` (request) or `DuplicateParameter` (part) | error codes are stable, so which error a repeat produces must be fixed, not left to the order of two checks |
| D-064 | 2026-10-23 | The `Driver` copies each part head into an owned head allocated once at construction, for the delayed `sniff`, `part_start`, `Sink::open`, and `field` calls; `memory_bound` counts it | a `PartHead` borrows the parser and the current chunk, so it cannot live past the `parse` call that produced it |
| D-065 | 2026-10-23 | The buffered inspected entry points use a second, internal driver whose events borrow the body; both drivers are checked by the same order tests; `MultipartReader` has no hooks | buffered parts must borrow the body for the result's lifetime, which the streaming driver's events cannot; the hook order is the guarantee, not the single type |
| D-066 | 2026-10-23 | `Driver::new(parser, sink)` takes limits and policy from the parser; `MultipartParser::limits` and `policy` added | two copies of the configuration could disagree |
| D-067 | 2026-10-23 | The parser enforces the token placement and duplicate rules and the `EmptyFilePolicy::Skip` deferral of `max_files`; an empty-file-name part under `Skip` is checked against the token placement only when its first data byte arrives; `TokenField`, `TokenPlacement`, `EmptyFilePolicy`, `captured_token`, and `declared_charset` move to 0.3 with `Policy` | the rules depend only on the bytes, so every entry point must get them; `Policy` ships at 0.3 and cannot hold types from a later version |
| D-068 | 2026-10-23 | The captured token must be UTF-8 and is `Option<&str>` everywhere | the four accessors disagreed (`&str` and `&[u8]`); csrf-sec tokens are ASCII, and a token that is not text is forged |
| D-069 | 2026-10-23 | `Refusal { status, code }`: `status` is the HTTP status, `code` a stable identifier chosen by the inspector, which `FormError::code` returns for a refusal; `Refusal` ships at 0.3 with `FormError` | the field named `code` held an HTTP status while customs-sec needs a stable code; `FormError::Refused` exists at 0.3 |
| D-070 | 2026-10-23 | `Limits::memory_bound(file_in_memory_bytes: u64)` (the most bytes of one file held in memory; `u64::MAX` for memory-only) replaces `memory_bound(Option<u64>)`; `MultipartReader::with` is `with_limits`; `UploadSession` has no lifetime (the store is shared through an `Arc`) | `None` read as "no threshold" when it meant the opposite; `with` alone broke the naming rule of D-037; the lifetime was unnecessary |
| D-071 | 2026-10-23 | `Store::quota` ships at 0.5 with `SpillBudget` and `SpillQuota` | a 0.4 setter cannot take a 0.5 type |
| D-072 | 2026-10-23 | The writer refuses a name or file name ending in `\` (`AmbiguousQuote`), allows HTAB, and refuses every other control byte except CR and LF, which it escapes | a trailing backslash before the closing quote reads as `\"`, which the parser refuses; HTAB is what browsers send and the parser accepts |
| D-073 | 2026-10-23 | `Verdict::Skip` from `field` leaves that field out of the result (the specification treated it as `Continue`); `Multipart::fields` yields `&str` (the specification yielded `&Part`), like `Upload::fields` and `Form::get_all` | an inspector that skips a field means "do not hand it to the handler"; text fields are checked as text at parse time (D-039), so the plain value is the useful view |
| D-074 | 2026-10-23 | `max_name_bytes` is capped by `max_body_bytes`, not by `max_value_bytes` as the design-lock text said | a name is not a value: capping one by the other would make a route with one-byte values refuse ordinary names |
| D-075 | 2026-10-23 | NFR-18 is 384 bytes plus the buffer (measured), not the estimated 256 | the boundary and its matcher table alone are 149 bytes, and the parser keeps its own copy of `Limits` (72) and `Policy` (32) so callers need not keep them alive; a test asserts the bound |
| D-076 | 2026-10-23 | At most 16 parameters on a `Content-Type` or `Content-Disposition` (request or part); more is `InvalidContentType` (request, part `Content-Type`) or `MalformedParameter` (part `Content-Disposition`) | form-net checks repeated names itself (D-063), which is quadratic in the parameter count; browsers send at most three; 16 bounds the check at 120 comparisons |
| D-077 | 2026-10-23 | A text part labelled US-ASCII is read as UTF-8 text (valid UTF-8 is required either way) | ASCII is a subset of UTF-8; FR-34 accepts both labels; a second check for bytes above 0x7F would refuse nothing a browser sends |
| D-078 | 2026-10-23 | With `--no-default-features`, CI runs the unit and integration tests (`--lib --tests`) and not the doc examples; integration test files are compiled only with `alloc`; four examples declare `required-features = ["alloc"]` | the doc examples show the owned results, which need `alloc`; the allocation-free core is tested by the unit tests and by the bare-metal build |
| D-079 | 2026-10-23 | `Part::is_empty` added beside `Part::len` | clippy's `len_without_is_empty`, and symmetry with `Multipart` |
| D-080 | 2026-10-23 | Every error location is split-invariant: header errors report the start of the header block, budget errors the first byte past the budget, framing errors the offending byte | a location that moved with the chunk boundaries would make the same body fail differently on different networks (T-32); the split tests compare errors byte for byte |
| D-081 | 2026-10-23 | A part's `Content-Disposition` parameter whose name contains `*`, other than the exact `filename*` (which `ExtendedFilenamePolicy` governs), is refused as `MalformedParameter`; this replaces "unknown parameters ignored" for starred names | RFC 2231 readers join `filename*0` into a file name and prefer `name*` over `name`: ignoring them let a body be a text field to form-net and a file, or another field, to an inspection layer in front (found by review) |
| D-082 | 2026-10-23 | A backslash in a part's `name` is refused as `MalformedParameter`; in a file name it stays literal | RFC 9110 readers drop it as a quoted pair, browsers send it as itself; no form needs one in a name, while file names on Unix can hold one, and reading it literally is the safe side (the name form-net sees is never shorter than the one an RFC reader sees) |
| D-083 | 2026-10-23 | With `alloc`, wiping is crypt-io's `wipe` and `wipe_vec` (volatile, whole allocation); this replaces D-028's `fill(0)` plus `black_box` everywhere but the allocation-free core | crypt-io is the portfolio's audited-cryptography exception and now exposes a vetted wipe, the condition D-028 set; it pulls `zeroize` and `subtle`, which the dependency allowlist names |
| D-084 | 2026-10-23 | `FileName::sanitized` replaces every Unicode `Default_Ignorable_Code_Point` with `_` before `path_lib::sanitize` | path-lib 1.0 replaces only the bidirectional controls and the BOM, so zero-width spaces, joiners, soft hyphens, variation selectors, and tag characters passed through (found by the T-21 test); the check runs only on non-ASCII names, one pass; the list belongs in path-lib, which is a follow-up there |
| D-085 | 2026-10-23 | T-31 is tested by a `cfg(test)` counter of the bytes each wipe covers, not by reading freed memory; file metadata (field name, file name, content type) is wiped with the bytes | reading freed memory is undefined behaviour, even through a test allocator; the counter proves every drop path wipes the whole allocation, and crypt-io's volatile write does the rest |
| D-086 | 2026-10-23 | 0.4 additions to the 0.2 contract: `impl Inspector for &mut I`; `OwnedPartHead::{new, with_capacity, copy_from, view}`; `StoredFile::persist` removes a partial destination when a copy fails | a caller keeps ownership of its inspector across calls; a driver-free consumer needs to keep a head past the parser's next call; a failed persist must leave nothing under the caller's name |
| D-088 | 2026-10-23 | Inspectors chain: `impl Inspector for (A, B)` (A first; the first refusal wins and B is not asked; otherwise `Skip` if either skips; `failed` reaches both) and `impl Inspector for Box<I>` | customs-sec, sentinel-sec, and the application all inspect one parse; the combination rules are easy to get wrong, so they live in form-net once (found by the customs-sec and web-net shape review); a pair is static dispatch and costs one extra call per hook |
| D-089 | 2026-10-23 | The driver opens the sink lazily: after `part_start`, `sniff`, and `data` for the first chunk have accepted the part, or at the end of an empty file; T-38 becomes "no sink call at all before the verdicts" | a sink that creates its destination in `open` (an object-store upload, a file named at open) created storage for a part the inspector then refused; now a refused or skipped part touches no storage (found by the shape review); no cost |
| D-090 | 2026-10-23 | Shapes left for 0.5, from the customs-sec and web-net review: `FromForm::from_upload_mut(&mut Upload)` (typed fields and accessors from one upload), an all-errors extraction for re-rendering a form, and one owned type for both encodings (`get`/`get_all` on `Upload`, or `Upload::from_form`) | each is additive, neither consumer has code yet, and 0.5 is the feature-complete release; they are in the 0.5 roadmap |
| D-091 | 2026-10-26 | The security job audits the newest-version resolution (`CARGO_RESOLVER_INCOMPATIBLE_RUST_VERSIONS=allow` for `cargo generate-lockfile`) | RUSTSEC-2026-0385 (`ctutils` before 0.4.4) failed the main-branch audit after 0.4.0: the 1.85 MSRV makes the resolver lock `ctutils` 0.4.2, which the lockfile lists only because it records crypt-io's optional HMAC dependencies; no form-net feature compiles it (`cargo tree -i ctutils --all-features` is empty), and a user on current Rust resolves 0.4.4 or later; the dependency allowlist job still checks the real build graph in every feature mode |
| D-092 | 2026-10-26 | The writer refuses, before writing a byte, every input the parser would read differently: an empty name (`EmptyName`), any backslash in a name and a trailing one in a file name, a literal `%0A` `%0D` `%22` (`AmbiguousQuote`), control bytes other than HTAB (`ControlInHeader`), a content type parse-net refuses, with a repeated parameter, or `multipart/*` (`InvalidContentType`), and a `_charset_` value other than UTF-8 or US-ASCII (`Encoding { UnsupportedCharset }`) | FR-66 promises that every body the writer produces parses back with form-net to the same fields; each case is one where the parser refuses (D-082, T-27, T-37) or decodes to other bytes (the HTML escapes); the property test and `fuzz_writer_roundtrip` hold it |
| D-093 | 2026-10-26 | `memory_bound` adds terms the 0.2 formula left out: field names (`max_fields × max_name_bytes`), 96 bytes of index per field and 320 per part, owned copies of names, file names, and content types (`max_parts × max_part_header_bytes`, capped by the body), and half the largest growing buffer; a memory file's buffer is capped at `min(threshold, max_file_bytes)` and the upload text arena at text plus names | the 0.2 formula was exceeded by 4% on the first hostile body measured (doubling growth holds the old half while it moves, and the arena was capped at twice the text budget); the bound must hold or admission control admits too much; `tests/memory_bound.rs` and `tests/concurrent_memory.rs` (T-33) measure it, and a test keeps it within 4 times the measured peak so it is not uselessly loose |
| D-094 | 2026-10-26 | `UrlencodedParser` enforces `max_body_bytes` on bytes consumed: the pairs whose `&` lies inside the budget are emitted, then `Limit { Body }`; a sequence that straddles chunks is refused as soon as its known bytes prove it too long, with the error the complete sequence would get (`Budget::admit_partial`) | a streaming parser cannot know the length up front as `raw_pairs` does, and refusing on the first chunk that crosses would make the pairs before the error depend on the split; with these rules pairs and errors are split invariant (property test, fuzz target) and the scratch buffer never overflows |
| D-095 | 2026-10-26 | `sweep` removes only the exact names form-net makes, `upload-` and 32 lowercase hex and `.part`, rather than any `upload-*.part` | a sweep at startup runs with the server's rights over a directory a deployment may share; deleting only names form-net could have made costs nothing and cannot remove a user's file |
| D-096 | 2026-10-26 | The `SpillQuota` loom model runs with a preemption bound of 3; `loom` is a `cfg(loom)` dev-dependency only, and CI runs it in its own job | the unbounded state space of three threads with CAS loops did not finish in 20 minutes; a bound of 3 is the standard for lock-free counters and finishes in seconds; loom never reaches a build of the crate |
| D-097 | 2026-10-26 | `from_body_all` reports one error per received value that fails and one per unknown name, plus `Missing` for an absent field unless that field already has an error; it stops at the first parse error, which is then the only error | for re-rendering a form, one message per field is what a user can act on; a malformed body has no fields to report on |
| D-098 | 2026-10-26 | Of the D-090 options for one owned accessor type, `Upload::from_form(form, &policy)` was chosen, capturing the policy's token field (a repeated one is `DuplicateToken`); no `get`/`get_all` aliases, and no `From<Form>` (it has no policy, so `captured_token` would be `None` for urlencoded requests and a server checking it would skip the check, found by review) | one type a server can cache per request whatever the encoding, with the same token semantics, without a second vocabulary for the same accessors |
| D-099 | 2026-10-26 | Feature `unicode` (unicode-lang 1.0.1): `FileName::nfc`, and `sanitized` normalizes to NFC first when it is on | the locked 0.2 design; NFC stays optional because the normalization tables are the largest code in the dependency tree |
| D-100 | 2026-10-26 | Every ```rust block of `docs/API.md` is compiled as a doctest (`#[cfg(doctest)]` with `include_str!`); signature listings are `rust,ignore` | "documented with runnable examples" (the 0.5 exit) is then checked by `cargo test`, and the contract cannot drift from the code unnoticed |
| D-101 | 2026-10-26 | A stored file whose temporary name cannot be removed keeps its bytes reserved in the shared budget; `sweep` skips entries it cannot read or remove and goes on | both found by review: releasing bytes that are still on disk lets the disk fill past the quota when deletes fail (a scanner holding files on Windows); one locked file must not stop a startup sweep from cleaning the rest |
| D-087 | 2026-10-23 | T-33 (1,000 concurrent parses within `memory_bound`) moves from 0.4 to 0.5 | it measures against `Limits::memory_bound` (#25) and `Store::memory_bound` (#71), both 0.5 items; it runs with the 0.5 `memory_bound.rs` peak-tracking test |
