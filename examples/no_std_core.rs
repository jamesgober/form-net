//! The allocation-free core: a stack buffer sized at compile time, borrowed
//! pairs, and decoding into a caller buffer. The same code builds on a
//! target without an allocator.
//!
//! Run with `cargo run --example no_std_core`.

use form_net::{Boundary, Event, FormError, Limits, MultipartParser, PercentPolicy, Policy};

const LIMITS: Limits = Limits::DEFAULT
    .with_max_part_header_bytes(512)
    .with_max_name_bytes(64);
const BUFFER_LEN: usize = LIMITS.multipart_buffer_len(&Policy::DEFAULT);

fn main() -> Result<(), FormError> {
    // urlencoded without allocating: borrowed when possible, decoded into a
    // stack buffer otherwise.
    let mut scratch = [0u8; 128];
    for pair in form_net::raw_pairs(b"city=Z%C3%BCrich&zip=8001", &LIMITS) {
        let pair = pair?;
        let name = pair.name_str().unwrap_or("?");
        let value = match pair.value_str() {
            Some(v) => v,
            None => pair.decode_value(&mut scratch, PercentPolicy::Refuse)?,
        };
        println!("{name} = {value}");
    }

    // multipart over a stack buffer.
    let mut buffer = [0u8; BUFFER_LEN];
    let mut parser = MultipartParser::with_buffer(
        Boundary::new(b"X")?,
        &LIMITS,
        &Policy::DEFAULT,
        &mut buffer[..],
    )?;
    let mut rest: &[u8] = b"--X\r\nContent-Disposition: form-data; name=\"a\"\r\n\r\n42\r\n--X--";
    loop {
        let step = parser.parse(rest, true)?;
        rest = &rest[step.consumed..];
        match step.event {
            Event::PartStart(head) => println!("part {}", head.name()),
            Event::Data(bytes) => println!("  data {bytes:?}"),
            Event::End => break,
            _ => {}
        }
    }
    println!("buffer: {BUFFER_LEN} bytes on the stack");
    Ok(())
}
