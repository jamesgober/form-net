//! Driving the streaming parser by hand, as a server's read loop would:
//! chunks of any size, events as parts complete.
//!
//! Run with `cargo run --example multipart_sans_io`.

use form_net::{Event, FormError, FormKind, Limits, MultipartParser, Policy};

fn main() -> Result<(), FormError> {
    let content_type = b"multipart/form-data; boundary=X";
    let body = b"--X\r\n\
Content-Disposition: form-data; name=\"note\"\r\n\r\n\
hello\r\n\
--X\r\n\
Content-Disposition: form-data; name=\"upload\"; filename=\"data.bin\"\r\n\r\n\
0123456789abcdef0123456789abcdef\r\n\
--X--\r\n";

    let FormKind::Multipart(boundary) = FormKind::detect(content_type)? else {
        return Err(FormError::UnsupportedMediaType);
    };
    let mut parser = MultipartParser::new(boundary, &Limits::DEFAULT, &Policy::DEFAULT)?;

    // Pretend the network delivers 9 bytes at a time.
    let mut chunks = body.chunks(9).peekable();
    while let Some(chunk) = chunks.next() {
        let last = chunks.peek().is_none();
        let mut rest = chunk;
        loop {
            let step = parser.parse(rest, last)?;
            rest = &rest[step.consumed..];
            match step.event {
                Event::PartStart(head) => println!(
                    "part {} {:?} ({:?})",
                    head.index(),
                    head.name(),
                    head.kind()
                ),
                Event::Data(bytes) => println!("  {} bytes", bytes.len()),
                Event::PartEnd(summary) => println!("  end: {} bytes in total", summary.bytes),
                Event::End => {
                    println!("done");
                    break;
                }
                Event::NeedInput => break,
                _ => {}
            }
        }
    }
    Ok(())
}
