//! Abort at every byte (FR-22, FR-50, T-8): a body cut anywhere, or refused
//! anywhere, leaves no file and no open handle behind.

#![cfg(feature = "fs")]

use std::path::PathBuf;

use form_net::{
    FormError, Inspector, Limits, NoInspector, PartSummary, Policy, Refusal, SpillMode, Store,
    UploadSession, Verdict,
};

const CT: &[u8] = b"multipart/form-data; boundary=X";

fn body() -> Vec<u8> {
    let mut out = Vec::new();
    for (i, size) in [40usize, 3, 70].iter().enumerate() {
        out.extend_from_slice(
            format!("--X\r\nContent-Disposition: form-data; name=f{i}; filename=f{i}.bin\r\n\r\n")
                .as_bytes(),
        );
        out.extend(std::iter::repeat_n(b'a' + i as u8, *size));
        out.extend_from_slice(b"\r\n");
    }
    out.extend_from_slice(b"--X--");
    out
}

fn dir(tag: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!(
        "form-net-abort-{tag}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&p).unwrap();
    p
}

#[cfg(target_os = "linux")]
fn open_handles() -> usize {
    std::fs::read_dir("/proc/self/fd").unwrap().count()
}

#[test]
fn test_truncated_at_every_byte_leaves_nothing() {
    let b = body();
    for mode in [SpillMode::Named, SpillMode::Anonymous] {
        let d = dir("cut");
        let store = Store::in_dir(&d).unwrap().memory_threshold(8).mode(mode);
        #[cfg(target_os = "linux")]
        let before = open_handles();
        for cut in 0..b.len() {
            let mut s = UploadSession::new(CT, &Limits::DEFAULT, &Policy::DEFAULT, &store).unwrap();
            let r = s
                .push(&b[..cut], &mut NoInspector)
                .and_then(|()| s.finish(&mut NoInspector).map(drop));
            assert!(r.is_err(), "cut at {cut} must fail");
            assert_eq!(
                std::fs::read_dir(&d).unwrap().count(),
                0,
                "{mode:?}: files left after a cut at {cut}"
            );
            assert_eq!(store.stats().files_on_disk, 0);
        }
        #[cfg(target_os = "linux")]
        assert_eq!(open_handles(), before, "{mode:?}: handles leaked");
        let _ = std::fs::remove_dir_all(&d);
    }
}

struct RefuseAt(u32);

impl Inspector for RefuseAt {
    fn part_end(&mut self, summary: &PartSummary) -> Verdict {
        if summary.index == self.0 {
            Verdict::Refuse(Refusal::new(422, "test.refused"))
        } else {
            Verdict::Continue
        }
    }
}

struct RefuseEnd;
impl Inspector for RefuseEnd {
    fn end(&mut self) -> Verdict {
        Verdict::Refuse(Refusal::new(422, "test.refused"))
    }
}

#[test]
fn test_refusal_after_files_were_stored_deletes_them() {
    let b = body();
    let d = dir("refuse");
    let store = Store::in_dir(&d)
        .unwrap()
        .memory_threshold(0)
        .mode(SpillMode::Named);
    for index in 0..3 {
        let mut s = UploadSession::new(CT, &Limits::DEFAULT, &Policy::DEFAULT, &store).unwrap();
        let r = s.push(&b, &mut RefuseAt(index));
        assert!(matches!(r, Err(FormError::Refused { .. })));
        assert_eq!(
            std::fs::read_dir(&d).unwrap().count(),
            0,
            "refusal at part {index}"
        );
    }
    let mut s = UploadSession::new(CT, &Limits::DEFAULT, &Policy::DEFAULT, &store).unwrap();
    let r = s.push(&b, &mut RefuseEnd);
    assert!(r.is_err());
    drop(s);
    assert_eq!(std::fs::read_dir(&d).unwrap().count(), 0);
    let _ = std::fs::remove_dir_all(&d);
}
