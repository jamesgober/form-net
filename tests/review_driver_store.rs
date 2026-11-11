//! The 0.9.x adversarial review of the driver, hooks, and storage (DIRECTIVES,
//! definition of done item 10): one test per finding. Each test failed on
//! 0.9.0; the comment on it describes the defect as found, and the fix is in
//! the decision log (D-139 to D-141, D-143, D-144).

#![cfg(feature = "fs")]

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

use form_net::{Limits, NoInspector, Policy, Store, UploadSession};

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

fn peak<R>(f: impl FnOnce() -> R) -> (u64, R) {
    let base = LIVE.with(Cell::get);
    PEAK.with(|p| p.set(base));
    let r = f();
    let top = PEAK.with(Cell::get);
    ((top - base).max(0) as u64, r)
}

const CT: &[u8] = b"multipart/form-data; boundary=X";

/// `OwnedPartHead::copy_from` copies the header block and then, again,
/// `name_raw` and `content_type`, which are slices of that same block. A
/// head whose `Content-Type` fills the header budget needs about twice
/// `max_part_header_bytes`, while `memory_bound` counts the driver's copy
/// at one `multipart_buffer_len`; the copy is also reallocated while the
/// old buffer is alive.
#[test]
fn test_owned_head_copy_exceeds_memory_bound() {
    const H: u32 = 64 * 1024;
    const ROUTE: Limits = Limits::DEFAULT
        .with_max_part_header_bytes(H)
        .with_max_parts(1)
        .with_max_files(1)
        .with_max_fields(1)
        .with_max_file_bytes(16)
        .with_max_text_bytes(16)
        .with_max_value_bytes(16)
        .with_max_name_bytes(16)
        .with_max_filename_bytes(16)
        .with_sniff_bytes(0);
    let mut body = Vec::new();
    body.extend_from_slice(
        b"--X\r\nContent-Disposition: form-data; name=\"f\"; filename=\"a\"\r\nContent-Type: application/octet-stream; x=\"",
    );
    let used = body.len() - 5 + 7; // "\"\r\n\r\n" closes the block
    body.extend(std::iter::repeat_n(b'a', H as usize - used - 16));
    body.extend_from_slice(b"\"\r\n\r\n");
    body.extend_from_slice(b"data\r\n--X--");

    let store = Store::memory_only();
    let bound = store.memory_bound(&ROUTE);
    let (p, n) = peak(|| {
        let mut s = UploadSession::new(CT, &ROUTE, &Policy::DEFAULT, &store).unwrap();
        s.push(&body, &mut NoInspector).unwrap();
        let upload = s.finish(&mut NoInspector).unwrap();
        upload.file_count()
    });
    assert_eq!(n, 1, "the body parses");
    assert!(p <= bound, "peak {p} > memory_bound {bound}");
}

/// The same body, two answers: the buffered parser reads the parser's
/// `_charset_` record after a second `_charset_` part has started, when
/// `start_part` has already reset it, so the second `_charset_` field is
/// read as UTF-8; the streaming driver reads the record before the part
/// started (iso-8859-1, D-128) and refuses it. `MultipartReader` agrees
/// with the driver.
#[test]
fn test_second_charset_field_buffered_and_session_disagree() {
    let body: &[u8] =
        b"--X\r\nContent-Disposition: form-data; name=\"_charset_\"\r\n\r\niso-8859-1\r\n\
--X\r\nContent-Disposition: form-data; name=\"_charset_\"\r\n\r\nutf-8\r\n\
--X\r\nContent-Disposition: form-data; name=\"a\"\r\n\r\nx\r\n--X--";
    let buffered = form_net::parse_multipart(CT, body).map(|m| m.len());
    let store = Store::memory_only();
    let streamed = (|| {
        let mut s = UploadSession::new(CT, &Limits::DEFAULT, &Policy::DEFAULT, &store)?;
        s.push(body, &mut NoInspector)?;
        s.finish(&mut NoInspector).map(|u| u.iter().count())
    })();
    assert_eq!(
        buffered.as_ref().map_err(|e| e.code()),
        streamed.as_ref().map_err(|e| e.code()),
        "buffered and streaming parses of one body must agree"
    );
}

/// A file part refused at `part_end` is closed, not aborted: the driver
/// calls `Sink::close` before `Inspector::part_end` (PLAN 6.2 shows
/// `part_end` then `close`; API.md: on a refusal "every sink is
/// aborted"), then drops the output and calls `abort` on a sink with no
/// open part, which the `Sink` contract says must do nothing. A sink that
/// commits in `close` (an object store, a named file) keeps a part the
/// inspector refused.
#[test]
fn test_part_end_refusal_closes_sink_instead_of_aborting() {
    use form_net::{
        Boundary, Driver, FormError, Inspector, MultipartParser, PartHead, PartSummary, Refusal,
        Sink, Verdict,
    };

    #[derive(Default)]
    struct Log(Vec<&'static str>, bool);
    impl Sink for Log {
        type Output = ();
        fn open(&mut self, _: &PartHead<'_>) -> Result<(), FormError> {
            self.0.push("open");
            self.1 = true;
            Ok(())
        }
        fn write(&mut self, _: &[u8]) -> Result<(), FormError> {
            self.0.push("write");
            Ok(())
        }
        fn close(&mut self) -> Result<(), FormError> {
            self.0.push("close");
            self.1 = false;
            Ok(())
        }
        fn abort(&mut self) {
            if self.1 {
                self.0.push("abort");
                self.1 = false;
            }
        }
    }
    struct RefuseAtEnd;
    impl Inspector for RefuseAtEnd {
        fn part_end(&mut self, s: &PartSummary) -> Verdict {
            if s.bytes > 0 {
                Verdict::Refuse(Refusal::new(422, "review.whole_file_rejected"))
            } else {
                Verdict::Continue
            }
        }
    }

    let parser = MultipartParser::new(
        Boundary::new(b"X").unwrap(),
        &Limits::DEFAULT,
        &Policy::DEFAULT,
    )
    .unwrap();
    let mut driver = Driver::new(parser, Log::default());
    let body =
        b"--X\r\nContent-Disposition: form-data; name=f; filename=a.bin\r\n\r\nmalware\r\n--X--";
    let mut rest: &[u8] = body;
    let err = loop {
        match driver.push(rest, true, &mut RefuseAtEnd) {
            Ok((n, _)) => rest = &rest[n..],
            Err(e) => break e,
        }
    };
    assert_eq!(err.code(), "review.whole_file_rejected");
    let (_, sink) = driver.into_parts();
    assert_eq!(
        sink.0,
        ["open", "write", "abort"],
        "a refused part must be aborted, never committed"
    );
}

/// `inspector.failed` is not called when `Upload::parse_with` (now `read_multipart_inspected`) fails on the
/// `Content-Type` (or on the limits), while `parse_multipart_inspected`
/// and `from_body_inspected` call it for the same failure (D-043: once on
/// every failed parse). A probe that sends bad media types to an upload
/// route is invisible to the detection layer.
#[test]
fn test_upload_parse_with_bad_content_type_skips_inspector_failed() {
    use form_net::{FormError, Inspector};

    #[derive(Default)]
    struct Count(u32);
    impl Inspector for Count {
        fn failed(&mut self, _: &FormError) {
            self.0 += 1;
        }
    }
    let bad = b"multipart/form-data";
    let mut a = Count::default();
    assert!(
        form_net::parse_multipart_inspected(bad, b"", &Limits::DEFAULT, &Policy::DEFAULT, &mut a)
            .is_err()
    );
    assert_eq!(a.0, 1, "buffered: failed once");
    let store = Store::memory_only();
    let mut b = Count::default();
    assert!(
        form_net::read_multipart_inspected(
            bad,
            &b""[..],
            &Limits::DEFAULT,
            &Policy::DEFAULT,
            &store,
            &mut b
        )
        .is_err()
    );
    assert_eq!(
        b.0, 1,
        "read_multipart_inspected: failed must be called once"
    );
}

/// D-101 keeps a file's bytes reserved when its temporary name cannot be
/// removed, but only for a finished `StoredFile`. `StoreSink::abort` (every
/// refused, failed, or skipped upload, which a client can force) releases
/// the bytes before it tries the removal and ignores the outcome, so with
/// deletes failing (a scanner holding the file on Windows) the quota
/// reads free while the bytes stay on disk.
#[cfg(windows)]
#[test]
fn test_aborted_named_spill_releases_quota_while_file_remains() {
    use form_net::{SpillMode, SpillQuota};
    use std::os::windows::fs::OpenOptionsExt;

    let dir = std::env::temp_dir().join(format!("form-net-review-abort-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let quota = SpillQuota::new(1 << 20);
    let store = Store::in_dir(&dir)
        .unwrap()
        .mode(SpillMode::Named)
        .memory_threshold(0)
        .quota(quota.clone());
    let mut s = UploadSession::new(CT, &Limits::DEFAULT, &Policy::DEFAULT, &store).unwrap();
    let mut body = b"--X\r\nContent-Disposition: form-data; name=f; filename=a\r\n\r\n".to_vec();
    body.extend(std::iter::repeat_n(b'd', 4096));
    s.push(&body, &mut NoInspector).unwrap();
    assert!(quota.in_use() > 0, "the spill reserved its bytes");

    // Another process opens the spill file without FILE_SHARE_DELETE.
    let spill = std::fs::read_dir(&dir)
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let holder = std::fs::OpenOptions::new()
        .read(true)
        .share_mode(0x1 | 0x2) // READ | WRITE, no DELETE
        .open(&spill)
        .unwrap();

    // The client ends the body with a bare line feed: the upload aborts.
    assert!(s.push(b"\n--X\r\n", &mut NoInspector).is_err() || s.finish(&mut NoInspector).is_err());
    let still_there = spill.exists();
    let in_use = quota.in_use();
    drop(holder);
    let _ = std::fs::remove_file(&spill);
    let _ = std::fs::remove_dir_all(&dir);
    assert!(still_there, "the delete failed, as a scanner makes it fail");
    assert!(
        in_use > 0,
        "bytes still on disk were given back to the quota"
    );
}

/// D-128: every entry point reads a text field's charset from the parser's
/// `_charset_` record, skipped `_charset_` fields included. `PartReader::text`
/// keeps its own record, set only when the caller read `_charset_` with
/// `text()`, so a caller that skips (or never reads) `_charset_` gets
/// `iso-8859-1` bytes back as UTF-8 text where `UploadSession` and the
/// buffered parser refuse the same body.
#[test]
fn test_reader_text_ignores_skipped_charset_field() {
    let body: &[u8] =
        b"--X\r\nContent-Disposition: form-data; name=\"_charset_\"\r\n\r\niso-8859-1\r\n\
--X\r\nContent-Disposition: form-data; name=\"a\"\r\n\r\ncaf\xc3\xa9\r\n--X--";
    let buffered = form_net::parse_multipart(CT, body).map(|m| m.len());
    assert!(buffered.is_err(), "the buffered parser refuses the field");

    let mut reader = form_net::MultipartReader::new(CT, body).unwrap();
    let text = {
        reader.next_part().unwrap().unwrap().skip().unwrap();
        let mut a = reader.next_part().unwrap().unwrap();
        a.text().map_err(|e| e.code())
    };
    assert_eq!(
        text,
        Err("encoding.unsupported_charset"),
        "the reader must read the field under the declared charset"
    );
}
