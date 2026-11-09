# form-net 1.0 stability contract

**Status: draft (0.9.1).** This is what `form-net 1.0.0` will commit to, written before the freeze so the release candidates can be checked against it. It takes effect when 1.0.0 ships, on James's decision, after the audit recorded at the end of this file.

---

## What 1.0 means here

When 1.0.0 ships, everything in [the frozen surface](#the-frozen-surface) and [the reading contract](#the-reading-contract) is fixed for the 1.x series:

- **Patch (`1.0.x`)**: bug fixes and security fixes only. No API change. A behaviour change only where the old behaviour was a defect: a body read differently from the contract below, a limit that did not hold, or a refusal missing for a construct readers disagree on.
- **Minor (`1.x.0`)**: additive. New items, new limits and policies with defaults that change nothing for existing callers, new error variants (the error enums are `#[non_exhaustive]`), new refusals under the rule below.
- **Major (`2.0.0`)**: anything else.

---

## The frozen surface

Every item in the [item index of `docs/API.md`](API.md#item-index) (#1 to #75) with its signature and documented behaviour as of 1.0.0: names, parameter and return types, trait items, feature gates, and the `Send` and `Sync` of each type. Items whose names start with `__` are macro support, hidden from the docs, and not part of the API.

The feature flags (`std` default, `alloc`, `random`, `fs`, `unicode`) and what each enables are frozen. A feature is never removed and never stops implying what it implies.

---

## The reading contract

form-net's security rests on how it reads bodies, not only on its signatures, so the reading rules are part of the contract:

- **Accepted bodies read the same.** A body that 1.0.0 accepts gives the same parts, names, values, file names, and kinds in every 1.x release, under the same `Limits` and `Policy`, however the body is split.
- **Refusals are never loosened.** A construct 1.0.0 refuses stays refused in 1.x. A new way to accept it can only arrive as a new opt-in policy whose default keeps the refusal.
- **New refusals are allowed, under one rule.** A 1.x release may refuse a construct that two readers read differently (the T-14 class), as a security fix. Each one is recorded in `docs/SECURITY.md` with the readers it was checked against and a decision-log entry, and it never refuses a body in the browser capture corpus. A patch release adds one only for a reported or found differential; otherwise it waits for a minor.
- **Default limits and policies are fixed.** `Limits::DEFAULT` and `Policy::DEFAULT` keep their values in 1.x. A new limit's default never refuses a body that the 1.0 defaults accept.
- **Errors are stable.** Every error code (`FormError::code`) keeps its string and meaning, and a code is never reused for another failure. `status_hint` keeps its mapping. New codes arrive with new variants in minor releases. `RefusalDigest`'s slot order (`SLOT_CODES`) only grows at the end.
- **Nothing leaks.** No `Debug`, `Display`, or error output shows a field value, in any release.
- **`Limits::memory_bound` stays a true upper bound** on what a request can allocate under those limits.

---

## MSRV policy

- 1.0's MSRV is Rust 1.85 (edition 2024), declared as `rust-version`.
- An MSRV raise is a minor-version event, never a patch. Every patch of a minor keeps that minor's MSRV.

---

## Dependencies

- First-party crates only (parse-net, crypt-io, mod-rand, path-lib, unicode-lang), at published versions; no third-party runtime dependency beyond the audited cryptography crypt-io brings (zeroize, subtle).
- A minor or patch bump of a first-party dependency may land in a patch release when it carries a fix. A major bump of one is a minor-version event for form-net and never changes the reading contract.

---

## Performance

No 1.x release may fall below the bar in any benchmark family (at least 1.20x the best other Rust crate, `docs/PERFORMANCE.md`), except where a recorded decision accepts it, as the 25-byte search query is accepted as level (D-111). A change that makes a ratio more than 5% worse needs its reason in the decision log before it merges.

---

## What requires a 2.0

- Removing or renaming an item of the frozen surface, or changing its signature.
- Accepting a construct 1.x refuses, other than through a new opt-in policy.
- Reading an accepted body differently.
- Changing a default limit or policy, an error code's string or meaning, or a status hint.
- Removing a feature flag, or changing what a feature implies.

---

## Migration policy

- 1.x to a later 1.x is `cargo update`-safe.
- 1.x to 2.0 ships with `MIGRATION.md`: each breaking change, its mechanical replacement where there is one, and its reason. The last 1.x line gets security fixes for at least 12 months after 2.0.

---

## How the contract is enforced

- `cargo-semver-checks` against the last published release before every release; a patch release must need no semver update.
- CI on Linux, macOS, and Windows, on 1.85 and stable, plus the bare-metal target, the loom model, the package check, and the dependency checks.
- The reading contract is held by tests: the browser capture corpus at every split (`tests/multipart_browsers.rs`), the mutation generator (`tests/waffled.rs`), the split tests that compare events and errors byte for byte, the attack tests for every threat (`tests/attacks.rs`), the review tests (`tests/review_*.rs`), and `tests/memory_bound.rs`. The error codes and their slots are held by the tests in `src/error.rs`.

---

## The 1.0 audit

To be recorded here before 1.0.0 (DIRECTIVES, section 3): API, docs, tests, benchmarks, and security, each with its date and result. Done so far, during 0.9:

| Area | Status |
|---|---|
| Polish pass (every accepted gap tried again) | done, 0.9.0; what stands is in `dev/READINESS-1.0.md` |
| Fresh performance baseline | done, 0.9.0 (`benches/baseline/2026-11-08`), ratios; absolute times wait for a quiet run |
| Adversarial review | done, 0.9.1: 18 findings, each fixed against its failing test (D-136 to D-146) |
| Use by web-net | done for the first site's routes (D-134) |
| Use by customs-sec as the inspector | open |
| Two weeks on a deployed site | open |
| This contract | draft; final text at 1.0 |
