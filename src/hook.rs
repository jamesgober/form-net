//! Hooks for other crates to take part in a parse. In 0.3 this holds
//! [`Refusal`], which [`FormError::Refused`](crate::FormError::Refused)
//! carries; the inspector and sink traits arrive in 0.4.

/// Why an inspector refused the input: the HTTP status it wants and a
/// stable, machine-readable code for the reason.
///
/// [`FormError::status_hint`](crate::FormError::status_hint) returns the
/// status (400 when it is outside 400 to 599: a refusal is never a success);
/// [`FormError::code`](crate::FormError::code) returns the code. The code
/// must never contain input.
///
/// # Examples
///
/// ```
/// use form_net::Refusal;
///
/// let r = Refusal::new(415, "customs.avatar_not_image");
/// assert_eq!((r.status, r.code), (415, "customs.avatar_not_image"));
/// ```
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Refusal {
    /// The HTTP status the inspector wants.
    pub status: u16,
    /// The inspector's stable code for the reason.
    pub code: &'static str,
}

impl Refusal {
    /// A refusal with this status and code.
    #[must_use]
    pub const fn new(status: u16, code: &'static str) -> Refusal {
        Refusal { status, code }
    }
}
