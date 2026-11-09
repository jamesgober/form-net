# form-net - 1.0 readiness note

For James: everything still open after the 0.9 polish pass (D-112), with its numbers and a recommendation. The pass tried every accepted gap, deferred item, known issue, and missed target again; what it fixed is in the changelog for 0.9.0 and in the decision log (D-126 to D-131). What stands is below.

**Written:** 2026-11-07, for 0.9.0; ratios updated 2026-11-08. Ratios against the competitors are the 0.9.0 baseline (`benches/baseline/2026-11-08`, Windows, median of five runs on a loaded machine); other numbers are Linux unless marked. The machine and method are in `docs/PERFORMANCE.md`.

---

## Needs you

| Item | Where it stands | Recommendation |
|---|---|---|
| The deployment test: web-net running the login and upload routes on form-net with the real customs-sec, for two weeks (the v0.9.x exit, D-125) | web-net now runs both routes on 0.9.0 with csrf-sec's real tokens, with no API change (D-134). customs-sec is still a test double, and nothing is deployed yet | Keep 1.0 behind it. It is the only test of the API by a real caller; the freeze should not come before it. What it needs from you: where and when the first site is deployed |
| The 25-byte search query `q=rust+form+parser&page=2` through `pairs` is level with form_urlencoded: 0.94x collecting, 1.05x iterating (bar 1.20x) | You accepted it as level at 0.6 (D-111). Both crates spend the time on the one owned string that `+` decoding needs; form-net checks more on top. `raw_pairs` + `decode_into` on the same input is now 1.55x (1.32x to 1.69x over five runs) | Accept for 1.0. Closing it means a different return type (a small-string type instead of `Cow<str>`), which is an API change for one input shape |

## Stands, with a reason (no decision needed)

| Item | Numbers | Why it stands |
|---|---|---|
| Two D-117 regressions against 0.6 | near misses in the middle of a delimiter 139.55 to 150.21 µs (+8%), still 2.45x multer; 100 parts of 10 bytes 15.08 to 15.76 µs (+4%) | The matcher that refuses bare line breaks (D-113) compares where it stepped; an adversarial shape pays for the check. Both pass the bar |
| Near-miss on a 1 MiB body | whole: 1.39x multer (was 1.23x); 1,500-byte chunks: 1.46x, one run of five at 1.10x (bar 1.20x) | Passes on the median; the chunked row is the one to watch, and a quiet run should settle it |
| The 1 MiB escaped body moves with code placement under the benchmark's default profile | 0.7: `pairs` 332 or 452 µs, 326 to 334 µs under the crate's release profile. 0.9: `pairs` gained and `raw_pairs` + `decode_into` lost (2.88x to 2.13x); under the release profile 0.9.0 reads 2.25x to 2.42x against 0.8.0's 1.91x to 2.25x | Not a cost users pay, who build with the release profile (D-117, D-133); every row passes either way |
| `parse_urlencoded` of ten fields | 379 to 419 ns against a 400 ns estimate | At the estimate: two allocations and a scan |
| `FormKind::detect` | 114 ns against 100 ns | parse-net's strict media-type grammar; belongs in parse-net |
| `RefusalDigest::record` | 5 to 6 ns against an estimate of 1 ns | One locked add costs that on this machine; the estimate was wrong, not the code |
| `FileName::sanitized` | 121 ns for a 24-byte name (172 ns on Windows), estimate about 100 ns | path-lib's sanitizer plus form-net's default-ignorable filter (D-084), one allocation |
| `Store::temp` | 133 µs (285 µs on Windows), estimate 10 to 50 µs | Once per process: the private directory's creation and permission checks |
| 100 MiB upload, CPU only | 5.7% of one core at 1 GB/s (target 5%); spilled to disk 0.7 to 1.0 GB/s, set by the disk | The parser and driver run at 17.7 GB/s; the margin is the sink calls |
| Dropping an in-memory upload | about 10 µs of a 64 KiB upload's cost is the wipe | crypt-io wipes one volatile byte at a time; a word-wide wipe there would cut it about eightfold (a crypt-io change) |
| `FileName::sanitized` filters default-ignorable characters itself | | Until path-lib does (D-084); a path-lib change |
| Go 1.27 sends `\"` in a file name; form-net refuses it (`AmbiguousQuote`) | | Accepted deviation: two readers disagree on it (D-015, `docs/SECURITY.md`) |
| A backslash in a part name, which browsers send as is, is refused | | Accepted deviation (D-082) |
| `RefusalDigest` is not built on targets without atomic adds (thumbv6m) | | Those targets have no atomic add; everything else in form-net builds there (D-123) |
| The security audit job works around RUSTSEC-2026-0385 | | `ctutils` comes in through crypt-io's optional dependencies under the 1.85 MSRV (D-091); a crypt-io change |
| Platform gaps: Windows ACLs are inherited from the per-user temporary directory; free disk space is not queried (a configured budget is used); Linux `O_TMPFILE` is not used | | Each needs `unsafe` or a third-party crate, which the rules forbid (PLAN 9, D-059, PLAN 14) |

## Left for 0.9.x, before the freeze

- Absolute times on a quiet machine: the 0.9.0 baseline ran at 100% load, so its absolute times read about 1.5x high; its ratios hold. The 0.7.0 quiet numbers stay the absolute reference until then.
- The deployment test above.
- The adversarial review (DIRECTIVES, definition of done item 10).
- `docs/STABILITY-1.0.md`, the full audit.

## After 1.0

`NamePath` (#59), the serde adapter (#60, only for a real user), and a nonblocking `UploadSession` example.
