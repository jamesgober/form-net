//! T-33: 1,000 uploads in flight at once hold at most 1,000 times
//! `memory_bound`. Alone in its binary: the counters are process-wide.

#![cfg(feature = "fs")]

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, Barrier};

use form_net::{Limits, NoInspector, Policy, Store, UploadSession};

struct Peak;

static LIVE: AtomicI64 = AtomicI64::new(0);
static PEAK: AtomicI64 = AtomicI64::new(0);

fn grow(n: usize) {
    let v = LIVE.fetch_add(n as i64, Ordering::Relaxed) + n as i64;
    PEAK.fetch_max(v, Ordering::Relaxed);
}

// SAFETY: forwards to the system allocator; only counts.
unsafe impl GlobalAlloc for Peak {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        grow(layout.size());
        // SAFETY: the caller upholds `GlobalAlloc::alloc`'s contract.
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        LIVE.fetch_sub(layout.size() as i64, Ordering::Relaxed);
        // SAFETY: the caller upholds `GlobalAlloc::dealloc`'s contract.
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static GLOBAL: Peak = Peak;

const ROUTE: Limits = Limits::DEFAULT
    .with_max_body_bytes(16 * 1024)
    .with_max_fields(8)
    .with_max_parts(10)
    .with_max_files(2)
    .with_max_file_bytes(4096)
    .with_max_part_header_bytes(512)
    .with_max_text_bytes(2048);

const UPLOADS: usize = 1000;
/// Heap a spawned thread itself needs (its handle, name, result slot).
const PER_THREAD: i64 = 2048;

#[test]
fn test_attack_t33_concurrent_uploads_stay_within_the_bound() {
    let mut body = Vec::new();
    for i in 0..8 {
        body.extend_from_slice(
            format!("--X\r\nContent-Disposition: form-data; name=t{i}\r\n\r\n").as_bytes(),
        );
        body.extend(std::iter::repeat_n(b'v', 200));
        body.extend_from_slice(b"\r\n");
    }
    for i in 0..2 {
        body.extend_from_slice(
            format!("--X\r\nContent-Disposition: form-data; name=f{i}; filename=a\r\n\r\n")
                .as_bytes(),
        );
        body.extend(std::iter::repeat_n(b'd', 4096));
        body.extend_from_slice(b"\r\n");
    }
    body.extend_from_slice(b"--X--");
    let body: Arc<[u8]> = body.into();
    let store = Arc::new(Store::memory_only());
    let bound = store.memory_bound(&ROUTE) as i64;
    let barrier = Arc::new(Barrier::new(UPLOADS + 1));

    let base = LIVE.load(Ordering::Relaxed);
    PEAK.store(base, Ordering::Relaxed);
    let threads: Vec<_> = (0..UPLOADS)
        .map(|_| {
            let (body, store, barrier) =
                (Arc::clone(&body), Arc::clone(&store), Arc::clone(&barrier));
            std::thread::Builder::new()
                .stack_size(64 * 1024)
                .spawn(move || {
                    let mut s = UploadSession::new(
                        b"multipart/form-data; boundary=X",
                        &ROUTE,
                        &Policy::DEFAULT,
                        &store,
                    )
                    .unwrap();
                    // Everything but the close delimiter: the upload holds all it will hold.
                    s.push(&body[..body.len() - 5], &mut NoInspector).unwrap();
                    barrier.wait();
                    s.push(&body[body.len() - 5..], &mut NoInspector).unwrap();
                    s.finish(&mut NoInspector).unwrap().file_count()
                })
                .unwrap()
        })
        .collect();
    barrier.wait();
    for t in threads {
        assert_eq!(t.join().unwrap(), 2);
    }
    let peak = PEAK.load(Ordering::Relaxed) - base;
    let allowed = UPLOADS as i64 * (bound + PER_THREAD);
    assert!(
        peak <= allowed,
        "peak {peak} > {allowed} ({UPLOADS} × ({bound} + {PER_THREAD}))"
    );
}
