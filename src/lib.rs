//! # form_net
//!
//! Sans-IO form parsing for Rust: `application/x-www-form-urlencoded` and
//! streaming `multipart/form-data`, with budgets checked before allocation,
//! safe spilling of uploads, and typed extraction. No third-party
//! dependencies.
//!
//! ## Status
//!
//! Design lock (`0.2.0`). The public API is specified item by item in
//! `docs/API.md`, the design in `dev/PLAN.md`, and the schedule in
//! `dev/ROADMAP.md` (all in the
//! [repository](https://github.com/jamesgober/form-net)). The parsers ship
//! in `0.3.0`.

#![doc(html_root_url = "https://docs.rs/form-net")]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

/// Crate version string, populated by Cargo at build time.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(test)]
mod tests {
    use super::VERSION;

    #[test]
    fn version_matches_manifest() {
        assert_eq!(VERSION, env!("CARGO_PKG_VERSION"));
        assert_eq!(VERSION.split('.').count(), 3, "expected MAJOR.MINOR.PATCH");
    }
}
