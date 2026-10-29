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

    /// What a file under `name` is to the struct: `Some(true)` a declared
    /// field, `Some(false)` an ignored name, `None` an unknown one.
    /// `FromForm::from_upload_mut` (feature `fs`) moves only declared files out of the
    /// upload. The default says every name is declared.
    fn file_field(&self, name: &str) -> Option<bool> {
        let _ = name;
        Some(true)
    }

    /// As [`finish`](Self::finish), but records every field's error in
    /// `errors` instead of stopping at the first; `None` when any field
    /// failed. The default records the one error `finish` returns.
    fn finish_all(self, errors: &mut alloc::vec::Vec<FormError>) -> Option<Self::Output>
    where
        Self: Sized,
    {
        match self.finish() {
            Ok(v) => Some(v),
            Err(e) => {
                errors.push(e);
                None
            }
        }
    }
}

/// Every problem with one submitted form, for showing the form again with
/// a message beside each field: what
/// [`FromForm::from_body_all`] returns.
///
/// A parse error (a malformed body, a limit) stops the parse, so it is the
/// only error; extraction errors are collected for every field, at most one
/// per received value plus one per missing field. No value is ever kept.
///
/// # Examples
///
/// ```
/// use form_net::{ExtractError, FromForm};
///
/// form_net::form! {
///     #[derive(Debug)]
///     pub struct Signup { pub email: String, pub age: u8, pub tos: bool }
/// }
///
/// let errors = Signup::from_body_all(b"application/x-www-form-urlencoded", b"age=300&role=admin").unwrap_err();
/// assert_eq!(errors.len(), 3);
/// assert_eq!(errors.field("age"), Some(ExtractError::Invalid("out of range")));
/// assert_eq!(errors.field("email"), Some(ExtractError::Missing));
/// assert_eq!(errors.unknown(), 1);
/// assert_eq!(errors.status_hint(), 422);
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FormErrors(alloc::vec::Vec<FormError>);

impl FormErrors {
    /// Every error, in the order found.
    pub fn iter(&self) -> impl Iterator<Item = &FormError> {
        self.0.iter()
    }

    /// How many errors.
    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Whether there are none (never true for a returned value).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// The first error.
    #[must_use]
    pub fn first(&self) -> Option<&FormError> {
        self.0.first()
    }

    /// The first extraction error for the field with this Rust name.
    #[must_use]
    pub fn field(&self, name: &str) -> Option<ExtractError> {
        self.0.iter().find_map(|e| match e {
            FormError::Extract {
                reason,
                field: Some(f),
            } if *f == name => Some(*reason),
            _ => None,
        })
    }

    /// How many received names the struct neither declares nor ignores.
    #[must_use]
    pub fn unknown(&self) -> usize {
        self.0
            .iter()
            .filter(|e| {
                matches!(
                    e,
                    FormError::Extract {
                        reason: ExtractError::Unknown,
                        ..
                    }
                )
            })
            .count()
    }

    /// The status hint of the first error (422 for extraction errors).
    #[must_use]
    pub fn status_hint(&self) -> u16 {
        self.first().map_or(422, FormError::status_hint)
    }
}

impl From<FormError> for FormErrors {
    fn from(e: FormError) -> FormErrors {
        FormErrors(alloc::vec![e])
    }
}

impl core::fmt::Display for FormErrors {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self.first() {
            Some(e) if self.0.len() > 1 => {
                write!(f, "{e} (and {} more)", self.0.len().saturating_sub(1))
            }
            Some(e) => write!(f, "{e}"),
            None => f.write_str("no errors"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for FormErrors {}

/// Feeds a builder and keeps going after extraction errors, recording them.
struct Collect<'e, B> {
    inner: B,
    errors: &'e mut alloc::vec::Vec<FormError>,
}

impl<B: FormBuilder> FormBuilder for Collect<'_, B> {
    type Output = B::Output;

    fn text(&mut self, name: &str, value: &str) -> Result<(), FormError> {
        match self.inner.text(name, value) {
            Err(e @ FormError::Extract { .. }) => {
                self.errors.push(e);
                Ok(())
            }
            other => other,
        }
    }

    fn file(&mut self, name: &str, file: FileValue<'_>) -> Result<(), FormError> {
        match self.inner.file(name, file) {
            Err(e @ FormError::Extract { .. }) => {
                self.errors.push(e);
                Ok(())
            }
            other => other,
        }
    }

    fn finish(self) -> Result<B::Output, FormError> {
        self.inner.finish()
    }
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
    // The most of each stack buffer any pair wrote: only that is wiped.
    let (mut name_hi, mut value_hi) = (0usize, 0usize);
    let mut heap = alloc::vec::Vec::new();
    // One UTF-8 pass over the body; plain names and values are cut from it.
    let whole = core::str::from_utf8(input).ok();
    let fed = (|| {
        for pair in crate::urlencoded::raw::raw_pairs(input, limits) {
            let pair = pair?;
            // Nothing to decode (the usual case): borrow, no copy.
            if let (Some(name), Some(value)) = (pair.name_text(whole), pair.value_text(whole)) {
                let at = crate::error::Location::at(pair.offset());
                match inspector.field(name, value.as_bytes()) {
                    crate::Verdict::Refuse(refusal) => {
                        return Err(FormError::Refused { refusal, at });
                    }
                    crate::Verdict::Skip => {}
                    _ => builder.text(name, value)?,
                }
                continue;
            }
            let name_len = pair.name_raw().len();
            let value_len = pair.value_raw().len();
            let (name_buf, value_buf): (&mut [u8], &mut [u8]) =
                if name_len <= STACK && value_len <= STACK {
                    name_hi = name_hi.max(name_len);
                    value_hi = value_hi.max(value_len);
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
    crate::wipe::wipe(name_stack.get_mut(..name_hi).unwrap_or(&mut []));
    crate::wipe::wipe(value_stack.get_mut(..value_hi).unwrap_or(&mut []));
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

    /// From either encoding, collecting every field's error instead of
    /// stopping at the first (D-090), so a form can be shown again with a
    /// message beside each field.
    ///
    /// # Errors
    ///
    /// [`FormErrors`]: the parse error alone, or every extraction error.
    fn from_body_all(content_type: &[u8], body: &[u8]) -> Result<Self, FormErrors> {
        Self::from_body_all_with(content_type, body, &Limits::DEFAULT, &Policy::DEFAULT)
    }

    /// As [`from_body_all`](Self::from_body_all), with explicit limits and
    /// policy.
    ///
    /// # Errors
    ///
    /// As [`from_body_all`](Self::from_body_all); `InvalidLimits`.
    fn from_body_all_with(
        content_type: &[u8],
        body: &[u8],
        limits: &Limits,
        policy: &Policy,
    ) -> Result<Self, FormErrors> {
        let mut errors = alloc::vec::Vec::new();
        let mut b = Collect {
            inner: Self::Builder::default(),
            errors: &mut errors,
        };
        match FormKind::detect(content_type)? {
            FormKind::Urlencoded => {
                limits.validate()?;
                feed_urlencoded(&mut b, body, limits, policy, &mut NoInspector)?;
            }
            FormKind::Multipart(_) => {
                let mp = crate::parse_multipart_with(content_type, body, limits, policy)?;
                feed_multipart(&mut b, &mp)?;
            }
        }
        let Collect { inner, errors: _ } = b;
        let value = inner.finish_all(&mut errors);
        match value {
            Some(v) if errors.is_empty() => Ok(v),
            _ => Err(FormErrors(errors)),
        }
    }

    /// From an upload that stays usable (D-090): text fields are read by
    /// reference, and only the files the struct declares are moved out of
    /// it; files under `ignore = [..]` names stay in the upload for the
    /// caller. Unknown names are refused as in
    /// [`from_upload`](Self::from_upload). On error, files already moved
    /// into the builder are dropped (deleted).
    ///
    /// # Errors
    ///
    /// `Extract { .. }`.
    ///
    /// # Examples
    ///
    /// ```
    /// use form_net::FromForm;
    ///
    /// form_net::form! {
    ///     pub struct Post { pub title: String, pub cover: form_net::StoredFile }
    ///     ignore = ["attachment"];
    /// }
    ///
    /// let body = b"--X\r\nContent-Disposition: form-data; name=title\r\n\r\nHi\r\n--X\r\nContent-Disposition: form-data; name=cover; filename=c.png\r\n\r\nC\r\n--X\r\nContent-Disposition: form-data; name=attachment; filename=a.pdf\r\n\r\nA\r\n--X--";
    /// let mut upload = form_net::read_multipart(b"multipart/form-data; boundary=X", &body[..])?;
    /// let post = Post::from_upload_mut(&mut upload)?;
    /// assert_eq!((post.title.as_str(), post.cover.len()), ("Hi", 1));
    /// assert_eq!(upload.field("title"), Some("Hi"));
    /// assert!(upload.file("cover").is_none() && upload.file("attachment").is_some());
    /// # Ok::<(), form_net::FormError>(())
    /// ```
    #[cfg(feature = "fs")]
    #[cfg_attr(docsrs, doc(cfg(feature = "fs")))]
    fn from_upload_mut(upload: &mut crate::store::Upload) -> Result<Self, FormError> {
        let mut b = Self::Builder::default();
        for (name, value) in upload.iter() {
            b.text(name, value)?;
        }
        let files = upload.files_mut();
        let mut i = 0usize;
        while let Some(file) = files.get(i) {
            match b.file_field(file.name()) {
                Some(true) => {
                    let file = files.remove(i);
                    let name = alloc::string::String::from(file.name());
                    b.file(&name, FileValue::Stored(file))?;
                }
                Some(false) => i = i.wrapping_add(1),
                None => return Err(__unknown()),
            }
        }
        b.finish()
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

/// Records a field's error at `finish_all`, leaving out a `Missing` for a
/// field that already failed when its value arrived. Not part of the API.
#[doc(hidden)]
pub fn __push_field_error(
    errors: &mut alloc::vec::Vec<FormError>,
    e: ValueError,
    field: &'static str,
) {
    let seen = errors
        .iter()
        .any(|x| matches!(x, FormError::Extract { field: Some(f), .. } if *f == field));
    if !(seen && e == ValueError::Missing) {
        errors.push(__value_error(e, field));
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
