//! Building a `multipart/form-data` request body for an outbound client,
//! and the encoder for code that writes into its own buffers.
//!
//! Run with `cargo run --example build_multipart_client`.

use form_net::{Boundary, FormError, MultipartWriter};

fn main() -> Result<(), FormError> {
    // A generated boundary (128 random bits) and a body in memory; any
    // `std::io::Write` works, a socket or a file included.
    let mut body = Vec::new();
    let mut w = MultipartWriter::new(&mut body)?;
    w.text("title", "Quarterly report")?;
    w.text("notes", "line one\nline two")?; // sent as CRLF, as browsers do
    w.file(
        "report",
        "Q3 \"final\".pdf",
        Some("application/pdf"),
        &b"%PDF-1.7 ..."[..],
    )?;
    let content_type = w.content_type();
    w.finish()?;
    println!("Content-Type: {content_type}");
    println!("{} bytes", body.len());

    // Whatever the writer produced, form-net reads back as the same fields.
    let mp = form_net::parse_multipart(content_type.as_bytes(), &body)?;
    let report = mp.file("report").expect("written above");
    println!(
        "report: {:?}",
        report.filename().map(|f| f.html_unescaped())
    );

    // Inputs that could read back differently are refused before any byte
    // of them is written.
    let mut w = MultipartWriter::new(Vec::new())?;
    let e = w.text("evil\0name", "x").unwrap_err();
    println!("refused: {}", e.code());

    // The allocation-free encoder, for a caller's own buffer.
    let boundary = Boundary::new(b"X")?;
    let mut head = [0u8; 128];
    let n = form_net::encode_part_head(
        &boundary,
        true,
        "a",
        Some("b.txt"),
        Some("text/plain"),
        &mut head,
    )?;
    println!("{}", String::from_utf8_lossy(&head[..n]).escape_debug());
    Ok(())
}
