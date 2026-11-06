//! A count of refusals by code, for the detection layer: a route whose
//! `syntax.*` or `policy.*` counts climb is being probed. Built on targets
//! with atomic adds (every target but the smallest microcontrollers).

use core::sync::atomic::Ordering;

#[cfg(not(target_has_atomic = "64"))]
use core::sync::atomic::AtomicU32 as Counter;
#[cfg(target_has_atomic = "64")]
use core::sync::atomic::AtomicU64 as Counter;

use crate::error::{FormError, SLOT_CODES, SLOTS};

/// How many times each failure code was returned, for one route.
///
/// One counter per code form-net returns, one per inspector code named in
/// [`with_codes`](Self::with_codes), and one for every other inspector
/// refusal. [`record`](Self::record) is a match and one relaxed atomic add:
/// no allocation, no lock, safe to call from every thread at once. Keep one
/// digest per route (a `static` works) and record every error the route
/// returns, whichever call returned it; a detection layer reads the counts
/// and compares them over time.
///
/// Counters are 64 bits where the target has 64-bit atomics and 32 bits
/// elsewhere; they wrap, so compare readings with `wrapping_sub`.
///
/// # Examples
///
/// ```
/// use form_net::RefusalDigest;
///
/// static LOGIN: RefusalDigest<1> = RefusalDigest::with_codes(["customs.unexpected_field"]);
///
/// for body in [&b"email=a%zz"[..], b"a=1", b"b=%"] {
///     if let Err(e) = form_net::parse_urlencoded(body) {
///         LOGIN.record(&e);
///     }
/// }
/// assert_eq!(LOGIN.count("encoding.invalid_percent"), 2);
/// assert_eq!(LOGIN.total(), 2);
/// ```
#[derive(Debug)]
pub struct RefusalDigest<const N: usize = 0> {
    form: [Counter; SLOTS],
    codes: [&'static str; N],
    named: [Counter; N],
    unnamed: Counter,
}

impl RefusalDigest<0> {
    /// A digest of form-net's own codes; every inspector refusal counts as
    /// [`unnamed`](Self::unnamed).
    #[must_use]
    pub const fn new() -> RefusalDigest<0> {
        RefusalDigest::with_codes([])
    }
}

impl Default for RefusalDigest<0> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const N: usize> RefusalDigest<N> {
    /// A digest that also counts these inspector codes one by one (the codes
    /// the route's inspectors can return, such as customs-sec's).
    #[must_use]
    pub const fn with_codes(codes: [&'static str; N]) -> RefusalDigest<N> {
        RefusalDigest {
            form: [const { Counter::new(0) }; SLOTS],
            codes,
            named: [const { Counter::new(0) }; N],
            unnamed: Counter::new(0),
        }
    }

    /// Counts `error` under its code.
    ///
    /// Cost: one match and one relaxed atomic add for form-net's codes; an
    /// inspector refusal compares its code with the named codes first.
    #[inline]
    pub fn record(&self, error: &FormError) {
        let counter = match error.slot() {
            Some(slot) => self.form.get(slot),
            None => {
                let code = error.code();
                self.codes
                    .iter()
                    .position(|c| *c == code)
                    .and_then(|i| self.named.get(i))
            }
        };
        counter
            .unwrap_or(&self.unnamed)
            .fetch_add(1, Ordering::Relaxed);
    }

    /// How many times `code` was recorded (form-net's codes and the named
    /// inspector codes; a code named twice, or named and also form-net's,
    /// sums its counters).
    #[must_use]
    pub fn count(&self, code: &str) -> u64 {
        self.iter_all()
            .filter(|(c, _)| *c == code)
            .fold(0u64, |sum, (_, n)| sum.wrapping_add(n))
    }

    /// How many inspector refusals had a code that was not named.
    #[must_use]
    pub fn unnamed(&self) -> u64 {
        load(&self.unnamed)
    }

    /// Everything recorded.
    #[must_use]
    pub fn total(&self) -> u64 {
        self.iter_all()
            .fold(self.unnamed(), |sum, (_, n)| sum.wrapping_add(n))
    }

    /// Every code with a count above zero, form-net's first in a fixed
    /// order, then the named codes; [`unnamed`](Self::unnamed) is apart.
    pub fn iter(&self) -> impl Iterator<Item = (&'static str, u64)> + '_ {
        self.iter_all().filter(|(_, n)| *n > 0)
    }

    fn iter_all(&self) -> impl Iterator<Item = (&'static str, u64)> + '_ {
        let form = SLOT_CODES.iter().copied().zip(self.form.iter().map(load));
        let named = self.codes.iter().copied().zip(self.named.iter().map(load));
        form.chain(named)
    }
}

/// A counter's value, widened to 64 bits.
#[inline]
fn load(c: &Counter) -> u64 {
    #[cfg(target_has_atomic = "64")]
    let n = c.load(Ordering::Relaxed);
    #[cfg(not(target_has_atomic = "64"))]
    let n = u64::from(c.load(Ordering::Relaxed));
    n
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::Location;
    use crate::hook::Refusal;

    fn refused(code: &'static str) -> FormError {
        FormError::Refused {
            refusal: Refusal::new(422, code),
            at: Location::NONE,
        }
    }

    #[test]
    fn test_every_form_net_code_counts_in_its_own_counter() {
        let digest = RefusalDigest::new();
        let all = crate::error::tests::every_error();
        for (i, e) in all.iter().enumerate() {
            for _ in 0..=i {
                digest.record(e);
            }
        }
        for (i, e) in all.iter().enumerate() {
            assert_eq!(digest.count(e.code()), i as u64 + 1, "{e:?}");
        }
        assert_eq!(digest.iter().count(), all.len());
        assert_eq!(digest.unnamed(), 0);
        let n = all.len() as u64;
        assert_eq!(digest.total(), n * (n + 1) / 2);
    }

    #[test]
    fn test_inspector_codes_count_by_name_and_the_rest_together() {
        let digest = RefusalDigest::with_codes(["customs.a", "customs.b"]);
        digest.record(&refused("customs.b"));
        digest.record(&refused("customs.b"));
        digest.record(&refused("customs.a"));
        digest.record(&refused("customs.other"));
        digest.record(&refused("csrf.rejected"));
        assert_eq!(digest.count("customs.a"), 1);
        assert_eq!(digest.count("customs.b"), 2);
        assert_eq!(digest.count("customs.other"), 0);
        assert_eq!(digest.unnamed(), 2);
        assert_eq!(digest.total(), 5);
        let seen: alloc::vec::Vec<_> = digest.iter().collect();
        assert_eq!(seen, [("customs.a", 1), ("customs.b", 2)]);
    }

    #[test]
    fn test_a_code_named_twice_or_shadowing_form_net_sums() {
        let digest = RefusalDigest::with_codes(["x", "x", "limit.parts"]);
        digest.record(&refused("x"));
        digest.record(&refused("limit.parts"));
        digest.record(&FormError::limit(crate::Limit::Parts, Location::NONE));
        assert_eq!(digest.count("x"), 1);
        assert_eq!(digest.count("limit.parts"), 2);
        assert_eq!(digest.total(), 3);
    }

    #[test]
    fn test_counts_from_many_threads_are_all_kept() {
        static DIGEST: RefusalDigest = RefusalDigest::new();
        let e = FormError::UnsupportedMediaType;
        std::thread::scope(|s| {
            for _ in 0..8 {
                s.spawn(|| {
                    for _ in 0..10_000 {
                        DIGEST.record(&e);
                    }
                });
            }
        });
        assert_eq!(DIGEST.count("unsupported_media_type"), 80_000);
    }
}
