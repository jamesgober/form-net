//! The disk budget shared across requests.

use core::fmt;

#[cfg(not(all(test, loom)))]
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

#[cfg(all(test, loom))]
use loom::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

/// A disk budget shared by every request that spills through a store, so
/// many uploads at once cannot fill the disk.
///
/// `reserve` is called before every disk write with the bytes about to be
/// written and must answer `false` when they do not fit (fail closed): the
/// request is refused with `Store { QuotaExceeded }` (status hint 503).
/// `release` is called exactly once for every reserved byte: when its file
/// is deleted, or when it is persisted.
///
/// [`SpillQuota`] is the in-process budget. The trait is the seam for one
/// budget over several worker processes on a host, or over a fleet (a
/// lease from a coordinator, handed out locally in large grants so
/// `reserve` stays a local operation).
///
/// # Examples
///
/// ```
/// use std::sync::atomic::{AtomicU64, Ordering};
/// use form_net::{SpillBudget, Store};
///
/// /// A budget that never refuses and counts what was reserved.
/// #[derive(Default)]
/// struct Counted(AtomicU64);
///
/// impl SpillBudget for Counted {
///     fn reserve(&self, bytes: u64) -> bool {
///         self.0.fetch_add(bytes, Ordering::Relaxed);
///         true
///     }
///     fn release(&self, bytes: u64) {
///         self.0.fetch_sub(bytes, Ordering::Relaxed);
///     }
/// }
///
/// let store = Store::temp()?.quota(Counted::default());
/// # Ok::<(), form_net::FormError>(())
/// ```
pub trait SpillBudget: Send + Sync + 'static {
    /// Takes `bytes` from the budget; `false` when they do not fit.
    fn reserve(&self, bytes: u64) -> bool;
    /// Gives back `bytes` reserved earlier.
    fn release(&self, bytes: u64);
}

struct Inner {
    limit: AtomicU64,
    in_use: AtomicU64,
}

/// The in-process [`SpillBudget`]: one atomic counter, lock-free.
///
/// Clones share the budget. One quota per tenant gives each tenant its own
/// share of the disk; one quota for several stores bounds them together.
///
/// # Examples
///
/// ```
/// use form_net::{SpillQuota, Store};
///
/// let quota = SpillQuota::new(4 << 30); // 4 GiB for all uploads on this host
/// let public = Store::temp()?.quota(quota.clone());
/// let staff = Store::temp()?.quota(quota.clone());
/// assert_eq!((quota.in_use(), quota.limit()), (0, 4 << 30));
/// # Ok::<(), form_net::FormError>(())
/// ```
#[derive(Clone)]
pub struct SpillQuota {
    inner: Arc<Inner>,
}

impl fmt::Debug for SpillQuota {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SpillQuota")
            .field("in_use", &self.in_use())
            .field("limit", &self.limit())
            .finish()
    }
}

impl SpillQuota {
    /// A budget of `max_bytes`.
    #[must_use]
    pub fn new(max_bytes: u64) -> SpillQuota {
        SpillQuota {
            inner: Arc::new(Inner {
                limit: AtomicU64::new(max_bytes),
                in_use: AtomicU64::new(0),
            }),
        }
    }

    /// Bytes reserved now.
    #[must_use]
    pub fn in_use(&self) -> u64 {
        self.inner.in_use.load(Ordering::Acquire)
    }

    /// The limit.
    #[must_use]
    pub fn limit(&self) -> u64 {
        self.inner.limit.load(Ordering::Acquire)
    }

    /// Changes the limit live: reservations already made stay, new ones see
    /// the new limit.
    pub fn set_limit(&self, max_bytes: u64) {
        self.inner.limit.store(max_bytes, Ordering::Release);
    }
}

impl SpillBudget for SpillQuota {
    fn reserve(&self, bytes: u64) -> bool {
        let limit = self.inner.limit.load(Ordering::Acquire);
        let mut current = self.inner.in_use.load(Ordering::Acquire);
        loop {
            let Some(next) = current.checked_add(bytes) else {
                return false;
            };
            if next > limit {
                return false;
            }
            match self.inner.in_use.compare_exchange_weak(
                current,
                next,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => return true,
                Err(now) => current = now,
            }
        }
    }

    fn release(&self, bytes: u64) {
        let mut current = self.inner.in_use.load(Ordering::Acquire);
        loop {
            // Never below zero, even if a caller releases too much.
            let next = current.saturating_sub(bytes);
            match self.inner.in_use.compare_exchange_weak(
                current,
                next,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => return,
                Err(now) => current = now,
            }
        }
    }
}

#[cfg(all(test, not(loom)))]
mod tests {
    use super::*;
    use alloc::vec::Vec;

    #[test]
    fn test_quota_reserve_release_and_limits() {
        let q = SpillQuota::new(10);
        assert!(q.reserve(6) && q.reserve(4));
        assert!(!q.reserve(1), "full");
        q.release(4);
        assert!(!q.reserve(5) && q.reserve(4));
        q.set_limit(5);
        assert_eq!(q.in_use(), 10, "reservations made stay");
        assert!(!q.reserve(1));
        q.release(100);
        assert_eq!(q.in_use(), 0, "never below zero");
        assert!(!q.reserve(u64::MAX) || q.limit() == u64::MAX);
    }

    #[test]
    fn test_quota_holds_under_threads() {
        let q = SpillQuota::new(1000);
        let threads: Vec<_> = (0..8)
            .map(|_| {
                let q = q.clone();
                std::thread::spawn(move || {
                    let mut got = 0u64;
                    for _ in 0..10_000 {
                        if q.reserve(7) {
                            assert!(q.in_use() <= 1000);
                            got += 7;
                            q.release(7);
                        }
                    }
                    got
                })
            })
            .collect();
        for t in threads {
            let _ = t.join().unwrap();
        }
        assert_eq!(q.in_use(), 0);
    }
}

/// The `loom` model: three threads never take the counter over the limit
/// or below zero, and no release is lost. Run with
/// `RUSTFLAGS="--cfg loom" cargo test --release --lib loom`.
#[cfg(all(test, loom))]
mod loom_model {
    use super::*;
    use alloc::vec::Vec;

    #[test]
    fn loom_quota_three_threads() {
        // A preemption bound of 3 covers every interleaving with up to three
        // forced switches, the standard bound for lock-free counters; the
        // unbounded space does not finish.
        let mut model = loom::model::Builder::new();
        model.preemption_bound = Some(3);
        model.check(|| {
            let q = SpillQuota::new(10);
            let handles: Vec<_> = [4u64, 5, 6]
                .into_iter()
                .map(|n| {
                    let q = q.clone();
                    loom::thread::spawn(move || {
                        if q.reserve(n) {
                            assert!(q.in_use() <= 10);
                            q.release(n);
                        }
                    })
                })
                .collect();
            for h in handles {
                h.join().unwrap();
            }
            assert_eq!(q.in_use(), 0, "a release was lost");
        });
    }
}
