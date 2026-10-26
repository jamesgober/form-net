# form-net architecture

How the crate is built, as of 0.5. The contract is [`API.md`](./API.md); the reasons are in the decision log of [`../dev/PLAN.md`](../dev/PLAN.md). This file is the map between the two.

---

## Shape

form-net is sans-IO. Every parser takes bytes and returns events; nothing in the crate opens a socket, starts a thread, sets a timer, or reads a clock (except `sweep`, at startup). The only I/O is the file system behind the `fs` feature, and the `std::io` adapters behind `std`.

There are three tiers over one engine per format:

| Tier | What | Allocation |
|---|---|---|
| 1 | `parse_urlencoded`, `parse_query`, `parse_multipart`, `read_multipart`, `form!` and `FromForm` | owned results |
| 2 | the `_with` and `_inspected` forms, `Store`, `UploadSession`, `MultipartReader`, `MultipartWriter` | owned results, caller limits |
| 3 | `raw_pairs`, `decode_into`, `UrlencodedParser`, `MultipartParser`, `Driver`, `encode_part_head` | none after construction; caller buffers |

Every Tier 1 and Tier 2 function runs the Tier 3 engine. There is no second parser to disagree with the first.

---

## Modules

```
src/
  lib.rs              crate docs, re-exports, VERSION, Send/Sync assertions
  error.rs            FormError (Copy), reason enums, Location, status_hint, code
  limits.rs           Limits (effective values, memory_bound), Policy and its enums
  kind.rs             FormKind::detect over parse-net's media-type grammar
  scan.rs             SWAR byte search shared by both formats
  hook.rs             Inspector (and chains), Verdict, Refusal, Sink, DiscardSink, MemorySink
  wipe.rs             crypt-io's volatile wipe, wiping growth, WipeVec
  random.rs           (random) 128-bit hex from the OS random source
  io.rs               (std) MultipartReader, PartReader
  urlencoded/
    mod.rs            parse_urlencoded*, parse_query, pairs, Form
    raw.rs            raw_pairs, RawPair, the shared per-sequence Budget
    stream.rs         UrlencodedParser
    decode.rs         decode_into: + and %XX in one pass, then UTF-8
    encode.rs         serialize_urlencoded, Serializer, encode_into
  multipart/
    mod.rs            parse_multipart*, Multipart, Part
    boundary.rs       Boundary: validation, delimiter, KMP failure table, generate
    parser.rs         MultipartParser state machine, Step, Event, PartHead
    header.rs         header blocks, Content-Disposition, part budgets
    filename.rs       FileName views
    driver.rs         Driver: the one implementation of the hook order
    owned_head.rs     OwnedPartHead
    writer.rs         encode_part_head, MultipartWriter
  extract/
    mod.rs            FromForm, FormBuilder, FromField, FileValue, FormErrors
    value.rs          FromField for the value types
    secret.rs         Secret
    file.rs           FileData
    macros.rs         form!
  store/              (fs)
    mod.rs            Store, StoreSink, StoredFile, Upload, UploadSession, read_multipart
    temp.rs           private directory, random names, platform open flags
    quota.rs          SpillBudget, SpillQuota
    sweep.rs          sweep
```

---

## urlencoded

`raw_pairs` splits on `&` only, finds `=` `%` `+` in the same SWAR pass, and hands out borrowed sequences. A `Budget` (field count, name length, value length, text total) admits each complete sequence. The decoder turns `+` and `%XX` into bytes and checks UTF-8 in one pass, into a caller buffer or a `Cow`.

`UrlencodedParser` is the same scanner over chunks. A sequence that ends inside a chunk is borrowed from it; one that straddles chunks is copied into a scratch buffer of `max_name_bytes + max_value_bytes + 1` bytes. Before any byte is held, `Budget::admit_partial` checks what is known so far and, when the complete sequence is certain to be refused, refuses it now with the error `admit` would give. So the scratch never overflows, and pairs and errors are the same for every split. The body budget is enforced on bytes consumed: pairs whose `&` lies inside the budget are emitted, then `Limit { Body }`.

## multipart

`MultipartParser` is a state machine over a streaming KMP matcher for `CRLF--boundary`. The matcher starts in state 2, as if the CRLF before the first delimiter had been seen, so a body that opens with the delimiter and one with a preamble take the same path. From state 0 it skips eight bytes at a time while none is a CR. Every byte is examined a bounded number of times whatever the split (T-4).

Each header block is buffered whole (up to `max_part_header_bytes`) and read by `header.rs` through parse-net: one `Content-Disposition` of type `form-data`, quoted strings read with backslash as a literal and `\"` refused, starred parameters other than `filename*` refused, and the strictness rules of `API.md`. Errors carry split-invariant locations (D-080).

The buffered API is the streaming parser fed the whole body. Part data and names borrow the body.

## Hooks and the driver

The `Driver` owns the hook order, so no caller can get it wrong:

```
PartStart ─▶ Inspector::part_start
first sniff_bytes of a file ─▶ Inspector::sniff
Data ─▶ Inspector::data ─▶ (file) Sink::open on the first accepted byte, then Sink::write
                         └▶ (text) buffered for Inspector::field
PartEnd ─▶ Inspector::field (text) ─▶ Sink::close (file) ─▶ Inspector::part_end
End ─▶ Inspector::end
any error or Refuse ─▶ Sink::abort ─▶ Inspector::failed (once)
```

The sink is opened lazily (D-089): a part refused or skipped by the inspector makes no sink call at all. `UploadSession`, `Upload::parse_with`, `parse_multipart_inspected`, and `FromForm::from_body_inspected` all run this one driver, or follow the same order over a buffered body. Inspectors chain as pairs: `(customs, (sentinel, app))`, the first refusal winning.

## Storage

`Store` is shared state behind an `Arc`: the private directory (created once, or lazily for one-shot parses), counters, and an optional `SpillBudget`. Each request gets a `StoreSink`. A file stays in memory up to the threshold, in a buffer capped at the most it can hold; past it the file spills to `upload-<32 hex>.part`, opened with `create_new`, owner-only on Unix, and in the default anonymous mode unlinked (Unix) or opened delete-on-close with no sharing (Windows). Before every disk write the sink checks the request's `max_disk_bytes` and reserves from the shared budget; every reserved byte is released once, when its file is deleted or persisted. `StoredFile` reads with positional reads, so readers do not share a cursor. `persist` never overwrites and removes a partial copy on failure. `sweep` removes named-mode leftovers at startup. Platform details are in [`PLATFORM-NOTES.md`](./PLATFORM-NOTES.md).

## Typed extraction

`form!` is a `macro_rules!` macro: it emits the struct as written and a builder inside an unnameable `const _: () = { ... }` block. The builder holds one `FromField::Slot` per field; names are matched with a chain of byte comparisons. Arity rules live in the slots (`Option<T>` refuses a second value, `Vec<T>` pushes, `bool` finishes absent as `false`). `finish_all` collects every field's error for re-rendering a form; `file_field` lets `from_upload_mut` move only declared files out of an upload.

## The client side

`encode_part_head` checks every input before writing a byte, then writes the delimiter, the `Content-Disposition` line with the browser escapes (`%0A`, `%0D`, `%22`), the optional `Content-Type`, and the blank line. It refuses what could read back differently: other control bytes, a backslash in a name, a trailing backslash in a file name, a literal `%0A` `%0D` `%22`, a content type parse-net does not accept or that is `multipart/*`. `MultipartWriter` adds CRLF normalisation for text and, under a caller boundary, runs the parser's own matcher over the data to refuse a delimiter inside it. The property test and the `fuzz_writer_roundtrip` target hold it to "every body parses back to the same fields".

## Memory

`Limits::memory_bound` is the worst case of one parse, term by term (`limits.rs`), and `tests/memory_bound.rs` measures hostile bodies against it with a peak-tracking allocator. Buffers that grow do so through `append_wiping`, which wipes the old allocation after each move; the bound includes half the largest growing buffer for that move. Owned values that can hold form bytes are wiped on drop with crypt-io's volatile wipe (D-083).
