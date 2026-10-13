# form-net - Project Plan

> The design, strategy, and decision record for `form-net`. A living document: every design decision, idea, or open question about form-net is written here or in [`ROADMAP.md`](./ROADMAP.md) before it is acted on.
>
> **Read order:** [`../REPS.md`](../REPS.md) &rarr; [`DIRECTIVES.md`](./DIRECTIVES.md) &rarr; **this file** &rarr; [`ROADMAP.md`](./ROADMAP.md).
>
> **Status:** scaffold, `0.1.0`. Design lock is `0.2.0`.

---

## 1. Purpose

form-net parses form submissions: `application/x-www-form-urlencoded` bodies and query strings, and `multipart/form-data` uploads. It is sans-IO and `no_std` (with `alloc`): bytes in, fields and file parts out.

## 2. Where it sits

```
http-net (body bytes) ─▶ form-net (fields, parts) ─▶ customs-sec (inspection) ─▶ web-net extractors
```

form-net decides whether the bytes are a well-formed form. customs-sec decides whether the form is acceptable for the route.

## 3. urlencoded

- WHATWG `application/x-www-form-urlencoded` parsing: `&`-separated pairs, `+` as space, percent-escapes decoded, strict UTF-8 after decoding (invalid sequences refused, not replaced).
- Zero-copy when a name or value needs no decoding; decoded into a caller-provided buffer otherwise.
- Serialization for building query strings and redirect targets.

## 4. multipart

- RFC 7578 `multipart/form-data`, streaming: feed body bytes as they arrive; events come out as parts complete.

```
Event = PartStart { name, filename, content_type } | Data(&[u8]) | PartEnd | End | NeedInput
```

- Boundary taken from the `Content-Type` parameter (a helper parses it), validated per RFC 2046 (length, characters).
- Boundary search is linear time over the whole body, however the input is split; no rescanning.
- Part headers parsed with their own budget; `Content-Disposition` `name` and `filename` (including RFC 5987 `filename*`) decoded; file names are passed through raw for path-lib and customs-sec to judge, never used as paths here.
- Preamble and epilogue ignored within budget; nested multipart refused.

## 5. Budgets

| Budget | Default (proposed) |
|---|---|
| total body | 10 MiB (route policy overrides) |
| fields | 1,000 |
| name length | 256 B |
| value length (non-file) | 1 MiB |
| parts | 100 |
| part header section | 8 KiB |
| file part size | route policy |

Each is checked before the bytes it governs are buffered. Exceeding one is a precise error.

## 6. Threats

| Threat | Defence |
|---|---|
| hash flooding through many fields | field count budget; no hash map inside form-net (callers choose their structure) |
| boundary tricks (boundary inside data, near-miss boundaries, split boundaries) | exact RFC matching, split-invariance tests |
| quadratic scanning | linear-time search |
| header bombs inside parts | per-part header budget |
| malformed encodings | strict decoding, refusal |
| huge single values | value budget before buffering |

## 7. Architecture

```
src/
  lib.rs         crate docs, Tier-1 entry points
  urlencoded.rs  parse and serialize
  multipart.rs   streaming parser, events
  boundary.rs    Content-Type parameter parsing, boundary validation, search
  disposition.rs Content-Disposition and filename* decoding
  budget.rs      Budgets
  error.rs       FormError (#[non_exhaustive])
```

## 8. Testing

RFC and WHATWG cases as data, each run whole and byte at a time; property tests (split invariance, no panics on arbitrary bytes, round trips); fuzz targets for both parsers; a differential test against a simple reference multipart parser; benchmarks at 1 KiB, 1 MiB, and 100 MiB bodies.

## 9. Open decisions

| Id | Question | Recommendation |
|---|---|---|
| O-1 | Publish `0.1.0` to crates.io to claim the name? | yes |

## 10. Decision log

| Id | Date | Decision | Reason |
|---|---|---|---|
| D-001 | 2026-10-13 | Sans-IO, `no_std` with `alloc` | the same parser serves any server and tests without sockets |
| D-002 | 2026-10-13 | Streaming multipart events, never the whole upload in memory | large uploads must not cost their size in RAM |
| D-003 | 2026-10-13 | Strict decoding: refuse, never replace | a parser that repairs input creates differentials |
