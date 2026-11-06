//! The 0.8 alpha's two routes, written as web-net will write them, with a
//! test double where customs-sec will be: a login form (urlencoded) and an
//! avatar upload (multipart, pushed chunk by chunk). Each route has its own
//! limits, policy, and refusal digest; the token is csrf-sec's, verified by
//! `TokenCheck`; customs runs as an `Inspector` beside it. When web-net and
//! customs-sec exist, these routes move there and this file stays as the
//! contract they must keep.

#![cfg(feature = "fs")]

use std::path::PathBuf;

use csrf_sec::Csrf;
use form_net::{
    FormError, FormKind, FromForm, Inspector, Limits, PartHead, Policy, Refusal, RefusalDigest,
    Secret, Store, StoredFile, TokenCheck, TokenField, UploadSession, Verdict, form,
};

const KEY: [u8; 32] = *b"form-net alpha key, 32 bytes ok!";
const SESSION: &str = "session-alpha";

/// The codes the customs double returns, named in each route's digest.
const CUSTOMS: [&str; 3] = [
    "customs.unexpected_field",
    "customs.not_an_image",
    "customs.control_character",
];

/// customs-sec's place: a schema of allowed fields per route, text without
/// control characters, and images judged by their bytes.
struct Customs {
    text: &'static [&'static str],
    files: &'static [&'static str],
}

impl Inspector for Customs {
    fn part_start(&mut self, head: &PartHead<'_>) -> Verdict {
        let allowed = if head.is_file() {
            self.files
        } else {
            self.text
        };
        if allowed.contains(&head.name()) {
            Verdict::Continue
        } else {
            Verdict::Refuse(Refusal::new(422, CUSTOMS[0]))
        }
    }
    fn sniff(&mut self, _head: &PartHead<'_>, first: &[u8]) -> Verdict {
        if first.starts_with(b"\x89PNG\r\n\x1a\n") {
            Verdict::Continue
        } else {
            Verdict::Refuse(Refusal::new(415, CUSTOMS[1]))
        }
    }
    fn field(&mut self, name: &str, value: &[u8]) -> Verdict {
        // Urlencoded pairs have no part_start: the schema is checked here.
        if !self.text.contains(&name) {
            return Verdict::Refuse(Refusal::new(422, CUSTOMS[0]));
        }
        if value
            .iter()
            .any(|b| b.is_ascii_control() && *b != b'\n' && *b != b'\r')
        {
            return Verdict::Refuse(Refusal::new(422, CUSTOMS[2]));
        }
        Verdict::Continue
    }
}

fn csrf_check(csrf: &Csrf) -> TokenCheck<impl FnMut(&str) -> Result<(), Refusal> + '_> {
    TokenCheck::new(csrf_sec::FIELD_NAME, move |token| {
        csrf.verify(SESSION, token)
            .map(|_| ())
            .map_err(|_| Refusal::new(403, "csrf.rejected"))
    })
}

// ---------------------------------------------------------------- login

form! {
    #[derive(Debug)]
    struct Login {
        email: String,
        password: Secret,
        remember: bool,
    }
    ignore = ["csrf_token"];
}

const LOGIN_LIMITS: Limits = Limits::DEFAULT
    .with_max_body_bytes(4 << 10)
    .with_max_fields(8);
static LOGIN_DIGEST: RefusalDigest<5> = RefusalDigest::with_codes([
    CUSTOMS[0],
    CUSTOMS[1],
    CUSTOMS[2],
    "csrf.rejected",
    "csrf.missing",
]);

/// POST /login, as web-net's handler: detect, extract with customs and the
/// token check in the same pass, record any refusal.
fn login(csrf: &Csrf, content_type: &[u8], body: &[u8]) -> Result<Login, FormError> {
    let result = (|| {
        FormKind::detect(content_type)?;
        let mut customs = Customs {
            text: &["email", "password", "remember", "csrf_token"],
            files: &[],
        };
        let mut check = csrf_check(csrf);
        Login::from_body_inspected(
            content_type,
            body,
            &LOGIN_LIMITS,
            &Policy::DEFAULT,
            &mut (&mut customs, &mut check),
        )
    })();
    result.inspect_err(|e| LOGIN_DIGEST.record(e))
}

#[test]
fn test_alpha_login_route() {
    let csrf = Csrf::new(KEY).unwrap();
    let token = csrf.issue(SESSION).unwrap();
    let form = b"application/x-www-form-urlencoded";
    let good = format!(
        "email=a%40b.example&password=hunter2&remember=on&csrf_token={}",
        token.as_str()
    );

    let ok = login(&csrf, form, good.as_bytes()).unwrap();
    assert_eq!(ok.email, "a@b.example");
    assert!(ok.remember);

    // The same form posted as multipart parses the same.
    let mp = format!(
        "--B\r\nContent-Disposition: form-data; name=csrf_token\r\n\r\n{}\r\n--B\r\nContent-Disposition: form-data; name=email\r\n\r\na@b.example\r\n--B\r\nContent-Disposition: form-data; name=password\r\n\r\nhunter2\r\n--B--",
        token.as_str()
    );
    let ok = login(&csrf, b"multipart/form-data; boundary=B", mp.as_bytes()).unwrap();
    assert_eq!((ok.email.as_str(), ok.remember), ("a@b.example", false));

    let cases: [(&[u8], String, u16, &str); 6] = [
        (
            form,
            "email=a%40b.example&password=x&csrf_token=forged".into(),
            403,
            "csrf.rejected",
        ),
        (
            form,
            "email=a%40b.example&password=x".into(),
            403,
            "csrf.missing",
        ),
        (
            form,
            format!("{good}&role=admin"),
            422,
            "customs.unexpected_field",
        ),
        (
            form,
            format!("{good}&email=x%07"),
            422,
            "customs.control_character",
        ),
        (form, "email=%zz".into(), 400, "encoding.invalid_percent"),
        (b"text/plain", good.clone(), 415, "unsupported_media_type"),
    ];
    for (ct, body, status, code) in &cases {
        let e = login(&csrf, ct, body.as_bytes()).unwrap_err();
        assert_eq!((e.status_hint(), e.code()), (*status, *code), "{body}");
    }
    let big = format!("{good}&pad={}", "a".repeat(5000));
    assert_eq!(
        login(&csrf, form, big.as_bytes()).unwrap_err().code(),
        "limit.body"
    );

    for (_, _, _, code) in &cases {
        assert_eq!(LOGIN_DIGEST.count(code), 1, "{code}");
    }
    assert_eq!(LOGIN_DIGEST.count("limit.body"), 1);
    assert_eq!((LOGIN_DIGEST.total(), LOGIN_DIGEST.unnamed()), (7, 0));
}

// ---------------------------------------------------------------- upload

form! {
    #[derive(Debug)]
    struct Avatar {
        title: String,
        avatar: StoredFile,
    }
    ignore = ["csrf_token"];
}

const UPLOAD_LIMITS: Limits = Limits::DEFAULT
    .with_max_body_bytes(128 << 10)
    .with_max_fields(4)
    .with_max_parts(4)
    .with_max_files(1)
    .with_max_file_bytes(64 << 10);
const UPLOAD_POLICY: Policy =
    Policy::DEFAULT.with_token_field(TokenField::new(csrf_sec::FIELD_NAME));

fn dir(tag: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!(
        "form-net-alpha-{tag}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&p).unwrap();
    p
}

/// POST /avatar, as web-net's handler: admission by the store's memory
/// bound, then the body pushed as reads complete.
fn upload(
    csrf: &Csrf,
    store: &Store,
    digest: &RefusalDigest<5>,
    body: &[u8],
    chunk: usize,
) -> Result<(Avatar, Option<String>), FormError> {
    let result = (|| {
        let ct = b"multipart/form-data; boundary=X";
        let mut customs = Customs {
            text: &["title", "csrf_token"],
            files: &["avatar"],
        };
        let mut check = csrf_check(csrf);
        let mut hooks = (&mut customs, &mut check);
        let mut session = UploadSession::new(ct, &UPLOAD_LIMITS, &UPLOAD_POLICY, store)?;
        for c in body.chunks(chunk) {
            session.push(c, &mut hooks)?;
        }
        let upload = session.finish(&mut hooks)?;
        let token = upload.captured_token().map(str::to_owned);
        Ok((Avatar::from_upload(upload)?, token))
    })();
    result.inspect_err(|e| digest.record(e))
}

fn avatar_body(token: &str, image: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(
        format!("--X\r\nContent-Disposition: form-data; name=csrf_token\r\n\r\n{token}\r\n")
            .as_bytes(),
    );
    out.extend_from_slice(b"--X\r\nContent-Disposition: form-data; name=title\r\n\r\nMe\r\n");
    out.extend_from_slice(b"--X\r\nContent-Disposition: form-data; name=avatar; filename=me.png\r\nContent-Type: image/png\r\n\r\n");
    out.extend_from_slice(image);
    out.extend_from_slice(b"\r\n--X--");
    out
}

#[test]
fn test_alpha_upload_route_at_every_chunk_size() {
    let csrf = Csrf::new(KEY).unwrap();
    let token = csrf.issue(SESSION).unwrap();
    let d = dir("upload");
    let store = Store::in_dir(&d).unwrap().memory_threshold(0);
    // Admission: what one request can hold in memory, before reading it.
    // The route's own caps set it; the defaults would allow about 5.5 MiB.
    let bound = store.memory_bound(&UPLOAD_LIMITS);
    assert!(bound > 0 && bound < 512 << 10, "{bound}");

    let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
    png.extend(std::iter::repeat_n(7u8, 600));
    let good = avatar_body(token.as_str(), &png);
    let gif = avatar_body(token.as_str(), b"GIF89a not a png at all");
    let forged = avatar_body("forged", &png);

    let digest = RefusalDigest::with_codes([
        CUSTOMS[0],
        CUSTOMS[1],
        CUSTOMS[2],
        "csrf.rejected",
        "csrf.missing",
    ]);
    let mut refusals = 0u64;
    for chunk in 1..=good.len() {
        let (avatar, captured) = upload(&csrf, &store, &digest, &good, chunk)
            .unwrap_or_else(|e| panic!("chunks of {chunk}: {e:?}"));
        assert_eq!(avatar.title, "Me");
        assert_eq!(avatar.avatar.read_to_vec(1 << 10).unwrap(), png);
        assert_eq!(captured.as_deref(), Some(token.as_str()));
        drop(avatar);

        for (body, code) in [(&gif, "customs.not_an_image"), (&forged, "csrf.rejected")] {
            let e = upload(&csrf, &store, &digest, body, chunk).unwrap_err();
            assert_eq!(e.code(), code, "chunks of {chunk}");
            refusals += 1;
            // Neither refusal stored a byte: customs refused on the sniff
            // window, before the sink opened; the token before the part.
            let stats = store.stats();
            assert_eq!(
                (stats.files_on_disk, stats.files_in_memory),
                (0, 0),
                "{code}, chunks of {chunk}"
            );
        }
    }
    assert_eq!(std::fs::read_dir(&d).unwrap().count(), 0);
    assert_eq!(store.stats().files_spilled, good.len() as u64);
    assert_eq!(digest.count("customs.not_an_image"), refusals / 2);
    assert_eq!(digest.count("csrf.rejected"), refusals / 2);
    assert_eq!(digest.total(), refusals);
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn test_alpha_upload_route_refuses_what_customs_and_the_token_refuse() {
    let csrf = Csrf::new(KEY).unwrap();
    let token = csrf.issue(SESSION).unwrap();
    let d = dir("refuse");
    let store = Store::in_dir(&d).unwrap().memory_threshold(0);
    let digest = RefusalDigest::with_codes([
        CUSTOMS[0],
        CUSTOMS[1],
        CUSTOMS[2],
        "csrf.rejected",
        "csrf.missing",
    ]);
    let png = b"\x89PNG\r\n\x1a\nrest";

    // A file before the token: the policy refuses it before customs or the
    // check sees the part.
    let mut late = Vec::new();
    late.extend_from_slice(
        b"--X\r\nContent-Disposition: form-data; name=avatar; filename=a.png\r\n\r\n",
    );
    late.extend_from_slice(png);
    late.extend_from_slice(
        format!(
            "\r\n--X\r\nContent-Disposition: form-data; name=csrf_token\r\n\r\n{}\r\n--X--",
            token.as_str()
        )
        .as_bytes(),
    );
    let mut extra = avatar_body(token.as_str(), png);
    extra.truncate(extra.len() - 5);
    extra.extend_from_slice(b"--X\r\nContent-Disposition: form-data; name=admin\r\n\r\n1\r\n--X--");
    let mut two = avatar_body(token.as_str(), png);
    two.truncate(two.len() - 5);
    two.extend_from_slice(
        b"--X\r\nContent-Disposition: form-data; name=avatar; filename=b.png\r\n\r\n",
    );
    two.extend_from_slice(png);
    two.extend_from_slice(b"\r\n--X--");

    for (body, code) in [
        (&late, "policy.token_field_late"),
        (&extra, "customs.unexpected_field"),
        (&two, "limit.files"),
    ] {
        let e = upload(&csrf, &store, &digest, body, 7).unwrap_err();
        assert_eq!(e.code(), code);
        assert_eq!(digest.count(code), 1);
    }
    assert_eq!(store.stats().files_on_disk, 0);
    assert_eq!(std::fs::read_dir(&d).unwrap().count(), 0);
    let _ = std::fs::remove_dir_all(&d);
}
