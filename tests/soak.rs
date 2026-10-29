//! NFR-15: the upload soak. 64 sessions upload in a loop for
//! `FORM_NET_SOAK_SECS` seconds (default 60; the release check runs 14,400),
//! with mixed sizes and 20% of uploads aborted part way. Every sample (every
//! minute, or a tenth of a short run) records heap bytes, files in the
//! store's directory, and open handles (Linux). The soak passes when the
//! second half never holds more than the first, and the end returns to the
//! start: no heap, no files, no handles left.
//!
//! `cargo test --release --all-features --test soak -- --ignored --nocapture`
//!
//! Alone in its binary: the counters are process-wide.

#![cfg(feature = "fs")]

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU64, Ordering};
use std::time::{Duration, Instant};

use form_net::{Limits, NoInspector, Policy, Store, UploadSession};

struct Counting;

static LIVE: AtomicI64 = AtomicI64::new(0);
static PEAK: AtomicI64 = AtomicI64::new(0);

// SAFETY: forwards to the system allocator; only counts.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let v = LIVE.fetch_add(layout.size() as i64, Ordering::Relaxed) + layout.size() as i64;
        PEAK.fetch_max(v, Ordering::Relaxed);
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
static GLOBAL: Counting = Counting;

const SESSIONS: usize = 64;
const CT: &[u8] = b"multipart/form-data; boundary=SoakBoundary";
/// File sizes, from an empty file input to a few MiB.
const SIZES: [usize; 6] = [0, 100, 8 * 1024, 64 * 1024, 300 * 1024, 3 * 1024 * 1024];

/// A small deterministic generator, one per session.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

fn body(rng: &mut Rng) -> Vec<u8> {
    let mut out = Vec::new();
    for i in 0..1 + rng.below(4) {
        out.extend_from_slice(
            format!("--SoakBoundary\r\nContent-Disposition: form-data; name=t{i}\r\n\r\n")
                .as_bytes(),
        );
        out.extend(std::iter::repeat_n(b'v', rng.below(2000)));
        out.extend_from_slice(b"\r\n");
    }
    for i in 0..rng.below(4) {
        let size = SIZES[rng.below(SIZES.len())];
        out.extend_from_slice(
            format!("--SoakBoundary\r\nContent-Disposition: form-data; name=f{i}; filename=f{i}.bin\r\nContent-Type: application/octet-stream\r\n\r\n")
                .as_bytes(),
        );
        let fill = rng.next() as u8;
        out.extend(std::iter::repeat_n(fill, size));
        out.extend_from_slice(b"\r\n");
    }
    out.extend_from_slice(b"--SoakBoundary--\r\n");
    out
}

fn files_in(store: &Store) -> usize {
    std::fs::read_dir(store.dir().unwrap()).unwrap().count()
}

#[cfg(target_os = "linux")]
fn handles() -> usize {
    std::fs::read_dir("/proc/self/fd").unwrap().count()
}

#[cfg(not(target_os = "linux"))]
fn handles() -> usize {
    0
}

struct Sample {
    at: Duration,
    peak: i64,
    files: usize,
    handles: usize,
}

#[test]
#[ignore = "long: run with --ignored (NFR-15)"]
fn test_soak_shows_no_growth() {
    let secs: u64 = std::env::var("FORM_NET_SOAK_SECS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(60);
    let run = Duration::from_secs(secs);
    let every = if secs >= 600 {
        Duration::from_secs(60)
    } else {
        run / 10
    };
    let store = Arc::new(Store::temp().unwrap().memory_threshold(16 * 1024));
    let stop = Arc::new(AtomicBool::new(false));
    let done = Arc::new(AtomicU64::new(0));
    let aborted = Arc::new(AtomicU64::new(0));
    let base_heap = LIVE.load(Ordering::Relaxed);
    let base_handles = handles();

    let workers: Vec<_> = (0..SESSIONS)
        .map(|n| {
            let (store, stop, done, aborted) = (
                Arc::clone(&store),
                Arc::clone(&stop),
                Arc::clone(&done),
                Arc::clone(&aborted),
            );
            std::thread::spawn(move || {
                let mut rng = Rng(0x9E37_79B9_7F4A_7C15 ^ (n as u64 + 1).wrapping_mul(0x2545_F491));
                while !stop.load(Ordering::Relaxed) {
                    let b = body(&mut rng);
                    let abort_at = (rng.below(5) == 0).then(|| rng.below(b.len()));
                    let mut s =
                        UploadSession::new(CT, &Limits::DEFAULT, &Policy::DEFAULT, &store).unwrap();
                    let end = abort_at.unwrap_or(b.len());
                    for chunk in b[..end].chunks(16 * 1024) {
                        s.push(chunk, &mut NoInspector).unwrap();
                    }
                    if abort_at.is_some() {
                        // The client went away: the session is dropped unfinished.
                        drop(s);
                        aborted.fetch_add(1, Ordering::Relaxed);
                    } else {
                        let up = s.finish(&mut NoInspector).unwrap();
                        std::hint::black_box(up.file_count());
                        done.fetch_add(1, Ordering::Relaxed);
                    }
                }
            })
        })
        .collect();

    let start = Instant::now();
    let mut samples = Vec::new();
    PEAK.store(LIVE.load(Ordering::Relaxed), Ordering::Relaxed);
    while start.elapsed() < run {
        std::thread::sleep(every.min(run.saturating_sub(start.elapsed())));
        let s = Sample {
            at: start.elapsed(),
            peak: PEAK.swap(LIVE.load(Ordering::Relaxed), Ordering::Relaxed) - base_heap,
            files: files_in(&store),
            handles: handles(),
        };
        println!(
            "soak {:>6}s heap peak {:>10} B files {:>4} handles {:>4} uploads {} aborted {}",
            s.at.as_secs(),
            s.peak,
            s.files,
            s.handles,
            done.load(Ordering::Relaxed),
            aborted.load(Ordering::Relaxed)
        );
        samples.push(s);
    }
    stop.store(true, Ordering::Relaxed);
    for w in workers {
        w.join().unwrap();
    }

    let (done, aborted) = (
        done.load(Ordering::Relaxed),
        aborted.load(Ordering::Relaxed),
    );
    assert!(done > 0 && aborted > 0, "uploads {done}, aborted {aborted}");
    // No growth: the second half's worst sample is no worse than the first
    // half's, with room for which sizes happened to coincide.
    let half = samples.len() / 2;
    let (first, second) = samples.split_at(half.max(1));
    let worst = |s: &[Sample], f: fn(&Sample) -> i64| s.iter().map(f).max().unwrap_or(0);
    for (what, f, slack) in [
        ("heap", (|s: &Sample| s.peak) as fn(&Sample) -> i64, 8 << 20),
        ("files", |s: &Sample| s.files as i64, 2 * SESSIONS as i64),
        (
            "handles",
            |s: &Sample| s.handles as i64,
            2 * SESSIONS as i64,
        ),
    ] {
        let (a, b) = (worst(first, f), worst(second, f));
        assert!(
            b <= a + a / 4 + slack,
            "{what} grew: first half {a}, second half {b}"
        );
    }
    // And the end is the start.
    assert_eq!(files_in(&store), 0, "files left");
    assert_eq!(handles(), base_handles, "handles left");
    let heap = LIVE.load(Ordering::Relaxed) - base_heap;
    assert!(heap <= 64 * 1024, "heap left: {heap} B");
    println!("soak done: {done} uploads, {aborted} aborted, heap left {heap} B");
}
