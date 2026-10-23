//! A counting allocator backs every "no allocation" claim (FR-8, FR-9,
//! FR-43, NFR-7).

#![cfg(feature = "alloc")]

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

use form_net::{Boundary, Event, Limits, MultipartParser, PercentPolicy, Policy};

struct Counting;

thread_local! {
    static ALLOCATIONS: Cell<usize> = const { Cell::new(0) };
}

// SAFETY: forwards to the system allocator; only counts.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let _ = ALLOCATIONS.try_with(|c| c.set(c.get() + 1));
        // SAFETY: the caller upholds `GlobalAlloc::alloc`'s contract.
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: the caller upholds `GlobalAlloc::dealloc`'s contract.
        unsafe { System.dealloc(ptr, layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let _ = ALLOCATIONS.try_with(|c| c.set(c.get() + 1));
        // SAFETY: the caller upholds `GlobalAlloc::realloc`'s contract.
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static GLOBAL: Counting = Counting;

fn allocations<R>(f: impl FnOnce() -> R) -> (usize, R) {
    let before = ALLOCATIONS.with(Cell::get);
    let r = f();
    (ALLOCATIONS.with(Cell::get) - before, r)
}

#[test]
fn test_raw_pairs_and_decode_into_never_allocate() {
    let input = b"email=a%40b.example&name=J+Doe&remember";
    let mut scratch = [0u8; 64];
    let (n, count) = allocations(|| {
        let mut count = 0;
        for pair in form_net::raw_pairs(input, &Limits::DEFAULT) {
            let pair = pair.unwrap();
            let _ = pair
                .decode_name(&mut scratch, PercentPolicy::Refuse)
                .unwrap()
                .len();
            let _ = pair
                .decode_value(&mut scratch, PercentPolicy::Refuse)
                .unwrap()
                .len();
            count += 1;
        }
        count
    });
    assert_eq!((n, count), (0, 3));
}

#[test]
fn test_borrowed_pairs_never_allocate() {
    let input = b"a=1&b=2&c=three";
    let (n, _) = allocations(|| form_net::pairs(input).filter(Result::is_ok).count());
    assert_eq!(n, 0);
}

#[test]
fn test_form_takes_two_allocations() {
    let input = b"a=1&b=%41%42&c=d+e&f&g=h&i=j&k=l&m=n&o=p&q=r";
    let (n, form) = allocations(|| form_net::parse_urlencoded(input).unwrap());
    assert_eq!(n, 2);
    assert_eq!(form.len(), 10);
}

#[test]
fn test_multipart_parser_never_allocates_after_construction() {
    let body = b"--X\r\nContent-Disposition: form-data; name=\"a\"\r\n\r\nhello\r\n--X\r\nContent-Disposition: form-data; name=\"f\"; filename=\"x.bin\"\r\nContent-Type: application/octet-stream\r\n\r\n\r\n--\r\n-X\r\n--Y\r\n--X--";
    let mut parser = MultipartParser::new(
        Boundary::new(b"X").unwrap(),
        &Limits::DEFAULT,
        &Policy::DEFAULT,
    )
    .unwrap();
    let (n, events) = allocations(|| {
        let mut events = 0usize;
        for chunk in body.chunks(3) {
            let mut rest = chunk;
            loop {
                let step = parser.parse(rest, false).unwrap();
                rest = &rest[step.consumed..];
                events += 1;
                if matches!(step.event, Event::NeedInput | Event::End) {
                    break;
                }
            }
        }
        events
    });
    assert_eq!(n, 0);
    assert!(events > 10);
    assert!(parser.is_done());
}

#[test]
fn test_multipart_with_buffer_never_allocates() {
    const LIMITS: Limits = Limits::DEFAULT.with_max_part_header_bytes(256);
    const LEN: usize = LIMITS.multipart_buffer_len(&Policy::DEFAULT);
    let mut buffer = [0u8; LEN];
    let body = b"--X\r\nContent-Disposition: form-data; name=\"a\"\r\n\r\nv\r\n--X--";
    let (n, ()) = allocations(|| {
        let mut p = MultipartParser::with_buffer(
            Boundary::new(b"X").unwrap(),
            &LIMITS,
            &Policy::DEFAULT,
            &mut buffer[..],
        )
        .unwrap();
        let mut rest: &[u8] = body;
        loop {
            let step = p.parse(rest, true).unwrap();
            rest = &rest[step.consumed..];
            if matches!(step.event, Event::End) {
                break;
            }
        }
    });
    assert_eq!(n, 0);
}

#[test]
fn test_detect_and_boundary_never_allocate() {
    let (n, _) = allocations(|| {
        form_net::FormKind::detect(b"multipart/form-data; boundary=----WebKitFormBoundaryx7Y")
            .unwrap()
    });
    assert_eq!(n, 0);
}
