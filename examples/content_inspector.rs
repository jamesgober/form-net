//! An inspector that refuses uploads whose bytes do not match an allowed
//! type, before any byte is stored: the shape customs-sec plugs in.
//!
//! Run with `cargo run --example content_inspector`.

use form_net::{FormError, Inspector, Limits, PartHead, PartSummary, Policy, Refusal, Verdict};

/// Allows PNG and JPEG by their first bytes, whatever the client declared.
struct ImagesOnly {
    parts: u32,
}

impl Inspector for ImagesOnly {
    fn sniff(&mut self, _head: &PartHead<'_>, first: &[u8]) -> Verdict {
        let png = first.starts_with(b"\x89PNG\r\n\x1a\n");
        let jpeg = first.starts_with(b"\xff\xd8\xff");
        if png || jpeg {
            Verdict::Continue
        } else {
            Verdict::Refuse(Refusal::new(415, "upload.not_an_image"))
        }
    }

    fn part_end(&mut self, _summary: &PartSummary) -> Verdict {
        self.parts += 1;
        Verdict::Continue
    }

    fn failed(&mut self, error: &FormError) {
        // Errors carry codes and positions, never the client's bytes.
        println!("refused: {} ({})", error.code(), error.status_hint());
    }
}

fn main() {
    let content_type = b"multipart/form-data; boundary=X";
    let good = b"--X\r\nContent-Disposition: form-data; name=a; filename=a.png\r\nContent-Type: image/png\r\n\r\n\x89PNG\r\n\x1a\n....\r\n--X--";
    // Declared as a PNG, but it is HTML.
    let bad = b"--X\r\nContent-Disposition: form-data; name=a; filename=a.png\r\nContent-Type: image/png\r\n\r\n<html><script>alert(1)</script>\r\n--X--";
    let limits = Limits::DEFAULT.with_sniff_bytes(16);
    for body in [&good[..], &bad[..]] {
        let mut inspector = ImagesOnly { parts: 0 };
        let r = form_net::parse_multipart_inspected(
            content_type,
            body,
            &limits,
            &Policy::DEFAULT,
            &mut inspector,
        );
        println!("ok: {}, parts inspected: {}", r.is_ok(), inspector.parts);
    }
}
