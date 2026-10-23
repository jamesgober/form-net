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
//! A typed form, from either encoding:
//!
//! ```
//! use form_net::FromForm;
//!
//! form_net::form! {
//!     pub struct Login {
//!         pub email: String,
//!         pub password: form_net::Secret,
//!         pub remember: bool,
//!     }
//!     ignore = ["csrf_token"];
//! }
//!
//! let body = b"email=a%40b.example&password=pw&remember=on&csrf_token=t";
//! let login = Login::from_body(b"application/x-www-form-urlencoded", body)?;
//! assert_eq!((login.email.as_str(), login.remember), ("a@b.example", true));
//! # Ok::<(), form_net::FormError>(())
//! ```
//!
//! An upload read from any `std::io::Read`, large files spilled to private
//! temporary files that are deleted on drop (feature `fs`):
//!
//! ```
//! # #[cfg(feature = "fs")] {
//! let body = b"--X\r\nContent-Disposition: form-data; name=doc; filename=a.txt\r\n\r\nhello\r\n--X--";
//! let upload = form_net::read_multipart(b"multipart/form-data; boundary=X", &body[..])?;
//! assert_eq!(upload.file("doc").map(|f| f.len()), Some(5));
//! # }
//! # Ok::<(), form_net::FormError>(())
//! ```
//!
//! ## Tiers 2 and 3
//!
//! Explicit [`Limits`] and [`Policy`] per route (`parse_urlencoded_with`,
//! `parse_multipart_with`); an [`Inspector`] that sees every part head, the
//! first bytes of every file, and every field before anything is stored;
//! a [`Sink`] that decides where file bytes go, driven by the [`Driver`]; the
//! allocation-free core ([`raw_pairs`], [`decode_into`], [`encode_into`],
//! [`Boundary`], [`MultipartParser`]), which builds without `alloc`.
//!
//! ## Features
//!
//! | Feature | Default | Adds |
//! |---|---|---|
//! | (none) | | the allocation-free core; `no_std` without `alloc` |
//! | `alloc` | | owned results, the Tier 1 functions, `form!`, `Driver`, `MemorySink` |
//! | `std` | yes | `MultipartReader`, `FormError::Io`, `FormError::Random`; implies `alloc` |
//! | `random` | | the OS random source; implies `std` |
//! | `fs` | | `Store`, `StoredFile`, `Upload`, `UploadSession`, `read_multipart`, `FileName::sanitized`; implies `random` |
//!
//! The full contract is `docs/API.md` in the
//! [repository](https://github.com/jamesgober/form-net). This release
//! (0.4) adds upload storage, inspection hooks, blocking readers, and typed
//! extraction to the 0.3 core; the multipart writer, the shared disk budget,
//! and `sweep` arrive in 0.5.

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
#[cfg(feature = "alloc")]
mod extract;
mod hook;
#[cfg(feature = "std")]
mod io;
mod kind;
mod limits;
mod multipart;
mod scan;
#[cfg(feature = "fs")]
mod store;
mod urlencoded;
mod wipe;

pub use error::{
    EncodingError, ExtractError, FormError, Limit, Location, PolicyRefusal, SyntaxError,
};
#[cfg(feature = "alloc")]
#[doc(hidden)]
pub use extract::{__unknown, __value_error};
#[cfg(feature = "alloc")]
#[cfg_attr(docsrs, doc(cfg(feature = "alloc")))]
pub use extract::{FileData, FileValue, FormBuilder, FromField, FromForm, Secret, ValueError};
#[cfg(feature = "alloc")]
#[cfg_attr(docsrs, doc(cfg(feature = "alloc")))]
pub use hook::MemorySink;
pub use hook::{DiscardSink, Inspector, NoInspector, Refusal, Sink, Verdict};
pub use kind::FormKind;
pub use limits::{
    EmptyFilePolicy, ExtendedFilenamePolicy, HeaderPolicy, Limits, PercentPolicy, Policy,
    TokenField, TokenPlacement,
};
pub use multipart::boundary::{Boundary, MAX_BOUNDARY_LEN, MAX_DELIMITER_LEN};
#[cfg(feature = "alloc")]
#[cfg_attr(docsrs, doc(cfg(feature = "alloc")))]
pub use multipart::driver::{Driver, Progress};
pub use multipart::filename::{FileName, FileNameSource};
#[cfg(feature = "alloc")]
#[cfg_attr(docsrs, doc(cfg(feature = "alloc")))]
pub use multipart::owned_head::OwnedPartHead;
pub use multipart::parser::{
    Charset, ContentType, Event, Headers, MultipartParser, PartHead, PartKind, PartSummary, Step,
};
#[cfg(feature = "alloc")]
#[cfg_attr(docsrs, doc(cfg(feature = "alloc")))]
pub use multipart::{
    Multipart, Part, parse_multipart, parse_multipart_inspected, parse_multipart_with,
};
pub use urlencoded::decode::decode_into;
pub use urlencoded::encode::encode_into;
#[cfg(feature = "alloc")]
#[cfg_attr(docsrs, doc(cfg(feature = "alloc")))]
pub use urlencoded::encode::{Serializer, serialize_urlencoded};
pub use urlencoded::raw::{RawPair, RawPairs, raw_pairs};
#[cfg(feature = "alloc")]
#[cfg_attr(docsrs, doc(cfg(feature = "alloc")))]
pub use urlencoded::{
    Form, Pairs, pairs, parse_query, parse_urlencoded, parse_urlencoded_inspected,
    parse_urlencoded_with,
};

#[cfg(feature = "std")]
#[cfg_attr(docsrs, doc(cfg(feature = "std")))]
pub use io::{MultipartReader, PartReader};
#[cfg(feature = "fs")]
#[cfg_attr(docsrs, doc(cfg(feature = "fs")))]
pub use store::{
    SpillMode, Store, StoreError, StoreSink, StoreStats, StoredFile, StoredReader, Upload,
    UploadSession, read_multipart,
};

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
            send_sync::<super::OwnedPartHead>();
            send_sync::<super::MemorySink>();
            send_sync::<super::Driver<alloc::vec::Vec<u8>, super::MemorySink>>();
            send_sync::<super::FileData>();
            send_sync::<super::Secret>();
        }
        #[cfg(feature = "std")]
        send_sync::<super::MultipartReader<std::fs::File>>();
        #[cfg(feature = "fs")]
        {
            send_sync::<super::Store>();
            send_sync::<super::StoreSink>();
            send_sync::<super::StoredFile>();
            send_sync::<super::Upload>();
            send_sync::<super::UploadSession>();
        }
    }
}
