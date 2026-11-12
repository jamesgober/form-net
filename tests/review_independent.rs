//! The independent review of 0.9.2: one test per finding (D-149 to D-161;
//! `persist`, F-4 and F-7, is in `tests/store.rs`). Each test failed on
//! 0.10.0, the release it was written against.

#![cfg(feature = "fs")]

use form_net::{FileName, Limits, Policy};

const CT: &[u8] = b"multipart/form-data; boundary=X";

fn file_part(filename: &str) -> Vec<u8> {
    format!("--X\r\nContent-Disposition: form-data; name=\"f\"; filename=\"{filename}\"\r\n\r\nx\r\n--X--\r\n")
        .into_bytes()
}

/// `(extension, sanitized)` for a file name, as an upload route sees it.
fn checked(filename: &str) -> (Option<String>, String) {
    let limits = Limits::DEFAULT.with_max_filename_bytes(1024);
    let body = file_part(filename);
    let mp = form_net::parse_multipart_with(CT, &body, &limits, &Policy::DEFAULT).unwrap();
    let name: FileName<'_> = mp.file("f").unwrap().filename().unwrap();
    (
        name.extension().map(str::to_owned),
        name.sanitized().unwrap(),
    )
}

/// F-1 (medium): `sanitized` cut long names at 255 bytes from the end, so
/// the extension `extension` reported was not the one saved: `A…A.php.jpg`
/// passed a `jpg` check and was saved as `A…A.php`. Now the stem is cut and
/// the extension kept, for every length.
#[test]
fn test_sanitized_keeps_the_extension_that_extension_reports() {
    for (stem, ext) in [
        ("A".repeat(251) + ".php", "jpg"),
        ("B".repeat(254), "png"),
        ("é".repeat(300), "pdf"),
    ] {
        let name = format!("{stem}.{ext}");
        let (seen, saved) = checked(&name);
        assert_eq!(seen.as_deref(), Some(ext), "{} bytes", name.len());
        assert!(saved.len() <= 255, "{} bytes saved", saved.len());
        assert_eq!(saved.rsplit('.').next(), Some(ext), "saved as {saved}");
        assert!(!saved.contains(".php.") || saved.ends_with(".jpg"));
    }
    // Every length from 240 to 300: the two always agree.
    for n in 240..300 {
        let name = format!("{}.jpg", "C".repeat(n));
        let (seen, saved) = checked(&name);
        assert_eq!(seen.as_deref(), Some("jpg"));
        assert!(
            saved.len() <= 255 && saved.ends_with(".jpg"),
            "{n}: {saved}"
        );
    }
}

/// F-8: a dot file passed through `sanitized`, and `extension` of
/// `.htaccess` was `htaccess`. A leading dot becomes `_`, and a name whose
/// only dot is its first byte has no extension.
#[test]
fn test_dot_files_are_not_kept_and_have_no_extension() {
    for name in [".htaccess", ".user.ini", ".env"] {
        let (ext, saved) = checked(name);
        assert!(!saved.starts_with('.'), "{name} saved as {saved}");
        if name == ".user.ini" {
            assert_eq!(ext.as_deref(), Some("ini"));
            assert!(saved.ends_with(".ini"));
        } else {
            assert_eq!(ext, None, "{name}");
        }
    }
    assert_eq!(checked("a.tar.gz").0.as_deref(), Some("gz"));
}

/// F-3: the D-136 rule refused any parameter value holding `name=`, so a
/// browser upload of `Surname=Smith.pdf` failed the whole request. Only a
/// `name=` or `filename=` that a splitting reader would take for a new
/// parameter is refused: at the start of the value, or after `;`, `,`, or
/// whitespace (multer 3.1 misreads exactly those, checked by running it).
#[test]
fn test_ordinary_file_names_holding_name_equals_are_accepted() {
    for name in [
        "Surname=Smith.pdf",
        "username=bob.csv",
        "rename=me.txt",
        "Xfilename=x.txt",
    ] {
        let body = file_part(name);
        let mp = form_net::parse_multipart(CT, &body).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!(
            mp.file("f").unwrap().filename().unwrap().raw(),
            name.as_bytes()
        );
    }
    for name in [
        "a name=x.txt",
        "a;name=x.txt",
        "a,name=x.txt",
        "a\tfilename=x.txt",
        "name=x.txt",
        "a; FILENAME=x",
    ] {
        let body = file_part(name);
        assert_eq!(
            form_net::parse_multipart(CT, &body)
                .map(|_| ())
                .map_err(|e| e.code()),
            Err("syntax.malformed_parameter"),
            "{name:?}"
        );
    }
}

/// F-2: a part labelled `us-ascii` was read as UTF-8, while WHATWG readers
/// decode the label as windows-1252. Under that label every byte must be
/// ASCII, which all three read the same.
#[test]
fn test_us_ascii_text_must_be_ascii() {
    let labelled = |value: &[u8]| {
        let mut b = b"--X\r\nContent-Disposition: form-data; name=\"a\"\r\nContent-Type: text/plain; charset=us-ascii\r\n\r\n".to_vec();
        b.extend_from_slice(value);
        b.extend_from_slice(b"\r\n--X--\r\n");
        b
    };
    assert_eq!(
        form_net::parse_multipart(CT, &labelled(b"rene"))
            .unwrap()
            .field("a"),
        Some("rene")
    );
    assert_eq!(
        form_net::parse_multipart(CT, &labelled(b"ren\xc3\xa9"))
            .map(|_| ())
            .map_err(|e| e.code()),
        Err("encoding.unsupported_charset")
    );
    let declared = b"--X\r\nContent-Disposition: form-data; name=\"_charset_\"\r\n\r\nUS-ASCII\r\n\
--X\r\nContent-Disposition: form-data; name=\"a\"\r\n\r\nren\xc3\xa9\r\n--X--\r\n";
    assert_eq!(
        form_net::parse_multipart(CT, declared)
            .map(|_| ())
            .map_err(|e| e.code()),
        Err("encoding.unsupported_charset")
    );
    // The streaming path reads it the same way.
    let store = form_net::Store::memory_only();
    let r = form_net::read_multipart_with(
        CT,
        &declared[..],
        &Limits::DEFAULT,
        &Policy::DEFAULT,
        &store,
    );
    assert_eq!(
        r.map(|_| ()).map_err(|e| e.code()),
        Err("encoding.unsupported_charset")
    );
}

/// F-6: crash leftovers under `Store::temp` sit in a per-process
/// `form-net-<32 hex>` directory, which `sweep` of the temporary directory
/// never entered. It now goes one level into such directories (and no
/// further, and never through a link), removing old spill files; the
/// directory stays, since a live store may own it.
#[test]
fn test_sweep_reaches_private_directories_one_level_down() {
    let root = std::env::temp_dir().join(format!("form-net-review-sweep-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let private = root.join("form-net-0123456789abcdef0123456789abcdef");
    let deeper = private.join("form-net-fedcba9876543210fedcba9876543210");
    let other = root.join("not-form-net");
    for d in [&deeper, &other] {
        std::fs::create_dir_all(d).unwrap();
    }
    let spill = "upload-0123456789abcdef0123456789abcdef.part";
    for d in [&root, &private, &deeper, &other] {
        std::fs::write(d.join(spill), b"left").unwrap();
    }
    std::fs::write(private.join("notes.txt"), b"mine").unwrap();
    let removed = form_net::sweep(&root, std::time::Duration::ZERO).unwrap();
    assert_eq!(removed, 2, "the top level and the private directory");
    assert!(!root.join(spill).exists() && !private.join(spill).exists());
    assert!(deeper.join(spill).exists(), "one level only");
    assert!(other.join(spill).exists(), "only form-net's directories");
    assert!(private.join("notes.txt").exists() && private.is_dir());
    std::fs::remove_dir_all(&root).unwrap();
}

/// F-11: a client that disconnects mid-upload made `FormError::Io`, a 500,
/// the same as a failing disk. Reading the body has its own error, a 400.
#[test]
fn test_a_failing_body_reader_is_a_client_error() {
    struct Broken(usize);
    impl std::io::Read for Broken {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            let head = b"--X\r\nContent-Disposition: form-data; name=a\r\n\r\nxx";
            if self.0 == 0 {
                self.0 = 1;
                buf[..head.len()].copy_from_slice(head);
                return Ok(head.len());
            }
            Err(std::io::ErrorKind::ConnectionReset.into())
        }
    }
    let e = form_net::read_multipart(CT, Broken(0)).unwrap_err();
    assert_eq!((e.code(), e.status_hint()), ("body_read", 400));
    let mut reader = form_net::MultipartReader::new(CT, Broken(0)).unwrap();
    let e = (|| -> Result<(), form_net::FormError> {
        while let Some(mut part) = reader.next_part()? {
            part.skip()?;
        }
        Ok(())
    })()
    .unwrap_err();
    assert_eq!((e.code(), e.status_hint()), ("body_read", 400));
}
