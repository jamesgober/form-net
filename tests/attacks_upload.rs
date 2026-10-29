//! Attack tests for the 0.4 surface (`dev/PLAN.md` section 10): file names,
//! sniffing, the token rule through the driver, hidden content, and the
//! hook order. Storage attacks (T-7 to T-11) are in `store.rs`, `abort.rs`,
//! and `crash.rs`; T-31 is a unit test in `src/wipe.rs`.

#![cfg(feature = "alloc")]

use std::cell::RefCell;

use form_net::{
    Boundary, Driver, FormError, Inspector, Limits, MultipartParser, PartHead, Policy,
    PolicyRefusal, Progress, Sink, TokenField, Verdict,
};

const CT: &[u8] = b"multipart/form-data; boundary=X";

/// One log for the inspector and the sink, so their order is visible.
struct Both<'a>(&'a RefCell<Vec<String>>);

impl Inspector for Both<'_> {
    fn part_start(&mut self, head: &PartHead<'_>) -> Verdict {
        self.0.borrow_mut().push(format!("start {}", head.name()));
        Verdict::Continue
    }
    fn sniff(&mut self, _: &PartHead<'_>, first: &[u8]) -> Verdict {
        self.0
            .borrow_mut()
            .push(format!("sniff {}", String::from_utf8_lossy(first)));
        Verdict::Continue
    }
}

impl Sink for Both<'_> {
    type Output = ();
    fn open(&mut self, _: &PartHead<'_>) -> Result<(), FormError> {
        self.0.borrow_mut().push("open".into());
        Ok(())
    }
    fn write(&mut self, _: &[u8]) -> Result<(), FormError> {
        self.0.borrow_mut().push("write".into());
        Ok(())
    }
    fn close(&mut self) -> Result<(), FormError> {
        Ok(())
    }
    fn abort(&mut self) {
        self.0.borrow_mut().push("abort".into());
    }
}

fn run(
    body: &[u8],
    split: usize,
    policy: &Policy,
    log: &RefCell<Vec<String>>,
) -> Result<(), FormError> {
    let limits = Limits::DEFAULT.with_sniff_bytes(6);
    let parser = MultipartParser::new(Boundary::new(b"X").unwrap(), &limits, policy).unwrap();
    let mut driver = Driver::new(parser, Both(log));
    let mut inspector = Both(log);
    let (a, b) = body.split_at(split);
    for (i, chunk) in [a, b].iter().enumerate() {
        let mut pos = 0;
        loop {
            let (n, p) = driver.push(&chunk[pos..], i == 1, &mut inspector)?;
            pos += n;
            match p {
                Progress::End => return Ok(()),
                Progress::NeedInput => break,
                _ => {}
            }
        }
    }
    Ok(())
}

#[test]
fn test_attack_t22_t38_sniff_sees_real_bytes_before_any_write() {
    // A part that claims to be a PNG but carries HTML.
    let body = b"--X\r\nContent-Disposition: form-data; name=f; filename=a.png\r\nContent-Type: image/png\r\n\r\n<html><script>x</script>\r\n--X--";
    for split in 0..=body.len() {
        let log = RefCell::new(Vec::new());
        run(body, split, &Policy::DEFAULT, &log).unwrap();
        let log = log.into_inner();
        let sniff = log.iter().position(|l| l.starts_with("sniff")).unwrap();
        let first_write = log.iter().position(|l| l == "write").unwrap();
        assert_eq!(log[sniff], "sniff <html>", "split {split}");
        assert!(
            log[0] == "start f" && sniff < first_write,
            "split {split}: {log:?}"
        );
    }
}

#[test]
fn test_attack_t23_late_token_refused_before_the_sink_opens() {
    let policy = Policy::DEFAULT.with_token_field(TokenField::new("csrf_token"));
    let body = b"--X\r\nContent-Disposition: form-data; name=f; filename=a\r\n\r\npayload\r\n--X\r\nContent-Disposition: form-data; name=csrf_token\r\n\r\nt\r\n--X--";
    for split in 0..=body.len() {
        let log = RefCell::new(Vec::new());
        let r = run(body, split, &policy, &log);
        assert!(
            matches!(
                r,
                Err(FormError::Policy {
                    reason: PolicyRefusal::TokenFieldLate,
                    ..
                })
            ),
            "split {split}: {r:?}"
        );
        let log = log.into_inner();
        assert!(
            !log.iter().any(|l| l == "open" || l == "write"),
            "split {split}: {log:?}"
        );
    }
}

#[cfg(feature = "fs")]
#[test]
fn test_attack_t23_session_token_is_available_before_the_files() {
    use form_net::{NoInspector, Store, UploadSession};
    let policy = Policy::DEFAULT.with_token_field(TokenField::new("csrf_token"));
    let store = Store::memory_only();
    let head = b"--X\r\nContent-Disposition: form-data; name=csrf_token\r\n\r\ntok\r\n--X\r\nContent-Disposition: form-data; name=f; filename=a\r\n\r\n";
    let mut s = UploadSession::new(CT, &Limits::DEFAULT, &policy, &store).unwrap();
    s.push(head, &mut NoInspector).unwrap();
    assert_eq!(
        s.captured_token(),
        Some("tok"),
        "verifiable before any file byte"
    );
    s.push(b"data\r\n--X--", &mut NoInspector).unwrap();
    assert_eq!(
        s.finish(&mut NoInspector).unwrap().captured_token(),
        Some("tok")
    );
}

#[test]
fn test_attack_t25_t26_t27_hidden_content_refused() {
    let cases: [&[u8]; 4] = [
        // UTF-7 declared through _charset_, then a UTF-7 script tag.
        b"--X\r\nContent-Disposition: form-data; name=_charset_\r\n\r\nutf-7\r\n--X\r\nContent-Disposition: form-data; name=a\r\n\r\n+ADw-script+AD4-\r\n--X--",
        // A UTF-7 charset on the part itself.
        b"--X\r\nContent-Disposition: form-data; name=a\r\nContent-Type: text/plain; charset=utf-7\r\n\r\n+ADw-script+AD4-\r\n--X--",
        // Quoted-printable hides bytes from a scanner.
        b"--X\r\nContent-Disposition: form-data; name=a\r\nContent-Transfer-Encoding: quoted-printable\r\n\r\n=3Cscript=3E\r\n--X--",
        // A nested multipart/mixed part hides more files.
        b"--X\r\nContent-Disposition: form-data; name=a\r\nContent-Type: multipart/mixed; boundary=Y\r\n\r\n--Y\r\nContent-Disposition: file; filename=x\r\n\r\nx\r\n--Y--\r\n--X--",
    ];
    for body in cases {
        assert!(
            form_net::parse_multipart(CT, body).is_err(),
            "{:?}",
            String::from_utf8_lossy(body)
        );
    }
}

#[cfg(feature = "fs")]
#[test]
fn test_attack_t20_t21_file_names_are_never_paths() {
    let cases = [
        "../../etc/passwd",
        "..\\..\\boot.ini",
        "C:\\Windows\\x",
        "\\\\host\\share\\x",
        "/abs/x",
        "a.txt:stream",
        "CON",
        "NUL.txt",
        "gpj\u{202e}.exe",
        "a\u{200b}b.txt",
        "..",
    ];
    for raw in cases {
        let body = format!(
            "--X\r\nContent-Disposition: form-data; name=f; filename=\"{raw}\"\r\n\r\n\r\n--X--"
        );
        let mp = form_net::parse_multipart(CT, body.as_bytes()).unwrap();
        let file = mp.file("f").unwrap();
        // The raw name is only ever a hint; the sanitized one is one segment.
        let Ok(s) = file.filename().unwrap().sanitized() else {
            continue;
        };
        assert!(
            !s.is_empty() && !s.contains(['/', '\\', ':', '\u{202e}', '\u{200b}']),
            "{raw:?} -> {s:?}"
        );
        assert!(
            s != "." && s != ".." && !std::path::Path::new(&s).is_absolute(),
            "{raw:?} -> {s:?}"
        );
        let stem = s.split('.').next().unwrap().to_ascii_uppercase();
        assert!(
            !["CON", "NUL", "PRN", "AUX"].contains(&stem.as_str()),
            "{raw:?} -> {s:?}"
        );
    }
}

#[cfg(all(feature = "fs", feature = "unicode"))]
#[test]
fn test_attack_t21_nfd_names_sanitize_like_nfc() {
    let name = |raw: &str| {
        let body = format!(
            "--X\r\nContent-Disposition: form-data; name=f; filename=\"{raw}\"\r\n\r\n\r\n--X--"
        );
        let mp = form_net::parse_multipart(CT, body.as_bytes()).unwrap();
        let f = mp.file("f").unwrap().filename().unwrap();
        (f.nfc().unwrap(), f.sanitized().unwrap())
    };
    let (nfc_nfd, san_nfd) = name("cafe\u{301}.txt");
    let (nfc_nfc, san_nfc) = name("caf\u{e9}.txt");
    assert_eq!(
        (nfc_nfd.as_str(), san_nfd.as_str()),
        ("caf\u{e9}.txt", "caf\u{e9}.txt")
    );
    assert_eq!((nfc_nfc, san_nfc), (nfc_nfd, san_nfd));
}
