//! Upload storage on every platform (FR-45 to FR-51, T-7 to T-10, T-20):
//! memory and spill thresholds, private files, anonymous and named modes,
//! persist without overwrite, disk limits, cleanup.

#![cfg(feature = "fs")]

use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use form_net::{
    FormError, Limit, Limits, NoInspector, Policy, SpillMode, Store, StoreError, Upload,
    UploadSession,
};

const CT: &[u8] = b"multipart/form-data; boundary=X";

/// A private directory per test, removed at the end.
struct Dir(PathBuf);

impl Dir {
    fn new(tag: &str) -> Dir {
        let mut name = [0u8; 8];
        for (i, b) in name.iter_mut().enumerate() {
            *b = b'a' + ((std::process::id() as usize + i * 7 + tag.len()) % 26) as u8;
        }
        let p = std::env::temp_dir().join(format!(
            "form-net-test-{tag}-{}-{}",
            std::str::from_utf8(&name).unwrap(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&p).unwrap();
        Dir(p)
    }

    fn entries(&self) -> Vec<String> {
        let mut v: Vec<String> = std::fs::read_dir(&self.0)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        v.sort();
        v
    }
}

impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn body(files: &[(&str, &[u8])]) -> Vec<u8> {
    let mut out = Vec::new();
    for (name, data) in files {
        out.extend_from_slice(
            format!("--X\r\nContent-Disposition: form-data; name=\"{name}\"; filename=\"../../{name}.bin\"\r\n\r\n").as_bytes(),
        );
        out.extend_from_slice(data);
        out.extend_from_slice(b"\r\n");
    }
    out.extend_from_slice(b"--X--");
    out
}

fn upload(store: &Store, b: &[u8]) -> Result<Upload, FormError> {
    Upload::parse_with(
        CT,
        b,
        &Limits::DEFAULT,
        &Policy::DEFAULT,
        store,
        &mut NoInspector,
    )
}

#[test]
fn test_memory_only_never_touches_the_file_system() {
    let store = Store::memory_only();
    let big = vec![7u8; 200 * 1024];
    let up = upload(&store, &body(&[("f", &big)])).unwrap();
    let f = up.file("f").unwrap();
    assert!(f.is_in_memory());
    assert_eq!(f.bytes().map(<[u8]>::len), Some(big.len()));
    assert!(store.dir().is_none());
    assert_eq!(store.stats().files_spilled, 0);
}

#[test]
fn test_threshold_spills_and_reads_back() {
    let dir = Dir::new("threshold");
    let store = Store::in_dir(&dir.0)
        .unwrap()
        .memory_threshold(16)
        .mode(SpillMode::Named);
    let at = vec![1u8; 16];
    let over = vec![2u8; 17];
    let up = upload(&store, &body(&[("at", &at), ("over", &over)])).unwrap();
    assert!(up.file("at").unwrap().is_in_memory());
    let f = up.file("over").unwrap();
    assert!(!f.is_in_memory());
    assert_eq!(f.read_to_vec(64).unwrap(), over);
    assert!(f.read_to_vec(16).is_err());
    // Two readers at once, each with its own position.
    let mut r1 = f.reader().unwrap();
    let mut r2 = f.reader().unwrap();
    let mut a = [0u8; 5];
    let mut b = [0u8; 17];
    r1.read_exact(&mut a).unwrap();
    r2.read_exact(&mut b).unwrap();
    assert_eq!((a, b.as_slice()), ([2u8; 5], over.as_slice()));
    assert_eq!(r1.seek(SeekFrom::End(-1)).unwrap(), 16);
    // Named spill files carry no client bytes in their names.
    let names = dir.entries();
    assert_eq!(names.len(), 1);
    assert!(
        names[0].starts_with("upload-")
            && names[0].ends_with(".part")
            && names[0].len() == 7 + 32 + 5,
        "{names:?}"
    );
    assert_eq!(store.stats().files_on_disk, 1);
    drop(up);
    assert!(dir.entries().is_empty(), "dropped files are deleted");
    assert_eq!(store.stats().files_on_disk, 0);
}

#[test]
fn test_anonymous_files_have_no_name_on_unix() {
    let dir = Dir::new("anon");
    let store = Store::in_dir(&dir.0).unwrap().memory_threshold(0);
    let up = upload(&store, &body(&[("f", b"secret bytes")])).unwrap();
    let f = up.file("f").unwrap();
    assert!(!f.is_in_memory() && f.path().is_none());
    assert_eq!(f.read_to_vec(64).unwrap(), b"secret bytes");
    if cfg!(unix) {
        assert!(dir.entries().is_empty(), "unlinked right after creation");
    }
    drop(up);
    assert!(dir.entries().is_empty(), "nothing left after drop");
}

#[cfg(unix)]
#[test]
fn test_permissions_are_owner_only() {
    use std::os::unix::fs::PermissionsExt;
    let store = Store::temp()
        .unwrap()
        .memory_threshold(0)
        .mode(SpillMode::Named);
    let mode = std::fs::metadata(store.dir().unwrap())
        .unwrap()
        .permissions()
        .mode()
        & 0o777;
    assert_eq!(mode, 0o700);
    let up = upload(&store, &body(&[("f", b"x")])).unwrap();
    let path = up.file("f").unwrap().path().unwrap().to_owned();
    let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o600);
}

#[test]
fn test_temp_store_directory_is_private_and_removed() {
    let store = Store::temp().unwrap();
    let dir = store.dir().unwrap().to_owned();
    assert!(dir.starts_with(std::env::temp_dir()));
    let name = dir.file_name().unwrap().to_string_lossy().into_owned();
    assert!(name.starts_with("form-net-") && name.len() == 9 + 32);
    drop(store);
    assert!(
        !dir.exists(),
        "an empty own directory is removed with the store"
    );
}

#[test]
fn test_persist_never_overwrites() {
    let dir = Dir::new("persist");
    for mode in [SpillMode::Named, SpillMode::Anonymous] {
        for threshold in [0, u64::MAX] {
            let store = Store::in_dir(&dir.0)
                .unwrap()
                .memory_threshold(threshold)
                .mode(mode);
            let mut up = upload(&store, &body(&[("f", b"payload")])).unwrap();
            let dest = dir.0.join(format!("kept-{mode:?}-{threshold}.bin"));
            up.take_file("f").unwrap().persist(&dest).unwrap();
            assert_eq!(std::fs::read(&dest).unwrap(), b"payload");
            let mut up = upload(&store, &body(&[("f", b"other")])).unwrap();
            let e = up.take_file("f").unwrap().persist(&dest).unwrap_err();
            assert_eq!(
                e,
                FormError::Store {
                    reason: StoreError::DestinationExists
                }
            );
            assert_eq!(std::fs::read(&dest).unwrap(), b"payload", "not overwritten");
            drop(up);
        }
    }
    let leftovers: Vec<String> = dir
        .entries()
        .into_iter()
        .filter(|n| n.ends_with(".part"))
        .collect();
    assert!(leftovers.is_empty(), "{leftovers:?}");
}

#[test]
fn test_disk_limit_per_request() {
    let dir = Dir::new("disk");
    let store = Store::in_dir(&dir.0)
        .unwrap()
        .memory_threshold(0)
        .max_disk_bytes(10);
    let e = upload(&store, &body(&[("a", b"123456"), ("b", b"78901")])).unwrap_err();
    assert!(
        matches!(
            e,
            FormError::Limit {
                limit: Limit::Disk,
                ..
            }
        ),
        "{e:?}"
    );
    assert_eq!(e.status_hint(), 413);
    assert!(dir.entries().is_empty(), "a refused request leaves nothing");
    assert!(upload(&store, &body(&[("a", b"123456"), ("b", b"7890")])).is_ok());
}

#[test]
fn test_in_dir_refuses_files_and_links() {
    let dir = Dir::new("indir");
    let file = dir.0.join("plain");
    std::fs::write(&file, b"x").unwrap();
    assert!(matches!(
        Store::in_dir(&file),
        Err(FormError::Store {
            reason: StoreError::NotADirectory
        })
    ));
    assert!(Store::in_dir(dir.0.join("missing")).is_err());
    #[cfg(unix)]
    {
        let link = dir.0.join("link");
        std::os::unix::fs::symlink(&dir.0, &link).unwrap();
        assert!(matches!(
            Store::in_dir(&link),
            Err(FormError::Store {
                reason: StoreError::NotADirectory
            })
        ));
    }
}

#[test]
fn test_session_drop_deletes_everything() {
    let dir = Dir::new("session");
    let store = Store::in_dir(&dir.0)
        .unwrap()
        .memory_threshold(0)
        .mode(SpillMode::Named);
    let b = body(&[("a", &[1u8; 100]), ("b", &[2u8; 100])]);
    let mut s = UploadSession::new(CT, &Limits::DEFAULT, &Policy::DEFAULT, &store).unwrap();
    s.push(&b[..b.len() - 5], &mut NoInspector).unwrap();
    assert!(!dir.entries().is_empty());
    drop(s);
    assert!(dir.entries().is_empty());
    assert_eq!(store.stats().files_on_disk, 0);
}

#[test]
fn test_sanitized_names_are_single_safe_segments() {
    let cases = [
        ("../../etc/passwd", "passwd"),
        ("C:\\Windows\\x", "x"),
        ("a.txt:stream", "a.txt_stream"),
        ("CON", "_CON"),
        ("gpj\u{202e}.exe", "gpj_.exe"),
    ];
    for (raw, expected) in cases {
        let b = format!(
            "--X\r\nContent-Disposition: form-data; name=f; filename=\"{raw}\"\r\n\r\n\r\n--X--"
        );
        let mp = form_net::parse_multipart(CT, b.as_bytes()).unwrap();
        let s = mp
            .file("f")
            .unwrap()
            .filename()
            .unwrap()
            .sanitized()
            .unwrap();
        assert_eq!(s, expected, "{raw:?}");
        assert!(!s.contains('/') && !s.contains('\\') && !Path::new(&s).is_absolute());
    }
}

#[test]
fn test_read_multipart_creates_nothing_for_small_uploads() {
    let up = form_net::read_multipart(CT, &body(&[("f", b"small")])[..]).unwrap();
    assert!(up.file("f").unwrap().is_in_memory());
    let big = vec![9u8; 100 * 1024];
    let up = form_net::read_multipart(CT, &body(&[("f", &big)])[..]).unwrap();
    let f = up.file("f").unwrap();
    assert!(!f.is_in_memory());
    assert_eq!(f.read_to_vec(u64::MAX).unwrap(), big);
}

#[test]
fn test_part_reader_save_shares_one_disk_limit_per_request() {
    let dir = Dir::new("save");
    let store = Store::in_dir(&dir.0)
        .unwrap()
        .memory_threshold(0)
        .max_disk_bytes(10);
    let b = body(&[("a", b"123456"), ("b", b"789012")]);
    let mut reader = form_net::MultipartReader::new(CT, &b[..]).unwrap();
    let first = reader.next_part().unwrap().unwrap().save(&store).unwrap();
    let e = reader
        .next_part()
        .unwrap()
        .unwrap()
        .save(&store)
        .unwrap_err();
    assert!(
        matches!(
            e,
            FormError::Limit {
                limit: Limit::Disk,
                ..
            }
        ),
        "{e:?}"
    );
    drop(first);
    assert!(dir.entries().is_empty());
}

#[test]
fn test_session_poisons_after_a_text_check_fails() {
    use form_net::{Inspector, Verdict};
    #[derive(Default)]
    struct Failures(u32);
    impl Inspector for Failures {
        fn failed(&mut self, _: &FormError) {
            self.0 += 1;
        }
        fn end(&mut self) -> Verdict {
            Verdict::Continue
        }
    }
    let store = Store::memory_only();
    let latin1 = b"--X\r\nContent-Disposition: form-data; name=a\r\nContent-Type: text/plain; charset=latin1\r\n\r\nx\r\n--X\r\n";
    let mut s = UploadSession::new(CT, &Limits::DEFAULT, &Policy::DEFAULT, &store).unwrap();
    let mut i = Failures::default();
    assert!(s.push(latin1, &mut i).is_err());
    assert_eq!(
        s.push(
            b"Content-Disposition: form-data; name=f; filename=a\r\n\r\n1\r\n--X--",
            &mut i
        )
        .unwrap_err(),
        FormError::Poisoned
    );
    assert_eq!(s.finish(&mut i).unwrap_err(), FormError::Poisoned);
    assert_eq!(i.0, 1, "failed is called once");
}

#[test]
fn test_stats_stay_right_when_a_spill_fails() {
    let dir = Dir::new("statspill");
    // The threshold is above the disk limit, so the spill itself fails.
    let store = Store::in_dir(&dir.0)
        .unwrap()
        .memory_threshold(8)
        .max_disk_bytes(4);
    let other = Store::in_dir(&dir.0).unwrap();
    assert!(upload(&store, &body(&[("f", &[1u8; 20])])).is_err());
    let s = store.stats();
    assert_eq!((s.files_on_disk, s.bytes_on_disk), (0, 0), "{s:?}");
    drop(other);
    assert!(dir.entries().is_empty());
}
