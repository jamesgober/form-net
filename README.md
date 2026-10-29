<h1 align="center">
    <img width="99" alt="Rust logo" src="https://raw.githubusercontent.com/jamesgober/rust-collection/72baabd71f00e14aa9184efcb16fa3deddda3a0a/assets/rust-logo.svg">
    <br>
    <b>form-net</b>
    <br>
    <sub><sup>FORMS AND MULTIPART</sup></sub>
</h1>

<div align="center">
    <a href="https://crates.io/crates/form-net"><img alt="Crates.io" src="https://img.shields.io/crates/v/form-net"></a>
    <a href="https://crates.io/crates/form-net"><img alt="Downloads" src="https://img.shields.io/crates/d/form-net?color=%230099ff"></a>
    <a href="https://docs.rs/form-net"><img alt="docs.rs" src="https://img.shields.io/docsrs/form-net"></a>
    <a href="https://github.com/jamesgober/form-net/actions"><img alt="CI" src="https://github.com/jamesgober/form-net/actions/workflows/ci.yml/badge.svg"></a>
    <a href="https://github.com/rust-lang/rfcs/blob/master/text/2495-min-rust-version.md"><img alt="MSRV" src="https://img.shields.io/badge/MSRV-1.85%2B-blue"></a>
</div>

<br>

<div align="left">
    <p>
        <strong>form-net</strong> parses what browsers send when a form is submitted: <code>application/x-www-form-urlencoded</code> fields and <code>multipart/form-data</code> uploads. It does no I/O; feed it bytes as they arrive, in chunks of any size, and it yields fields and file parts as they complete, so a large upload is streamed, never held in memory whole.
    </p>
    <p>
        Every budget (field count, name and value length, part count, part header size, file count and size, total size) is checked before the bytes it governs are buffered. Input that two parsers could read differently is refused with a precise error, never repaired. Uploads larger than a threshold go to private, randomly named temporary files that cannot outlive the request, or even a crash. A client's file name is never used as a path.
    </p>
    <br>
    <hr>
    <p>
        <strong>MSRV is 1.85+</strong> (Rust 2024 edition). <code>#![forbid(unsafe_code)]</code>. No third-party dependencies apart from the audited <code>zeroize</code> and <code>subtle</code>, which first-party <code>crypt-io</code> brings for wiping secrets.
    </p>
    <blockquote>
        <strong>Status: optimised (<code>0.6.0</code>).</strong> Every item of the design is in, and every benchmark family runs at least 20% faster than the best other Rust form parser, with one short input level (<a href="./docs/PERFORMANCE.md"><code>docs/PERFORMANCE.md</code></a>). Next: hardening (<code>0.7.0</code>). The contract is <a href="./docs/API.md"><code>docs/API.md</code></a>; the schedule is <a href="./dev/ROADMAP.md"><code>dev/ROADMAP.md</code></a>.
    </blockquote>
</div>

<hr>
<br>

## Features

The plan for 1.0. Each item names the version that ships it.

### **Parsing**
- **urlencoded** (0.3) - the WHATWG algorithm with strict refusal: `&` is the only separator, a malformed `%` and invalid UTF-8 are refused, never replaced. Zero copy when nothing needs decoding; a decoder that writes into your buffer and never allocates.
- **Streaming multipart** (0.3) - sans-IO: bytes in, events out, identical for every split of the input. The boundary search is linear and never rescans a byte.
- **Strict framing** (0.3) - bare LF, header folding, duplicate headers and parameters, transfer encodings, nested multipart, `\"` in quoted values, and a missing close delimiter are refused: the constructs where a firewall in front and the application behind could disagree.
- **`no_std` core** (0.3) - the Tier 3 parsers run without `alloc`, on buffers you provide.

### **Uploads**
- **Safe spilling** (0.4) - files over a threshold go to temporary files in a private directory, created with random names and `create_new`, owner-only, deleted on drop and on abort; in the default mode the operating system removes them if the process dies.
- **Disk budgets** (0.4, 0.5) - a limit per request and a budget shared across requests (pluggable, so it can cover a host or a fleet); the request that would cross it is refused with a 503 hint.
- **File names as data** (0.3, 0.4) - raw bytes, base name, a sanitized portable name, an NFC form; never a path.

### **Integration**
- **Inspection hooks** (0.4) - an inspector sees every part head, the first bytes of every file, and every field before anything is stored, in one pass.
- **Anti-forgery token first** (0.4) - the token field is captured, and a file that arrives before it is refused, so a forged upload costs no disk.
- **Typed forms without serde** (0.4) - one declarative macro; unknown, repeated, and missing fields refused; errors name the field, never echo input.
- **Client side** (0.3, 0.5) - a urlencoded serializer and a multipart writer whose output always parses back to the same fields.

---

<br>

## Installation

```toml
[dependencies]
form-net = { version = "0.6", features = ["fs"] }
```

The `fs` feature adds upload storage; the default features cover parsing, hooks, readers, and typed forms. Without default features the crate is `no_std` and needs no allocator: the allocation-free core (`raw_pairs`, `decode_into`, `MultipartParser::with_buffer`) works on any target. Add `features = ["alloc"]` for the owned results.

---

<br>

## Quick start

Everything below works in 0.4.

```rust
use form_net as fnet;

// A urlencoded body (0.3)
let form = fnet::parse_urlencoded(b"email=a%40b.example&remember=on")?;
assert_eq!(form.get("email"), Some("a@b.example"));

// A query string (0.3)
let query = fnet::parse_query(b"?q=rust+forms&page=2")?;
assert_eq!(query.get("q"), Some("rust forms"));

// A buffered multipart body (0.3)
let mp = fnet::parse_multipart(content_type, body)?;
let title = mp.field("title").unwrap_or("");

// A typed form, either encoding (0.4)
use fnet::FromForm;
fnet::form! {
    pub struct Login {
        pub email: String,
        pub password: fnet::Secret,
        pub remember: bool,
    }
    ignore = ["csrf_token"];
}
let login = Login::from_body(content_type, body)?;

// An upload streamed to private temporary files (0.4, feature `fs`)
let upload = fnet::read_multipart(content_type, request_body)?;
if let Some(file) = upload.file("document") {
    file.persist(&destination)?;          // never overwrites
}
```

Configuration (limits per route, policies, inspectors, stores) and the allocation-free Tier 3 surface are in [`docs/API.md`](./docs/API.md).

---

<br>

## Scope

form-net decides whether bytes are a well-formed form. Whether the form is acceptable for a route (schemas, allowed file types by content, Unicode rules) is customs-sec's job; whether the request came from the site's own page is csrf-sec's. HTTP framing is http-net's. See [`dev/PLAN.md`](./dev/PLAN.md) for the full scope, what is deliberately left to other crates, the threat model, and the decision log.

---

<br>

## Documentation

- [`docs/API.md`](./docs/API.md) - every public item, with signatures, behaviour, errors, and examples.
- [`dev/PLAN.md`](./dev/PLAN.md) - design, requirements, threats, performance targets, decisions.
- [`dev/ROADMAP.md`](./dev/ROADMAP.md) - versions and exit criteria.
- [`CHANGELOG.md`](./CHANGELOG.md) and [`docs/release/`](./docs/release/).

<hr>
<br>

## Contributing

See <a href="./dev/DIRECTIVES.md"><code>dev/DIRECTIVES.md</code></a> for engineering standards and the definition of done. Before a PR: `cargo fmt --all`, `cargo clippy --all-targets --all-features -- -D warnings`, and `cargo test --all-features` must be clean.

<br>

<div id="license">
    <h2>License</h2>
    <p>Licensed under either of</p>
    <ul>
        <li><b>Apache License, Version 2.0</b> - <a href="./LICENSE-APACHE">LICENSE-APACHE</a></li>
        <li><b>MIT License</b> - <a href="./LICENSE-MIT">LICENSE-MIT</a></li>
    </ul>
    <p>at your option.</p>
</div>

<div align="center">
  <h2></h2>
  <sup>COPYRIGHT <small>&copy;</small> 2026 <strong>James Gober.</strong></sup>
</div>
