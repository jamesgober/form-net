//! Wiping owned values that may hold secrets.
//!
//! With `alloc` (and so in every build that has owned values) the bytes go
//! through crypt-io's `wipe`, whose volatile writes the optimiser cannot
//! remove (D-083). The allocation-free build, which owns only the parser
//! buffer, overwrites with `fill(0)` and passes the slice through
//! `core::hint::black_box`, which is best effort.
//!
//! Either way only the bytes form-net owns are cleared, not copies the
//! allocator or the caller made.

/// Overwrites `bytes` with zeros.
#[inline]
pub(crate) fn wipe(bytes: &mut [u8]) {
    #[cfg(test)]
    tests::count(bytes.len());
    #[cfg(feature = "alloc")]
    {
        crypt_io::wipe(bytes);
    }
    #[cfg(not(feature = "alloc"))]
    {
        bytes.fill(0);
        core::hint::black_box(&*bytes);
    }
}

/// Overwrites a vector's whole allocation (length and spare capacity) and
/// empties it, keeping the allocation.
#[cfg(feature = "alloc")]
#[inline]
pub(crate) fn wipe_vec(v: &mut alloc::vec::Vec<u8>) {
    #[cfg(test)]
    tests::count(v.capacity());
    crypt_io::wipe_vec(v);
}

/// Wipes and drops a string.
#[cfg(feature = "alloc")]
pub(crate) fn wipe_string(s: &mut alloc::string::String) {
    let mut bytes = core::mem::take(s).into_bytes();
    wipe_vec(&mut bytes);
}

/// A byte buffer that wipes its whole allocation when dropped.
#[cfg(feature = "alloc")]
#[derive(Default)]
pub(crate) struct WipeVec(pub(crate) alloc::vec::Vec<u8>);

#[cfg(feature = "alloc")]
impl core::ops::Deref for WipeVec {
    type Target = alloc::vec::Vec<u8>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[cfg(feature = "alloc")]
impl core::ops::DerefMut for WipeVec {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

#[cfg(feature = "alloc")]
impl Drop for WipeVec {
    fn drop(&mut self) {
        wipe_vec(&mut self.0);
    }
}

/// A read buffer that wipes, on drop, only the bytes reads ever filled:
/// the front `used` bytes. A 64 KiB buffer that carried a 2 KiB form costs
/// a 2 KiB wipe (D-103).
#[cfg(feature = "fs")]
pub(crate) struct ReadBuf {
    pub(crate) buf: alloc::vec::Vec<u8>,
    used: usize,
}

#[cfg(feature = "fs")]
impl ReadBuf {
    pub(crate) fn new(len: usize) -> ReadBuf {
        ReadBuf {
            buf: alloc::vec![0u8; len],
            used: 0,
        }
    }

    /// Records that a read filled the first `n` bytes.
    pub(crate) fn filled(&mut self, n: usize) {
        self.used = self.used.max(n.min(self.buf.len()));
    }

    /// Records a failed read: it may have written anywhere in the buffer
    /// before failing, so all of it is wiped.
    pub(crate) fn failed(&mut self) {
        self.used = self.buf.len();
    }
}

#[cfg(feature = "fs")]
impl Drop for ReadBuf {
    fn drop(&mut self) {
        if let Some(used) = self.buf.get_mut(..self.used) {
            wipe(used);
        }
    }
}

/// Wipes the bytes of an optional string and leaves `None`.
#[cfg(feature = "alloc")]
pub(crate) fn wipe_opt_string(s: &mut Option<alloc::string::String>) {
    if let Some(mut s) = s.take() {
        wipe_string(&mut s);
    }
}

/// Appends `bytes` to `v`, growing it without leaving a copy behind: when
/// the capacity is too small, the old allocation is wiped after the move
/// (a plain `Vec` reallocation would free it with its contents intact).
#[cfg(feature = "alloc")]
pub(crate) fn append_wiping(v: &mut alloc::vec::Vec<u8>, bytes: &[u8], step: usize, cap: usize) {
    let needed = v.len().saturating_add(bytes.len());
    if needed > v.capacity() {
        let mut target = v.capacity().max(step);
        while target < needed {
            target = target.saturating_mul(2);
        }
        // Near the cap, go straight to it: a last doubling clamped to the
        // cap could leave an old buffer of almost the cap alive beside the
        // new one during the move, nearly twice what `memory_bound` allows
        // for growth (found by review).
        let ceiling = cap.max(needed);
        let target = if target > ceiling / 2 {
            ceiling
        } else {
            target
        };
        let mut grown = alloc::vec::Vec::with_capacity(target);
        grown.extend_from_slice(v);
        wipe_vec(v);
        *v = grown;
    }
    v.extend_from_slice(bytes);
}

#[cfg(test)]
pub(crate) mod tests {
    use core::cell::Cell;

    std::thread_local! {
        static WIPED: Cell<usize> = const { Cell::new(0) };
    }

    /// Test-only: records how many bytes went through a wipe on this thread.
    pub(crate) fn count(n: usize) {
        WIPED.with(|w| w.set(w.get() + n));
    }

    /// Bytes wiped on this thread while `f` ran.
    #[cfg(feature = "alloc")]
    pub(crate) fn wiped(f: impl FnOnce()) -> usize {
        let before = WIPED.with(Cell::get);
        f();
        WIPED.with(Cell::get) - before
    }

    #[test]
    fn test_wipe_zeroes_every_byte() {
        let mut b = [0xAAu8; 37];
        super::wipe(&mut b);
        assert!(b.iter().all(|&x| x == 0));
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn test_append_wiping_grows_and_keeps_contents() {
        let mut v = alloc::vec::Vec::new();
        for i in 0..100u8 {
            super::append_wiping(&mut v, &[i; 10], 64, 4096);
        }
        assert_eq!(v.len(), 1000);
        assert!(
            v.chunks(10)
                .enumerate()
                .all(|(i, c)| c.iter().all(|&b| usize::from(b) == i))
        );
        let mut s = alloc::string::String::from("secret");
        super::wipe_string(&mut s);
        assert!(s.is_empty());
    }

    /// T-31: every owned value that can hold form bytes passes all of them
    /// through the wipe when it is dropped. (Reading the freed memory itself
    /// would need `unsafe`, which the crate forbids; the wipe is crypt-io's
    /// volatile write, D-083.)
    #[cfg(feature = "alloc")]
    #[test]
    fn test_t31_owned_values_are_wiped_on_drop() {
        use alloc::string::String;
        let s = crate::Secret::from(String::from("hunter2"));
        assert!(wiped(|| drop(s)) >= 7);
        let f = crate::FileData::new(
            alloc::vec![1u8; 100],
            Some(String::from("a.png")),
            Some(String::from("image/png")),
        );
        assert!(wiped(|| drop(f)) >= 100 + 5 + 9);
        let form = crate::parse_urlencoded(b"password=hunter2").unwrap();
        assert!(wiped(|| drop(form)) >= 15);
        let mp = crate::parse_multipart(
            b"multipart/form-data; boundary=X",
            b"--X\r\nContent-Disposition: form-data; name=f; filename=a.png\r\n\r\nsecret\r\n--X--",
        )
        .unwrap();
        let head = mp.file("f").unwrap().head();
        let mut owned = crate::OwnedPartHead::new();
        owned.copy_from(&head);
        assert!(wiped(|| drop(owned)) >= 6);
    }

    #[cfg(feature = "fs")]
    #[test]
    fn test_t31_stored_files_and_uploads_are_wiped_on_drop() {
        let ct = b"multipart/form-data; boundary=X";
        let body = b"--X\r\nContent-Disposition: form-data; name=csrf\r\n\r\ntok\r\n--X\r\nContent-Disposition: form-data; name=pin\r\n\r\n1234\r\n--X\r\nContent-Disposition: form-data; name=f; filename=a.png\r\n\r\nsecret\r\n--X--";
        let policy = crate::Policy::DEFAULT.with_token_field(crate::TokenField::new("csrf"));
        let store = crate::Store::memory_only();
        let mut up = crate::Upload::parse_with(
            ct,
            &body[..],
            &crate::Limits::DEFAULT,
            &policy,
            &store,
            &mut crate::NoInspector,
        )
        .unwrap();
        let file = up.take_file("f").unwrap();
        // The bytes, the field name, the file name.
        assert!(wiped(|| drop(file)) >= 6 + 1 + 5);
        // The text arena ("pin" "1234" and the token field) and the token copy.
        assert!(wiped(|| drop(up)) >= 3 + 4 + 3);
    }
}
