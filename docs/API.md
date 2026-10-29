# form-net - API Reference

> Reference for every public item in `form-net`: signature, behaviour, the errors each item can return, cost, and examples.
> **Current release: `0.5.0` (feature complete).** This document, locked at `0.2.0`, is the contract for `0.3.0` to `1.0.0`. Each item carries the version that ships it; items marked with a later version than the current release are specified here and not yet in the crate, and their examples are not compiled until they ship. Names and signatures do not change after `0.2.0` without a decision-log entry in [`../dev/PLAN.md`](../dev/PLAN.md).

<sub>Copyright &copy; 2026 <strong>James Gober</strong>.</sub>

## Table of contents

- [Conventions](#conventions)
  - [Ids, grades, tiers, versions](#ids-grades-tiers-versions)
  - [Feature flags](#feature-flags)
  - [Strict by default](#strict-by-default)
  - [Refusal and poisoning](#refusal-and-poisoning)
  - [Borrowing](#borrowing)
- [Tier 1 at a glance](#tier-1-at-a-glance)
- [Crate root](#crate-root)
  - [`VERSION`](#version)
- [Detection](#detection)
  - [`FormKind`](#formkind)
- [urlencoded](#urlencoded)
  - [`parse_urlencoded`](#parse_urlencoded)
  - [`parse_query`](#parse_query)
  - [`pairs`, `Pairs`](#pairs-pairs)
  - [`Form`](#form)
  - [`serialize_urlencoded`](#serialize_urlencoded)
  - [`parse_urlencoded_with`](#parse_urlencoded_with)
  - [`parse_urlencoded_inspected`](#parse_urlencoded_inspected)
  - [`Serializer`](#serializer)
  - [`raw_pairs`, `RawPairs`, `RawPair`](#raw_pairs-rawpairs-rawpair)
  - [`decode_into`](#decode_into)
  - [`encode_into`](#encode_into)
  - [`UrlencodedParser`, `UrlencodedStep`, `UrlencodedEvent`](#urlencodedparser-urlencodedstep-urlencodedevent)
- [multipart, buffered](#multipart-buffered)
  - [`parse_multipart`](#parse_multipart)
  - [`Multipart`, `Part`](#multipart-part)
  - [`parse_multipart_with`](#parse_multipart_with)
  - [`parse_multipart_inspected`](#parse_multipart_inspected)
- [multipart, streaming](#multipart-streaming)
  - [`Boundary`](#boundary)
  - [`MultipartParser`](#multipartparser)
  - [`Step`, `Event`](#step-event)
  - [`PartHead`](#parthead)
  - [`PartKind`, `Charset`](#partkind-charset)
  - [`ContentType`](#contenttype)
  - [`Headers`](#headers)
  - [`PartSummary`](#partsummary)
- [File names](#file-names)
  - [`FileName`, `FileNameSource`](#filename-filenamesource)
- [Limits and policy](#limits-and-policy)
  - [`Limits`](#limits)
  - [`Policy`](#policy)
  - [`PercentPolicy`](#percentpolicy)
  - [`ExtendedFilenamePolicy`](#extendedfilenamepolicy)
  - [`HeaderPolicy`](#headerpolicy)
  - [`EmptyFilePolicy`](#emptyfilepolicy)
  - [`TokenField`, `TokenPlacement`](#tokenfield-tokenplacement)
- [Hooks](#hooks)
  - [`Inspector`](#inspector)
  - [`Verdict`, `Refusal`, `NoInspector`](#verdict-refusal-noinspector)
  - [`Sink`, `DiscardSink`, `MemorySink`](#sink-discardsink-memorysink)
  - [`Driver`, `Progress`](#driver-progress)
- [Storage](#storage)
  - [`read_multipart`](#read_multipart)
  - [`Store`](#store)
  - [`SpillMode`](#spillmode)
  - [`StoreStats`](#storestats)
  - [`StoreSink`](#storesink)
  - [`StoredFile`, `StoredReader`](#storedfile-storedreader)
  - [`Upload`](#upload)
  - [`UploadSession`](#uploadsession)
  - [`SpillBudget`, `SpillQuota`](#spillbudget-spillquota)
  - [`sweep`](#sweep)
- [Blocking readers](#blocking-readers)
  - [`MultipartReader`, `PartReader`](#multipartreader-partreader)
- [Typed extraction](#typed-extraction)
  - [`form!`](#form-1)
  - [`FromForm`](#fromform)
  - [`FormBuilder`](#formbuilder)
  - [`FromField`, `ValueError`](#fromfield-valueerror)
  - [`FileValue`](#filevalue)
  - [Value rules](#value-rules)
  - [`Secret`](#secret)
  - [`FileData`](#filedata)
- [Client side](#client-side)
  - [`MultipartWriter`](#multipartwriter)
  - [`Boundary::generate`](#boundarygenerate)
  - [`encode_part_head`](#encode_part_head)
- [Errors](#errors)
  - [`FormError`](#formerror)
  - [`Location`](#location)
  - [Reason enums](#reason-enums)
  - [Every error, and what to do](#every-error-and-what-to-do)
  - [`status_hint`](#status_hint)
  - [`code`](#code)
- [Thread safety and sizes](#thread-safety-and-sizes)
- [Enterprise and distributed use](#enterprise-and-distributed-use)
- [Consumer checklists](#consumer-checklists)
- [After 1.0](#after-10)
- [Item index](#item-index)

---

## Conventions

### Ids, grades, tiers, versions

Every item has an id (`#7`), a **grade**, a **tier**, the **version** that ships it, and the **feature** that gates it. The [item index](#item-index) at the end lists all of them.

| Grade | Meaning |
|---|---|
| P0 | the crate's job cannot be done without it |
| P1 | what users of the best libraries of this kind expect |
| P2 | strongly wanted by serious users (high traffic, security, operations) |
| P3 | after 1.0 is acceptable |

| Tier | Meaning |
|---|---|
| 1 | one plain call: a free function, a one-line constructor, or a provided trait method; no configuration, no type parameters to name |
| 2 | configuration: explicit `Limits` and `Policy`, inspectors, stores |
| 3 | the full allocation-free surface, for other libraries and drivers |

Costs are targets marked **[est.]** until `0.6.0` measures them; `docs/PERFORMANCE.md` will hold the measured numbers.

### Feature flags

| Feature | Default | Implies | Adds | Dependencies |
|---|---|---|---|---|
| (none) | | | the Tier 3 core: `raw_pairs`, `decode_into`, `encode_into`, `Boundary`, `MultipartParser::with_buffer`, `UrlencodedParser::with_buffer`, `Inspector`, `Sink`, `DiscardSink`, `encode_part_head`, `FileName`, `Limits`, `Policy`, every error type; `no_std` without `alloc` | `parse-net` |
| `alloc` | | | `Form`, `pairs`, `parse_urlencoded*`, `parse_query`, `serialize_urlencoded`, `Serializer`, `parse_multipart*`, `Multipart`, `MultipartParser::new`, `UrlencodedParser::new`, `MemorySink`, `Driver`, typed extraction, `Secret`, `FileData` | `crypt-io` (D-083) |
| `std` | yes | `alloc` | `MultipartReader`, `MultipartWriter::with_boundary`, `FormError::Io`, `FormError::Random` | |
| `random` | | `std` | `Boundary::generate`, `MultipartWriter::new` | `mod-rand` |
| `fs` | | `std`, `random` | `read_multipart`, `Store`, `StoredFile`, `Upload`, `UploadSession`, `SpillBudget`, `SpillQuota`, `sweep`, `FileName::sanitized`, `PartReader::save`, `FormError::Store` | `path-lib` |
| `unicode` | | `alloc` | `FileName::nfc`; `FileName::sanitized` normalizes to NFC first when both are on | `unicode-lang` |

Every dependency is first-party and used at its published version. The only third-party code is `zeroize` and `subtle`, which `crypt-io` brings under `alloc` for the volatile wipe: the portfolio's audited-cryptography exception (D-083). The allocation-free core depends on `parse-net` alone.

### Strict by default

form-net refuses any input that two reasonable parsers could read differently, and never repairs input. Every leniency is opt-in, named after what it allows, and documented with the differential it risks. The deliberate differences from the WHATWG and RFC texts are listed under each item and gathered in `docs/SECURITY.md` at `0.7.0`. Every body that current Chrome, Firefox, and Safari send for a form is accepted (NFR-14); a test corpus of browser captures proves it.

### Refusal and poisoning

"Refuses" means the call returns the named `FormError`. A streaming parser that refused is **poisoned**: every later call returns `FormError::Poisoned` until `reset`. Drivers that hold sinks abort every open sink and drop every finished file before they return the error, so no partial upload survives a refusal.

### Borrowing

Views (`RawPair`, `PartHead`, `FileName`, `ContentType`, `Part`) borrow the caller's input or the parser's own buffer; they never copy. Owned results (`Form`, `Upload`, `Secret`, `FileData`, `StoredFile`) own their bytes and wipe text values on drop (best effort without `unsafe`, see [`Secret`](#secret)). No `Debug`, `Display`, or error output of form-net ever contains a field value or a file name.

---

## Tier 1 at a glance

The convention in every example is `use form_net as fnet;` where a short prefix helps.

```rust
use form_net as fnet;
use fnet::FromForm;

// A urlencoded body (0.3)
let form = fnet::parse_urlencoded(b"email=a%40b.example&remember=on")?;
assert_eq!(form.get("email"), Some("a@b.example"));
assert!(form.contains("remember"));

// A query string (0.3)
let query = fnet::parse_query(b"?q=rust+forms&page=2")?;
assert_eq!(query.get("q"), Some("rust forms"));

// Building one (0.3)
assert_eq!(fnet::serialize_urlencoded([("q", "rust forms"), ("page", "2")]), "q=rust+forms&page=2");

// A buffered multipart body (0.3)
# let content_type = b"multipart/form-data; boundary=X";
# let body = b"--X\r\nContent-Disposition: form-data; name=\"title\"\r\n\r\nHoliday\r\n--X\r\nContent-Disposition: form-data; name=\"avatar\"; filename=\"me.png\"\r\nContent-Type: image/png\r\n\r\n\x89PNG...\r\n--X--\r\n";
let mp = fnet::parse_multipart(content_type, body)?;
let title = mp.field("title").unwrap_or("");
if let Some(avatar) = mp.file("avatar") {
    let bytes: &[u8] = avatar.bytes();
}

// A typed form, either encoding (0.4)
fnet::form! {
    pub struct Login {
        pub email: String,
        pub password: fnet::Secret,
        pub remember: bool,
    }
    ignore = ["csrf_token"];
}
# let content_type = b"application/x-www-form-urlencoded";
# let body = b"email=a%40b.example&password=pw&remember=on&csrf_token=t";
let login = Login::from_body(content_type, body)?;

// An upload streamed to private temporary files (0.4, feature `fs`)
# let content_type = b"multipart/form-data; boundary=X";
# let request_body_reader = &b"--X\r\nContent-Disposition: form-data; name=\"document\"; filename=\"a.txt\"\r\n\r\nhello\r\n--X--\r\n"[..];
# let destination = std::env::temp_dir().join(format!("form-net-api-glance-{}.txt", std::process::id()));
let mut upload = fnet::read_multipart(content_type, request_body_reader)?;
if let Some(file) = upload.take_file("document") {
    file.persist(&destination)?;          // never overwrites
}
# std::fs::remove_file(&destination).unwrap();
# Ok::<(), form_net::FormError>(())
```

---

## Crate root

### `VERSION`

`#1` · P0 · Tier 1 · 0.1 · core

```rust,ignore
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
```

The crate version, populated by Cargo at build time.

```rust
println!("form-net {}", form_net::VERSION);
```

---

## Detection

### `FormKind`

`#2` · P0 · Tier 1 · 0.3 · core

```rust,ignore
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FormKind {
    Urlencoded,
    Multipart(Boundary),
}

impl FormKind {
    pub fn detect(content_type: &[u8]) -> Result<FormKind, FormError>;
}
```

Reads a request's `Content-Type` value and says which form encoding it is. It is a pure function of the header bytes and never looks at the body.

**Behaviour**

- The value is parsed with parse-net's media-type grammar (RFC 9110 section 8.3.1). Type and subtype compare ASCII case-insensitively; repeated parameters are refused.
- `application/x-www-form-urlencoded` gives `Urlencoded`. Its parameters are ignored, except a `charset` that is not `utf-8` or `us-ascii` (ASCII case-insensitive, quoted or not), which is refused.
- `multipart/form-data` gives `Multipart` with the validated boundary: exactly one `boundary` parameter, token or quoted, 1 to 70 bytes of RFC 2046 `bchars`, not ending in a space.
- Anything else, including `text/plain`, `multipart/mixed`, and `application/json`, is `UnsupportedMediaType`. `text/plain` form bodies are not machine-readable by the HTML standard's own account and are never parsed.

**Errors**

| Error | When |
|---|---|
| `UnsupportedMediaType` | any other media type, or an empty value |
| `Syntax { InvalidContentType }` | parse-net refuses the value (bad token, trailing bytes), or a parameter other than `boundary` is repeated |
| `Syntax { MissingBoundary }` | `multipart/form-data` without `boundary` |
| `Syntax { DuplicateBoundary }` | two `boundary` parameters, whatever their values (form-net reads the parameters itself, so this is never reported as a generic repeated parameter) |
| `Syntax { InvalidBoundary }` | empty, longer than 70 bytes, a byte outside `bchars`, or a trailing space |
| `Encoding { UnsupportedCharset }` | a urlencoded `charset` other than UTF-8 or US-ASCII |

**Cost:** under 100 ns for a browser value (`multipart/form-data; boundary=----WebKitFormBoundary` plus 16 characters), no allocation [est.].

```rust
use form_net::{FormKind, FormError};

match FormKind::detect(b"multipart/form-data; boundary=----WebKitFormBoundaryx7Y")? {
    FormKind::Urlencoded => { /* parse_urlencoded */ }
    FormKind::Multipart(boundary) => assert_eq!(boundary.as_bytes(), b"----WebKitFormBoundaryx7Y"),
    _ => return Err(FormError::UnsupportedMediaType),
}

assert_eq!(FormKind::detect(b"Application/X-WWW-Form-Urlencoded; charset=UTF-8")?, FormKind::Urlencoded);
assert_eq!(FormKind::detect(b"text/plain"), Err(FormError::UnsupportedMediaType));
# Ok::<(), FormError>(())
```

---

## urlencoded

The urlencoded parser follows the WHATWG URL standard section 5.1, with strict refusal where WHATWG would silently repair:

| Input | WHATWG | form-net |
|---|---|---|
| separators | `&` only | `&` only; `;` is an ordinary byte (FR-4) |
| empty sequences (`&&`) | skipped | skipped; their bytes still count toward `max_body_bytes` |
| a sequence without `=` | name with an empty value | the same; the raw view reports the missing `=` |
| `+` | space | space |
| `%` not followed by two hex digits | kept as a literal byte | refused under the default `PercentPolicy::Refuse`; kept under `PercentPolicy::Literal` |
| invalid UTF-8 after decoding | replaced with U+FFFD | refused, always (no option: replacement merges distinct inputs) |
| order and duplicates | kept | kept; nothing is merged, sorted, or deduplicated |

### `parse_urlencoded`

`#3` · P0 · Tier 1 · 0.3 · `alloc`

```rust,ignore
pub fn parse_urlencoded(input: &[u8]) -> Result<Form, FormError>;
```

Parses a whole `application/x-www-form-urlencoded` body with `Limits::DEFAULT` and `Policy::DEFAULT` and returns the fields, owned, in input order.

**Errors:** `Limit { Body | Fields | NameLength | ValueLength | TextTotal }`; `Encoding { InvalidPercent | InvalidUtf8 }`.

**Cost:** two allocations however many fields; under 400 ns for a 10-field, 300-byte body [est.]; at least 20% faster than `form_urlencoded::parse` collected into a `Vec<(String, String)>`.

```rust
let form = form_net::parse_urlencoded(b"email=a%40b.example&remember=on")?;
assert_eq!(form.get("email"), Some("a@b.example"));
assert_eq!(form.get("remember"), Some("on"));
assert_eq!(form.len(), 2);

// `;` is not a separator.
let form = form_net::parse_urlencoded(b"a=1;b=2")?;
assert_eq!(form.get("a"), Some("1;b=2"));

// Invalid input is refused, never repaired.
assert!(form_net::parse_urlencoded(b"name=%zz").is_err());
assert!(form_net::parse_urlencoded(b"name=%C0%AF").is_err());     // overlong UTF-8
# Ok::<(), form_net::FormError>(())
```

### `parse_query`

`#4` · P1 · Tier 1 · 0.3 · `alloc`

```rust,ignore
pub fn parse_query(query: &[u8]) -> Result<Form, FormError>;
```

As `parse_urlencoded`, after stripping exactly one leading `?` if there is one. A second `?` is part of the first name. The fragment never reaches a server; a caller that has a full URL passes the query component only.

```rust
let q = form_net::parse_query(b"?q=rust+forms&page=2")?;
assert_eq!(q.get("q"), Some("rust forms"));
assert_eq!(form_net::parse_query(b"q=x")?.get("q"), Some("x"));
assert_eq!(form_net::parse_query(b"??q=x")?.get("?q"), Some("x"));
# Ok::<(), form_net::FormError>(())
```

### `pairs`, `Pairs`

`#5` · P0 · Tier 1 · 0.3 · `alloc`

```rust,ignore
pub fn pairs(input: &[u8]) -> Pairs<'_>;

pub struct Pairs<'a> { /* raw scanner, limits */ }

impl<'a> Iterator for Pairs<'a> {
    type Item = Result<(Cow<'a, str>, Cow<'a, str>), FormError>;
}
```

Lazy iteration with the default limits. A name or value that contains no `%` and no `+` is borrowed from the input (FR-8); others are decoded into a new `String`. After the first `Err` the iterator returns `None`.

The decoded `String`s belong to the caller and are not wiped by form-net. For secret fields use `Form` (which wipes its arena) or the Tier 3 decoder with a buffer the caller wipes.

**Errors:** as `parse_urlencoded`, one per item. **Cost:** no allocation for undecoded pairs; one allocation per decoded name or value.

```rust
use std::borrow::Cow;

let mut it = form_net::pairs(b"a=1&b=x+y");
let (name, value) = it.next().unwrap()?;
assert!(matches!((name, value), (Cow::Borrowed("a"), Cow::Borrowed("1"))));
let (_, value) = it.next().unwrap()?;
assert!(matches!(value, Cow::Owned(ref s) if s == "x y"));
assert!(it.next().is_none());
# Ok::<(), form_net::FormError>(())
```

### `Form`

`#7` · P0 · Tier 1 · 0.3 · `alloc`

```rust,ignore
#[must_use]
pub struct Form { /* one arena: Vec<u8>; one span table: Vec<Span> */ }

impl Form {
    pub fn get(&self, name: &str) -> Option<&str>;
    pub fn get_one(&self, name: &str) -> Result<Option<&str>, FormError>;
    pub fn get_all<'f>(&'f self, name: &'f str) -> impl Iterator<Item = &'f str> + 'f;
    pub fn iter(&self) -> impl Iterator<Item = (&str, &str)>;
    pub fn names(&self) -> impl Iterator<Item = &str>;
    pub fn len(&self) -> usize;
    pub fn is_empty(&self) -> bool;
    pub fn contains(&self, name: &str) -> bool;
}

impl Drop for Form { /* wipes the arena */ }
impl core::fmt::Debug for Form { /* names and value lengths only */ }
```

The owned result of a urlencoded parse.

**Behaviour**

- Every decoded name and value lives in one arena allocated once, sized from the input length (decoding never makes text longer), and one table of `u32` spans sized from the `&` count of the first scan, both capped by the limits. Neither reallocates.
- Lookups are linear scans. There is no hash map anywhere in form-net, so hash flooding cannot apply; `max_fields` bounds every scan.
- `get` returns the first value, as browsers' `URLSearchParams.get` does. `get_one` is the strict accessor: it refuses a name given more than once (parameter pollution defence). `get_all` yields every value in input order.
- `iter` yields `(name, value)` in input order; `names` yields names in input order, repeats included.
- On drop the used part of the arena is overwritten with zeros. `Debug` prints names and value lengths, never values: `Form { email: <11 bytes>, password: <9 bytes> }`.
- `Form` does not implement `Clone`: a clone would be a second copy of every secret to wipe.

**Errors:** `get_one` returns `Extract { reason: Duplicate, field: None }` for a repeated name.

**Cost:** `get` under 5 ns per field compared [est.]; no method allocates.

```rust
let form = form_net::parse_urlencoded(b"role=user&role=admin&tag=red&tag=blue")?;
assert_eq!(form.get("role"), Some("user"));                 // first wins, documented
assert!(form.get_one("role").is_err());                     // strict: refused
assert_eq!(form.get_one("missing")?, None);
assert_eq!(form.get_all("tag").collect::<Vec<_>>(), ["red", "blue"]);
assert_eq!(form.iter().count(), 4);
assert_eq!(format!("{form:?}"), "Form { role: <4 bytes>, role: <5 bytes>, tag: <3 bytes>, tag: <4 bytes> }");
# Ok::<(), form_net::FormError>(())
```

### `serialize_urlencoded`

`#6` · P1 · Tier 1 · 0.3 · `alloc`

```rust,ignore
pub fn serialize_urlencoded<I, N, V>(pairs: I) -> String
where
    I: IntoIterator<Item = (N, V)>,
    N: AsRef<str>,
    V: AsRef<str>;
```

Builds a urlencoded body or query, byte-identical to the WHATWG serializer for UTF-8: every byte outside ASCII alphanumerics and `*-._` is percent-encoded with uppercase hex, space is written as `+`, names and values are joined with `=`, pairs with `&`. Infallible: every `str` is valid input.

**Cost:** one allocation when the iterator can be walked twice cheaply (the capacity is computed first) [est. at least 1 GB/s on ASCII alphanumeric text].

```rust
assert_eq!(form_net::serialize_urlencoded([("q", "rust forms"), ("page", "2")]), "q=rust+forms&page=2");
assert_eq!(form_net::serialize_urlencoded([("a+b", "1&2=3")]), "a%2Bb=1%262%3D3");
assert_eq!(form_net::serialize_urlencoded([("city", "Zürich")]), "city=Z%C3%BCrich");
assert_eq!(form_net::serialize_urlencoded(Vec::<(&str, &str)>::new()), "");
```

### `parse_urlencoded_with`

`#32` · P0 · Tier 2 · 0.3 · `alloc`

```rust,ignore
pub fn parse_urlencoded_with(input: &[u8], limits: &Limits, policy: &Policy) -> Result<Form, FormError>;
```

As `parse_urlencoded` with explicit limits and policy. `limits.validate()` runs first. Of the policy, only `percent` applies to urlencoded bodies.

**Errors:** as `parse_urlencoded`; `InvalidLimits { limit }` from `validate`.

```rust
use form_net::{Limits, Policy, PercentPolicy};

// A login route: a few short text fields, nothing else.
const LOGIN: Limits = Limits::DEFAULT
    .with_max_body_bytes(4 * 1024)
    .with_max_fields(8);

let form = form_net::parse_urlencoded_with(b"email=a%40b.example&password=x", &LOGIN, &Policy::DEFAULT)?;
assert_eq!(form.len(), 2);

// Mirror browsers' URLSearchParams for a malformed `%`.
const LIKE_BROWSERS: Policy = Policy::DEFAULT.with_percent(PercentPolicy::Literal);
let form = form_net::parse_urlencoded_with(b"discount=100%", &Limits::DEFAULT, &LIKE_BROWSERS)?;
assert_eq!(form.get("discount"), Some("100%"));
# Ok::<(), form_net::FormError>(())
```

### `parse_urlencoded_inspected`

`#63` · P1 · Tier 2 · 0.4 · `alloc`

```rust,ignore
pub fn parse_urlencoded_inspected<I: Inspector>(
    input: &[u8],
    limits: &Limits,
    policy: &Policy,
    inspector: &mut I,
) -> Result<Form, FormError>;
```

As `parse_urlencoded_with`, calling `inspector.field(name, value)` for every pair in input order (decoded value bytes), then `inspector.end()`. On any error, including a refusal, `inspector.failed(&error)` is called once before the error is returned. A `Verdict::Skip` from `field` drops that pair from the returned `Form`. This is the single-pass shape for content inspection: the inspector and the handler see the same decoded bytes, and inspection never runs on a body form-net refuses.

**Errors:** as `parse_urlencoded_with`, plus `Refused { refusal, at }`.

```rust
use form_net::{Inspector, Limits, Policy, Refusal, Verdict};

struct NoAdmins;
impl Inspector for NoAdmins {
    fn field(&mut self, name: &str, value: &[u8]) -> Verdict {
        if name == "role" && value == b"admin" {
            Verdict::Refuse(Refusal::new(422, "example.role_not_allowed"))
        } else {
            Verdict::Continue
        }
    }
}

let err = form_net::parse_urlencoded_inspected(b"role=admin", &Limits::DEFAULT, &Policy::DEFAULT, &mut NoAdmins)
    .unwrap_err();
assert_eq!(err.status_hint(), 422);
```

### `Serializer`

`#41` · P1 · Tier 2 · 0.3 · `alloc`

```rust,ignore
pub struct Serializer<'s> { /* out: &'s mut String, first: bool */ }

impl<'s> Serializer<'s> {
    pub fn new(out: &'s mut String) -> Serializer<'s>;
    pub fn append_pair(&mut self, name: &str, value: &str) -> &mut Self;
    pub fn append_pairs<I, N, V>(&mut self, pairs: I) -> &mut Self
    where
        I: IntoIterator<Item = (N, V)>,
        N: AsRef<str>,
        V: AsRef<str>;
    pub fn finish(self) -> &'s mut String;
}
```

Appends to an existing string, for example a URL that already ends in `?`. Output as `serialize_urlencoded`. The first appended pair is not preceded by `&`; whatever `out` held before is left untouched. Parity with `form_urlencoded::Serializer`.

```rust
let mut url = String::from("https://example.com/search?");
form_net::Serializer::new(&mut url)
    .append_pair("q", "rust forms")
    .append_pairs([("page", "2"), ("sort", "new")]);
assert_eq!(url, "https://example.com/search?q=rust+forms&page=2&sort=new");
```

### `raw_pairs`, `RawPairs`, `RawPair`

`#44` · P0 · Tier 3 · 0.3 · core

```rust,ignore
pub fn raw_pairs<'a>(input: &'a [u8], limits: &Limits) -> RawPairs<'a>;

pub struct RawPairs<'a> { /* input, position, counters */ }

impl<'a> Iterator for RawPairs<'a> {
    type Item = Result<RawPair<'a>, FormError>;
}

#[derive(Clone, Copy, Debug)]
pub struct RawPair<'a> { /* name, value, flags, offset */ }

impl<'a> RawPair<'a> {
    pub fn name_raw(&self) -> &'a [u8];
    pub fn value_raw(&self) -> &'a [u8];
    pub fn has_equals(&self) -> bool;
    pub fn name_needs_decoding(&self) -> bool;
    pub fn value_needs_decoding(&self) -> bool;
    pub fn name_str(&self) -> Option<&'a str>;
    pub fn value_str(&self) -> Option<&'a str>;
    pub fn decode_name<'b>(&self, out: &'b mut [u8], policy: PercentPolicy) -> Result<&'b str, FormError>;
    pub fn decode_value<'b>(&self, out: &'b mut [u8], policy: PercentPolicy) -> Result<&'b str, FormError>;
    pub fn offset(&self) -> u64;
}
```

The scanner every urlencoded API is built on, with no allocation and no copy.

**Behaviour**

- One forward pass. A SWAR loop (eight bytes per step, safe Rust) finds the next `&`, `=`, `%`, or `+`; nothing between them is copied. For each sequence it records where the first `=` is and whether the name and the value contain `%` or `+`.
- Budgets are checked per sequence, before the sequence is handed out: body size (`max_body_bytes`, counting every byte including empty sequences), field count, raw name length, raw value length, and the running total of raw value lengths (`max_text_bytes`).
- `has_equals` tells `a` from `a=` (equal in every decoded view).
- `name_str` and `value_str` are `Some` only when no decoding is needed and the raw bytes are valid UTF-8: the zero-copy fast path.
- `decode_name` and `decode_value` decode into `out` (see `decode_into`); a buffer as long as the raw component always suffices.
- `offset` is the byte offset of the sequence in the input; errors carry it too.
- `Debug` prints offsets and lengths only.

**Errors:** iteration: `Limit { Body | Fields | NameLength | ValueLength | TextTotal }`. `decode_*`: `Encoding { InvalidPercent | InvalidUtf8 }`, `BufferTooSmall { needed }`.

**Cost:** zero allocation (FR-9); scan at least 1.5 GB/s on input without escapes, decode at least 700 MB/s on fully escaped input [est.].

```rust
use form_net::{Limits, PercentPolicy};

let mut scratch = [0u8; 256];
let mut email = None;
for pair in form_net::raw_pairs(b"email=a%40b.example&remember", &Limits::DEFAULT) {
    let pair = pair?;
    let name = match pair.name_str() {
        Some(s) => s,
        None => pair.decode_name(&mut scratch, PercentPolicy::Refuse)?,
    };
    if name == "email" {
        let mut value = [0u8; 256];
        email = Some(pair.decode_value(&mut value, PercentPolicy::Refuse)?.len());
    }
    if name == "remember" {
        assert!(!pair.has_equals());
    }
}
assert_eq!(email, Some(11));
# Ok::<(), form_net::FormError>(())
```

### `decode_into`

`#45` · P0 · Tier 3 · 0.3 · core

```rust,ignore
pub fn decode_into<'b>(raw: &[u8], out: &'b mut [u8], policy: PercentPolicy) -> Result<&'b str, FormError>;
```

Decodes one urlencoded component: `+` becomes space, `%XX` becomes the byte (either hex case), and the result must be valid UTF-8 (no overlong forms, no surrogates, no truncated sequences). The output is never longer than `raw`, so `out.len() >= raw.len()` always suffices; a shorter buffer is checked before anything is written.

**Errors:** `Encoding { InvalidPercent }` (under `Refuse`), `Encoding { InvalidUtf8 }`, `BufferTooSmall { needed: raw.len() }`. The `at` offset is relative to `raw`.

```rust
use form_net::PercentPolicy;

let mut out = [0u8; 32];
assert_eq!(form_net::decode_into(b"a+b%2Fc", &mut out, PercentPolicy::Refuse)?, "a b/c");
assert_eq!(form_net::decode_into(b"100%", &mut out, PercentPolicy::Literal)?, "100%");
assert!(form_net::decode_into(b"100%", &mut out, PercentPolicy::Refuse).is_err());
assert!(form_net::decode_into(b"%ED%A0%80", &mut out, PercentPolicy::Refuse).is_err());   // surrogate
assert_eq!(form_net::decode_into(b"%00", &mut out, PercentPolicy::Refuse)?, "\0");        // U+0000 is valid
# Ok::<(), form_net::FormError>(())
```

### `encode_into`

`#46` · P1 · Tier 3 · 0.3 · core

```rust,ignore
pub fn encode_into<W: core::fmt::Write>(text: &str, out: &mut W) -> core::fmt::Result;
```

Encodes one component (a name or a value) with the rules of `serialize_urlencoded`, without allocating: runs of bytes that need no escape are written with one `write_str`.

```rust
let mut s = String::new();
form_net::encode_into("a b&c", &mut s).unwrap();
assert_eq!(s, "a+b%26c");
```

### `UrlencodedParser`, `UrlencodedStep`, `UrlencodedEvent`

`#47` · P2 · Tier 3 · 0.5 · core (`new`: `alloc`)

```rust,ignore
pub struct UrlencodedParser<B> { /* scratch: B, counters, state */ }

impl<B: AsMut<[u8]> + AsRef<[u8]>> UrlencodedParser<B> {
    pub fn with_buffer(limits: &Limits, scratch: B) -> Result<Self, FormError>;
    pub fn parse<'p>(&'p mut self, input: &'p [u8], eof: bool) -> Result<UrlencodedStep<'p>, FormError>;
    pub fn reset(&mut self);
}

#[cfg(feature = "alloc")]
impl UrlencodedParser<Vec<u8>> {
    pub fn new(limits: &Limits) -> Result<Self, FormError>;
}

#[non_exhaustive]
pub struct UrlencodedStep<'p> {
    pub consumed: usize,
    pub event: UrlencodedEvent<'p>,
}

#[non_exhaustive]
pub enum UrlencodedEvent<'p> {
    Pair(RawPair<'p>),
    NeedInput,
    End,
}
```

The streaming form, for urlencoded bodies too large to buffer or arriving in pieces.

**Behaviour**

- Feed chunks of any size, split at any byte. The pairs are identical for every split (FR-11).
- A sequence that ends inside the current chunk is returned borrowed from the input. A sequence that straddles chunks is assembled in the scratch buffer (`max_name_bytes + max_value_bytes + 1` bytes, `Limits::urlencoded_buffer_len`) and returned borrowed from it. A sequence whose known bytes already prove it too long is refused before it is copied, with the error the complete sequence would get, so errors are split invariant too (D-094).
- `max_body_bytes` is enforced on the bytes consumed: the pairs whose `&` lies inside the budget are returned, then `Limit { Body }`. (`raw_pairs`, which sees the whole body, refuses it before the first pair; the error is the same.)
- `NeedInput` is returned only when every byte of `input` was consumed or held; pass the next chunk. `End` comes only after `eof = true`.
- `reset` clears the counters and the poisoned state and wipes the scratch.

**Errors:** as `raw_pairs`; `InvalidLimits` from `with_buffer` and `new`; `BufferTooSmall { needed }` from `with_buffer`; `Poisoned`.

**Cost:** zero allocation after construction; throughput as `raw_pairs` for large chunks.

```rust
use form_net::{Limits, UrlencodedEvent, UrlencodedParser};

let mut parser = UrlencodedParser::new(&Limits::DEFAULT)?;
let mut names = Vec::new();
for (chunk, last) in [(&b"na"[..], false), (b"me=Al&ci", false), (b"ty=Paris", true)] {
    let mut rest = chunk;
    loop {
        let step = parser.parse(rest, last)?;
        rest = &rest[step.consumed..];
        match step.event {
            UrlencodedEvent::Pair(p) => names.push(String::from_utf8(p.name_raw().to_vec()).unwrap()),
            UrlencodedEvent::NeedInput => break,
            UrlencodedEvent::End => break,
            _ => {}
        }
    }
}
assert_eq!(names, ["name", "city"]);
# Ok::<(), form_net::FormError>(())
```

---

## multipart, buffered

For bodies that are already in memory (the usual case for small forms, where the server buffered the body). Every multipart API runs the same streaming parser; the buffered ones simply feed it the whole body at once.

### `parse_multipart`

`#8` · P0 · Tier 1 · 0.3 · `alloc`

```rust,ignore
pub fn parse_multipart<'a>(content_type: &[u8], body: &'a [u8]) -> Result<Multipart<'a>, FormError>;
```

Detects the boundary, parses the whole body with `Limits::DEFAULT` and `Policy::DEFAULT`, and returns the parts. Part data borrows the body (zero copy); a part's bytes are the exact slice between its blank line and the CRLF before the next delimiter. Names borrow too, except where an HTML escape (`%22`, `%0D`, `%0A`) had to be decoded, which costs one small owned copy for that name.

Text fields are checked as text at parse time (a US-ASCII label is read as UTF-8, of which it is a subset): a text field that is not valid UTF-8, or whose charset (its own, else the `_charset_` field before it, else UTF-8) is not UTF-8 or US-ASCII, refuses the body. That is why `Multipart::field` can return a plain `Option<&str>` without hiding an error. Callers who must accept other charsets use the Tier 3 parser, which hands out bytes.

**Errors:** every multipart error in [Every error](#every-error-and-what-to-do); `UnsupportedMediaType` when the content type is not `multipart/form-data`.

**Cost:** one `Vec<Part>` allocation plus at most one small allocation per escaped name; throughput as the streaming parser.

```rust
let content_type = b"multipart/form-data; boundary=X";
let body = b"--X\r\n\
Content-Disposition: form-data; name=\"title\"\r\n\r\n\
Holiday\r\n\
--X\r\n\
Content-Disposition: form-data; name=\"avatar\"; filename=\"me.png\"\r\n\
Content-Type: image/png\r\n\r\n\
\x89PNG...\r\n\
--X--\r\n";

let mp = form_net::parse_multipart(content_type, body)?;
assert_eq!(mp.field("title"), Some("Holiday"));
let avatar = mp.file("avatar").unwrap();
assert_eq!(avatar.filename().unwrap().raw(), b"me.png");
assert!(avatar.content_type().unwrap().is("image", "png"));   // a hint, never trusted
assert_eq!(avatar.bytes(), b"\x89PNG...");
# Ok::<(), form_net::FormError>(())
```

### `Multipart`, `Part`

`#9` · P0 · Tier 1 · 0.3 · `alloc`

```rust,ignore
#[must_use]
pub struct Multipart<'a> { /* parts: Vec<Part<'a>> */ }

impl<'a> Multipart<'a> {
    pub fn parts(&self) -> &[Part<'a>];
    pub fn field(&self, name: &str) -> Option<&str>;
    pub fn field_one(&self, name: &str) -> Result<Option<&str>, FormError>;
    pub fn fields<'m>(&'m self, name: &'m str) -> impl Iterator<Item = &'m str> + 'm;
    pub fn file(&self, name: &str) -> Option<&Part<'a>>;
    pub fn files<'m>(&'m self, name: &'m str) -> impl Iterator<Item = &'m Part<'a>> + 'm;
    pub fn len(&self) -> usize;
    pub fn is_empty(&self) -> bool;
}

pub struct Part<'a> { /* head views, body: &'a [u8] */ }

impl<'a> Part<'a> {
    pub fn index(&self) -> u32;
    pub fn name(&self) -> &str;
    pub fn kind(&self) -> PartKind;
    pub fn is_file(&self) -> bool;
    pub fn filename(&self) -> Option<FileName<'_>>;
    pub fn content_type(&self) -> Option<ContentType<'_>>;
    pub fn bytes(&self) -> &'a [u8];
    pub fn text(&self) -> Result<&'a str, FormError>;
    pub fn len(&self) -> usize;
    pub fn is_empty(&self) -> bool;
}
```

**Behaviour**

- `parts` is every part in body order. `field` is the first text field with that name; `fields` yields every text field with that name; `field_one` refuses a name given twice. `file` and `files` do the same for file parts. A text field and a file part never answer for each other.
- `Part::index` counts from 0 in body order. `name` is the HTML-unescaped field name. `kind` is `File` when the part has a `filename` parameter (or an accepted `filename*`), even an empty one; `Field` otherwise.
- `Part::text` is the part's bytes as `&str`. For text fields it cannot fail (they were checked at parse time); for file parts it checks UTF-8 now.
- `Part::content_type` is what the client declared. It is a hint: form-net never acts on it (except to refuse a nested `multipart/*`), and the true type of a file is only known from its bytes (see `Inspector::sniff`).
- An empty file input (browsers send `filename=""` with an empty body) is reported as a file part whose `filename().unwrap().is_empty()` is true, under the default `EmptyFilePolicy::Report`.

**Errors:** `field_one`: `Extract { reason: Duplicate, field: None }`. `text`: `Encoding { InvalidUtf8 | UnsupportedCharset }`.

```rust
# let content_type = b"multipart/form-data; boundary=X";
# let body = b"--X\r\nContent-Disposition: form-data; name=\"title\"\r\n\r\nHoliday\r\n--X\r\nContent-Disposition: form-data; name=\"avatar\"; filename=\"me.png\"\r\nContent-Type: image/png\r\n\r\n\x89PNG...\r\n--X--\r\n";
let mp = form_net::parse_multipart(content_type, body)?;
for part in mp.parts() {
    if part.is_file() {
        let name = part.filename().unwrap();
        println!("{} bytes in {:?}", part.len(), name.basename());
    } else {
        println!("{} = {}", part.name(), part.text()?);
    }
}
let tags: Vec<&str> = mp.fields("tag").collect();
# Ok::<(), form_net::FormError>(())
```

### `parse_multipart_with`

`#33` · P1 · Tier 2 · 0.3 · `alloc`

```rust,ignore
pub fn parse_multipart_with<'a>(
    content_type: &[u8],
    body: &'a [u8],
    limits: &Limits,
    policy: &Policy,
) -> Result<Multipart<'a>, FormError>;
```

As `parse_multipart` with explicit limits and policy. `limits.validate()` runs first. The token rule of `Policy::token_field` applies; the captured token is the value of the text field with that name.

```rust
use form_net::{Limits, Policy};

// An avatar route: one small file, a couple of fields.
const AVATAR: Limits = Limits::DEFAULT
    .with_max_body_bytes(512 * 1024)
    .with_max_parts(4)
    .with_max_files(1)
    .with_max_file_bytes(500 * 1024);

# let content_type = b"multipart/form-data; boundary=X";
# let body = b"--X\r\nContent-Disposition: form-data; name=\"title\"\r\n\r\nHoliday\r\n--X\r\nContent-Disposition: form-data; name=\"avatar\"; filename=\"me.png\"\r\nContent-Type: image/png\r\n\r\n\x89PNG...\r\n--X--\r\n";
let mp = form_net::parse_multipart_with(content_type, body, &AVATAR, &Policy::DEFAULT)?;
# Ok::<(), form_net::FormError>(())
```

### `parse_multipart_inspected`

`#64` · P1 · Tier 2 · 0.4 · `alloc`

```rust,ignore
pub fn parse_multipart_inspected<'a, I: Inspector>(
    content_type: &[u8],
    body: &'a [u8],
    limits: &Limits,
    policy: &Policy,
    inspector: &mut I,
) -> Result<Multipart<'a>, FormError>;
```

As `parse_multipart_with`, with an inspector called in the order described under [`Inspector`](#inspector). For a buffered body the sniff window is a slice of the body, not a copy. A part the inspector skips is left out of the result.

**Errors:** as `parse_multipart_with`, plus `Refused { refusal, at }`.

---

## multipart, streaming

The sans-IO core. Bytes go in as they arrive, in chunks of any size split at any byte, and events come out. The parser never allocates after construction, never rescans a byte, and gives identical events for every split of the same body (FR-42).

The framing rules (RFC 2046 section 5.1.1 and RFC 7578), with form-net's strict choices:

| Construct | Rule |
|---|---|
| first delimiter | `--boundary` at offset 0, or `CRLF--boundary` after a preamble of at most `max_preamble_bytes` |
| after a delimiter | `--` (the close delimiter), or transport padding (SP and HTAB, at most `max_padding_bytes`) then CRLF; any other byte after a full `CRLF--boundary` is `BoundaryInContent` (RFC 2046: the delimiter must not appear inside a part) |
| the CRLF before a delimiter | belongs to the delimiter, never to the part's data |
| line endings | CRLF only; a bare LF where CRLF is required is refused |
| after the close delimiter | at most `max_epilogue_bytes` more, counted in constant time per chunk and discarded |
| end of input before the close delimiter | `Truncated` |
| part headers | `name ":" OWS value OWS CRLF`; no whitespace before the colon, no folding, no control bytes except HTAB; bytes 0x80 to 0xFF allowed in values (browsers send raw UTF-8 file names) |
| `Content-Disposition` | exactly once; type `form-data`; `name` exactly once and not empty; `filename` at most once; no repeated parameter; unknown parameters ignored, except that a parameter name with `*` other than `filename*` (RFC 2231 continuations, `name*`) and a backslash in `name` are refused as `MalformedParameter`; at most 16 parameters (more is `MalformedParameter`); a quoted value containing `\"` refused (browsers never backslash-escape, so the RFC 9110 and HTML readings disagree only there) |
| `filename*` | refused by default; see [`ExtendedFilenamePolicy`](#extendedfilenamepolicy) |
| part `Content-Type` | at most once; parsed; `multipart/*` refused (no nested multipart) |
| `Content-Transfer-Encoding` | absent, `7bit`, `8bit`, or `binary` (any case); anything else refused |
| other part headers | ignored under `HeaderPolicy::Ignore` (RFC 7578 section 4.8), refused under `HeaderPolicy::Refuse`; counted toward both header budgets either way |

### `Boundary`

`#48` · P0 · Tier 3 · 0.3 · core

```rust,ignore
#[must_use]
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Boundary { /* delimiter: [u8; 74], len: u8, fail: [u8; 74] */ }

impl Boundary {
    pub fn parse(content_type: &[u8]) -> Result<Boundary, FormError>;
    pub fn new(boundary: &[u8]) -> Result<Boundary, FormError>;
    pub fn as_bytes(&self) -> &[u8];
    pub fn delimiter(&self) -> &[u8];
}

impl core::fmt::Debug for Boundary { /* prints the boundary: it is not secret */ }

pub const MAX_BOUNDARY_LEN: usize = 70;      // #58
pub const MAX_DELIMITER_LEN: usize = 74;     // #58
```

A validated boundary with its delimiter (`CRLF "--" boundary`, 5 to 74 bytes) and the failure table of the streaming matcher, built once. `parse` takes a whole `Content-Type` value (as `FormKind::detect` does for multipart); `new` takes the bare boundary. `as_bytes` is the boundary without `--`; `delimiter` is the full `CRLF--boundary`. Comparison in the body is exact and case-sensitive.

**Errors:** `Syntax { InvalidBoundary | MissingBoundary | DuplicateBoundary | InvalidContentType }`; `UnsupportedMediaType` from `parse`.

**Cost:** under 100 ns including the table; no allocation; 149 bytes, `Copy` [est.].

```rust
use form_net::Boundary;

let b = Boundary::new(b"----WebKitFormBoundaryx7Y")?;
assert_eq!(b.delimiter(), b"\r\n------WebKitFormBoundaryx7Y");
assert!(Boundary::new(b"").is_err());
assert!(Boundary::new(&[b'a'; 71]).is_err());
assert!(Boundary::new(b"ends with space ").is_err());
assert!(Boundary::new(b"has\"quote").is_err());
assert_eq!(Boundary::parse(b"multipart/form-data; boundary=\"a b\"")?.as_bytes(), b"a b");
# Ok::<(), form_net::FormError>(())
```

### `MultipartParser`

`#49` · P0 · Tier 3 · 0.3 · core (`new`: `alloc`)

```rust,ignore
pub struct MultipartParser<B> { /* state, matcher, counters, buffer: B */ }

#[cfg(feature = "alloc")]
impl MultipartParser<Vec<u8>> {
    pub fn new(boundary: Boundary, limits: &Limits, policy: &Policy) -> Result<Self, FormError>;
}

impl<B: AsMut<[u8]> + AsRef<[u8]>> MultipartParser<B> {
    pub fn with_buffer(boundary: Boundary, limits: &Limits, policy: &Policy, buffer: B)
        -> Result<Self, FormError>;
    pub fn parse<'p>(&'p mut self, input: &'p [u8], eof: bool) -> Result<Step<'p>, FormError>;
    pub fn skip_part(&mut self);                         // #69
    pub fn reset(&mut self, boundary: Boundary);
    pub fn bytes_seen(&self) -> u64;
    pub fn parts_seen(&self) -> u32;
    pub fn files_seen(&self) -> u32;
    pub fn is_done(&self) -> bool;
    pub fn limits(&self) -> &Limits;
    pub fn policy(&self) -> &Policy;
    pub fn captured_token(&self) -> Option<&str>;        // #51
    pub fn declared_charset(&self) -> Option<&[u8]>;     // #51
}
```

**Behaviour**

- `parse(input, eof)` consumes a prefix of `input` and returns one event with the number of bytes consumed. Call it again with the rest of `input` (and later with new bytes). `eof = true` says `input` ends the body.
- The caller never has to keep bytes the parser consumed: a delimiter or header block split across chunks is carried as parser state (the matcher's position, or a copy in the header buffer), never as a request to re-send.
- `NeedInput` is returned only when every byte of `input` was consumed. `End` is returned as soon as the close delimiter is read; later calls count epilogue bytes (constant work per chunk) and return `End` again, until `max_epilogue_bytes` is exceeded. `eof = true` before the close delimiter is `Truncated`.
- Text field data is not buffered by the parser: it is emitted as `Data`, so a Tier 3 caller can stream even a 1 MiB text value. The parser buffers only part headers, the token field (when `Policy::token_field` is set), and the `_charset_` value (at most 32 bytes; a longer value cannot be a charset label and is refused as `Encoding { UnsupportedCharset }`).
- `skip_part` discards the rest of the current part: its bytes are consumed and counted against the budgets, but produce no `Data` events, and its `PartEnd` summary has `skipped = true`. Called outside a part, it does nothing.
- The buffer holds the header block, the decoded field name, the decoded `filename*` (only under `ExtendedFilenamePolicy::AcceptAlone`), the token (only when a token field is set), and the `_charset_` value: `Limits::multipart_buffer_len(&policy)` bytes, a `const fn` so a stack buffer can be sized at compile time. `with_buffer` refuses a shorter buffer.
- `reset` returns the parser to its initial state for a new body with `boundary`, wipes the buffer's used part, and clears the poisoned state. It never allocates, so parsers can be pooled.
- `captured_token` is the token field's value once its part has ended (see [`TokenField`](#tokenfield-tokenplacement)). `declared_charset` is the value of the last complete `_charset_` field. `limits` and `policy` are the configuration the parser was built with.
- The parser itself enforces every rule that depends only on the bytes: all budgets, the token placement and duplicate rules, and the empty-file deferral of `EmptyFilePolicy::Skip` (a file part with an empty file name counts against `max_files`, and is checked against the token placement, only when its first data byte arrives).
- The delimiter search is a streaming Knuth-Morris-Pratt automaton with an eight-byte SWAR skip to the next CR. Each input byte is examined a bounded number of times however the input is split; the worst case (a body of CR bytes or repeated near-miss delimiters) is linear with a constant about 4 to 8 times the typical case [est.].

**Errors:** every multipart variant in [Every error](#every-error-and-what-to-do); `InvalidLimits` and `BufferTooSmall { needed }` from the constructors; `Poisoned` after any error until `reset`.

**Cost:** no allocation after construction (FR-43; `with_buffer` never allocates at all); at least 3 GB/s on large file parts in 64 KiB chunks, under 1 µs for a 1 KiB body of three text fields [est.]; the struct is 384 bytes plus its buffer on 64-bit targets, 149 of them the boundary and its matcher table (NFR-18, measured). `Send` when `B: Send`.

```rust
use form_net::{Event, FormKind, MultipartParser, Policy, Limits};

# let content_type = b"multipart/form-data; boundary=X";
# let body = b"--X\r\nContent-Disposition: form-data; name=\"title\"\r\n\r\nHoliday\r\n--X\r\nContent-Disposition: form-data; name=\"avatar\"; filename=\"me.png\"\r\nContent-Type: image/png\r\n\r\n\x89PNG...\r\n--X--\r\n";
let FormKind::Multipart(boundary) = FormKind::detect(content_type)? else {
    return Err(form_net::FormError::UnsupportedMediaType);
};
let mut parser = MultipartParser::new(boundary, &Limits::DEFAULT, &Policy::DEFAULT)?;

let mut chunks = body.chunks(1500).peekable();       // any split gives the same events
while let Some(chunk) = chunks.next() {
    let last = chunks.peek().is_none();
    let mut rest = chunk;
    loop {
        let step = parser.parse(rest, last)?;
        rest = &rest[step.consumed..];
        match step.event {
            Event::PartStart(head) => println!("part {} named {}", head.index(), head.name()),
            Event::Data(bytes) => { /* write bytes to this part's destination */ }
            Event::PartEnd(summary) => println!("{} bytes", summary.bytes),
            Event::End => break,
            Event::NeedInput => break,
            _ => {}
        }
    }
}
assert!(parser.is_done());
# Ok::<(), form_net::FormError>(())
```

Without an allocator:

```rust
use form_net::{Boundary, Limits, MultipartParser, Policy};

const LIMITS: Limits = Limits::DEFAULT.with_max_part_header_bytes(1024);
const BUFFER_LEN: usize = LIMITS.multipart_buffer_len(&Policy::DEFAULT);
let mut buffer = [0u8; BUFFER_LEN];
let mut parser = MultipartParser::with_buffer(Boundary::new(b"X")?, &LIMITS, &Policy::DEFAULT, &mut buffer[..])?;
# Ok::<(), form_net::FormError>(())
```

### `Step`, `Event`

`#50` · P0 · Tier 3 · 0.3 · core

```rust,ignore
#[non_exhaustive]
pub struct Step<'p> {
    pub consumed: usize,
    pub event: Event<'p>,
}

#[non_exhaustive]
pub enum Event<'p> {
    PartStart(PartHead<'p>),
    Data(&'p [u8]),
    PartEnd(PartSummary),
    End,
    NeedInput,
}
```

`'p` ties the event to both the parser and the input, so data can borrow the input, the header buffer, or the boundary's own delimiter bytes (when a partial delimiter match fails, the bytes that turned out to be data are a prefix of the delimiter). The borrow checker makes the caller finish with the event before the next `parse`.

Per part the order is always `PartStart`, any number of `Data` (none for an empty part), `PartEnd`. A `Data` slice is never empty. After the last part comes `End`. `NeedInput` can appear anywhere between.

### `PartHead`

`#50` · P0 · Tier 3 · 0.3 · core (`headers`: P2)

```rust,ignore
pub struct PartHead<'p> { /* borrowed views */ }

impl<'p> PartHead<'p> {
    pub fn index(&self) -> u32;
    pub fn name(&self) -> &'p str;
    pub fn name_raw(&self) -> &'p [u8];
    pub fn kind(&self) -> PartKind;
    pub fn is_file(&self) -> bool;
    pub fn filename(&self) -> Option<FileName<'p>>;
    pub fn content_type(&self) -> Option<ContentType<'p>>;
    pub fn charset(&self) -> Charset;
    pub fn headers(&self) -> Headers<'p>;
    pub fn header_bytes(&self) -> u32;
}
```

The validated header block of one part. A head is only produced after every header budget and rule passed, so an inspector never spends work on a part form-net would refuse.

- `name` is the field name with the HTML escapes `%0A`, `%0D`, `%22` decoded to LF, CR, `"` (no other escape is decoded), as UTF-8; a name that is not UTF-8 is refused when the head is read. `name_raw` is the bytes as sent.
- `charset` is the part's own `charset` parameter (`Unspecified` when absent).
- `headers` yields every header line, known or not, as raw `(name, value)` bytes.
- `header_bytes` is the size of the header block including the final CRLF CRLF.

### `PartKind`, `Charset`

`#50` · P0 · Tier 3 · 0.3 · core

```rust,ignore
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PartKind { Field, File }

#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Charset { Utf8, Ascii, Unspecified, Other }
```

`Charset::Unspecified` means no `charset` parameter: UTF-8 is assumed unless a `_charset_` field before the part said otherwise. `Other` is any label that is not UTF-8 or US-ASCII; its text is refused by every `text` accessor and stays available as bytes.

### `ContentType`

`#50` · P0 · Tier 3 · 0.3 · core

```rust,ignore
#[derive(Clone, Copy)]
pub struct ContentType<'p> { /* raw value, type and subtype spans */ }

impl<'p> ContentType<'p> {
    pub fn raw(&self) -> &'p [u8];
    pub fn type_(&self) -> &'p str;
    pub fn subtype(&self) -> &'p str;
    pub fn is(&self, type_: &str, subtype: &str) -> bool;
    pub fn parameter(&self, name: &str) -> Option<&'p [u8]>;
    pub fn charset(&self) -> Charset;
}
```

A part's declared `Content-Type`, already checked by parse-net. `is` compares ASCII case-insensitively. `parameter` finds a parameter by name (case-insensitive) and returns its value without surrounding quotes; backslashes are literal, as browsers send them. **It is a hint**: the client chose it.

### `Headers`

`#50` · P2 · Tier 3 · 0.3 · core

```rust,ignore
#[derive(Clone)]
pub struct Headers<'p> { /* header block */ }

impl<'p> Iterator for Headers<'p> {
    type Item = (&'p [u8], &'p [u8]);
}
```

Every header line of a part in order, name and trimmed value as raw bytes.

### `PartSummary`

`#50` · P0 · Tier 3 · 0.3 · core

```rust,ignore
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PartSummary {
    pub index: u32,
    pub kind: PartKind,
    pub bytes: u64,
    pub skipped: bool,
}
```

What one part was: its index, kind, data length, and whether it was skipped (by `skip_part` or an inspector's `Verdict::Skip`).

---

## File names

### `FileName`, `FileNameSource`

`#19` (core views) P0 · `#20` (`basename`, `html_unescaped`) P1 · `#22` (`extension`) P2 · `#21` (`sanitized`) P1 · `#23` (`nfc`) P2 · Tier 1

```rust,ignore
#[derive(Clone, Copy)]
pub struct FileName<'a> { /* raw bytes, source */ }

#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileNameSource { Plain, Extended }

impl<'a> FileName<'a> {
    pub fn raw(&self) -> &'a [u8];                                                         // 0.3, core
    pub fn as_str(&self) -> Result<&'a str, FormError>;                                    // 0.3, core
    pub fn is_empty(&self) -> bool;                                                        // 0.3, core
    pub fn source(&self) -> FileNameSource;                                                // 0.3, core
    pub fn basename(&self) -> &'a [u8];                                                    // 0.3, core
    pub fn html_unescape_into<'b>(&self, out: &'b mut [u8]) -> Result<&'b str, FormError>; // 0.3, core
    pub fn html_unescaped(&self) -> Result<Cow<'a, str>, FormError>;                       // 0.3, alloc
    pub fn extension(&self) -> Option<&'a str>;                                            // 0.4, core
    #[cfg(feature = "fs")]
    pub fn sanitized(&self) -> Result<String, FormError>;                                  // 0.4
    #[cfg(feature = "unicode")]
    pub fn nfc(&self) -> Result<String, FormError>;                                        // 0.5, D-099
}

impl core::fmt::Debug for FileName<'_> { /* "FileName(n bytes)": file names are user data */ }
```

The client's file name, with views for each way a caller might want it. **form-net itself never uses a client file name as a path or as part of one** (FR-53): spill files have random names, and nothing below touches the file system.

| View | Result |
|---|---|
| `raw` | the bytes between the quotes of `filename`, untouched; or the decoded bytes of an accepted `filename*` |
| `as_str` | `raw` as UTF-8 |
| `is_empty` | `raw` is empty (a file input left empty) |
| `source` | `Plain` for `filename`, `Extended` for `filename*` |
| `basename` | the bytes after the last `/` or `\` (some non-browser clients send full client paths) |
| `html_unescape_into`, `html_unescaped` | `%0A`, `%0D`, `%22` decoded to LF, CR, `"`; nothing else. Browsers do not escape `%` itself, so a file the user named `a%22b` reads as `a"b`; that ambiguity is inherent in the HTML encoding |
| `extension` | the text after the last `.` of the base name when it is 1 to 16 ASCII alphanumeric bytes; a hint that proves nothing about the content |
| `sanitized` | `html_unescaped` of `basename`, then NFC (with `unicode`, 0.5), then every Unicode default-ignorable character (zero-width spaces and joiners, soft hyphens, variation selectors, tags, bidirectional controls) replaced by `_` (D-084), then `path_lib::sanitize`: one portable file name that `path_lib::join` accepts as a single segment, with separators, control and invisible characters, Windows-reserved characters and device names, bidirectional overrides, and trailing dots and spaces made harmless. It is still a name the client chose: never let it overwrite anything (`StoredFile::persist` refuses to) |
| `nfc` | Unicode NFC of `as_str` (macOS clients may send NFD) |

**Errors:** `Encoding { InvalidUtf8 }`; `BufferTooSmall { needed }` from `html_unescape_into`.

**Cost:** views are free; `sanitized` and `nfc` allocate one `String` (sanitize about 100 ns for a short name [est.]).

```rust
# let body = b"--X\r\nContent-Disposition: form-data; name=\"document\"; filename=\"C:\\Users\\me\\Q3 report: final?.pdf\"\r\n\r\n%PDF\r\n--X--\r\n";
# let mp = form_net::parse_multipart(b"multipart/form-data; boundary=X", body)?;
let part = mp.file("document").unwrap();
let name = part.filename().unwrap();
assert_eq!(name.raw(), b"C:\\Users\\me\\Q3 report: final?.pdf");
assert_eq!(name.basename(), b"Q3 report: final?.pdf");
assert_eq!(name.extension(), Some("pdf"));
assert_eq!(name.sanitized()?, "Q3 report_ final_.pdf");      // feature `fs`
# Ok::<(), form_net::FormError>(())
```

---

## Limits and policy

`Limits` holds numbers; `Policy` holds behaviours. Both are small `Copy` values with `const` setters, so a route's configuration is a `const` checked at compile time and validated once at startup.

### `Limits`

`#24` · P0 · Tier 2 · 0.3 · core (`multipart_buffer_len`, `urlencoded_buffer_len`: `#72`, 0.3; `memory_bound`: `#25`, P2, 0.5)

```rust,ignore
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    pub max_body_bytes: u64,
    pub max_fields: u32,
    pub max_name_bytes: u32,
    pub max_value_bytes: u32,
    pub max_text_bytes: u64,
    pub max_parts: u32,
    pub max_files: u32,
    pub max_file_bytes: u64,
    pub max_part_header_bytes: u32,
    pub max_part_headers: u16,
    pub max_filename_bytes: u32,
    pub max_preamble_bytes: u32,
    pub max_epilogue_bytes: u32,
    pub max_padding_bytes: u8,
    pub sniff_bytes: u32,
    pub max_token_bytes: u16,
}

impl Limits {
    pub const DEFAULT: Limits;
    pub const fn with_max_body_bytes(self, bytes: u64) -> Self;
    pub const fn with_max_fields(self, count: u32) -> Self;
    pub const fn with_max_name_bytes(self, bytes: u32) -> Self;
    pub const fn with_max_value_bytes(self, bytes: u32) -> Self;
    pub const fn with_max_text_bytes(self, bytes: u64) -> Self;
    pub const fn with_max_parts(self, count: u32) -> Self;
    pub const fn with_max_files(self, count: u32) -> Self;
    pub const fn with_max_file_bytes(self, bytes: u64) -> Self;
    pub const fn with_max_part_header_bytes(self, bytes: u32) -> Self;
    pub const fn with_max_part_headers(self, count: u16) -> Self;
    pub const fn with_max_filename_bytes(self, bytes: u32) -> Self;
    pub const fn with_max_preamble_bytes(self, bytes: u32) -> Self;
    pub const fn with_max_epilogue_bytes(self, bytes: u32) -> Self;
    pub const fn with_max_padding_bytes(self, bytes: u8) -> Self;
    pub const fn with_sniff_bytes(self, bytes: u32) -> Self;
    pub const fn with_max_token_bytes(self, bytes: u16) -> Self;
    pub fn validate(&self) -> Result<(), FormError>;
    pub const fn multipart_buffer_len(&self, policy: &Policy) -> usize;     // #72
    pub const fn urlencoded_buffer_len(&self) -> usize;                     // #72
    pub const fn memory_bound(&self, file_in_memory_bytes: u64) -> u64;     // #25
}

impl Default for Limits { /* Limits::DEFAULT */ }
```

Every budget is checked **before** the bytes it governs are buffered, stored, or passed to a sink (FR-37), with checked arithmetic on `u64` counters. A limit of 0 means "none allowed" (`max_files = 0` refuses every file part); there is no "unlimited" value other than the type's maximum (FR-38).

| Field | Default | Governs | Checked |
|---|---|---|---|
| `max_body_bytes` | 10 MiB (10,485,760) | every byte fed, both formats, preamble and epilogue included | before a chunk is scanned |
| `max_fields` | 1,000 | urlencoded pairs; multipart text fields | when a sequence or part starts |
| `max_name_bytes` | 256 | raw (encoded) name length, both formats | while scanning, before copying |
| `max_value_bytes` | 1 MiB | one urlencoded value or one multipart text field | while scanning or receiving data |
| `max_text_bytes` | 2 MiB | all text values of one body together | before each value is buffered |
| `max_parts` | 100 | multipart parts of either kind | at each delimiter, before headers are read |
| `max_files` | 20 | multipart file parts | when a part is classified as a file |
| `max_file_bytes` | 10 MiB | one file part | before each sink write |
| `max_part_header_bytes` | 8 KiB | one header block, CRLF CRLF included | before copying into the header buffer |
| `max_part_headers` | 8 | header lines per part | per line |
| `max_filename_bytes` | 1,024 | raw `filename`, or decoded `filename*` | while the disposition is parsed |
| `max_preamble_bytes` | 1 KiB | bytes before the first delimiter | while the preamble is scanned |
| `max_epilogue_bytes` | 1 KiB | bytes after the close delimiter | per chunk, in constant time |
| `max_padding_bytes` | 32 | transport padding after a delimiter | per byte |
| `sniff_bytes` | 0 (off) | the sniff window per file part | at construction (buffer size) |
| `max_token_bytes` | 128 | the captured anti-forgery token (csrf-sec tokens are 59 characters, one-time tokens 102) | at capture |

**Containing limits cap contained ones**, so a route sets only the limits it cares about: the effective value of a limit is never more than the limit that contains it. `max_file_bytes`, `max_text_bytes`, `max_preamble_bytes`, and `max_epilogue_bytes` are capped by `max_body_bytes`; `max_value_bytes` by `max_text_bytes`; `max_name_bytes` by `max_body_bytes`; `max_token_bytes` by `max_value_bytes`; `sniff_bytes` by `max_file_bytes`; `max_filename_bytes` by `max_part_header_bytes`. `Limits::DEFAULT.with_max_body_bytes(4096)` is a complete, consistent configuration for a small form. Every check and every size (buffers, `memory_bound`) uses the effective values.

`validate` refuses only what no body could satisfy, so a misconfiguration is found at startup, not under attack (FR-39): `max_part_header_bytes` below 64 (the shortest useful `Content-Disposition` line must fit), and buffer sizes that do not fit in `usize` on the target. With a token field set, the policy is checked too: an effective `max_token_bytes` of 0 is refused. Every parser constructor and every `*_with` function calls it.

`memory_bound` is the worst-case number of bytes one parse with these limits can hold in memory (FR-40), for admission control (D-093): parser state (256); the parser buffer and the driver's copy of the current part head (each `multipart_buffer_len` under the most demanding policy); `sniff_bytes`; the field being received (`max_value_bytes`); the text kept and its names (`max_text_bytes` plus `max_fields × max_name_bytes`, the names capped by `max_body_bytes`); 96 bytes of index per field and 320 per part; owned copies of part names, file names, and content types (`max_parts × max_part_header_bytes`, capped by `max_body_bytes`); the files kept in memory, `max_files × min(file_in_memory_bytes, max_file_bytes)`, capped by twice `max_body_bytes` (a buffer that grew by doubling can hold twice its bytes); and half the largest growing buffer (a buffer that doubles holds its old half while it moves). `file_in_memory_bytes` is the most bytes of one file a store keeps in memory: its `memory_threshold`, or `u64::MAX` for a memory-only store (`Store::memory_bound` passes the right value). The caller's read buffer is not included (the blocking readers add 64 KiB). With the defaults and `Store::temp()` the bound is about 6.5 MiB per request against 10 MiB of input; a login route with `max_body_bytes = 4 KiB`, 8 fields and parts, no files, and a 1 KiB header budget is under 32 KiB. `tests/memory_bound.rs` measures hostile bodies against it with a peak-tracking allocator.

**Errors:** `validate`: `InvalidLimits { limit }`, naming the first field of the first inconsistent pair.

**Cost:** `validate` a few nanoseconds, once per construction.

```rust
use form_net::Limits;

const LOGIN: Limits = Limits::DEFAULT
    .with_max_body_bytes(4 * 1024)
    .with_max_fields(8)
    .with_max_parts(8)
    .with_max_files(0)
    .with_max_part_header_bytes(1024);
LOGIN.validate()?;
assert!(LOGIN.memory_bound(64 * 1024) < 32 * 1024);

// A header budget too small for any part is refused at startup.
let broken = Limits::DEFAULT.with_max_part_header_bytes(16);
assert_eq!(broken.validate().unwrap_err().code(), "invalid_limits");
# Ok::<(), form_net::FormError>(())
```

### `Policy`

`#26` · P1 · Tier 2 · 0.3 · core

```rust,ignore
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Policy {
    pub percent: PercentPolicy,                      // default Refuse
    pub extended_filename: ExtendedFilenamePolicy,   // default Refuse
    pub unknown_headers: HeaderPolicy,               // default Ignore
    pub empty_files: EmptyFilePolicy,                // default Report
    pub token_field: Option<TokenField>,             // default None
}

impl Policy {
    pub const DEFAULT: Policy;
    pub const fn with_percent(self, policy: PercentPolicy) -> Self;
    pub const fn with_extended_filename(self, policy: ExtendedFilenamePolicy) -> Self;
    pub const fn with_unknown_headers(self, policy: HeaderPolicy) -> Self;
    pub const fn with_empty_files(self, policy: EmptyFilePolicy) -> Self;
    pub const fn with_token_field(self, field: TokenField) -> Self;
}

impl Default for Policy { /* Policy::DEFAULT */ }
```

The behaviours a route may choose. The defaults are the strict ones; every alternative is named after what it allows.

```rust
use form_net::{EmptyFilePolicy, HeaderPolicy, Policy, TokenField};

const UPLOADS: Policy = Policy::DEFAULT
    .with_token_field(TokenField::new("csrf_token"))     // verify before any file is stored
    .with_empty_files(EmptyFilePolicy::Skip)             // an empty file input produces nothing
    .with_unknown_headers(HeaderPolicy::Refuse);         // only the three known part headers
```

### `PercentPolicy`

`#27` · P1 · Tier 2 · 0.3 · core

```rust,ignore
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PercentPolicy { Refuse, Literal }
```

What a `%` not followed by two hex digits means in urlencoded input. `Refuse` (default) refuses it; `Literal` keeps it as a literal byte, exactly as WHATWG percent-decode does, for routes that must mirror browsers' `URLSearchParams`. Risk of `Literal`: a front end that refuses such input and a back end that keeps it see different requests.

### `ExtendedFilenamePolicy`

`#28` · P1 · Tier 2 · 0.3 · core

```rust,ignore
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExtendedFilenamePolicy { Refuse, AcceptAlone, Ignore }
```

What a `filename*` parameter (RFC 8187) in a part's `Content-Disposition` means. RFC 7578 section 4.2 says senders must not use it in `multipart/form-data`, and browsers never do.

| Variant | Behaviour |
|---|---|
| `Refuse` (default) | any `filename*` is refused (`Policy { ExtendedFilename }`) |
| `AcceptAlone` | accepted and decoded (UTF-8 only, at most `max_filename_bytes` decoded) when it is the only file name parameter; `FileName::source` is `Extended` |
| `Ignore` | the parameter is dropped; a part with only `filename*` is then a text field |

A part with both `filename` and `filename*` is refused under every policy (`Policy { BothFilenames }`): which name wins is exactly where readers disagree. A malformed `filename*` is `Encoding { InvalidExtendedValue }` under `AcceptAlone`.

### `HeaderPolicy`

`#29` · P2 · Tier 2 · 0.3 · core

```rust,ignore
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HeaderPolicy { Ignore, Refuse }
```

Part headers other than `Content-Disposition`, `Content-Type`, and `Content-Transfer-Encoding`. `Ignore` (default) skips them as RFC 7578 section 4.8 requires; they still count toward `max_part_header_bytes` and `max_part_headers` and are visible through `PartHead::headers`. `Refuse` refuses them (`Policy { UnknownHeader }`) on routes that want the smallest surface.

### `EmptyFilePolicy`

`#30` · P2 · Tier 2 · 0.3 · core

```rust,ignore
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EmptyFilePolicy { Report, Skip }
```

A file input left empty arrives as a file part with `filename=""` and no data. `Report` (default) reports it as a file part with an empty file name. `Skip` makes drivers produce nothing for it: no inspector call, no sink, nothing on disk, and it does not count against `max_files`. The parser defers counting such a part against `max_files` (and checking it against the token placement) until its first data byte; drivers delay `Inspector::part_start` and `Sink::open` for a file part with an empty file name until its first data byte; if the part ends with no data, it is dropped. It still counts against `max_parts` and the body budget. In typed extraction, `Option<StoredFile>` maps an empty file to `None` under either policy.

### `TokenField`, `TokenPlacement`

`#31` · P2 · Tier 2 · 0.3 · core

```rust,ignore
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TokenField { /* name: &'static str, placement */ }

impl TokenField {
    pub const fn new(name: &'static str) -> TokenField;      // placement BeforeFirstFile
    pub const fn anywhere(self) -> TokenField;
    pub const fn name(&self) -> &'static str;
    pub const fn placement(&self) -> TokenPlacement;
}

#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TokenPlacement { BeforeFirstFile, Anywhere }
```

Captures the anti-forgery token field of a multipart body so it can be verified before any file is stored.

- The parser enforces every rule below, so they hold for every entry point. The text field whose decoded name equals `name` exactly (byte for byte; case matters) is captured, at most `max_token_bytes` bytes (`Limit { TokenLength }` beyond). Its value must be UTF-8 (a token that is not is refused as `Encoding { InvalidUtf8 }`), so it is handed out as `&str`.
- With `BeforeFirstFile` (the default of `new`), a file part that starts before the token was captured (for an empty file name under `EmptyFilePolicy::Skip`: whose first data byte arrives before it) is refused (`Policy { TokenFieldLate }`) before its sink opens, so a forged upload costs no disk. Browsers send fields in document order, so the form puts the hidden token input before its file inputs.
- A second field with the token's name is refused (`Policy { DuplicateToken }`): two tokens would let an attacker choose which one the verifier reads.
- The value is available from `MultipartParser::captured_token`, `UploadSession::captured_token`, `MultipartReader::captured_token`, and `Upload::captured_token`. form-net compares nothing: the verifier (csrf-sec) does, in constant time.

```rust
use form_net::{Policy, TokenField};

// csrf-sec names its field `csrf_token` (csrf_sec::FIELD_NAME).
const POLICY: Policy = Policy::DEFAULT.with_token_field(TokenField::new("csrf_token"));
const LENIENT: Policy = Policy::DEFAULT.with_token_field(TokenField::new("csrf_token").anywhere());
```

---

## Hooks

Hooks let other crates take part in a parse without a second pass: customs-sec inspects every part before it is stored, and storage is whatever the sink is. Hooks are generic parameters, so a hook that does nothing (`NoInspector`, `DiscardSink`) compiles to nothing.

### `Inspector`

`#52` · P0 · Tier 3 · 0.4 · core (`failed`: `#66`, P2)

```rust,ignore
pub trait Inspector {
    fn part_start(&mut self, head: &PartHead<'_>) -> Verdict { Verdict::Continue }
    fn sniff(&mut self, head: &PartHead<'_>, first: &[u8]) -> Verdict { Verdict::Continue }
    fn data(&mut self, part: u32, chunk: &[u8]) -> Verdict { Verdict::Continue }
    fn field(&mut self, name: &str, value: &[u8]) -> Verdict { Verdict::Continue }
    fn part_end(&mut self, summary: &PartSummary) -> Verdict { Verdict::Continue }
    fn end(&mut self) -> Verdict { Verdict::Continue }
    fn failed(&mut self, error: &FormError) {}
}
```

Every method has a default, so an inspector implements only what it checks.

**Order guarantee** (FR-55), per part, at every split of the input:

```text
part_start ─▶ sniff (file parts, when sniff_bytes > 0) ─▶ data* ─▶ field (text fields) ─▶ part_end
... after the last part: end
... on any error, at any point: failed (once), then the error is returned
```

- `part_start` comes before any byte of the part reaches a sink, and only after every header budget and rule passed.
- `sniff` receives the first `min(sniff_bytes, part length)` bytes of each file part as one contiguous slice, however the input was split, before any `data` call for that part; the driver holds those bytes back until the window is full or the part ends. This is where customs-sec compares magic bytes with `FileName::extension` and `ContentType`.
- `data` sees every byte of every part, text fields included.
- `field` sees each complete text field once: the name, and the value as raw bytes so the inspector can apply its own charset rules. For urlencoded bodies (`parse_urlencoded_inspected`, `FromForm::from_body_inspected`) it sees each decoded pair, and only `field`, `end`, and `failed` are called.
- `failed` is called once when the parse fails for any reason (a limit, a syntax error, a refusal by this inspector or by the sink), with the error that will be returned. It cannot change the outcome. It is the hook for security signals: a stream of refusals from one client is a probe, and the inspector is where that is reported.

A `Verdict::Refuse` from any method stops the parse: every sink is aborted, every finished file is dropped (deleting stored files), and `FormError::Refused { refusal, at }` is returned. A `Verdict::Skip` from `part_start`, `sniff`, or `data` turns the rest of the part into counted, discarded bytes: no further `data` or `field` for it, its sink is aborted, and it is left out of the result; `part_end` still comes, with `skipped = true`. `Skip` from `field`, `part_end`, or `end` is treated as `Continue` (nothing is left to skip), except that a skipped `field` leaves that field out of the result.

**Cost:** one monomorphized call per event, nothing for `NoInspector` [est.]; the sniff window is one copy of up to `sniff_bytes` per file (none for buffered bodies).

How customs-sec's body checks map onto the hooks:

| customs-sec check | Hook |
|---|---|
| structural budgets | already enforced by form-net: customs compiles each route contract into a `Limits` |
| schema: types, required, unknown fields | `part_start` (is this name allowed?), `field`, `end` (were the required fields present?) |
| declared against actual type | `sniff` with `FileName::extension` and `ContentType` |
| encoding, Unicode, confusables | `field` |
| file name rules | `part_start` with `FileName` |
| injection-shaped input (flag) | `field`, `data` |
| reporting probes | `failed` with `FormError::code` |

```rust
use form_net::{Inspector, PartHead, Refusal, Verdict};

/// Allows only PNG and JPEG avatars, judged by their bytes.
struct ImagesOnly;

impl Inspector for ImagesOnly {
    fn part_start(&mut self, head: &PartHead<'_>) -> Verdict {
        match (head.name(), head.is_file()) {
            ("avatar", true) | ("title", false) => Verdict::Continue,
            _ => Verdict::Refuse(Refusal::new(422, "example.unexpected_field")),
        }
    }
    fn sniff(&mut self, _head: &PartHead<'_>, first: &[u8]) -> Verdict {
        if first.starts_with(b"\x89PNG\r\n\x1a\n") || first.starts_with(b"\xff\xd8\xff") {
            Verdict::Continue
        } else {
            Verdict::Refuse(Refusal::new(415, "example.avatar_not_image"))
        }
    }
}
```

### `Verdict`, `Refusal`, `NoInspector`

`#52` (`Verdict`, `NoInspector`) 0.4 · `#73` (`Refusal`) 0.3 · P0 · Tier 3 · core

```rust,ignore
#[must_use]
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict { Continue, Skip, Refuse(Refusal) }

#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Refusal {
    pub status: u16,
    pub code: &'static str,
}

impl Refusal {
    pub const fn new(status: u16, code: &'static str) -> Refusal;
}

#[derive(Clone, Copy, Debug, Default)]
pub struct NoInspector;
impl Inspector for NoInspector {}
impl<I: Inspector + ?Sized> Inspector for &mut I {}   // D-086: lend an inspector to a call and keep it
impl<A: Inspector, B: Inspector> Inspector for (A, B) {}   // D-088: both in one pass, A first
impl<I: Inspector + ?Sized> Inspector for Box<I> {}   // D-088, alloc: a chain chosen at run time
```

`Refusal::status` is the HTTP status the inspector wants: `FormError::status_hint` returns it, and a status outside 400 to 599 is reported as 400, because a refusal is never a success. `Refusal::code` is the inspector's stable, machine-readable name for the reason (for example `customs.unexpected_field`), which `FormError::code` returns for a refusal; it must never contain input. `Refusal` ships at 0.3 with `FormError`; inspectors at 0.4.

### `Sink`, `DiscardSink`, `MemorySink`

`#53` · P1 · Tier 3 · 0.4 · core (`MemorySink`: `alloc`)

```rust,ignore
pub trait Sink {
    type Output;
    fn open(&mut self, head: &PartHead<'_>) -> Result<(), FormError>;
    fn write(&mut self, chunk: &[u8]) -> Result<(), FormError>;
    fn close(&mut self) -> Result<Self::Output, FormError>;
    fn abort(&mut self);
}

#[derive(Clone, Copy, Debug, Default)]
pub struct DiscardSink;                    // Output = u64, the bytes discarded

#[derive(Debug, Default)]
pub struct MemorySink { /* Vec<u8> */ }    // Output = FileData
impl MemorySink {
    pub fn new() -> MemorySink;
}
```

Where file parts go. A sink handles one part at a time (multipart is sequential): `open`, any number of `write`, then `close` or `abort`. The driver opens the sink only after the inspector has accepted the part's head, its sniff window, and the first chunk (or at the end of an empty file), so a refused or skipped part never reaches storage (D-089). Drivers call `abort` on every error path (FR-50), and drop the outputs of parts that already closed, which deletes stored files. `abort` must release everything, cannot fail, and does nothing when no part is open.

A custom sink can send uploads anywhere: to an object store, to a worker pool that does the blocking writes, or through a virus scanner. Budgets are enforced by the driver before `write` is called, so a sink never has to count.

**Errors:** sinks return `FormError`, usually `Io { kind }`, `Store { reason }`, or `Limit { FileSize | Disk }`.

### `Driver`, `Progress`

`#54` · P2 · Tier 3 · 0.4 · `alloc`

```rust,ignore
pub struct Driver<B, S: Sink> { /* parser, sink, text buffer, sniff buffer, state */ }

impl<B: AsMut<[u8]> + AsRef<[u8]>, S: Sink> Driver<B, S> {
    pub fn new(parser: MultipartParser<B>, sink: S) -> Self;
    pub fn push<'d, I: Inspector>(&'d mut self, input: &'d [u8], eof: bool, inspector: &mut I)
        -> Result<(usize, Progress<'d, S::Output>), FormError>;
    pub fn parser(&self) -> &MultipartParser<B>;
    pub fn into_parts(self) -> (MultipartParser<B>, S);
}

#[non_exhaustive]
pub enum Progress<'d, F> {
    Field { name: &'d str, value: &'d [u8] },
    File(F),
    NeedInput,
    End,
}
```

The streaming implementation of the hook order: parser, inspector, sink, the text buffer (each text value held up to `max_value_bytes`), the sniff window, and the empty-file delay. It takes its limits and policy from the parser. Because a `PartHead` cannot outlive the `parse` call that produced it, the driver copies each head once into an owned head allocated at construction (the size of the parser buffer), and hands that copy to the delayed `sniff`, `part_start`, `Sink::open`, and `field` calls. `UploadSession`, `Upload::parse_with`, `read_multipart`, and integrations by other crates use it. The buffered entry points (`parse_multipart_inspected`, `FromForm::from_body_inspected`) use a second, internal driver whose events borrow the body instead, and keep the same order; one test suite checks both at every split. `MultipartReader` is a pull API with no hooks: the caller sees each part itself. `push` returns how much of `input` it consumed and what completed: a text field, a closed file (the sink's output), a request for more input, or the end.

**Errors:** every multipart error, `Refused`, and the sink's errors. On error the driver has already aborted the sink and called `inspector.failed`.

---

## Storage

Behind the `fs` feature. Uploads are kept in memory up to a threshold and then spilled to private temporary files that the store creates itself. Temporary files are removed on drop, on abort, and, in the default anonymous mode, by the operating system when the process dies. **No client byte is ever part of a path**: every name is random.

Spilling is blocking file I/O. A server with an event loop runs the parse on a thread that may block, or gives the driver its own `Sink`.

### `read_multipart`

`#10` · P1 · Tier 1 · 0.4 · `fs`

```rust,ignore
pub fn read_multipart<R: std::io::Read>(content_type: &[u8], body: R) -> Result<Upload, FormError>;
```

The blocking convenience for small servers and tools: reads `body` to the end in 64 KiB chunks with `Limits::DEFAULT` and `Policy::DEFAULT`, keeps files up to 64 KiB in memory, and spills larger ones to an anonymous temporary file in a private directory under the system temporary directory. The directory is created only when the first file spills, and removed when the last file from it is dropped. There is no process-wide state.

**Errors:** every multipart error; `Io { kind }` from the reader or the file system; `Limit { Disk }` when the request's disk limit is reached; `Random` when the OS random source fails.

**Cost:** parser throughput plus disk speed for spilled files; one 64 KiB read buffer.

```rust
# let content_type = b"multipart/form-data; boundary=X";
# let documents_dir = std::env::temp_dir();
# let new_id = || format!("form-net-api-read-{}.pdf", std::process::id());
// Any std::io::Read: a socket, a file, a server's request body.
let request_body = &b"--X\r\n\
Content-Disposition: form-data; name=\"document\"; filename=\"q3.pdf\"\r\n\
Content-Type: application/pdf\r\n\r\n\
%PDF-1.7 ...\r\n\
--X--\r\n"[..];

let mut upload = form_net::read_multipart(content_type, request_body)?;
let title = upload.field("title").unwrap_or("untitled").to_owned();
if let Some(file) = upload.take_file("document") {
    println!("{} bytes, declared {:?}", file.len(), file.content_type().map(|t| t.raw()));
    file.persist(&documents_dir.join(new_id()))?;     // the caller builds the path
}
# std::fs::remove_file(documents_dir.join(new_id())).unwrap();
# Ok::<(), form_net::FormError>(())
```

### `Store`

`#12` (`temp`, `memory_only`) P1 Tier 1 · `#37` (`in_dir` and setters) P1 Tier 2 · `#38` (`cleanup_failures`) P2 · `#68` (`stats`) P2 · `#71` (`memory_bound`) P2 0.5 · 0.4 · `fs`

```rust,ignore
pub struct Store { /* shared: Arc<StoreShared> (directory, settings, counters, budget) */ }

impl Store {
    pub fn temp() -> Result<Store, FormError>;
    pub fn memory_only() -> Store;
    pub fn in_dir(dir: impl AsRef<std::path::Path>) -> Result<Store, FormError>;
    pub fn memory_threshold(self, bytes: u64) -> Store;
    pub fn max_disk_bytes(self, bytes: u64) -> Store;
    pub fn mode(self, mode: SpillMode) -> Store;
    pub fn quota(self, budget: impl SpillBudget) -> Store;
    pub fn dir(&self) -> Option<&std::path::Path>;
    pub fn cleanup_failures(&self) -> u64;
    pub fn stats(&self) -> StoreStats;
    pub fn memory_bound(&self, limits: &Limits) -> u64;     // 0.5, D-093
    pub fn sink(&self, limits: &Limits) -> StoreSink;
}
```

Where uploads are kept. A `Store` is `Send + Sync`, created once per process or per route, and shared by reference across requests.

**Behaviour**

- `temp` creates `<temp dir>/form-net-<32 hex>` (128 bits from the OS random source) with `create` that fails if the path exists, so the store never adopts a directory someone else made; mode 0700 on Unix. Defaults: `memory_threshold` 64 KiB, `SpillMode::Anonymous`, no quota, and a per-request disk limit equal to that request's `max_body_bytes`.
- `memory_only` never touches the file system (FR-45): a file larger than `max_file_bytes` is refused, never spilled.
- `in_dir` uses a directory the caller made. It must exist and be a directory, not a link to one (checked with `symlink_metadata`); otherwise `Store { NotADirectory }`. Use it to put uploads on a volume of their own.
- `memory_threshold(0)` spills every file at its first byte. A file is spilled when its next write would take it past the threshold; the bytes held so far are written in one call, and every later chunk goes from the caller's buffer straight to the file.
- `max_disk_bytes` caps the bytes one request may put on disk (`Limit { Disk }` beyond). `quota` (0.5, with `SpillBudget`) adds a budget shared across requests (see [`SpillBudget`](#spillbudget-spillquota)); bytes are reserved from it before every disk write.
- Files hold a reference to the store's shared state, so a `Store` may be dropped while its files live; a store's own directory is removed when the store and every file from it are gone, if it is empty.
- Errors while deleting during a drop cannot be returned; each one increments `cleanup_failures` so operations can alert on it.
- `memory_bound` is `limits.memory_bound(threshold)`, or `limits.memory_bound(u64::MAX)` for a memory-only store.
- `sink` returns the `Sink` that stores files of one request under `limits`.

**Errors:** `temp`: `Io { kind }` when the directory cannot be created, `Random` when the OS random source fails (fail closed: no predictable fallback name). `in_dir`: `Store { NotADirectory }`, `Io { kind }`.

**Cost:** `temp` one random read and one `mkdir` [est. 10 to 50 µs], once per process.

```rust
use form_net::{SpillMode, SpillQuota, Store};

// Defaults: files over 64 KiB spill to anonymous temporary files.
let store = Store::temp()?;

// A document service that keeps large uploads: named files so `persist` is a link, not a copy,
// on a volume of their own, with a shared 2 GiB budget across all requests.
# let incoming = std::env::temp_dir().join(format!("form-net-api-store-{}", std::process::id()));
# std::fs::create_dir_all(&incoming).unwrap();
let store = Store::in_dir(&incoming)?                  // for example /srv/uploads/incoming
    .mode(SpillMode::Named)
    .memory_threshold(256 * 1024)
    .quota(SpillQuota::new(2 << 30));
# drop(store);
# std::fs::remove_dir_all(&incoming).unwrap();
# Ok::<(), form_net::FormError>(())
```

### `SpillMode`

`#37` · P1 · Tier 2 · 0.4 · `fs`

```rust,ignore
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum SpillMode {
    #[default]
    Anonymous,
    Named,
}
```

| | `Anonymous` (default) | `Named` |
|---|---|---|
| Unix | the file is unlinked right after it is created; it lives only while its handle is open | the file keeps its random name until it is dropped or persisted |
| Windows | opened with delete-on-close and no sharing; removed when its last handle closes, including when the process is killed | readable by the persist step; deleted on drop |
| after a crash | nothing is left: the operating system frees it (on Unix, a crash in the instant between creating the file and unlinking it can leave one `upload-*.part`, which `sweep` also removes) | `sweep` at the next start removes leftovers |
| `persist` | a copy into the destination | a hard link when the destination is on the same file system, a copy otherwise |
| `StoredFile::path` | `None` | `Some` |

Every file is created with `create_new` (fails if the path exists and never follows a planted link), mode 0600 on Unix, in the store's directory, named `upload-<32 hex>.part` from 128 random bits. On Windows the private directory inherits the ACL of the per-user temporary directory; the details are in `docs/PLATFORM-NOTES.md` (0.5).

### `StoreStats`

`#68` · P2 · Tier 2 · 0.4 · `fs`

```rust,ignore
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StoreStats {
    pub files_in_memory: u64,
    pub files_on_disk: u64,
    pub bytes_in_memory: u64,
    pub bytes_on_disk: u64,
    pub files_spilled: u64,         // since the store was created
    pub cleanup_failures: u64,      // since the store was created
}
```

A snapshot of what the store holds now, for metrics and health checks, read from atomic counters (no lock).

### `StoreSink`

`#37` · P1 · Tier 3 · 0.4 · `fs`

```rust,ignore
pub struct StoreSink { /* store reference, request counters, current file */ }
impl Sink for StoreSink { type Output = StoredFile; /* ... */ }
```

The `Sink` a `Store` hands out for one request (`Store::sink`). It enforces `max_disk_bytes` for that request and reserves from the store's budget before each disk write. `abort` closes and deletes the current file.

### `StoredFile`, `StoredReader`

`#13` · P1 · Tier 1 · 0.4 · `fs`

```rust,ignore
#[must_use]
pub struct StoredFile { /* field name, file name, content type, length, location, reservation */ }

impl StoredFile {
    pub fn name(&self) -> &str;
    pub fn filename(&self) -> Option<FileName<'_>>;
    pub fn content_type(&self) -> Option<ContentType<'_>>;
    pub fn len(&self) -> u64;
    pub fn is_empty(&self) -> bool;
    pub fn is_in_memory(&self) -> bool;
    pub fn bytes(&self) -> Option<&[u8]>;
    pub fn reader(&self) -> Result<StoredReader<'_>, FormError>;
    pub fn read_to_vec(&self, max: u64) -> Result<Vec<u8>, FormError>;
    pub fn path(&self) -> Option<&std::path::Path>;
    pub fn persist(self, dest: &std::path::Path) -> Result<(), FormError>;
}

impl Drop for StoredFile { /* close, then delete (named); wipe (memory) */ }
impl core::fmt::Debug for StoredFile { /* field name, length, location; never the file name */ }

pub struct StoredReader<'f> { /* positional reads */ }
impl std::io::Read for StoredReader<'_> {}
impl std::io::Seek for StoredReader<'_> {}
```

One received file, in memory or in a temporary file.

**Behaviour**

- `name` is the form field name; `filename` and `content_type` are what the client sent (hints).
- `bytes` is `Some` only while the file is in memory. `reader` works in both cases. It reads with positional reads (`read_at` on Unix, `seek_read` on Windows), so several readers can exist at once without sharing a cursor.
- `read_to_vec(max)` refuses a file longer than `max` before allocating (`Limit { FileSize }`).
- `path` is `Some` only in `SpillMode::Named`.
- `persist` moves the finished upload to `dest`, which the caller built (with `path_lib::join` for a user-facing name, or from its own id). It never overwrites (FR-51): named mode makes a hard link (which fails if `dest` exists) and then removes the temporary name, and copies into `dest` opened with `create_new` when the link fails for any reason but an existing `dest` (another file system, a file system without links); anonymous and memory files are copied the same way. A copy that fails part way removes the partial `dest` (D-086). Persisted bytes are released from the store's budget. Whether to fsync `dest` and its directory is the caller's choice; `docs/PLATFORM-NOTES.md` explains when it matters.
- Dropping a `StoredFile` without `persist` closes its handle first and then removes its name (named mode). Closing first matters on Windows, where an open file cannot be removed, and avoids the bug class of a removed name with a descriptor still open. Memory-held bytes are wiped.

**Errors:** `Io { kind }`; `Store { DestinationExists }` from `persist`; `Limit { FileSize }` from `read_to_vec`.

**Cost:** `persist` in named mode on one file system: one link and one unlink [est. under 50 µs]; otherwise a copy at disk speed.

```rust
use std::io::Read;

# let body = &b"--X\r\nContent-Disposition: form-data; name=\"document\"; filename=\"q3.pdf\"\r\n\r\n%PDF-1.7 ...\r\n--X--\r\n"[..];
# let upload = form_net::read_multipart(b"multipart/form-data; boundary=X", body)?;
# let dest = std::env::temp_dir().join(format!("form-net-api-stored-{}.pdf", std::process::id()));
let file = upload.file("document").unwrap();
let mut first = [0u8; 8];
file.reader()?.read_exact(&mut first)?;
if file.len() <= 64 * 1024 {
    let small = file.read_to_vec(64 * 1024)?;
}
let mut upload = upload;
let file = upload.take_file("document").unwrap();
file.persist(&dest)?;                                 // for example /srv/docs/2026/10/7f3a.pdf
# std::fs::remove_file(&dest)?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

### `Upload`

`#11` · P1 · Tier 1 · 0.4 · `fs` (`iter`: `#70`; `parse_with`: `#36`, Tier 2)

```rust,ignore
#[must_use]
pub struct Upload { /* text arena, text spans, files: Vec<StoredFile>, token */ }

impl Upload {
    pub fn from_form(form: Form, policy: &Policy) -> Result<Upload, FormError>; // 0.5, D-098
    pub fn field(&self, name: &str) -> Option<&str>;
    pub fn field_one(&self, name: &str) -> Result<Option<&str>, FormError>;
    pub fn fields<'u>(&'u self, name: &'u str) -> impl Iterator<Item = &'u str> + 'u;
    pub fn iter(&self) -> impl Iterator<Item = (&str, &str)>;
    pub fn file(&self, name: &str) -> Option<&StoredFile>;
    pub fn files<'u>(&'u self, name: &'u str) -> impl Iterator<Item = &'u StoredFile> + 'u;
    pub fn take_file(&mut self, name: &str) -> Option<StoredFile>;
    pub fn into_files(self) -> Vec<StoredFile>;
    pub fn captured_token(&self) -> Option<&str>;
    pub fn text_len(&self) -> u64;
    pub fn file_count(&self) -> usize;

    pub fn parse_with<R: std::io::Read, I: Inspector>(
        content_type: &[u8],
        reader: R,
        limits: &Limits,
        policy: &Policy,
        store: &Store,
        inspector: &mut I,
    ) -> Result<Upload, FormError>;
}

impl core::fmt::Debug for Upload { /* names, lengths, file count; never values */ }
```

The result of a completed multipart parse with storage. Text values are owned, checked as text the way `parse_multipart` checks them, and wiped on drop; the text arena holds the names and values of the text fields, grows in 64 KiB doubling steps, and is bounded by `max_text_bytes` for the values plus `max_name_bytes` per field for the names. Files are `StoredFile`s, deleted on drop unless persisted or taken. `iter` yields text fields `(name, value)` in body order. `take_file` removes and returns the first file with that name. `captured_token` is the token field's value (see [`TokenField`](#tokenfield-tokenplacement)).

`parse_with` is the blocking loop over a reader with explicit limits, policy, store, and inspector; it reads in 64 KiB chunks.

**Errors:** `field_one`: `Extract { Duplicate }`. `parse_with`: every multipart error, `Refused`, `Io`, `Store`, `Limit { Disk }`.

```rust
use form_net::{Limits, NoInspector, Policy, Store, TokenField, Upload};

# let content_type = b"multipart/form-data; boundary=X";
# let body_reader = &b"--X\r\nContent-Disposition: form-data; name=\"csrf_token\"\r\n\r\ntok\r\n--X--\r\n"[..];
# // Stand-ins for csrf-sec's verifier and the request's session.
# struct Csrf;
# impl Csrf { fn verify(&self, token: &str, _session: &()) -> bool { token == "tok" } }
# let (csrf, session) = (Csrf, &());
let store = Store::temp()?;
let policy = Policy::DEFAULT.with_token_field(TokenField::new("csrf_token"));
let upload = Upload::parse_with(content_type, body_reader, &Limits::DEFAULT, &policy, &store, &mut NoInspector)?;
let verified = upload.captured_token().is_some_and(|token| csrf.verify(token, session));
# assert!(verified);
# Ok::<(), form_net::FormError>(())
```

### `UploadSession`

`#35` · P1 · Tier 2 · 0.4 · `fs`

```rust,ignore
pub struct UploadSession { /* driver over a StoreSink, text arena, files */ }

impl UploadSession {
    pub fn new(content_type: &[u8], limits: &Limits, policy: &Policy, store: &Store) -> Result<Self, FormError>;
    pub fn push<I: Inspector>(&mut self, chunk: &[u8], inspector: &mut I) -> Result<(), FormError>;
    pub fn finish<I: Inspector>(self, inspector: &mut I) -> Result<Upload, FormError>;
    pub fn captured_token(&self) -> Option<&str>;
    pub fn bytes_seen(&self) -> u64;
}
```

The push-style form for event-driven servers: the server hands each body chunk to `push` as its reads complete; no `Read` is needed. `push` consumes the whole chunk or fails. `finish` says the body ended (`Truncated` if the close delimiter was not seen) and returns the `Upload`. Dropping a session without `finish` deletes every file it stored. `captured_token` is available as soon as the token field ends, so the server can verify the token while the files are still arriving and stop early.

**Errors:** every multipart error, `Refused`, `Io`, `Store`, `Limit { Disk }`; `Poisoned` after an error.

**Cost:** no allocation per chunk after construction except text arena growth; disk writes for spilled files.

```rust
use form_net::{Limits, NoInspector, Policy, Store, UploadSession};

# let store = Store::temp()?;
# let content_type = b"multipart/form-data; boundary=X";
# let body = b"--X\r\nContent-Disposition: form-data; name=\"title\"\r\n\r\nHoliday\r\n--X\r\nContent-Disposition: form-data; name=\"photo\"; filename=\"beach.jpg\"\r\n\r\n\xff\xd8\xff...\r\n--X--\r\n";
let mut session = UploadSession::new(content_type, &Limits::DEFAULT, &Policy::DEFAULT, &store)?;
for chunk in body.chunks(16) {                         // each chunk as the server reads it
    session.push(chunk, &mut NoInspector)?;
}
let upload = session.finish(&mut NoInspector)?;
# Ok::<(), form_net::FormError>(())
```

### `SpillBudget`, `SpillQuota`

`#67` (`SpillBudget`) P2 Tier 3 · `#39` (`SpillQuota`, `Store::quota`) P2 Tier 2 · 0.5 · `fs`

```rust,ignore
pub trait SpillBudget: Send + Sync + 'static {
    fn reserve(&self, bytes: u64) -> bool;
    fn release(&self, bytes: u64);
}

#[derive(Clone)]
pub struct SpillQuota { /* Arc<(AtomicU64 limit, AtomicU64 in use)> */ }

impl SpillQuota {
    pub fn new(max_bytes: u64) -> SpillQuota;
    pub fn in_use(&self) -> u64;
    pub fn limit(&self) -> u64;
    pub fn set_limit(&self, max_bytes: u64);
}

impl SpillBudget for SpillQuota {}
```

A disk budget shared by every request that uses the store, so many uploads at once cannot fill the disk; the request that would cross it is refused with `Store { QuotaExceeded }` (status hint 503: the server is full, not the request wrong).

- `reserve` is called before every disk write with the bytes about to be written and must answer `false` when they do not fit (fail closed). `release` is called exactly once for every reserved byte: when a file is deleted, or when it is persisted (persisted bytes leave the temporary area).
- `SpillQuota` is the in-process budget: a compare-and-swap loop on one atomic counter, lock-free, tested with `loom`. `set_limit` changes the limit live; reservations already made stay, new ones see the new limit. One quota per tenant gives each tenant its own share of the disk.
- The trait is the seam for deployments where one budget must cover several worker processes on a host (a shared-memory or file-based counter) or a fleet (a lease from a coordinator, handed out locally in large grants so `reserve` stays a local operation). form-net needs nothing from the implementation but the two methods.

**Cost:** `SpillQuota::reserve` one uncontended compare-and-swap per write call, about 5 to 20 ns [est.].

```rust
use form_net::{SpillQuota, Store};

let quota = SpillQuota::new(4 << 30);                  // 4 GiB for all uploads on this host
let public = Store::temp()?.quota(quota.clone());
let staff = Store::temp()?.quota(quota.clone());
assert_eq!(quota.in_use(), 0);
# Ok::<(), form_net::FormError>(())
```

### `sweep`

`#40` · P2 · Tier 2 · 0.5 · `fs`

```rust,ignore
pub fn sweep(dir: &std::path::Path, older_than: core::time::Duration) -> Result<usize, FormError>;
```

Removes files named `upload-*.part`, and nothing else, directly inside `dir`, whose modification time is older than `older_than`. It never recurses and never follows links (a link with a matching name is removed as a link, never its target). Returns how many it removed. For `SpillMode::Named` stores after a crash: run it at startup, before the server accepts connections. It is the only function in form-net that reads a clock.

**Errors:** `Store { NotADirectory }`, `Io { kind }`.

```rust
# let incoming = std::env::temp_dir().join(format!("form-net-api-sweep-{}", std::process::id()));
# std::fs::create_dir_all(&incoming).unwrap();
// `incoming` is the store's directory, for example /srv/uploads/incoming.
let removed = form_net::sweep(&incoming, std::time::Duration::from_secs(3600))?;
# assert_eq!(removed, 0);
# std::fs::remove_dir_all(&incoming).unwrap();
# Ok::<(), form_net::FormError>(())
```

---

## Blocking readers

### `MultipartReader`, `PartReader`

`#34` · P1 · Tier 2 · 0.4 · `std` (`save`: `fs`)

```rust,ignore
pub struct MultipartReader<R: std::io::Read> { /* parser, 64 KiB read buffer, reader */ }

impl<R: std::io::Read> MultipartReader<R> {
    pub fn new(content_type: &[u8], reader: R) -> Result<Self, FormError>;
    pub fn with_limits(content_type: &[u8], reader: R, limits: &Limits, policy: &Policy) -> Result<Self, FormError>;
    pub fn next_part(&mut self) -> Result<Option<PartReader<'_, R>>, FormError>;
    pub fn captured_token(&self) -> Option<&str>;
}

pub struct PartReader<'r, R: std::io::Read> { /* borrow of the reader */ }

impl<'r, R: std::io::Read> PartReader<'r, R> {
    pub fn head(&self) -> &OwnedPartHead;
    pub fn text(&mut self) -> Result<String, FormError>;
    pub fn skip(&mut self) -> Result<u64, FormError>;
    #[cfg(feature = "fs")]
    pub fn save(&mut self, store: &Store) -> Result<StoredFile, FormError>;
}

impl<'r, R: std::io::Read> std::io::Read for PartReader<'r, R> {}

pub struct OwnedPartHead { /* copies of the head's views; wiped on drop */ }
impl OwnedPartHead {
    pub fn new() -> OwnedPartHead;                          // D-086
    pub fn with_capacity(capacity: usize) -> OwnedPartHead; // D-086
    pub fn copy_from(&mut self, head: &PartHead<'_>);       // D-086: reuses the allocation
    pub fn view(&self) -> PartHead<'_>;
}
```

A pull-style blocking API, synchronous and free of any runtime: ask for the next part, read it like a file, ask for the next.

- The head is copied once per part into an `OwnedPartHead` (the header buffer is reused for the next part, so it cannot be borrowed across reads); `view` gives the usual `PartHead`.
- `text` reads the rest of a part into a `String`, bounded by `max_value_bytes` (for a file part too), and checks it as text. `skip` discards the rest of the part and returns its length. `save` streams a file part into a store.
- Dropping a `PartReader` before its end skips the rest of the part at the next `next_part` call. `next_part` returns `Ok(None)` after the close delimiter.
- `Read for PartReader` reports a parse error as `std::io::Error` with kind `InvalidData` and the `FormError` as its source.

**Errors:** every multipart error; `Io { kind }` from the reader.

**Cost:** one 64 KiB read buffer; parser throughput.

```rust
# let content_type = b"multipart/form-data; boundary=X";
# let body_reader = &b"--X\r\nContent-Disposition: form-data; name=\"title\"\r\n\r\nHoliday\r\n--X\r\nContent-Disposition: form-data; name=\"photo\"; filename=\"beach.jpg\"\r\n\r\n\xff\xd8\xff...\r\n--X--\r\n"[..];
let mut reader = form_net::MultipartReader::new(content_type, body_reader)?;
while let Some(mut part) = reader.next_part()? {
    if part.head().view().is_file() {
        let mut out = Vec::new();                            // or a File, a socket
        let copied = std::io::copy(&mut part, &mut out)?;    // a parse error arrives as InvalidData
    } else {
        let value = part.text()?;
    }
}
# Ok::<(), Box<dyn std::error::Error>>(())
```

---

## Typed extraction

Plain Rust structs filled from a form, without serde and without a procedural macro (which would need third-party crates): one declarative macro and two small traits. Unknown names are refused, a scalar given twice is refused, a required field that is missing is refused, and every error names the field by its Rust-side name, never by echoing input (FR-58 to FR-60).

### `form!`

`#15` · P1 · Tier 1 · 0.4 · `alloc`

```rust
use form_net::FromForm;

form_net::form! {
    /// The sign-up form.
    #[derive(Debug)]
    pub struct Signup {
        pub email: String,
        pub password: form_net::Secret,
        pub display_name: Option<String> = "display-name",
        pub age: u8,
        pub newsletter: bool,
        pub tags: Vec<String> = "tags[]",
        pub avatar: Option<form_net::StoredFile>,
    }
    ignore = ["csrf_token", "_charset_"];
}

# let content_type = b"application/x-www-form-urlencoded";
# let body = b"email=a%40b.example&password=hunter2&age=30&newsletter=on&tags[]=rust&tags[]=web&csrf_token=t";
let signup = Signup::from_body(content_type, body)?;
# assert_eq!((signup.age, signup.newsletter, signup.tags.len()), (30, true, 2));
# Ok::<(), form_net::FormError>(())
```

**Behaviour**

- Expands to the struct exactly as written (attributes, doc comments, and visibility passed through), a hidden builder with one slot per field, and `impl FromForm`. The struct may not be generic and must have named fields.
- The wire name of a field is its Rust name; `= "wire-name"` after the type overrides it (for names that are not Rust identifiers, such as `display-name` or `tags[]`). Bracket names are plain names: `tags[]` is matched byte for byte, never expanded into a structure.
- The optional `ignore = [ ... ];` after the struct lists names that are accepted and dropped: the anti-forgery field and `_charset_` are the usual ones. An ignored name may appear any number of times. Every other name the struct does not declare is refused (`Extract { Unknown }`).
- Field types implement [`FromField`](#fromfield-valueerror); the shipped types are listed under [Value rules](#value-rules).
- Matching a name is a length switch plus byte comparisons, which the compiler builds from the macro's comparisons: no hash map, no allocation beyond what the field types own.

**Errors:** compile errors for syntax the macro does not support; at run time, as [`FromForm`](#fromform).

**Cost:** nothing beyond `FromForm`; compile time grows linearly with the field count.

```rust
use form_net::FromForm;

form_net::form! {
    #[derive(Debug)]
    pub struct Search {
        pub q: String,
        pub page: Option<u32>,
    }
}

let s = Search::from_query(b"q=rust+forms&page=2")?;
assert_eq!((s.q.as_str(), s.page), ("rust forms", Some(2)));

let err = Search::from_query(b"q=a&q=b").unwrap_err();
assert_eq!(err.code(), "extract.duplicate");               // names the field `q`
assert!(Search::from_query(b"q=a&admin=1").is_err());     // unknown name
assert!(Search::from_query(b"page=2").is_err());          // `q` is required
# Ok::<(), form_net::FormError>(())
```

### `FromForm`

`#14` (provided methods) P1 Tier 1 · `#61` (`from_query`) P1 Tier 1 · `#62` (`from_body*`) P1 Tier 1 and 2 · 0.4 · `alloc` (`from_upload`: `fs`)

```rust,ignore
pub trait FromForm: Sized {
    type Builder: FormBuilder<Output = Self> + Default;

    // Tier 1, provided
    fn from_urlencoded(input: &[u8]) -> Result<Self, FormError>;
    fn from_query(query: &[u8]) -> Result<Self, FormError>;
    fn from_multipart(content_type: &[u8], body: &[u8]) -> Result<Self, FormError>;
    fn from_body(content_type: &[u8], body: &[u8]) -> Result<Self, FormError>;
    #[cfg(feature = "fs")]
    fn from_upload(upload: Upload) -> Result<Self, FormError>;
    #[cfg(feature = "fs")]
    fn from_upload_mut(upload: &mut Upload) -> Result<Self, FormError>;            // 0.5, D-090
    fn from_body_all(content_type: &[u8], body: &[u8]) -> Result<Self, FormErrors>; // 0.5, D-090

    // Tier 2, provided
    fn from_urlencoded_with(input: &[u8], limits: &Limits, policy: &Policy) -> Result<Self, FormError>;
    fn from_body_with(content_type: &[u8], body: &[u8], limits: &Limits, policy: &Policy)
        -> Result<Self, FormError>;
    fn from_body_inspected<I: Inspector>(
        content_type: &[u8],
        body: &[u8],
        limits: &Limits,
        policy: &Policy,
        inspector: &mut I,
    ) -> Result<Self, FormError>;
    fn from_body_all_with(content_type: &[u8], body: &[u8], limits: &Limits, policy: &Policy)
        -> Result<Self, FormErrors>;                                                 // 0.5
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FormErrors { /* Vec<FormError> */ }                                       // 0.5
impl FormErrors {
    pub fn iter(&self) -> impl Iterator<Item = &FormError>;
    pub fn len(&self) -> usize;
    pub fn is_empty(&self) -> bool;
    pub fn first(&self) -> Option<&FormError>;
    pub fn field(&self, name: &str) -> Option<ExtractError>;
    pub fn unknown(&self) -> usize;
    pub fn status_hint(&self) -> u16;
}
impl From<FormError> for FormErrors {}
impl core::fmt::Display for FormErrors {}
impl std::error::Error for FormErrors {}
```

Every method feeds each field to a fresh `Builder` in input order and then calls `finish`.

- `from_urlencoded` and `from_query` (one leading `?` stripped) work over `raw_pairs` and decode each name and value into a 512-byte stack buffer when it fits, and a heap scratch otherwise, so typical forms extract with no allocation beyond what the field types own.
- `from_multipart` parses a buffered multipart body; file fields receive [`FileValue::Buffered`](#filevalue).
- `from_body` reads the `Content-Type` and does either: the one call a handler needs when it does not care which encoding the form used. Which encodings a route accepts is the route's decision (customs-sec checks the declared content type before the body is read); a route that accepts only one calls `from_urlencoded` or `from_multipart`.
- `from_upload` takes a finished `Upload`; file fields receive `FileValue::Stored`, moving the `StoredFile` into the struct.
- `from_upload_mut` leaves the upload usable (D-090): text is read by reference, and only files under declared names are moved out; files under `ignore = [..]` names stay for the caller. Unknown names are refused as in `from_upload`; on error, files already moved are dropped (deleted).
- `from_body_all` collects every field's error instead of stopping at the first, for showing a form again with a message beside each field: one error per received value that fails and one per unknown name, plus `Missing` for an absent field unless that field already has an error. A parse error stops the parse and is then the only error (D-097).
- `from_body_inspected` is the single-pass form for content inspection: the inspector and the struct see the same bytes in one parse.

**Errors:** the parser's errors; `Extract { reason, field }` (see [`ExtractError`](#reason-enums)).

**Cost:** parse cost plus 10 to 30 ns per field for the name match and value conversion [est.]; at least 20% faster than `serde_urlencoded::from_bytes` into the same struct.

### `FormBuilder`

`#56` · P1 · Tier 3 · 0.4 · `alloc`

```rust,ignore
pub trait FormBuilder {
    type Output;
    fn text(&mut self, name: &str, value: &str) -> Result<(), FormError>;
    fn file(&mut self, name: &str, file: FileValue<'_>) -> Result<(), FormError>;
    fn finish(self) -> Result<Self::Output, FormError>;
    // 0.5, provided:
    fn file_field(&self, name: &str) -> Option<bool> { Some(true) }
    fn finish_all(self, errors: &mut Vec<FormError>) -> Option<Self::Output> where Self: Sized;
}
```

What `form!` generates, and what a hand-written `FromForm` implementation provides. `file_field` says whether a file name is declared (`Some(true)`), ignored (`Some(false)`), or unknown (`None`), for `from_upload_mut`; `finish_all` records every field's error for `from_body_all`. `form!` implements both; the defaults treat every name as declared and record the one error `finish` returns. A builder reports problems as `FormError::Extract { reason, field }`.

```rust
use form_net::{ExtractError, FileValue, FormBuilder, FormError, FromForm};

/// Accepts any field names, keeping the last `q` (a builder may choose its own rules).
#[derive(Default)]
struct LastQ(Option<String>);

impl FormBuilder for LastQ {
    type Output = Query;
    fn text(&mut self, name: &str, value: &str) -> Result<(), FormError> {
        if name == "q" { self.0 = Some(value.to_owned()); }
        Ok(())
    }
    fn file(&mut self, _name: &str, _file: FileValue<'_>) -> Result<(), FormError> {
        Err(FormError::Extract { reason: ExtractError::WrongKind, field: Some("q") })
    }
    fn finish(self) -> Result<Query, FormError> {
        Ok(Query { q: self.0.unwrap_or_default() })
    }
}

struct Query { q: String }
impl FromForm for Query { type Builder = LastQ; }
```

### `FromField`, `ValueError`

`#56` · P1 · Tier 3 · 0.4 · `alloc`

```rust,ignore
pub trait FromField: Sized {
    type Slot: Default;
    fn accept_text(slot: &mut Self::Slot, value: &str) -> Result<(), ValueError> {
        let _ = (slot, value);
        Err(ValueError::ExpectedFile)
    }
    fn accept_file(slot: &mut Self::Slot, file: FileValue<'_>) -> Result<(), ValueError> {
        let _ = (slot, file);
        Err(ValueError::ExpectedText)
    }
    fn finish(slot: Self::Slot) -> Result<Self, ValueError>;
}

#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ValueError { Missing, Duplicate, Invalid(&'static str), ExpectedFile, ExpectedText }
```

How one field type takes its values. The slot holds what has arrived so far, so every arity rule lives in one place: a scalar's slot is `Option<T>` (a second value is `Duplicate`, none is `Missing`), `Vec<T>` pushes, `bool` finishes a missing value as `false`. `form!` turns a `ValueError` into `FormError::Extract` with the field's name. Implement it for an application type (an email address, a slug) to validate while extracting.

```rust
use form_net::{FromField, ValueError};

pub struct Slug(String);

impl FromField for Slug {
    type Slot = Option<Slug>;
    fn accept_text(slot: &mut Option<Slug>, value: &str) -> Result<(), ValueError> {
        if slot.is_some() { return Err(ValueError::Duplicate); }
        if value.is_empty() || value.len() > 64 || !value.bytes().all(|b| b.is_ascii_lowercase() || b == b'-') {
            return Err(ValueError::Invalid("lowercase letters and hyphens, 1 to 64"));
        }
        *slot = Some(Slug(value.to_owned()));
        Ok(())
    }
    fn finish(slot: Option<Slug>) -> Result<Slug, ValueError> {
        slot.ok_or(ValueError::Missing)
    }
}
```

### `FileValue`

`#56` · P1 · Tier 3 · 0.4 · `alloc`

```rust,ignore
#[non_exhaustive]
pub enum FileValue<'a> {
    Buffered(&'a Part<'a>),
    #[cfg(feature = "fs")]
    Stored(StoredFile),
}
```

A file field's value: a part of a buffered body (`from_multipart`, `from_body`), or a stored file (`from_upload`).

### Value rules

The `FromField` implementations that ship (FR-58, FR-59):

| Type | Accepts | Missing |
|---|---|---|
| `String` | any text | `Missing` |
| `Secret` | any text | `Missing` |
| `u8` to `u128`, `usize` | ASCII digits only: no sign, no whitespace, no empty string; leading zeros accepted; overflow is `Invalid("out of range")` | `Missing` |
| `i8` to `i128`, `isize` | an optional `-`, then digits as above; no `+` | `Missing` |
| `f32`, `f64` | `core` float syntax, then NaN and infinities refused (`1e309` overflows to infinity and is refused) | `Missing` |
| `bool` | exactly `on`, `true`, `1` (true) or `off`, `false`, `0` (false); `ON`, `yes` refused | `false` (an unchecked checkbox sends nothing) |
| `char` | exactly one Unicode scalar value | `Missing` |
| `Option<T>` | as `T` | `None` |
| `Vec<T>` | each value as `T`, in order | empty |
| `FileData` | files only; an empty file input counts as missing | `Missing` |
| `StoredFile` | files only (`Buffered` parts are copied into a memory-held file); an empty file input counts as missing | `Missing` |

Text given to a file field is `ExpectedFile`, and a file given to a text field is `ExpectedText`; both surface as `Extract { WrongKind }`. A scalar given twice is `Duplicate` for every type except `Vec`.

### `Secret`

`#16` · P1 · Tier 1 · 0.4 · `alloc`

```rust,ignore
pub struct Secret { /* String */ }

impl Secret {
    pub fn expose(&self) -> &str;
    pub fn len(&self) -> usize;
    pub fn is_empty(&self) -> bool;
}

impl From<String> for Secret {}
impl FromField for Secret {}
impl core::fmt::Debug for Secret { /* "Secret(..)" */ }
impl Drop for Secret { /* wipes */ }
```

For passwords and other secret fields. No `Display`, no `Clone` (a clone is a second copy to wipe), no way to take the `String` out. `expose` is the only way to read it, which makes every read easy to find in review.

**Wiping without `unsafe` in form-net.** With `alloc`, owned values are wiped with crypt-io's `wipe` and `wipe_vec`, volatile writes over the whole allocation including its spare capacity (D-083); the allocation-free core, which owns no allocation, uses `fill(0)` followed by `core::hint::black_box`, which is best effort. Buffers that grow move through a wiping grow, so a reallocation never frees a copy. Either way the wipe clears the bytes form-net owns, not copies the caller made. The cost is one write over the allocation, about 50 to 100 ns for a 2 KiB form [est.]. `Form`, `Upload`'s text and token, `Secret`, `FileData`, `OwnedPartHead`, memory-held `StoredFile` bytes and every file's name, file name, and content type, the driver's text and sniff buffers, the blocking reader's buffers, and the parsers' header and token buffers (on `reset` and drop) are wiped this way, on success and on every error path (D-085).

### `FileData`

`#17` · P1 · Tier 1 · 0.4 · `alloc`

```rust,ignore
pub struct FileData { /* bytes: Vec<u8>, filename: Option<String>, content_type: Option<String> */ }

impl FileData {
    pub fn bytes(&self) -> &[u8];
    pub fn into_bytes(self) -> Vec<u8>;
    pub fn len(&self) -> usize;
    pub fn filename(&self) -> Option<&str>;
    pub fn content_type(&self) -> Option<&str>;
}

impl FromField for FileData {}
impl core::fmt::Debug for FileData { /* length only */ }
```

An owned file for buffered bodies, where a `StoredFile` is not available or not wanted; also the output of `MemorySink`. The file name is the raw name as UTF-8 (refused as `Extract { Invalid }` if it is not); sanitizing it is the caller's choice. Given a `StoredFile` that spilled to disk, `FileData` reads it into memory, bounded by the file's length, which `max_file_bytes` already bounded.

---

## Client side

For services that send forms: outbound HTTP clients, webhooks, tests.

### `MultipartWriter`

`#42` · P2 · Tier 2 · 0.5 · `std` (`new`: `random`)

```rust,ignore
pub struct MultipartWriter<W: std::io::Write> { /* out, boundary, matcher, state */ }

impl<W: std::io::Write> MultipartWriter<W> {
    pub fn new(out: W) -> Result<Self, FormError>;
    pub fn with_boundary(out: W, boundary: Boundary) -> Self;
    pub fn boundary(&self) -> &Boundary;
    pub fn content_type(&self) -> String;
    pub fn text(&mut self, name: &str, value: &str) -> Result<(), FormError>;
    pub fn file_start(&mut self, name: &str, filename: &str, content_type: Option<&str>) -> Result<(), FormError>;
    pub fn file_data(&mut self, chunk: &[u8]) -> Result<(), FormError>;
    pub fn file<R: std::io::Read>(&mut self, name: &str, filename: &str, content_type: Option<&str>, data: R)
        -> Result<u64, FormError>;
    pub fn verify_boundary(&mut self, on: bool);
    pub fn finish(self) -> Result<W, FormError>;
}
```

Writes a `multipart/form-data` body as browsers do (FR-64 to FR-66).

- `new` generates a boundary (see `Boundary::generate`); `with_boundary` uses the caller's. `content_type` is the exact header value to send: `multipart/form-data; boundary=...`.
- Each part is `--boundary CRLF`, the `Content-Disposition` line, an optional `Content-Type` line, a blank line, the data, and `CRLF`; `finish` writes the close delimiter `--boundary-- CRLF` and returns the writer.
- Names and file names are escaped as browsers do: LF, CR, and `"` become `%0A`, `%0D`, `%22`, nothing else. Every input that would read back differently is refused before a byte of its part is written (D-092): any other control byte except HTAB (`Syntax { ControlInHeader }`); an empty name (`Syntax { EmptyName }`); any backslash in a name (form-net refuses one, D-082), a file name that ends with `\` (it would read as `\"` before the closing quote), and a literal `%0A`, `%0D`, or `%22` in either (a reader decodes it) (`Syntax { AmbiguousQuote }`); a `Content-Type` value that fails parse-net's media-type grammar, repeats a parameter, or is `multipart/*` (`Syntax { InvalidContentType }`); a `_charset_` field naming anything but UTF-8 or US-ASCII (`Encoding { UnsupportedCharset }`). So no CR, LF, or NUL can reach a header and **every body the writer produces parses back with form-net to the same fields**, under limits that admit its sizes (`max_name_bytes` and `max_filename_bytes` count the escaped bytes, so each CR, LF, and `"` counts three).
- Text values have their line breaks normalized to CRLF, as browsers do (a lone CR or LF becomes CRLF).
- With a caller boundary, every data chunk goes through the same streaming matcher the parser uses, across chunk splits, and data that contains the delimiter is refused (`Syntax { BoundaryInContent }`). With a generated boundary the scan is skipped (128 random bits make a collision negligible) unless `verify_boundary(true)` is set.
- `file_data` outside a file part, or any call after an error, is `Poisoned`. A refusal of a head writes nothing of that part; a `BoundaryInContent` or `Io` error can come after part of the data was written, so after any error the output is incomplete and must be discarded.

**Errors:** `Syntax { BoundaryInContent | ControlInHeader | EmptyName | AmbiguousQuote | InvalidContentType }`; `Encoding { UnsupportedCharset }`; `Io { kind }` from the writer; `Random` from `new`; `Poisoned`.

**Cost:** writes go straight to `out`; head encoding under 100 ns per part [est.]; the boundary scan about 0.1 to 0.3 ns per byte when on [est.].

```rust
let mut body = Vec::new();
let mut w = form_net::MultipartWriter::new(&mut body)?;
w.text("title", "Holiday")?;
w.file("photo", "beach.jpg", Some("image/jpeg"), &b"\xff\xd8\xff..."[..])?;   // any std::io::Read, such as a File
let content_type = w.content_type();
w.finish()?;

let mp = form_net::parse_multipart(content_type.as_bytes(), &body)?;
assert_eq!(mp.field("title"), Some("Holiday"));
# Ok::<(), Box<dyn std::error::Error>>(())
```

### `Boundary::generate`

`#43` · P2 · Tier 2 · 0.5 · `random`

```rust,ignore
impl Boundary {
    pub fn generate() -> Result<Boundary, FormError>;
}
```

`form-net-` followed by 32 lowercase hex characters from 16 bytes of the OS random source: 41 bytes, all `bchars`.

**Errors:** `Random` when the OS random source fails (fail closed). **Cost:** one random read, a system call [est. under 1 µs].

### `encode_part_head`

`#55` · P2 · Tier 3 · 0.5 · core

```rust,ignore
pub fn encode_part_head(
    boundary: &Boundary,
    first: bool,
    name: &str,
    filename: Option<&str>,
    content_type: Option<&str>,
    out: &mut [u8],
) -> Result<usize, FormError>;
```

Writes one part's head into a caller buffer without allocating: the delimiter (with its leading CRLF unless `first`), the `Content-Disposition` line with browser escaping, the optional `Content-Type` line, and the blank line. Returns the bytes written. After the last part's data the caller writes the close delimiter: `boundary.delimiter()` (`\r\n--` and the boundary) followed by `--\r\n`; for a body with no parts, `--`, the boundary, and `--\r\n`.

**Errors:** `BufferTooSmall { needed }`; `Syntax { ControlInHeader | AmbiguousQuote | InvalidContentType }`.

```rust
let b = form_net::Boundary::new(b"X")?;
let mut out = [0u8; 128];
let n = form_net::encode_part_head(&b, true, "a\"b", None, None, &mut out)?;
assert_eq!(&out[..n], b"--X\r\nContent-Disposition: form-data; name=\"a%22b\"\r\n\r\n");
# Ok::<(), form_net::FormError>(())
```

---

## Errors

### `FormError`

`#18` · P0 · Tier 1 · 0.3 · core

```rust,ignore
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FormError {
    Limit { limit: Limit, at: Location },
    Syntax { reason: SyntaxError, at: Location },
    Encoding { reason: EncodingError, at: Location },
    Policy { reason: PolicyRefusal, at: Location },
    Refused { refusal: Refusal, at: Location },
    Extract { reason: ExtractError, field: Option<&'static str> },
    UnsupportedMediaType,
    InvalidLimits { limit: Limit },
    BufferTooSmall { needed: usize },
    #[cfg(feature = "std")]
    Io { kind: std::io::ErrorKind },
    #[cfg(feature = "std")]
    Random,
    #[cfg(feature = "fs")]
    Store { reason: StoreError },
    Poisoned,
}

impl FormError {
    pub fn status_hint(&self) -> u16;
    pub fn code(&self) -> &'static str;          // #65
    pub fn location(&self) -> Option<Location>;
}

impl core::fmt::Display for FormError {}
impl core::error::Error for FormError {}
```

One error type for the whole crate. It is `Copy` and holds no input bytes, no paths, and no OS message, so it costs nothing to return and is safe to log. Every variant names what was refused; `Display` gives one line with the reason, the location, and what to do, for example `too many parts (part 100, byte 52311): raise max_parts if this route accepts more`.

- `Limit`, `Syntax`, `Encoding`, `Policy`: the input broke a rule; `at` says where.
- `Refused`: an inspector refused the input; `code` returns the inspector's own code.
- `Extract`: typed extraction failed; `field` is the Rust-side field name when there is one.
- `UnsupportedMediaType`: not a form encoding form-net parses.
- `InvalidLimits`, `BufferTooSmall`, `Poisoned`: the caller's configuration or code is wrong, not the input. Their status hint is 500.
- `Io`, `Random`, `Store`: the environment failed.

`core::error::Error` is stable since Rust 1.81, inside the 1.85 MSRV, so the error trait is implemented without `std`.

### `Location`

`#57` · P0 · Tier 3 · 0.3 · core

```rust,ignore
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Location {
    pub part: Option<u32>,
    pub offset: u64,
}

impl Location {
    pub const NONE: Location;
}
```

Where an error was found: the multipart part index (from 0) when inside a part, and the byte offset in the body (or in the component, for `decode_into`) of the element that broke the rule.

### Reason enums

`#57` · P0 · Tier 3 · 0.3 · core (`StoreError`: 0.4, `fs`)

```rust,ignore
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Limit {
    Body, Fields, NameLength, ValueLength, TextTotal, Parts, Files, FileSize,
    PartHeaderBytes, PartHeaderCount, FilenameLength, Preamble, Epilogue, Padding,
    TokenLength, Disk,
}

#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SyntaxError {
    InvalidContentType, MissingBoundary, DuplicateBoundary, InvalidBoundary,
    NoInitialDelimiter, BoundaryInContent, BareLineFeed, MalformedHeaderLine,
    HeaderFolding, InvalidHeaderName, ControlInHeader, MissingContentDisposition,
    DuplicateHeader, NotFormData, MissingName, EmptyName, DuplicateParameter,
    MalformedParameter, AmbiguousQuote, Truncated,
}

#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EncodingError { InvalidPercent, InvalidUtf8, UnsupportedCharset, InvalidExtendedValue }

#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PolicyRefusal {
    TransferEncoding, NestedMultipart, UnknownHeader, BothFilenames,
    ExtendedFilename, TokenFieldLate, DuplicateToken,
}

#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExtractError { Unknown, Missing, Duplicate, Invalid(&'static str), WrongKind }

#[cfg(feature = "fs")]
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StoreError { QuotaExceeded, NotADirectory, DestinationExists }
```

`Limit` names a `Limits` field, or the per-request disk limit (`Disk`). `InvalidLimits` names the field `validate` refused.

### Every error, and what to do

| Error | Raised when | What the caller should do |
|---|---|---|
| `Limit { Body }` | bytes fed exceed `max_body_bytes` | answer 413; raise the route's limit if the upload is legitimate |
| `Limit { Fields }` | too many urlencoded pairs or text fields | 413; look for runaway repeated inputs in the form |
| `Limit { NameLength }` | a raw name longer than `max_name_bytes` | 413 |
| `Limit { ValueLength }` | one text value too long | 413; use a file input for large text |
| `Limit { TextTotal }` | all text values together too long | 413 |
| `Limit { Parts }` | more than `max_parts` parts | 413 |
| `Limit { Files }` | more file parts than `max_files` (0: this route takes no files) | 413 |
| `Limit { FileSize }` | one file larger than `max_file_bytes`, or than `read_to_vec`'s `max` | 413; tell the user the maximum size |
| `Limit { PartHeaderBytes }` | a part's header block too long | 413 |
| `Limit { PartHeaderCount }` | more header lines than `max_part_headers` | 413 |
| `Limit { FilenameLength }` | a file name longer than `max_filename_bytes` | 413 |
| `Limit { Preamble }` | too many bytes before the first delimiter | 413; usually a wrong boundary |
| `Limit { Epilogue }` | too many bytes after the close delimiter | 413 |
| `Limit { Padding }` | transport padding longer than `max_padding_bytes` | 413 |
| `Limit { TokenLength }` | the token field longer than `max_token_bytes` | 413; treat as a forged request |
| `Limit { Disk }` | the request's disk limit reached | 413 |
| `Syntax { InvalidContentType }` | a `Content-Type` (request or part) parse-net refuses | 400 |
| `Syntax { MissingBoundary }`, `DuplicateBoundary`, `InvalidBoundary` | the boundary parameter is absent, repeated, or malformed | 400 |
| `Syntax { NoInitialDelimiter }` | end of input without any delimiter | 400; the body is not multipart or the boundary is wrong |
| `Syntax { BoundaryInContent }` | a full delimiter followed by anything but `--`, padding, or CRLF; or writer data containing the delimiter | 400; a writer caller should use a generated boundary |
| `Syntax { BareLineFeed }` | LF without CR where CRLF is required | 400 |
| `Syntax { MalformedHeaderLine }`, `HeaderFolding`, `InvalidHeaderName`, `ControlInHeader` | a part header line breaks the header grammar | 400 |
| `Syntax { MissingContentDisposition }`, `DuplicateHeader`, `NotFormData`, `MissingName`, `EmptyName`, `DuplicateParameter`, `MalformedParameter` | a part's headers break the RFC 7578 rules | 400 |
| `Syntax { AmbiguousQuote }` | a quoted parameter value contains `\"` | 400 |
| `Syntax { Truncated }` | the body ended before the close delimiter | 400; the client disconnected or the body was cut |
| `Encoding { InvalidPercent }` | a malformed `%` under `PercentPolicy::Refuse` | 400 |
| `Encoding { InvalidUtf8 }` | decoded text, a name, or a requested text view is not UTF-8 | 400 |
| `Encoding { UnsupportedCharset }` | a charset other than UTF-8 or US-ASCII for text, or a `_charset_` value over 32 bytes | 415 |
| `Encoding { InvalidExtendedValue }` | a malformed `filename*` under `AcceptAlone` | 400 |
| `Policy { TransferEncoding }` | a `Content-Transfer-Encoding` other than identity | 415 |
| `Policy { NestedMultipart }` | a part declared `multipart/*` | 415 |
| `Policy { UnknownHeader }` | an unknown part header under `HeaderPolicy::Refuse` | 400 |
| `Policy { BothFilenames }` | `filename` and `filename*` in one part | 400 |
| `Policy { ExtendedFilename }` | `filename*` under `ExtendedFilenamePolicy::Refuse` | 400 |
| `Policy { TokenFieldLate }` | a file part before the token field under `BeforeFirstFile` | 400; treat as a possible forgery (many callers answer 403) |
| `Policy { DuplicateToken }` | the token field given twice | 400; treat as a possible forgery |
| `Refused { refusal }` | an inspector returned `Verdict::Refuse` | answer with `refusal.status`; log `refusal.code` |
| `Extract { Unknown, None }` | a name the struct neither declares nor ignores | 422; add the name to `ignore` if it is expected |
| `Extract { Missing, Some(field) }` | a required field absent | 422 |
| `Extract { Duplicate, Some(field) }` | a scalar given twice; or `get_one` and `field_one` with `field: None` | 422 |
| `Extract { Invalid(reason), Some(field) }` | a value that does not convert | 422 |
| `Extract { WrongKind, Some(field) }` | a file where text was expected, or the reverse | 422 |
| `UnsupportedMediaType` | not urlencoded or `multipart/form-data` | 415 |
| `InvalidLimits { limit }` | `Limits::validate` found an inconsistent pair | fix the configuration (500) |
| `BufferTooSmall { needed }` | a caller buffer shorter than `needed` | size it with `Limits::multipart_buffer_len`, `Limits::urlencoded_buffer_len`, or the raw length (500) |
| `Io { kind }` | the reader, the writer, or the file system failed | 500; check the disk and permissions |
| `Random` | the OS random source failed | 500; never fall back to predictable names |
| `Store { QuotaExceeded }` | the shared spill budget is full | 503 with `Retry-After` |
| `Store { NotADirectory }` | `Store::in_dir` or `sweep` given a non-directory or a link | fix the configuration (500) |
| `Store { DestinationExists }` | `persist` target exists | choose another name (500) |
| `Poisoned` | a call after an error without `reset` | fix the caller (500) |

### `status_hint`

`#18` · P0 · Tier 1 · 0.3 · core

The HTTP status a server should answer with (FR-63): 400, 413, 415, 422, 500, or 503 as in the table above, and the inspector's own code for `Refused` (400 if that code is outside 400 to 599). The caller may map differently; form-net knows nothing about HTTP beyond this hint.

```rust
let err = form_net::parse_urlencoded(&vec![b'a'; 11 << 20]).unwrap_err();
assert_eq!(err.status_hint(), 413);
assert_eq!(form_net::FormError::UnsupportedMediaType.status_hint(), 415);
```

### `code`

`#65` · P1 · Tier 1 · 0.3 · core

A stable, machine-readable name for the error, for metrics, logs, and security signals: the variant and the reason in snake case, joined with a dot.

| Variant | Codes |
|---|---|
| `Limit` | `limit.body`, `limit.fields`, `limit.name_length`, `limit.value_length`, `limit.text_total`, `limit.parts`, `limit.files`, `limit.file_size`, `limit.part_header_bytes`, `limit.part_header_count`, `limit.filename_length`, `limit.preamble`, `limit.epilogue`, `limit.padding`, `limit.token_length`, `limit.disk` |
| `Syntax` | `syntax.invalid_content_type`, `syntax.missing_boundary`, ..., `syntax.truncated` (one per `SyntaxError` variant) |
| `Encoding` | `encoding.invalid_percent`, `encoding.invalid_utf8`, `encoding.unsupported_charset`, `encoding.invalid_extended_value` |
| `Policy` | `policy.transfer_encoding`, `policy.nested_multipart`, `policy.unknown_header`, `policy.both_filenames`, `policy.extended_filename`, `policy.token_field_late`, `policy.duplicate_token` |
| `Extract` | `extract.unknown`, `extract.missing`, `extract.duplicate`, `extract.invalid`, `extract.wrong_kind` |
| others | the inspector's `Refusal::code` for `Refused`; `unsupported_media_type`, `invalid_limits`, `buffer_too_small`, `io`, `random`, `store.quota_exceeded`, `store.not_a_directory`, `store.destination_exists`, `poisoned` |

Codes never change meaning after 1.0; new variants add new codes.

---

## Thread safety and sizes

| Type | `Send` | `Sync` | Notes |
|---|---|---|---|
| `Limits`, `Policy`, `Boundary`, `FormError` | yes | yes | `Copy` values |
| `MultipartParser<B>`, `UrlencodedParser<B>`, `Driver<B, S>` | when `B` (and `S`) are | when `B` (and `S`) are | owned by one request; never shared |
| `Form`, `Multipart`, `Upload`, `StoredFile`, `Secret`, `FileData` | yes | yes | |
| `Store` | yes | yes | shared by reference across requests |
| `SpillQuota` | yes | yes | one atomic counter pair behind an `Arc` |
| `UploadSession` | yes | yes | owned by one request |
| `MultipartReader<R>` | when `R` is | when `R` is | |

Checked at compile time by static assertions in the test suite (NFR-11).

| Value | Size (64-bit) [est.] |
|---|---|
| `Boundary` | 149 bytes |
| `MultipartParser<B>` | 384 bytes plus its buffer (measured; asserted by a test, NFR-18) |
| `FormError` | at most 48 bytes |
| `Limits` | 72 bytes |
| `RawPair` | 48 bytes |

There is no global mutable state anywhere in the crate, no thread, and no timer: form-net uses no CPU while it waits for bytes (NFR-9). The only clock read is in `sweep`.

---

## Enterprise and distributed use

form-net is a parser: it holds no state between requests, and the same version gives the same result for the same bytes on every node and every platform, however the bytes were split. Running it on many nodes needs nothing more. The pieces that touch shared resources are built for operation at scale:

- **Every limit is configurable** with a safe default, per route, as `const` values validated at startup (`Limits::validate`). Overload is refused cleanly: `Limit { .. }` (413) for oversized input, `Store { QuotaExceeded }` (503) when the disk budget is full.
- **Capacity planning:** `Limits::memory_bound` and `Store::memory_bound` give the worst-case memory of one request, for admission control before a body is read.
- **Shared disk budget behind a trait:** `SpillBudget` lets one budget cover a process (`SpillQuota`), several worker processes on a host, or a fleet; `Store::in_dir` puts uploads on a volume of their own; a custom `Sink` sends uploads anywhere, including object storage.
- **Observable at no cost when unused:** `FormError::code` gives stable codes for metrics; `Inspector::failed` reports every refusal to the security layer in the same pass; `Store::stats` and `Store::cleanup_failures` are lock-free counters for metrics and health.
- **Crash behaviour defined:** anonymous spill files cannot survive a crash; named ones are removed by `sweep` at startup. Retries are safe: parsing is deterministic and a failed parse leaves nothing behind.
- **Secrets never leak through the crate:** errors and `Debug` output carry no values; owned values are wiped on drop.

---

## Consumer checklists

### web-net (framework)

| Need | form-net |
|---|---|
| request accessors as short as PHP's `$_GET` / `$_POST` / `$_FILES`: `req.get("q")`, `req.post("email")`, `req.file("avatar")`, parsed once on first access and cached for the request | `parse_query`, `FromForm::from_body` / `parse_multipart_with` or `UploadSession` underneath; web-net owns the request, so it owns the cache |
| decide the body format from `Content-Type` | `FormKind::detect` |
| a typed form in one line, either encoding | `form!` + `FromForm::from_body` |
| a typed form with the route's limits and customs-sec in the same pass | `FromForm::from_body_inspected(ct, body, &route.limits, &route.policy, &mut customs)` |
| GET forms | `FromForm::from_query`, `parse_query` |
| uploads, push-style from the server's read loop | `UploadSession` + `Store` |
| uploads, blocking | `Upload::parse_with`, `read_multipart` |
| re-render an invalid form with its error | `FormError::Extract { field, .. }` (422) |
| the response status | `FormError::status_hint` |
| per-request memory for admission | `Store::memory_bound(&limits)` (0.5) |

### customs-sec (body inspection)

| Need | form-net |
|---|---|
| "the same budget model for forms and multipart" | `Limits`, compiled from each route contract |
| inspect every part before it is stored | `Inspector` (order guarantee above) |
| magic bytes against the declared type | `Inspector::sniff` with `Limits::sniff_bytes`, `FileName::extension`, `ContentType` |
| schema checks | `Inspector::part_start`, `field`, `end` |
| file name rules | `PartHead::filename` (raw views; customs applies path-lib) |
| a stable code for each refusal it reports | `Refusal::new(status, code)`; `FormError::code` for every failure, its own and form-net's, via `failed` |

### csrf-sec (anti-forgery)

| Need | form-net |
|---|---|
| the hidden field's name agreed without a convention document | `TokenField::new(csrf_sec::FIELD_NAME)` (`csrf_token`) |
| verify the token before any upload reaches the disk | `TokenPlacement::BeforeFirstFile` (the default) + `UploadSession::captured_token` |
| a token of 59 or 102 characters fits | `max_token_bytes` default 128 |
| an attacker cannot choose which of two tokens is read | `Policy { DuplicateToken }` |
| constant-time comparison | done by csrf-sec; form-net only captures |

### Outbound clients and webhooks

| Need | form-net |
|---|---|
| a query string or urlencoded body | `serialize_urlencoded`, `Serializer` |
| a multipart body with files | `MultipartWriter` |
| the exact `Content-Type` to send | `MultipartWriter::content_type` |

---

## After 1.0

Specified so the design leaves room for them; neither ships before 1.0.

| Item | Grade | Shape |
|---|---|---|
| `#59` `NamePath` | P3 | `NamePath::parse(name, max_depth, max_index)` splits `a[b][0]` into `Segment::Key`, `Segment::Index`, `Segment::Append`, with depth and index checked while scanning, before any segment is produced (an index of 10^9 costs nothing). Opt-in only; no other API ever applies it. |
| `#60` serde adapter | P3 | a non-default `serde` feature with a `Deserialize` adapter over `FormBuilder`, only if a real user needs it; it would be the only third-party dependency, behind a feature. |

---

## Item index

| # | Item | Kind | Tier | Grade | Version | Feature |
|---|---|---|---|---|---|---|
| 1 | `VERSION` | const | 1 | P0 | 0.1 | core |
| 2 | `FormKind`, `FormKind::detect` | enum, fn | 1 | P0 | 0.3 | core |
| 3 | `parse_urlencoded` | fn | 1 | P0 | 0.3 | alloc |
| 4 | `parse_query` | fn | 1 | P1 | 0.3 | alloc |
| 5 | `pairs`, `Pairs` | fn, iterator | 1 | P0 | 0.3 | alloc |
| 6 | `serialize_urlencoded` | fn | 1 | P1 | 0.3 | alloc |
| 7 | `Form` | struct | 1 | P0 | 0.3 | alloc |
| 8 | `parse_multipart` | fn | 1 | P0 | 0.3 | alloc |
| 9 | `Multipart`, `Part` | structs | 1 | P0 | 0.3 | alloc |
| 10 | `read_multipart` | fn | 1 | P1 | 0.4 | fs |
| 11 | `Upload` | struct | 1 | P1 | 0.4 | fs |
| 12 | `Store::temp`, `Store::memory_only` | fns | 1 | P1 | 0.4 | fs |
| 13 | `StoredFile`, `StoredReader` | structs | 1 | P1 | 0.4 | fs |
| 14 | `FromForm` provided methods | trait | 1 | P1 | 0.4 | alloc |
| 15 | `form!` | macro | 1 | P1 | 0.4 | alloc |
| 16 | `Secret` | struct | 1 | P1 | 0.4 | alloc |
| 17 | `FileData` | struct | 1 | P1 | 0.4 | alloc |
| 18 | `FormError`, `status_hint`, `location` | enum, fns | 1 | P0 | 0.3 | core |
| 19 | `FileName` core views (`raw`, `as_str`, `is_empty`, `source`), `FileNameSource` | struct, enum | 1 | P0 | 0.3 | core |
| 20 | `FileName::basename`, `html_unescape_into`, `html_unescaped` | fns | 1 | P1 | 0.3 | core / alloc |
| 21 | `FileName::sanitized` | fn | 1 | P1 | 0.4 | fs |
| 22 | `FileName::extension` | fn | 1 | P2 | 0.4 | core |
| 23 | `FileName::nfc` | fn | 1 | P2 | 0.5 | unicode |
| 24 | `Limits`, `DEFAULT`, setters, `validate` | struct | 2 | P0 | 0.3 | core |
| 25 | `Limits::memory_bound` | fn | 2 | P2 | 0.5 | core |
| 26 | `Policy`, `DEFAULT`, setters | struct | 2 | P1 | 0.3 | core |
| 27 | `PercentPolicy` | enum | 2 | P1 | 0.3 | core |
| 28 | `ExtendedFilenamePolicy` | enum | 2 | P1 | 0.3 | core |
| 29 | `HeaderPolicy` | enum | 2 | P2 | 0.3 | core |
| 30 | `EmptyFilePolicy` | enum | 2 | P2 | 0.3 | core |
| 31 | `TokenField`, `TokenPlacement` | struct, enum | 2 | P2 | 0.3 | core |
| 32 | `parse_urlencoded_with` | fn | 2 | P0 | 0.3 | alloc |
| 33 | `parse_multipart_with` | fn | 2 | P1 | 0.3 | alloc |
| 34 | `MultipartReader`, `PartReader`, `OwnedPartHead` | structs | 2 | P1 | 0.4 | std (`save`: fs) |
| 35 | `UploadSession` | struct | 2 | P1 | 0.4 | fs |
| 36 | `Upload::parse_with` | fn | 2 | P1 | 0.4 | fs |
| 37 | `Store::in_dir` and setters (except `quota`), `SpillMode`, `StoreSink` | fns, enum, struct | 2 | P1 | 0.4 | fs |
| 38 | `Store::cleanup_failures` | fn | 2 | P2 | 0.4 | fs |
| 39 | `SpillQuota`, `Store::quota` | struct, fn | 2 | P2 | 0.5 | fs |
| 40 | `sweep` | fn | 2 | P2 | 0.5 | fs |
| 41 | `Serializer` | struct | 2 | P1 | 0.3 | alloc |
| 42 | `MultipartWriter` | struct | 2 | P2 | 0.5 | std (`new`: random) |
| 43 | `Boundary::generate` | fn | 2 | P2 | 0.5 | random |
| 44 | `raw_pairs`, `RawPairs`, `RawPair` | fn, structs | 3 | P0 | 0.3 | core |
| 45 | `decode_into` | fn | 3 | P0 | 0.3 | core |
| 46 | `encode_into` | fn | 3 | P1 | 0.3 | core |
| 47 | `UrlencodedParser`, `UrlencodedStep`, `UrlencodedEvent` | struct, enums | 3 | P2 | 0.5 | core (`new`: alloc) |
| 48 | `Boundary` (`parse`, `new`, `as_bytes`, `delimiter`) | struct | 3 | P0 | 0.3 | core |
| 49 | `MultipartParser` (with `limits`, `policy`) | struct | 3 | P0 | 0.3 | core (`new`: alloc) |
| 50 | `Step`, `Event`, `PartHead`, `PartSummary`, `PartKind`, `Charset`, `ContentType`, `Headers` | types | 3 | P0 | 0.3 | core |
| 51 | `MultipartParser::captured_token`, `declared_charset` | fns | 3 | P2 | 0.3 | core |
| 52 | `Inspector`, `Verdict`, `NoInspector` | trait, enum, struct | 3 | P0 | 0.4 | core |
| 53 | `Sink`, `DiscardSink`, `MemorySink` | trait, structs | 3 | P1 | 0.4 | core / alloc |
| 54 | `Driver`, `Progress` | struct, enum | 3 | P2 | 0.4 | alloc |
| 55 | `encode_part_head` | fn | 3 | P2 | 0.5 | core |
| 56 | `FormBuilder`, `FromField`, `FileValue`, `ValueError` | traits, enums | 3 | P1 | 0.4 | alloc |
| 57 | `Location`, `Limit`, `SyntaxError`, `EncodingError`, `PolicyRefusal`, `ExtractError`, `StoreError` | types | 3 | P0 | 0.3 (`StoreError`: 0.4) | core (`StoreError`: fs) |
| 58 | `MAX_BOUNDARY_LEN`, `MAX_DELIMITER_LEN` | consts | 3 | P1 | 0.3 | core |
| 59 | `NamePath` | struct | 3 | P3 | after 1.0 | alloc |
| 60 | serde adapter | module | 2 | P3 | after 1.0, if needed | serde |
| 61 | `FromForm::from_query` | fn | 1 | P1 | 0.4 | alloc |
| 62 | `FromForm::from_body`, `from_body_with`, `from_body_inspected` | fns | 1, 2 | P1 | 0.4 | alloc |
| 63 | `parse_urlencoded_inspected` | fn | 2 | P1 | 0.4 | alloc |
| 64 | `parse_multipart_inspected` | fn | 2 | P1 | 0.4 | alloc |
| 65 | `FormError::code` | fn | 1 | P1 | 0.3 | core |
| 66 | `Inspector::failed` | trait method | 3 | P2 | 0.4 | core |
| 67 | `SpillBudget` | trait | 3 | P2 | 0.5 | fs |
| 68 | `Store::stats`, `StoreStats` | fn, struct | 2 | P2 | 0.4 | fs |
| 69 | `MultipartParser::skip_part` | fn | 3 | P1 | 0.3 | core |
| 70 | `Upload::iter` | fn | 1 | P1 | 0.4 | fs |
| 71 | `Store::memory_bound` | fn | 2 | P2 | 0.5 | fs |
| 72 | `Limits::multipart_buffer_len`, `Limits::urlencoded_buffer_len` | const fns | 3 | P1 | 0.3 | core |
| 73 | `Refusal`, `Refusal::new` | struct, fn | 3 | P0 | 0.3 | core |

Items 61 to 73 were added at the design lock; [`../dev/PLAN.md`](../dev/PLAN.md) records why (D-036 to D-073).
