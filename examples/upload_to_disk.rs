//! Reading an upload from any `Read`, with small files kept in memory and
//! large ones spilled to private temporary files, then keeping one.
//!
//! Run with `cargo run --example upload_to_disk`.

use form_net::FormError;

fn main() -> Result<(), FormError> {
    let content_type = b"multipart/form-data; boundary=X";
    let mut body = Vec::new();
    body.extend_from_slice(
        b"--X\r\nContent-Disposition: form-data; name=\"title\"\r\n\r\nQ3 report\r\n",
    );
    body.extend_from_slice(b"--X\r\nContent-Disposition: form-data; name=\"doc\"; filename=\"../../report.pdf\"\r\nContent-Type: application/pdf\r\n\r\n");
    body.extend(std::iter::repeat_n(b'%', 200 * 1024));
    body.extend_from_slice(b"\r\n--X--");

    // In a server, `body` is the request body stream.
    let mut upload = form_net::read_multipart(content_type, &body[..])?;
    println!("title: {:?}", upload.field("title"));

    let doc = upload.take_file("doc").expect("the form has a doc");
    println!("{} bytes, in memory: {}", doc.len(), doc.is_in_memory());
    // The client's name is only a hint: the sanitized one is a single
    // segment, and it is used here under a directory the server chose.
    let shown = doc.filename().map(|n| n.sanitized()).transpose()?;
    println!("client name, sanitized: {shown:?}");

    let dest = std::env::temp_dir().join(format!("form-net-example-{}.pdf", std::process::id()));
    // persist never overwrites: an existing file is an error.
    doc.persist(&dest)?;
    println!("kept at {}", dest.display());
    let _ = std::fs::remove_file(&dest);
    Ok(())
}
