//! Allocations per operation, for docs/PERFORMANCE.md: a counting
//! allocator around each driver. Run with
//! `cargo run --release --bin allocs`.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

use bytes::Bytes;
use form_net_bench as b;

struct Counting;

thread_local! {
    static COUNT: Cell<u64> = const { Cell::new(0) };
    static BYTES: Cell<u64> = const { Cell::new(0) };
}

// SAFETY: forwards to the system allocator; only counts.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let _ = COUNT.try_with(|c| c.set(c.get() + 1));
        let _ = BYTES.try_with(|c| c.set(c.get() + layout.size() as u64));
        // SAFETY: the caller upholds `GlobalAlloc::alloc`'s contract.
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: the caller upholds `GlobalAlloc::dealloc`'s contract.
        unsafe { System.dealloc(ptr, layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let _ = COUNT.try_with(|c| c.set(c.get() + 1));
        let _ = BYTES.try_with(|c| c.set(c.get() + new_size as u64));
        // SAFETY: the caller upholds `GlobalAlloc::realloc`'s contract.
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static GLOBAL: Counting = Counting;

fn count<R>(f: impl FnOnce() -> R) -> (u64, u64) {
    let (c0, b0) = (COUNT.with(Cell::get), BYTES.with(Cell::get));
    let r = f();
    std::hint::black_box(r);
    (COUNT.with(Cell::get) - c0, BYTES.with(Cell::get) - b0)
}

fn row(family: &str, input: &str, name: &str, (n, bytes): (u64, u64)) {
    println!("| {family} | {input} | {name} | {n} | {bytes} |");
}

fn main() {
    let ct = b::content_type();
    println!("| Family | Input | Crate | Allocations | Bytes allocated |");
    println!("|---|---|---|---|---|");
    let mut scratch = vec![0u8; 1 << 20];
    for (input, body) in [
        ("login", b::login()),
        ("signup", b::signup()),
        ("1000-fields", b::thousand_fields()),
    ] {
        row(
            "urlencoded pairs",
            input,
            "form-net parse_urlencoded",
            count(|| b::fnet_pairs(&body)),
        );
        row(
            "urlencoded pairs",
            input,
            "form-net raw_pairs+decode_into",
            count(|| b::fnet_raw(&body, &mut scratch)),
        );
        row(
            "urlencoded pairs",
            input,
            "form_urlencoded",
            count(|| b::form_urlencoded_pairs(&body)),
        );
    }
    let body = b::signup();
    row(
        "urlencoded struct",
        "signup",
        "form-net form!",
        count(|| b::fnet_signup(&body)),
    );
    row(
        "urlencoded struct",
        "signup",
        "serde_urlencoded",
        count(|| b::serde_urlencoded_signup(&body)),
    );
    row(
        "urlencoded struct",
        "signup",
        "serde_qs",
        count(|| b::serde_qs_signup(&body)),
    );
    row(
        "urlencoded serialize",
        "10 pairs",
        "form-net",
        count(b::fnet_serialize),
    );
    row(
        "urlencoded serialize",
        "10 pairs",
        "form_urlencoded",
        count(b::form_urlencoded_serialize),
    );
    row(
        "urlencoded serialize",
        "10 pairs",
        "serde_urlencoded",
        count(b::serde_urlencoded_serialize),
    );
    for (input, body) in [
        ("login", b::mp_login()),
        ("avatar-64KiB", b::mp_avatar()),
        ("document-10MiB", b::mp_document()),
    ] {
        let shared = Bytes::from(body.clone());
        for chunk in [body.len(), 64 * 1024, 1500] {
            let label = format!(
                "{input}, {} chunks",
                if chunk == body.len() {
                    "whole".to_string()
                } else {
                    format!("{chunk} B")
                }
            );
            row(
                "multipart stream",
                &label,
                "form-net MultipartParser",
                count(|| b::fnet_stream(&body, chunk)),
            );
            row(
                "multipart stream",
                &label,
                "multer",
                count(|| b::multer_stream(&shared, chunk)),
            );
        }
        row(
            "multipart buffered",
            input,
            "form-net parse_multipart",
            count(|| b::fnet_buffered(&ct, &body)),
        );
        row(
            "multipart buffered",
            input,
            "multipart 0.18",
            count(|| b::multipart_018_buffered(&ct, &body)),
        );
    }
}
