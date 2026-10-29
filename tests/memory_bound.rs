//! `memory_bound` holds (FR-40, NFR-8): a peak-tracking allocator measures
//! what one parse holds at its worst, for hostile bodies shaped to fill
//! every budget, and the measurement stays under the bound.

#![cfg(feature = "fs")]

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

use form_net::{FromForm, Limits, NoInspector, Policy, Store, UploadSession};

struct Peak;

thread_local! {
    static LIVE: Cell<i64> = const { Cell::new(0) };
    static PEAK: Cell<i64> = const { Cell::new(0) };
}

fn grow(n: usize) {
    let _ = LIVE.try_with(|l| {
        let v = l.get() + n as i64;
        l.set(v);
        let _ = PEAK.try_with(|p| p.set(p.get().max(v)));
    });
}

fn shrink(n: usize) {
    let _ = LIVE.try_with(|l| l.set(l.get() - n as i64));
}

// SAFETY: forwards to the system allocator; only counts.
unsafe impl GlobalAlloc for Peak {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        grow(layout.size());
        // SAFETY: the caller upholds `GlobalAlloc::alloc`'s contract.
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        shrink(layout.size());
        // SAFETY: the caller upholds `GlobalAlloc::dealloc`'s contract.
        unsafe { System.dealloc(ptr, layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        grow(new_size);
        // SAFETY: the caller upholds `GlobalAlloc::realloc`'s contract.
        let p = unsafe { System.realloc(ptr, layout, new_size) };
        shrink(layout.size());
        p
    }
}

#[global_allocator]
static GLOBAL: Peak = Peak;

/// The peak bytes allocated on this thread above the level before `f`.
fn peak<R>(f: impl FnOnce() -> R) -> (u64, R) {
    let base = LIVE.with(Cell::get);
    PEAK.with(|p| p.set(base));
    let r = f();
    let top = PEAK.with(Cell::get);
    ((top - base).max(0) as u64, r)
}

const CT: &[u8] = b"multipart/form-data; boundary=X";

/// A route whose budgets are small enough to fill completely.
const ROUTE: Limits = Limits::DEFAULT
    .with_max_body_bytes(256 * 1024)
    .with_max_fields(40)
    .with_max_name_bytes(64)
    .with_max_value_bytes(2048)
    .with_max_text_bytes(16 * 1024)
    .with_max_parts(48)
    .with_max_files(8)
    .with_max_file_bytes(16 * 1024)
    .with_max_part_header_bytes(1024)
    .with_max_filename_bytes(128)
    .with_sniff_bytes(64);

fn hostile_multipart() -> Vec<u8> {
    let mut b = Vec::new();
    for i in 0..40 {
        let name = format!("{i:0>64}");
        b.extend_from_slice(
            format!("--X\r\nContent-Disposition: form-data; name=\"{name}\"\r\n\r\n").as_bytes(),
        );
        b.extend(std::iter::repeat_n(b'v', 400));
        b.extend_from_slice(b"\r\n");
    }
    for i in 0..8 {
        let filename = format!("{i:%>120}");
        b.extend_from_slice(
            format!("--X\r\nContent-Disposition: form-data; name=\"f{i}\"; filename=\"{filename}\"\r\n\r\n").as_bytes(),
        );
        b.extend(std::iter::repeat_n(b'd', 16 * 1024));
        b.extend_from_slice(b"\r\n");
    }
    b.extend_from_slice(b"--X--");
    b
}

fn session_peak(store: &Store, body: &[u8], chunk: usize) -> u64 {
    let (p, r) = peak(|| {
        let mut s = UploadSession::new(CT, &ROUTE, &Policy::DEFAULT, store).unwrap();
        for c in body.chunks(chunk) {
            s.push(c, &mut NoInspector).unwrap();
        }
        let upload = s.finish(&mut NoInspector).unwrap();
        upload.file_count()
    });
    assert_eq!(r, 8);
    p
}

#[test]
fn test_upload_session_stays_under_the_bound() {
    let body = hostile_multipart();
    assert!(body.len() as u64 <= ROUTE.max_body_bytes);
    for store in [
        Store::memory_only(),
        Store::temp().unwrap().memory_threshold(1024),
    ] {
        let bound = store.memory_bound(&ROUTE);
        for chunk in [1usize, 7, 4096, body.len()] {
            let p = session_peak(&store, &body, chunk);
            assert!(
                p <= bound,
                "peak {p} > bound {bound} (chunk {chunk}, {store:?})"
            );
        }
    }
}

#[test]
fn test_buffered_parse_and_extraction_stay_under_the_bound() {
    let body = hostile_multipart();
    let bound = ROUTE.memory_bound(u64::MAX);
    let (p, mp) = peak(|| {
        form_net::parse_multipart_with(CT, &body, &ROUTE, &Policy::DEFAULT).map(|m| m.len())
    });
    assert_eq!(mp.unwrap(), 48);
    assert!(p <= bound, "buffered peak {p} > {bound}");

    // urlencoded: every field at its budgets.
    let mut u = Vec::new();
    for i in 0..40 {
        u.extend_from_slice(format!("{i:0>64}=").as_bytes());
        u.extend(std::iter::repeat_n(b'%', 1).chain(*b"41").cycle().take(399));
        u.push(b'&');
    }
    let (p, form) =
        peak(|| form_net::parse_urlencoded_with(&u, &ROUTE, &Policy::DEFAULT).map(|f| f.len()));
    assert!(form.is_ok(), "{form:?}");
    assert!(p <= bound, "urlencoded peak {p} > {bound}");

    struct Any;
    #[derive(Default)]
    struct AnyBuilder;
    impl form_net::FormBuilder for AnyBuilder {
        type Output = Any;
        fn text(&mut self, _: &str, _: &str) -> Result<(), form_net::FormError> {
            Ok(())
        }
        fn file(&mut self, _: &str, _: form_net::FileValue<'_>) -> Result<(), form_net::FormError> {
            Ok(())
        }
        fn finish(self) -> Result<Any, form_net::FormError> {
            Ok(Any)
        }
    }
    impl FromForm for Any {
        type Builder = AnyBuilder;
    }
    let (p, r) = peak(|| Any::from_body_with(CT, &body, &ROUTE, &Policy::DEFAULT).map(|_| ()));
    assert!(r.is_ok());
    assert!(p <= bound, "extraction peak {p} > {bound}");
}

#[test]
fn test_files_just_past_a_power_of_two_stay_under_the_bound() {
    // Found by review: a buffer that grew by doubling holds nearly twice
    // its bytes, so many files of 64 KiB + 1 hold about twice the body.
    const WIDE: Limits = Limits::DEFAULT
        .with_max_body_bytes(1 << 20)
        .with_max_files(16)
        .with_max_file_bytes(1 << 20)
        .with_max_text_bytes(4096)
        .with_max_value_bytes(1024)
        .with_max_fields(4)
        .with_max_parts(20)
        .with_max_part_header_bytes(1024);
    let mut body = Vec::new();
    for i in 0..15 {
        body.extend_from_slice(
            format!("--X\r\nContent-Disposition: form-data; name=f{i}; filename=a\r\n\r\n")
                .as_bytes(),
        );
        body.extend(std::iter::repeat_n(b'd', 65_537));
        body.extend_from_slice(b"\r\n");
    }
    body.extend_from_slice(b"--X--");
    let store = Store::memory_only();
    let bound = store.memory_bound(&WIDE);
    let (p, n) = peak(|| {
        let mut s = UploadSession::new(CT, &WIDE, &Policy::DEFAULT, &store).unwrap();
        for c in body.chunks(4096) {
            s.push(c, &mut NoInspector).unwrap();
        }
        s.finish(&mut NoInspector).unwrap().file_count()
    });
    assert_eq!(n, 15);
    assert!(p <= bound, "peak {p} > bound {bound}");
}

#[test]
fn test_the_bound_is_not_loose_by_orders_of_magnitude() {
    // The bound is for admission control: it must be near what a hostile
    // body really costs, or servers admit too few requests.
    let body = hostile_multipart();
    let store = Store::memory_only();
    let p = session_peak(&store, &body, 4096);
    let bound = store.memory_bound(&ROUTE);
    assert!(bound < p * 4, "bound {bound} vs peak {p}");
}
