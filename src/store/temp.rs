//! Random names, the private directory, and the platform open flags for
//! temporary files.

use alloc::format;
use alloc::string::String;
use std::fs::{DirBuilder, File, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};

use crate::error::FormError;

fn name_with(prefix: &str, suffix: &str) -> Result<String, FormError> {
    let hex = crate::random::random_hex()?;
    let hex = core::str::from_utf8(&hex).map_err(|_| FormError::Random)?;
    Ok(format!("{prefix}{hex}{suffix}"))
}

pub(crate) fn io(e: &io::Error) -> FormError {
    FormError::Io { kind: e.kind() }
}

/// Creates `<temp dir>/form-net-<32 hex>`, owner-only on Unix. Fails if the
/// path exists, so a directory someone else made is never adopted.
pub(crate) fn create_private_dir() -> Result<PathBuf, FormError> {
    let path = std::env::temp_dir().join(name_with("form-net-", "")?);
    #[cfg_attr(not(unix), allow(unused_mut))]
    let mut b = DirBuilder::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        b.mode(0o700);
    }
    b.create(&path).map_err(|e| io(&e))?;
    Ok(path)
}

/// Checks that `dir` is a directory and not a link to one.
pub(crate) fn check_dir(dir: &Path) -> Result<(), FormError> {
    let meta = std::fs::symlink_metadata(dir).map_err(|e| io(&e))?;
    if meta.file_type().is_symlink() || !meta.is_dir() {
        return Err(FormError::Store {
            reason: crate::store::StoreError::NotADirectory,
        });
    }
    Ok(())
}

/// Creates a new temporary file in `dir`, named `upload-<32 hex>.part`, with
/// `create_new` (it never opens an existing path, never follows a planted
/// link), owner-only on Unix. When `anonymous`, the file has no name after
/// this returns (Unix: unlinked; Windows: delete-on-close with no sharing),
/// and the returned path is `None`.
pub(crate) fn create_file(
    dir: &Path,
    anonymous: bool,
) -> Result<(File, Option<PathBuf>), FormError> {
    let path = dir.join(name_with("upload-", ".part")?);
    let file = open_new(&path, anonymous)?;
    if !anonymous {
        return Ok((file, Some(path)));
    }
    #[cfg(not(windows))]
    {
        if let Err(e) = std::fs::remove_file(&path) {
            drop(file);
            let _ = std::fs::remove_file(&path);
            return Err(io(&e));
        }
    }
    Ok((file, None))
}

/// Opens `path` as a new file with the platform flags. Separate from the
/// random name so a test can plant a file or a link at a known path.
#[cfg_attr(not(windows), allow(unused_variables))]
fn open_new(path: &Path, anonymous: bool) -> Result<File, FormError> {
    let mut o = OpenOptions::new();
    o.read(true).write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        o.mode(0o600);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        const FILE_ATTRIBUTE_TEMPORARY: u32 = 0x0000_0100;
        const FILE_FLAG_DELETE_ON_CLOSE: u32 = 0x0400_0000;
        const GENERIC_READ: u32 = 0x8000_0000;
        const GENERIC_WRITE: u32 = 0x4000_0000;
        const DELETE: u32 = 0x0001_0000;
        const FILE_SHARE_READ: u32 = 0x0000_0001;
        const FILE_SHARE_DELETE: u32 = 0x0000_0004;
        o.attributes(FILE_ATTRIBUTE_TEMPORARY);
        if anonymous {
            o.custom_flags(FILE_FLAG_DELETE_ON_CLOSE)
                .access_mode(GENERIC_READ | GENERIC_WRITE | DELETE)
                .share_mode(0);
        } else {
            o.share_mode(FILE_SHARE_READ | FILE_SHARE_DELETE);
        }
    }
    o.open(path).map_err(|e| io(&e))
}

/// Reads at `offset` without moving a shared cursor.
pub(crate) fn read_at(file: &File, buf: &mut [u8], offset: u64) -> io::Result<usize> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::FileExt;
        file.read_at(buf, offset)
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::FileExt;
        file.seek_read(buf, offset)
    }
    #[cfg(not(any(unix, windows)))]
    {
        use std::io::{Read, Seek, SeekFrom};
        let mut f = file;
        f.seek(SeekFrom::Start(offset))?;
        f.read(buf)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn private() -> PathBuf {
        let d = create_private_dir().unwrap();
        check_dir(&d).unwrap();
        d
    }

    #[test]
    fn test_names_are_random_and_hex() {
        let a = crate::random::random_hex().unwrap();
        let b = crate::random::random_hex().unwrap();
        assert_ne!(a, b);
        assert!(
            a.iter()
                .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
        );
    }

    #[test]
    fn test_attack_t9_planted_file_is_never_opened() {
        // T-9: an attacker who guessed the next name and made the file first.
        let d = private();
        let planted = d.join("upload-planted.part");
        std::fs::write(&planted, b"theirs").unwrap();
        for anonymous in [false, true] {
            assert!(open_new(&planted, anonymous).is_err());
        }
        assert_eq!(std::fs::read(&planted).unwrap(), b"theirs");
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn test_attack_t9_planted_link_is_never_followed() {
        let d = private();
        let target = d.join("target");
        let link = d.join("upload-link.part");
        std::os::unix::fs::symlink(&target, &link).unwrap();
        for anonymous in [false, true] {
            assert!(open_new(&link, anonymous).is_err());
        }
        assert!(!target.exists(), "nothing was written through the link");
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn test_create_file_names_and_cleanup() {
        let d = private();
        let (f, path) = create_file(&d, false).unwrap();
        let path = path.unwrap();
        let name = path.file_name().unwrap().to_str().unwrap();
        assert!(name.starts_with("upload-") && name.ends_with(".part") && name.len() == 44);
        drop(f);
        std::fs::remove_file(&path).unwrap();
        let (f, path) = create_file(&d, true).unwrap();
        assert!(path.is_none());
        drop(f);
        assert_eq!(std::fs::read_dir(&d).unwrap().count(), 0);
        std::fs::remove_dir_all(&d).unwrap();
    }
}
