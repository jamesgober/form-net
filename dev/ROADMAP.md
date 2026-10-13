# form-net - Roadmap

> Path from scaffold to a stable 1.0. Hard parts are front-loaded; each phase has hard exit criteria.
> Design and decisions: [`PLAN.md`](./PLAN.md).
>
> **Anti-deferral rule:** no listed hard task moves to a later phase unless this file records the move and the reason.

## v0.1.0 - Scaffold (DONE, 2026-10-13)
- [x] Manifest, README, CHANGELOG, REPS, dual license, CI, deny, clippy, rustfmt, DIRECTIVES, ROADMAP, PLAN.

## v0.2.0 - Design lock
Exit: urlencoded and multipart APIs, events, and budgets written in `docs/API.md` and approved.

## v0.3.0 - Parsers
Exit: both parsers pass the RFC and WHATWG cases whole and byte at a time; no panics on arbitrary bytes.
- [ ] urlencoded parse and serialize.
- [ ] Streaming multipart with linear-time boundary search; Content-Disposition and filename*.
- [ ] Budgets; property tests; fuzz targets; differential reference test; benchmarks.

## v1.0.0 - Stable
- [ ] Frozen after web-net and customs-sec have used it end to end.
