//! The best-effort wipe for owned values that may hold secrets.
//!
//! A volatile write needs `unsafe`, which this crate forbids, so the bytes
//! are overwritten with `fill(0)` and then passed through
//! `core::hint::black_box`, so the compiler cannot prove the store dead and
//! remove it. It clears the bytes form-net owns, not copies the allocator or
//! the caller made (`dev/PLAN.md` D-028).

/// Overwrites `bytes` with zeros.
#[inline]
pub(crate) fn wipe(bytes: &mut [u8]) {
    bytes.fill(0);
    core::hint::black_box(&*bytes);
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_wipe_zeroes_every_byte() {
        let mut b = [0xAAu8; 37];
        super::wipe(&mut b);
        assert!(b.iter().all(|&x| x == 0));
    }
}
