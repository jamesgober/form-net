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

[Unreleased]: https://github.com/jamesgober/form-net/compare/v0.2.0...HEAD
[0.2.0]: https://github.com/jamesgober/form-net/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/jamesgober/form-net/releases/tag/v0.1.0
