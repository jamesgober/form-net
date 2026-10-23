//! Reading a urlencoded form body, with the default limits and with a
//! route's own.
//!
//! Run with `cargo run --example urlencoded_basic`.

use form_net::{FormError, Limits, Policy};

fn main() -> Result<(), FormError> {
    // What a browser sends for a login form.
    let body = b"email=ada%40example.com&password=s3cret%21&remember=on";

    let form = form_net::parse_urlencoded(body)?;
    println!("email:    {:?}", form.get("email"));
    println!("remember: {}", form.contains("remember"));
    // `Debug` never prints values: safe to log.
    println!("debug:    {form:?}");

    // A route with tighter limits.
    const LOGIN: Limits = Limits::DEFAULT
        .with_max_body_bytes(4 * 1024)
        .with_max_fields(8);
    let form = form_net::parse_urlencoded_with(body, &LOGIN, &Policy::DEFAULT)?;
    println!("fields:   {}", form.len());

    // A repeated name, refused by the strict accessor.
    let polluted = form_net::parse_urlencoded(b"role=user&role=admin")?;
    match polluted.get_one("role") {
        Ok(v) => println!("role: {v:?}"),
        Err(e) => println!("refused: {e} (status {})", e.status_hint()),
    }

    // Malformed input is refused, never repaired.
    if let Err(e) = form_net::parse_urlencoded(b"name=%zz") {
        println!("refused: {e} (code {})", e.code());
    }
    Ok(())
}
