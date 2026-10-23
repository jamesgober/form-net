//! Typed extraction: plain Rust structs filled from a form, without serde
//! and without a procedural macro.
//!
//! [`form!`](crate::form) generates a struct and its [`FromForm`]
//! implementation; each field type implements [`FromField`]. Unknown names
//! are refused, a scalar given twice is refused, a missing required field is
//! refused, and every error names the field by its Rust-side name.

mod file;
mod macros;
mod secret;
mod value;

pub use file::FileData;
pub use secret::Secret;

use crate::error::{ExtractError, FormError};
use crate::hook::{Inspector, NoInspector};
use crate::kind::FormKind;
use crate::limits::{Limits, Policy};
use crate::multipart::Part;

/// Why one field's value did not convert. [`form!`](crate::form) turns it
/// into `FormError::Extract` with the field's name.
///
/// # Examples
///
/// ```
/// use form_net::{FromField, ValueError};
///
/// let mut slot = <u8 as FromField>::Slot::default();
/// assert_eq!(u8::accept_text(&mut slot, "300"), Err(ValueError::Invalid("out of range")));
/// ```
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ValueError {
    /// A required field is absent.
    Missing,
    /// A single-valued field was given twice.
    Duplicate,
    /// The value does not convert; the text says why.
    Invalid(&'static str),
    /// Text was given to a file field.
    ExpectedFile,
    /// A file was given to a text field.
    ExpectedText,
}

/// A file field's value.
#[non_exhaustive]
pub enum FileValue<'a> {
    /// A part of a buffered body.
    Buffered(&'a Part<'a>),
    /// A stored file from an upload.
    #[cfg(feature = "fs")]
    #[cfg_attr(docsrs, doc(cfg(feature = "fs")))]
    Stored(crate::store::StoredFile),
}

impl core::fmt::Debug for FileValue<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            FileValue::Buffered(p) => f.debug_tuple("Buffered").field(p).finish(),
            #[cfg(feature = "fs")]
            FileValue::Stored(s) => f.debug_tuple("Stored").field(s).finish(),
        }
    }
}

/// How one field type takes its values.
///
/// The slot holds what has arrived so far, so every arity rule lives in one
/// place: a scalar's slot is `Option<T>` (a second value is `Duplicate`,
/// none is `Missing`), `Vec<T>` pushes, `bool` finishes a missing value as
/// `false`. Implement it for an application type to validate while
/// extracting.
///
/// # Examples
///
/// ```
/// use form_net::{FromField, ValueError};
///
/// pub struct Slug(String);
///
/// impl FromField for Slug {
///     type Slot = Option<Slug>;
///     fn accept_text(slot: &mut Option<Slug>, value: &str) -> Result<(), ValueError> {
///         if slot.is_some() { return Err(ValueError::Duplicate); }
///         if value.is_empty() || value.len() > 64 || !value.bytes().all(|b| b.is_ascii_lowercase() || b == b'-') {
///             return Err(ValueError::Invalid("lowercase letters and hyphens, 1 to 64"));
///         }
///         *slot = Some(Slug(value.to_owned()));
///         Ok(())
///     }
///     fn finish(slot: Option<Slug>) -> Result<Slug, ValueError> {
///         slot.ok_or(ValueError::Missing)
///     }
/// }
/// ```
pub trait FromField: Sized {
    /// What has arrived so far.
    type Slot: Default;

    /// Takes one text value.
    ///
    /// # Errors
    ///
    /// `ExpectedFile` by default; the type's own conversion errors.
    fn accept_text(slot: &mut Self::Slot, value: &str) -> Result<(), ValueError> {
        let _ = (slot, value);
        Err(ValueError::ExpectedFile)
    }

    /// Takes one file.
    ///
    /// # Errors
    ///
    /// `ExpectedText` by default; the type's own errors.
    fn accept_file(slot: &mut Self::Slot, file: FileValue<'_>) -> Result<(), ValueError> {
        let _ = (slot, file);
        Err(ValueError::ExpectedText)
    }

    /// The value, once every field has arrived.
    ///
    /// # Errors
    ///
    /// `Missing`, or the type's own errors.
    fn finish(slot: Self::Slot) -> Result<Self, ValueError>;
}

/// What [`form!`](crate::form) generates, and what a hand-written
/// [`FromForm`] implementation provides. Reports problems as
/// `FormError::Extract { reason, field }`.
///
/// # Examples
///
/// ```
/// use form_net::{ExtractError, FileValue, FormBuilder, FormError, FromForm};
///
/// /// Accepts any field names, keeping the last `q`.
/// #[derive(Default)]
/// struct LastQ(Option<String>);
///
/// impl FormBuilder for LastQ {
///     type Output = Query;
///     fn text(&mut self, name: &str, value: &str) -> Result<(), FormError> {
///         if name == "q" { self.0 = Some(value.to_owned()); }
///         Ok(())
///     }
///     fn file(&mut self, _name: &str, _file: FileValue<'_>) -> Result<(), FormError> {
///         Err(FormError::Extract { reason: ExtractError::WrongKind, field: Some("q") })
///     }
///     fn finish(self) -> Result<Query, FormError> {
///         Ok(Query { q: self.0.unwrap_or_default() })
///     }
/// }
///
/// struct Query { q: String }
/// impl FromForm for Query { type Builder = LastQ; }
///
/// assert_eq!(Query::from_query(b"q=a&q=b")?.q, "b");
/// # Ok::<(), FormError>(())
/// ```
pub trait FormBuilder {
    /// The struct being built.
    type Output;

    /// One text field.
    ///
    /// # Errors
    ///
    /// `Extract { .. }`.
    fn text(&mut self, name: &str, value: &str) -> Result<(), FormError>;

    /// One file.
    ///
    /// # Errors
    ///
    /// `Extract { .. }`.
    fn file(&mut self, name: &str, file: FileValue<'_>) -> Result<(), FormError>;

    /// The struct, once every field has arrived.
    ///
    /// # Errors
    ///
    /// `Extract { Missing | Invalid, .. }`.
    fn finish(self) -> Result<Self::Output, FormError>;
}

/// The bytes the decoders use without allocating.
const STACK: usize = 512;

/// Feeds a urlencoded body to `builder`, decoding each name and value into a
/// stack buffer when it fits and a heap buffer otherwise.
fn feed_urlencoded<B: FormBuilder, I: Inspector>(
    builder: &mut B,
    input: &[u8],
    limits: &Limits,
    policy: &Policy,
    inspector: &mut I,
) -> Result<(), FormError> {
    let mut name_stack = [0u8; STACK];
    let mut value_stack = [0u8; STACK];
    let mut heap = alloc::vec::Vec::new();
    let fed = (|| {
        for pair in crate::urlencoded::raw::raw_pairs(input, limits) {
            let pair = pair?;
            let name_len = pair.name_raw().len();
            let value_len = pair.value_raw().len();
            let (name_buf, value_buf): (&mut [u8], &mut [u8]) =
                if name_len <= STACK && value_len <= STACK {
                    (&mut name_stack, &mut value_stack)
                } else {
                    let needed = name_len.saturating_add(value_len);
                    crate::wipe::wipe(&mut heap);
                    heap.clear();
                    if needed > heap.capacity() {
                        // A new allocation; the old one is wiped, not freed
                        // with a decoded value in it.
                        let mut old =
                            core::mem::replace(&mut heap, alloc::vec::Vec::with_capacity(needed));
                        crate::wipe::wipe_vec(&mut old);
                    }
                    heap.resize(needed, 0);
                    heap.split_at_mut(name_len)
                };
            let name = pair.decode_name(name_buf, policy.percent)?;
            let value = pair.decode_value(value_buf, policy.percent)?;
            let at = crate::error::Location::at(pair.offset());
            match inspector.field(name, value.as_bytes()) {
                crate::Verdict::Refuse(refusal) => return Err(FormError::Refused { refusal, at }),
                crate::Verdict::Skip => {}
                _ => builder.text(name, value)?,
            }
        }
        Ok(())
    })();
    crate::wipe::wipe(&mut name_stack);
    crate::wipe::wipe(&mut value_stack);
    crate::wipe::wipe_vec(&mut heap);
    fed?;
    if let crate::Verdict::Refuse(refusal) = inspector.end() {
        return Err(FormError::Refused {
            refusal,
            at: crate::error::Location::at(input.len() as u64),
        });
    }
    Ok(())
}

fn feed_multipart<B: FormBuilder>(
    builder: &mut B,
    mp: &crate::Multipart<'_>,
) -> Result<(), FormError> {
    for part in mp.parts() {
        if part.is_file() {
            builder.file(part.name(), FileValue::Buffered(part))?;
        } else {
            builder.text(part.name(), part.text()?)?;
        }
    }
    Ok(())
}

/// A struct that can be read from a form. Implemented by
/// [`form!`](crate::form); every method is provided.
///
/// # Examples
///
/// ```
/// use form_net::FromForm;
///
/// form_net::form! {
///     pub struct Login {
///         pub email: String,
///         pub password: form_net::Secret,
///         pub remember: bool,
///     }
///     ignore = ["csrf_token"];
/// }
///
/// let login = Login::from_body(b"application/x-www-form-urlencoded", b"email=a%40b.example&password=x&csrf_token=t")?;
/// assert_eq!((login.email.as_str(), login.remember), ("a@b.example", false));
/// # Ok::<(), form_net::FormError>(())
/// ```
pub trait FromForm: Sized {
    /// The builder that collects the fields.
    type Builder: FormBuilder<Output = Self> + Default;

    /// From a urlencoded body, with the default limits and policy.
    ///
    /// # Errors
    ///
    /// The parser's errors; `Extract { .. }`.
    fn from_urlencoded(input: &[u8]) -> Result<Self, FormError> {
        Self::from_urlencoded_with(input, &Limits::DEFAULT, &Policy::DEFAULT)
    }

    /// From a query string; one leading `?` is stripped.
    ///
    /// # Errors
    ///
    /// As [`from_urlencoded`](Self::from_urlencoded).
    fn from_query(query: &[u8]) -> Result<Self, FormError> {
        Self::from_urlencoded(query.strip_prefix(b"?").unwrap_or(query))
    }

    /// From a buffered `multipart/form-data` body.
    ///
    /// # Errors
    ///
    /// The parser's errors; `Extract { .. }`.
    fn from_multipart(content_type: &[u8], body: &[u8]) -> Result<Self, FormError> {
        let mp = crate::parse_multipart(content_type, body)?;
        let mut b = Self::Builder::default();
        feed_multipart(&mut b, &mp)?;
        b.finish()
    }

    /// From either encoding, by the `Content-Type`.
    ///
    /// # Errors
    ///
    /// `UnsupportedMediaType`; the parsers' errors; `Extract { .. }`.
    fn from_body(content_type: &[u8], body: &[u8]) -> Result<Self, FormError> {
        Self::from_body_with(content_type, body, &Limits::DEFAULT, &Policy::DEFAULT)
    }

    /// From a urlencoded body with explicit limits and policy.
    ///
    /// # Errors
    ///
    /// As [`from_urlencoded`](Self::from_urlencoded); `InvalidLimits`.
    fn from_urlencoded_with(
        input: &[u8],
        limits: &Limits,
        policy: &Policy,
    ) -> Result<Self, FormError> {
        limits.validate()?;
        let mut b = Self::Builder::default();
        feed_urlencoded(&mut b, input, limits, policy, &mut NoInspector)?;
        b.finish()
    }

    /// From either encoding with explicit limits and policy.
    ///
    /// # Errors
    ///
    /// As [`from_body`](Self::from_body); `InvalidLimits`.
    fn from_body_with(
        content_type: &[u8],
        body: &[u8],
        limits: &Limits,
        policy: &Policy,
    ) -> Result<Self, FormError> {
        Self::from_body_inspected(content_type, body, limits, policy, &mut NoInspector)
    }

    /// From either encoding, with an inspector in the same pass: the
    /// inspector and the struct see the same bytes.
    ///
    /// # Errors
    ///
    /// As [`from_body_with`](Self::from_body_with); `Refused`.
    fn from_body_inspected<I: Inspector>(
        content_type: &[u8],
        body: &[u8],
        limits: &Limits,
        policy: &Policy,
        inspector: &mut I,
    ) -> Result<Self, FormError> {
        let result = (|| {
            let mut b = Self::Builder::default();
            match FormKind::detect(content_type)? {
                FormKind::Urlencoded => {
                    limits.validate()?;
                    feed_urlencoded(&mut b, body, limits, policy, inspector)?;
                }
                FormKind::Multipart(_) => {
                    let mp = crate::multipart::parse_multipart_inspected_raw(
                        content_type,
                        body,
                        limits,
                        policy,
                        inspector,
                    )?;
                    feed_multipart(&mut b, &mp)?;
                }
            }
            b.finish()
        })();
        if let Err(e) = &result {
            inspector.failed(e);
        }
        result
    }

    /// From a finished upload: text fields as text, files moved into the
    /// struct as `StoredFile`s.
    ///
    /// # Errors
    ///
    /// `Extract { .. }`.
    #[cfg(feature = "fs")]
    #[cfg_attr(docsrs, doc(cfg(feature = "fs")))]
    fn from_upload(upload: crate::store::Upload) -> Result<Self, FormError> {
        let (fields, files) = upload.into_parts();
        let mut b = Self::Builder::default();
        for (name, value) in fields.iter() {
            b.text(name, value)?;
        }
        for file in files {
            let name = alloc::string::String::from(file.name());
            b.file(&name, FileValue::Stored(file))?;
        }
        b.finish()
    }
}

/// Maps a field's [`ValueError`] to the error `form!` returns. Not part of
/// the API: used by the macro expansion.
#[doc(hidden)]
#[must_use]
pub fn __value_error(e: ValueError, field: &'static str) -> FormError {
    let reason = match e {
        ValueError::Missing => ExtractError::Missing,
        ValueError::Duplicate => ExtractError::Duplicate,
        ValueError::Invalid(why) => ExtractError::Invalid(why),
        _ => ExtractError::WrongKind,
    };
    FormError::Extract {
        reason,
        field: Some(field),
    }
}

/// The error for a name the struct does not declare. Not part of the API.
#[doc(hidden)]
#[must_use]
pub const fn __unknown() -> FormError {
    FormError::Extract {
        reason: ExtractError::Unknown,
        field: None,
    }
}
