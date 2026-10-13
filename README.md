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
        <strong>form-net</strong> parses what browsers send when a form is submitted: <code>application/x-www-form-urlencoded</code> fields and <code>multipart/form-data</code> uploads. It does no I/O; feed it bytes as they arrive and it yields fields and file parts as they complete, so a large upload is streamed, never held in memory whole.
    </p>
    <p>
        Every budget (field count, name and value length, part count, part header size, file size, total size) is checked before the bytes it governs are buffered. Malformed input is refused with a precise error, never guessed at.
    </p>
    <br>
    <hr>
    <p>
        <strong>MSRV is 1.85+</strong> (Rust 2024 edition). <code>#![forbid(unsafe_code)]</code>.
    </p>
    <blockquote>
        <strong>Status: scaffold (<code>0.1.0</code>).</strong> No functional code yet. The design is written down in <a href="./dev/PLAN.md"><code>dev/PLAN.md</code></a> and the schedule in <a href="./dev/ROADMAP.md"><code>dev/ROADMAP.md</code></a>; the public API is locked in <code>0.2.0</code>.
    </blockquote>
</div>

<hr>
<br>

## Scope

urlencoded decoding (percent-escapes, plus as space, strict UTF-8), streaming multipart parsing (boundary scanning in linear time, part headers, Content-Disposition names and file names), budgets for every dimension, and zero-copy field views. Content inspection of the parts is customs-sec's job.

See [`dev/PLAN.md`](./dev/PLAN.md) for the full scope, what is deliberately left to other crates, and the decision log.

<hr>
<br>

## Contributing

See <a href="./dev/DIRECTIVES.md"><code>dev/DIRECTIVES.md</code></a> for engineering standards and the definition of done. Before a PR: `cargo fmt --all`, `cargo clippy --all-targets --all-features -- -D warnings`, and `cargo test --all-features` must be clean.

<br>

<div id="license">
    <h2>License</h2>
    <p>Licensed under either of</p>
    <ul>
        <li><b>Apache License, Version 2.0</b> &mdash; <a href="./LICENSE-APACHE">LICENSE-APACHE</a></li>
        <li><b>MIT License</b> &mdash; <a href="./LICENSE-MIT">LICENSE-MIT</a></li>
    </ul>
    <p>at your option.</p>
</div>

<div align="center">
  <h2></h2>
  <sup>COPYRIGHT <small>&copy;</small> 2026 <strong>James Gober.</strong></sup>
</div>
