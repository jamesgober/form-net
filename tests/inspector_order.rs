//! The hook order guarantee (FR-55, T-38): part_start, sniff, data, field,
//! part_end, end, and failed once; at every split, through the streaming
//! driver, the buffered parser, and the urlencoded parser.

#![cfg(feature = "alloc")]

use form_net::{
    Boundary, Driver, FormError, Inspector, Limits, MultipartParser, PartHead, PartSummary, Policy,
    Progress, Refusal, Sink, Verdict,
};

#[derive(Default)]
struct Recorder {
    log: Vec<String>,
    refuse_at: Option<&'static str>,
    skip_at: Option<&'static str>,
}

impl Recorder {
    fn verdict(&mut self, hook: &'static str) -> Verdict {
        if self.refuse_at == Some(hook) {
            Verdict::Refuse(Refusal::new(422, "test.refused"))
        } else if self.skip_at == Some(hook) {
            Verdict::Skip
        } else {
            Verdict::Continue
        }
    }
}

impl Inspector for Recorder {
    fn part_start(&mut self, head: &PartHead<'_>) -> Verdict {
        self.log.push(format!("start {}", head.name()));
        self.verdict("part_start")
    }
    fn sniff(&mut self, head: &PartHead<'_>, first: &[u8]) -> Verdict {
        self.log.push(format!(
            "sniff {} {:?}",
            head.name(),
            String::from_utf8_lossy(first)
        ));
        self.verdict("sniff")
    }
    fn data(&mut self, part: u32, chunk: &[u8]) -> Verdict {
        // Data may arrive in any number of pieces: join consecutive ones.
        let piece = String::from_utf8_lossy(chunk).into_owned();
        match self.log.last_mut() {
            Some(last) if last.starts_with(&format!("data {part} ")) => last.push_str(&piece),
            _ => self.log.push(format!("data {part} {piece}")),
        }
        self.verdict("data")
    }
    fn field(&mut self, name: &str, value: &[u8]) -> Verdict {
        self.log
            .push(format!("field {name}={}", String::from_utf8_lossy(value)));
        self.verdict("field")
    }
    fn part_end(&mut self, summary: &PartSummary) -> Verdict {
        self.log.push(format!(
            "end {} {}{}",
            summary.index,
            summary.bytes,
            if summary.skipped { " skipped" } else { "" }
        ));
        self.verdict("part_end")
    }
    fn end(&mut self) -> Verdict {
        self.log.push("done".into());
        self.verdict("end")
    }
    fn failed(&mut self, error: &FormError) {
        self.log.push(format!("failed {}", error.code()));
    }
}

/// A sink that records into the same log, to prove no write comes before
/// the inspector calls for that part.
struct LogSink<'a> {
    log: &'a std::cell::RefCell<Vec<String>>,
}

impl Sink for LogSink<'_> {
    type Output = ();
    fn open(&mut self, head: &PartHead<'_>) -> Result<(), FormError> {
        self.log.borrow_mut().push(format!("open {}", head.name()));
        Ok(())
    }
    fn write(&mut self, chunk: &[u8]) -> Result<(), FormError> {
        let piece = String::from_utf8_lossy(chunk).into_owned();
        let mut log = self.log.borrow_mut();
        match log.last_mut() {
            Some(last) if last.starts_with("write ") => last.push_str(&piece),
            _ => log.push(format!("write {piece}")),
        }
        Ok(())
    }
    fn close(&mut self) -> Result<(), FormError> {
        self.log.borrow_mut().push("close".into());
        Ok(())
    }
    fn abort(&mut self) {
        self.log.borrow_mut().push("abort".into());
    }
}

const BODY: &[u8] = b"--X\r\nContent-Disposition: form-data; name=title\r\n\r\nHoliday\r\n--X\r\nContent-Disposition: form-data; name=photo; filename=a.png\r\n\r\n\x89PNGrest-of-file\r\n--X--";

fn limits() -> Limits {
    Limits::DEFAULT.with_sniff_bytes(4)
}

/// Runs the driver over `chunks`; returns the inspector log and the sink log
/// merged by position is not possible, so they are returned apart.
fn drive(chunks: &[&[u8]], rec: &mut Recorder) -> (Result<(), FormError>, Vec<String>) {
    let log = std::cell::RefCell::new(Vec::new());
    let parser =
        MultipartParser::new(Boundary::new(b"X").unwrap(), &limits(), &Policy::DEFAULT).unwrap();
    let mut driver = Driver::new(parser, LogSink { log: &log });
    let r = (|| {
        for (i, chunk) in chunks.iter().enumerate() {
            let last = i + 1 == chunks.len();
            let mut pos = 0;
            loop {
                let (n, p) = driver.push(&chunk[pos..], last, rec)?;
                pos += n;
                match p {
                    Progress::End => return Ok(()),
                    Progress::NeedInput => break,
                    _ => {}
                }
            }
        }
        Ok(())
    })();
    drop(driver);
    (r, log.into_inner())
}

const EXPECTED: &[&str] = &[
    "start title",
    "data 0 Holiday",
    "field title=Holiday",
    "end 0 7",
    "start photo",
    "sniff photo \"\u{fffd}PNG\"",
    "data 1 \u{fffd}PNGrest-of-file",
    "end 1 16",
    "done",
];

#[test]
fn test_driver_order_at_every_split() {
    for split in 0..=BODY.len() {
        let mut rec = Recorder::default();
        let (a, b) = BODY.split_at(split);
        let (r, sink) = drive(&[a, b], &mut rec);
        r.unwrap();
        assert_eq!(rec.log, EXPECTED, "split {split}");
        assert_eq!(
            sink,
            ["open photo", "write \u{fffd}PNGrest-of-file", "close"],
            "split {split}"
        );
    }
    let mut rec = Recorder::default();
    let bytes: Vec<&[u8]> = BODY.chunks(1).collect();
    drive(&bytes, &mut rec).0.unwrap();
    assert_eq!(rec.log, EXPECTED, "bytewise");
}

#[test]
fn test_buffered_order_matches_driver() {
    let mut rec = Recorder::default();
    let _ = form_net::parse_multipart_inspected(
        b"multipart/form-data; boundary=X",
        BODY,
        &limits(),
        &Policy::DEFAULT,
        &mut rec,
    )
    .unwrap();
    assert_eq!(rec.log, EXPECTED);
}

#[test]
fn test_refuse_from_each_hook_stops_and_reports_failed_once() {
    for hook in ["part_start", "sniff", "data", "field", "part_end", "end"] {
        let mut rec = Recorder {
            refuse_at: Some(hook),
            ..Recorder::default()
        };
        let (r, sink) = drive(&[BODY], &mut rec);
        assert!(matches!(r, Err(FormError::Refused { .. })), "{hook}");
        if hook == "part_start" || hook == "sniff" {
            assert!(
                !sink
                    .iter()
                    .any(|l| l.starts_with("open") || l.starts_with("write")),
                "{hook}: storage touched before the verdict: {sink:?}"
            );
        }
        assert_eq!(
            rec.log.iter().filter(|l| l.starts_with("failed")).count(),
            1,
            "{hook}"
        );
        assert_eq!(
            rec.log.last().map(String::as_str),
            Some("failed test.refused"),
            "{hook}"
        );
        if sink.iter().any(|l| l.starts_with("open")) && !sink.contains(&"close".to_string()) {
            assert!(sink.contains(&"abort".to_string()), "{hook}: {sink:?}");
        }
        let mut rec = Recorder {
            refuse_at: Some(hook),
            ..Recorder::default()
        };
        let b = form_net::parse_multipart_inspected(
            b"multipart/form-data; boundary=X",
            BODY,
            &limits(),
            &Policy::DEFAULT,
            &mut rec,
        );
        assert!(
            matches!(b, Err(FormError::Refused { .. })),
            "buffered {hook}"
        );
        assert_eq!(
            rec.log.iter().filter(|l| l.starts_with("failed")).count(),
            1
        );
    }
}

#[test]
fn test_skip_drops_the_part() {
    // Skipping at part_start drops both parts' data from the result.
    let mut rec = Recorder {
        skip_at: Some("part_start"),
        ..Recorder::default()
    };
    let mp = form_net::parse_multipart_inspected(
        b"multipart/form-data; boundary=X",
        BODY,
        &limits(),
        &Policy::DEFAULT,
        &mut rec,
    )
    .unwrap();
    assert!(mp.is_empty());
    assert!(rec.log.iter().all(|l| !l.starts_with("data")));
    // Skipping at sniff drops the file before the sink is even opened.
    let mut rec = Recorder {
        skip_at: Some("sniff"),
        ..Recorder::default()
    };
    let (r, sink) = drive(&[BODY], &mut rec);
    r.unwrap();
    assert!(sink.is_empty(), "{sink:?}");
    assert!(rec.log.contains(&"end 1 16 skipped".to_string()));
    // Skipping a field drops it.
    let mut rec = Recorder {
        skip_at: Some("field"),
        ..Recorder::default()
    };
    let mp = form_net::parse_multipart_inspected(
        b"multipart/form-data; boundary=X",
        BODY,
        &limits(),
        &Policy::DEFAULT,
        &mut rec,
    )
    .unwrap();
    assert_eq!(mp.field("title"), None);
    assert!(mp.file("photo").is_some());
}

#[test]
fn test_parse_failure_reports_failed_once() {
    let mut rec = Recorder::default();
    let bad = b"--X\r\nContent-Disposition: form-data; name=a\r\n\r\n1\r\n--Xjunk";
    let (r, _) = drive(&[bad], &mut rec);
    assert!(r.is_err());
    assert_eq!(
        rec.log.last().map(String::as_str),
        Some("failed syntax.boundary_in_content")
    );
}

#[test]
fn test_urlencoded_inspected_order_and_skip() {
    let mut rec = Recorder::default();
    let form = form_net::parse_urlencoded_inspected(
        b"a=1&b=2",
        &Limits::DEFAULT,
        &Policy::DEFAULT,
        &mut rec,
    )
    .unwrap();
    assert_eq!(rec.log, ["field a=1", "field b=2", "done"]);
    assert_eq!(form.len(), 2);
    let mut rec = Recorder {
        skip_at: Some("field"),
        ..Recorder::default()
    };
    let form = form_net::parse_urlencoded_inspected(
        b"a=1&b=2",
        &Limits::DEFAULT,
        &Policy::DEFAULT,
        &mut rec,
    )
    .unwrap();
    assert!(form.is_empty());
    let mut rec = Recorder::default();
    assert!(
        form_net::parse_urlencoded_inspected(
            b"a=%zz",
            &Limits::DEFAULT,
            &Policy::DEFAULT,
            &mut rec
        )
        .is_err()
    );
    assert_eq!(rec.log, ["failed encoding.invalid_percent"]);
}

#[test]
fn test_sniff_window_for_short_and_split_files() {
    // A file shorter than the window: sniff gets all of it, once.
    let body = b"--X\r\nContent-Disposition: form-data; name=f; filename=a\r\n\r\nab\r\n--X--";
    for split in 0..=body.len() {
        let mut rec = Recorder::default();
        let log = std::cell::RefCell::new(Vec::new());
        let parser =
            MultipartParser::new(Boundary::new(b"X").unwrap(), &limits(), &Policy::DEFAULT)
                .unwrap();
        let mut driver = Driver::new(parser, LogSink { log: &log });
        let (a, b) = body.split_at(split);
        let mut done = false;
        for (i, c) in [a, b].iter().enumerate() {
            let mut pos = 0;
            while !done {
                let (n, p) = driver.push(&c[pos..], i == 1, &mut rec).unwrap();
                pos += n;
                match p {
                    Progress::End => done = true,
                    Progress::NeedInput => break,
                    _ => {}
                }
            }
        }
        assert_eq!(
            rec.log,
            ["start f", "sniff f \"ab\"", "data 0 ab", "end 0 2", "done"],
            "split {split}"
        );
    }
}

#[test]
fn test_progress_debug_never_shows_values() {
    let parser =
        MultipartParser::new(Boundary::new(b"X").unwrap(), &limits(), &Policy::DEFAULT).unwrap();
    let mut driver = Driver::new(parser, form_net::DiscardSink::default());
    let body = b"--X\r\nContent-Disposition: form-data; name=pw\r\n\r\nhunter2\r\n--X--";
    let (_, p) = driver.push(body, true, &mut form_net::NoInspector).unwrap();
    let shown = format!("{p:?}");
    assert!(
        shown.contains("pw") && !shown.contains("hunter2"),
        "{shown}"
    );
}
