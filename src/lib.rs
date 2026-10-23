//! # form_net
//!
//! Parses what a browser sends when a form is submitted:
//! `application/x-www-form-urlencoded` fields and `multipart/form-data`
//! uploads. It does no I/O: feed it bytes, get fields and parts.
//!
//! Every budget (field count, name and value length, part count, part
//! header size, file count and size, total size) is checked before the bytes
//! it governs are buffered. Input that two parsers could read differently is
//! refused with a precise error, never repaired. No error, `Debug`, or
//! `Display` output ever contains a field value or a file name.
//!
//! ## Tier 1
//!
//! ```
//! // A urlencoded body
//! let form = form_net::parse_urlencoded(b"email=a%40b.example&remember=on")?;
//! assert_eq!(form.get("email"), Some("a@b.example"));
//!
//! // A query string
//! let query = form_net::parse_query(b"?q=rust+forms&page=2")?;
//! assert_eq!(query.get("q"), Some("rust forms"));
//!
//! // Building one
//! assert_eq!(form_net::serialize_urlencoded([("q", "rust forms")]), "q=rust+forms");
//!
//! // A buffered multipart body
//! let body = b"--X\r\nContent-Disposition: form-data; name=\"title\"\r\n\r\nHoliday\r\n--X--";
//! let mp = form_net::parse_multipart(b"multipart/form-data; boundary=X", body)?;
//! assert_eq!(mp.field("title"), Some("Holiday"));
//! # Ok::<(), form_net::FormError>(())
//! ```
//!
//! ## Tiers 2 and 3
//!
//! Explicit [`Limits`] and [`Policy`] per route (`parse_urlencoded_with`,
//! `parse_multipart_with`); the allocation-free core ([`raw_pairs`],
//! [`decode_into`], [`encode_into`], [`Boundary`], [`MultipartParser`]),
//! which builds without `alloc`.
//!
//! ## Features
//!
//! | Feature | Default | Adds |
//! |---|---|---|
//! | (none) | | the allocation-free core; `no_std` without `alloc` |
//! | `alloc` | | owned results and the Tier 1 functions |
//! | `std` | yes | `FormError::Io`, `FormError::Random`; implies `alloc` |
//!
//! The full contract is `docs/API.md` in the
//! [repository](https://github.com/jamesgober/form-net). This release
//! (0.3) is the core; upload storage, inspection hooks, and typed
//! extraction arrive in 0.4.

#![no_std]
#![doc(html_root_url = "https://docs.rs/form-net")]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![forbid(unsafe_code)]
#![deny(missing_docs)]
#![cfg_attr(
    not(test),
    deny(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::indexing_slicing,
        clippy::arithmetic_side_effects,
        clippy::todo,
        clippy::unimplemented,
        clippy::unreachable,
        clippy::dbg_macro,
        clippy::print_stdout,
        clippy::print_stderr
    )
)]

#[cfg(any(feature = "alloc", test))]
extern crate alloc;
#[cfg(any(feature = "std", test))]
extern crate std;

mod error;
mod hook;
mod kind;
mod limits;
mod multipart;
mod scan;
mod urlencoded;
mod wipe;

pub use error::{
    EncodingError, ExtractError, FormError, Limit, Location, PolicyRefusal, SyntaxError,
};
pub use hook::Refusal;
pub use kind::FormKind;
pub use limits::{
    EmptyFilePolicy, ExtendedFilenamePolicy, HeaderPolicy, Limits, PercentPolicy, Policy,
    TokenField, TokenPlacement,
};
pub use multipart::boundary::{Boundary, MAX_BOUNDARY_LEN, MAX_DELIMITER_LEN};
pub use multipart::filename::{FileName, FileNameSource};
pub use multipart::parser::{
    Charset, ContentType, Event, Headers, MultipartParser, PartHead, PartKind, PartSummary, Step,
};
#[cfg(feature = "alloc")]
#[cfg_attr(docsrs, doc(cfg(feature = "alloc")))]
pub use multipart::{Multipart, Part, parse_multipart, parse_multipart_with};
pub use urlencoded::decode::decode_into;
pub use urlencoded::encode::encode_into;
#[cfg(feature = "alloc")]
#[cfg_attr(docsrs, doc(cfg(feature = "alloc")))]
pub use urlencoded::encode::{Serializer, serialize_urlencoded};
pub use urlencoded::raw::{RawPair, RawPairs, raw_pairs};
#[cfg(feature = "alloc")]
#[cfg_attr(docsrs, doc(cfg(feature = "alloc")))]
pub use urlencoded::{Form, Pairs, pairs, parse_query, parse_urlencoded, parse_urlencoded_with};

/// Crate version string, populated by Cargo at build time.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(test)]
mod tests {
    use super::VERSION;

    #[test]
    fn test_version_matches_manifest() {
        assert_eq!(VERSION, env!("CARGO_PKG_VERSION"));
        assert_eq!(VERSION.split('.').count(), 3, "expected MAJOR.MINOR.PATCH");
    }

    #[test]
    fn test_public_types_are_send_and_sync() {
        fn send_sync<T: Send + Sync>() {}
        send_sync::<super::Limits>();
        send_sync::<super::Policy>();
        send_sync::<super::Boundary>();
        send_sync::<super::FormError>();
        send_sync::<super::MultipartParser<&mut [u8]>>();
        #[cfg(feature = "alloc")]
        {
            send_sync::<super::MultipartParser<alloc::vec::Vec<u8>>>();
            send_sync::<super::Form>();
            send_sync::<super::Multipart<'static>>();
        }
    }
}
