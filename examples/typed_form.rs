//! A typed form: declare the struct once, get strict extraction from either
//! encoding.
//!
//! Run with `cargo run --example typed_form`.

use form_net::{FormError, FromForm, Secret};

form_net::form! {
    /// The sign-up form.
    #[derive(Debug)]
    pub struct SignUp {
        pub email: String,
        pub password: Secret,
        pub age: Option<u8>,
        pub newsletter: bool,
        pub interests: Vec<String> = "interests[]",
    }
    ignore = ["csrf_token"];
}

fn main() -> Result<(), FormError> {
    let body = b"email=ada%40example.com&password=hunter2&age=36&newsletter=on&interests[]=math&interests[]=engines&csrf_token=abc";
    let form = SignUp::from_body(b"application/x-www-form-urlencoded", body)?;
    // Debug never shows the password.
    println!("{form:?}");
    println!("password length: {}", form.password.expose().len());

    // Unknown fields, repeats, and bad values are refused, never ignored.
    for bad in [
        &b"email=a&password=b&role=admin"[..],
        b"email=a&email=b&password=c",
        b"email=a&password=b&age=300",
    ] {
        let e = SignUp::from_urlencoded(bad).unwrap_err();
        println!(
            "{} -> {} ({})",
            String::from_utf8_lossy(bad),
            e.code(),
            e.status_hint()
        );
    }
    Ok(())
}
