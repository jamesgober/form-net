//! `Secret`: a text field that is wiped on drop and never printed.

use alloc::string::String;
use core::fmt;

use super::{FromField, ValueError};

/// A text field that holds a secret (a password, a token).
///
/// No `Display`, no `Clone` (a clone is a second copy to wipe), no way to
/// take the `String` out: [`expose`](Self::expose) is the only way to read
/// it, which makes every read easy to find in review. The bytes are wiped
/// on drop with crypt-io's volatile wipe; `Debug` prints `Secret(..)`.
///
/// # Examples
///
/// ```
/// use form_net::Secret;
///
/// let s = Secret::from(String::from("hunter2"));
/// assert_eq!(s.expose(), "hunter2");
/// assert_eq!(format!("{s:?}"), "Secret(..)");
/// ```
pub struct Secret(String);

impl Secret {
    /// The secret text.
    #[must_use]
    pub fn expose(&self) -> &str {
        &self.0
    }

    /// Its length in bytes.
    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Whether it is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl From<String> for Secret {
    fn from(s: String) -> Secret {
        Secret(s)
    }
}

impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Secret(..)")
    }
}

impl Drop for Secret {
    fn drop(&mut self) {
        crate::wipe::wipe_string(&mut self.0);
    }
}

impl FromField for Secret {
    type Slot = Option<Secret>;

    fn accept_text(slot: &mut Self::Slot, value: &str) -> Result<(), ValueError> {
        if slot.is_some() {
            return Err(ValueError::Duplicate);
        }
        *slot = Some(Secret(String::from(value)));
        Ok(())
    }

    fn finish(slot: Self::Slot) -> Result<Self, ValueError> {
        slot.ok_or(ValueError::Missing)
    }
}
