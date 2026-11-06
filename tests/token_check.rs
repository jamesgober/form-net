//! The anti-forgery check with csrf-sec's real tokens (0.8): `TokenCheck`
//! verifies the token the moment its field ends, so a forged upload is
//! refused before its first file reaches a sink, at every split of the body;
//! the login form's token is checked the same way. Every refusal lands in
//! the route's `RefusalDigest`.

#![cfg(feature = "fs")]

use std::path::PathBuf;

use csrf_sec::Csrf;
use form_net::{
    FormError, FromForm, Inspector, Limits, PartHead, Policy, Refusal, RefusalDigest, Store,
    TokenCheck, UploadSession, Verdict, form,
};

const KEY: [u8; 32] = *b"form-net test key, 32 bytes long";
const SESSION: &str = "session-1234";
const CT: &[u8] = b"multipart/form-data; boundary=X";

/// csrf-sec's verdict as the check's: any rejection is a 403 with one code.
fn verifier<'c>(
    csrf: &'c Csrf,
    session: &'static str,
) -> impl FnMut(&str) -> Result<(), Refusal> + 'c {
    move |token| {
        csrf.verify(session, token)
            .map(|_| ())
            .map_err(|_| Refusal::new(403, "csrf.rejected"))
    }
}

fn dir(tag: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!(
        "form-net-token-{tag}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&p).unwrap();
    p
}

/// Records what reached the inspector after the check let it through.
#[derive(Default)]
struct Seen {
    file_starts: u32,
    file_bytes: usize,
    file_parts: Vec<u32>,
}

impl Inspector for Seen {
    fn part_start(&mut self, head: &PartHead<'_>) -> Verdict {
        if head.is_file() {
            self.file_starts += 1;
            self.file_parts.push(head.index());
        }
        Verdict::Continue
    }
    fn data(&mut self, part: u32, chunk: &[u8]) -> Verdict {
        if self.file_parts.contains(&part) {
            self.file_bytes += chunk.len();
        }
        Verdict::Continue
    }
}

fn upload_body(token: &str, token_first: bool) -> Vec<u8> {
    let token_part =
        format!("--X\r\nContent-Disposition: form-data; name=csrf_token\r\n\r\n{token}\r\n");
    let title = "--X\r\nContent-Disposition: form-data; name=title\r\n\r\nHoliday\r\n";
    let file = format!(
        "--X\r\nContent-Disposition: form-data; name=avatar; filename=a.png\r\nContent-Type: image/png\r\n\r\n{}\r\n",
        "p".repeat(300)
    );
    let mut out = String::new();
    if token_first {
        out.push_str(&token_part);
        out.push_str(title);
        out.push_str(&file);
    } else {
        out.push_str(title);
        out.push_str(&file);
        out.push_str(&token_part);
    }
    out.push_str("--X--");
    out.into_bytes()
}

/// Feeds `body` in chunks of `size`; returns the result and what got past.
fn run(
    store: &Store,
    body: &[u8],
    size: usize,
    check: &mut impl Inspector,
) -> (Result<form_net::Upload, FormError>, Seen) {
    let mut seen = Seen::default();
    let result = (|| {
        let mut s = UploadSession::new(CT, &Limits::DEFAULT, &Policy::DEFAULT, store)?;
        for chunk in body.chunks(size) {
            s.push(chunk, &mut (&mut *check, &mut seen))?;
        }
        s.finish(&mut (&mut *check, &mut seen))
    })();
    (result, seen)
}

#[test]
fn test_a_real_token_lets_the_upload_through_at_every_split() {
    let csrf = Csrf::new(KEY).unwrap();
    let token = csrf.issue(SESSION).unwrap();
    let body = upload_body(token.as_str(), true);
    let d = dir("good");
    let store = Store::in_dir(&d).unwrap().memory_threshold(0);
    for size in 1..=body.len() {
        let mut check = TokenCheck::new("csrf_token", verifier(&csrf, SESSION));
        let (result, seen) = run(&store, &body, size, &mut check);
        let upload = result.unwrap_or_else(|e| panic!("chunks of {size}: {e:?}"));
        assert!(check.verified());
        assert_eq!(upload.captured_token(), None, "no TokenField in the policy");
        assert_eq!(upload.field("title"), Some("Holiday"));
        assert_eq!(upload.file("avatar").unwrap().len(), 300);
        assert_eq!((seen.file_starts, seen.file_bytes), (1, 300));
    }
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn test_attack_a_forged_token_stops_the_upload_before_any_file_byte() {
    let csrf = Csrf::new(KEY).unwrap();
    let other = Csrf::new(*b"another key, also 32 bytes long!").unwrap();
    let digest = RefusalDigest::with_codes(["csrf.rejected", "csrf.missing", "csrf.duplicate"]);
    let forged = [
        // Signed with another key.
        other.issue(SESSION).unwrap().as_str().to_owned(),
        // Issued for another session.
        csrf.issue("someone-else").unwrap().as_str().to_owned(),
        // Not a token at all.
        "x".repeat(59),
        String::new(),
    ];
    let d = dir("forged");
    let store = Store::in_dir(&d).unwrap().memory_threshold(0);
    let mut cases = 0u64;
    for token in &forged {
        let body = upload_body(token, true);
        for size in 1..=body.len() {
            let mut check = TokenCheck::new("csrf_token", verifier(&csrf, SESSION));
            let (result, seen) = run(&store, &body, size, &mut check);
            let err = result.unwrap_err();
            digest.record(&err);
            cases += 1;
            assert_eq!((err.status_hint(), err.code()), (403, "csrf.rejected"));
            assert!(!check.verified());
            assert_eq!(
                (seen.file_starts, seen.file_bytes),
                (0, 0),
                "chunks of {size}"
            );
            let stats = store.stats();
            assert_eq!(
                (
                    stats.files_on_disk,
                    stats.files_spilled,
                    stats.files_in_memory
                ),
                (0, 0, 0)
            );
            assert_eq!(std::fs::read_dir(&d).unwrap().count(), 0);
        }
    }
    assert_eq!(digest.count("csrf.rejected"), cases);
    assert_eq!(digest.total(), cases);
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn test_attack_a_file_before_the_token_is_refused_before_it_opens() {
    let csrf = Csrf::new(KEY).unwrap();
    let token = csrf.issue(SESSION).unwrap();
    let body = upload_body(token.as_str(), false);
    let d = dir("late");
    let store = Store::in_dir(&d).unwrap().memory_threshold(0);
    for size in 1..=body.len() {
        let mut check = TokenCheck::new("csrf_token", verifier(&csrf, SESSION));
        let (result, seen) = run(&store, &body, size, &mut check);
        let err = result.unwrap_err();
        assert_eq!((err.status_hint(), err.code()), (403, "csrf.missing"));
        assert_eq!((seen.file_starts, seen.file_bytes), (0, 0));
        assert_eq!(store.stats().files_spilled, 0);
    }
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn test_attack_two_tokens_are_refused_whichever_is_valid() {
    let csrf = Csrf::new(KEY).unwrap();
    let good = csrf.issue(SESSION).unwrap();
    let good = good.as_str();
    for body in [
        format!("csrf_token={good}&csrf_token=forged"),
        format!("csrf_token=forged&csrf_token={good}"),
        format!("csrf_token={good}&csrf_token={good}"),
    ] {
        let mut check = TokenCheck::new("csrf_token", verifier(&csrf, SESSION));
        let r = form_net::parse_urlencoded_inspected(
            body.as_bytes(),
            &Limits::DEFAULT,
            &Policy::DEFAULT,
            &mut check,
        );
        let code = r.unwrap_err().code();
        assert!(
            code == "csrf.duplicate" || code == "csrf.rejected",
            "{body}: {code}"
        );
    }
}

form! {
    #[derive(Debug)]
    struct Login {
        email: String,
        password: form_net::Secret,
        remember: bool,
    }
    ignore = ["csrf_token"];
}

#[test]
fn test_the_login_form_checks_its_token_and_extracts_the_rest() {
    static LOGIN: RefusalDigest<2> = RefusalDigest::with_codes(["csrf.rejected", "csrf.missing"]);
    let csrf = Csrf::new(KEY).unwrap();
    let token = csrf.issue(SESSION).unwrap();
    let ct = b"application/x-www-form-urlencoded";
    let login = |body: String| {
        let mut check = TokenCheck::new("csrf_token", verifier(&csrf, SESSION));
        Login::from_body_inspected(
            ct,
            body.as_bytes(),
            &Limits::DEFAULT,
            &Policy::DEFAULT,
            &mut check,
        )
        .inspect_err(|e| LOGIN.record(e))
    };

    let ok = login(format!(
        "email=a%40b.example&password=pw&remember=on&csrf_token={}",
        token.as_str()
    ))
    .unwrap();
    assert_eq!(ok.email, "a@b.example");
    assert!(ok.remember);

    let forged = login("email=a%40b.example&password=pw&csrf_token=forged".into()).unwrap_err();
    assert_eq!(forged.code(), "csrf.rejected");
    let missing = login("email=a%40b.example&password=pw".into()).unwrap_err();
    assert_eq!(missing.code(), "csrf.missing");
    let broken = login("email=%zz".into()).unwrap_err();
    assert_eq!(broken.code(), "encoding.invalid_percent");

    let counts: Vec<_> = LOGIN.iter().collect();
    assert_eq!(
        counts,
        [
            ("encoding.invalid_percent", 1),
            ("csrf.rejected", 1),
            ("csrf.missing", 1)
        ]
    );
}
