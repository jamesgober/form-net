//! Requiring the anti-forgery token before the first file, so a forged
//! request is refused before anything is written to disk.
//!
//! Run with `cargo run --example token_first`.

use form_net::{FormError, Limits, NoInspector, Policy, Store, TokenField, UploadSession};

/// Stands in for csrf-sec, which compares in constant time.
fn token_is_valid(token: &str) -> bool {
    token == "expected-token"
}

fn main() -> Result<(), FormError> {
    let store = Store::temp()?;
    let policy = Policy::DEFAULT.with_token_field(TokenField::new("csrf_token"));
    let content_type = b"multipart/form-data; boundary=X";

    let head = b"--X\r\nContent-Disposition: form-data; name=\"csrf_token\"\r\n\r\nexpected-token\r\n--X\r\nContent-Disposition: form-data; name=\"f\"; filename=\"a.bin\"\r\n\r\n";
    let mut session = UploadSession::new(content_type, &Limits::DEFAULT, &policy, &store)?;
    session.push(head, &mut NoInspector)?;
    // The token is known as soon as its part ends: check it before reading
    // the rest of the body.
    match session.captured_token() {
        Some(t) if token_is_valid(t) => println!("token ok, reading the files"),
        _ => {
            println!("forged request: dropped before any file byte");
            return Ok(());
        }
    }
    session.push(b"file bytes\r\n--X--", &mut NoInspector)?;
    let upload = session.finish(&mut NoInspector)?;
    println!("{} file(s)", upload.file_count());

    // A token after the file is refused at the file's first header.
    let late = b"--X\r\nContent-Disposition: form-data; name=\"f\"; filename=\"a\"\r\n\r\nx\r\n--X\r\nContent-Disposition: form-data; name=\"csrf_token\"\r\n\r\nt\r\n--X--";
    let mut session = UploadSession::new(content_type, &Limits::DEFAULT, &policy, &store)?;
    let e = session.push(late, &mut NoInspector).unwrap_err();
    println!("late token: {}", e.code());
    Ok(())
}
