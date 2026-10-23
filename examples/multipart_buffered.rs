//! Reading a multipart body that is already in memory.
//!
//! Run with `cargo run --example multipart_buffered`.

use form_net::FormError;

fn main() -> Result<(), FormError> {
    let content_type = b"multipart/form-data; boundary=----WebKitFormBoundary7MA4YWxkTrZu0gW";
    let body = b"------WebKitFormBoundary7MA4YWxkTrZu0gW\r\n\
Content-Disposition: form-data; name=\"title\"\r\n\r\n\
Holiday photos\r\n\
------WebKitFormBoundary7MA4YWxkTrZu0gW\r\n\
Content-Disposition: form-data; name=\"photo\"; filename=\"C:\\Users\\me\\beach.png\"\r\n\
Content-Type: image/png\r\n\r\n\
\x89PNG\r\n\x1a\n...\r\n\
------WebKitFormBoundary7MA4YWxkTrZu0gW--\r\n";

    let mp = form_net::parse_multipart(content_type, body)?;
    println!("title: {:?}", mp.field("title"));
    if let Some(photo) = mp.file("photo") {
        let name = photo.filename().expect("a file part has a file name");
        // The raw name is the client's; form-net never uses it as a path.
        println!("raw name: {:?}", String::from_utf8_lossy(name.raw()));
        println!("basename: {:?}", String::from_utf8_lossy(name.basename()));
        // The declared type is a hint; the bytes tell the truth.
        println!("declared: {:?}", photo.content_type().map(|t| t.raw()));
        println!("bytes:    {}", photo.len());
    }
    Ok(())
}
