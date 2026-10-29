//! Startup cleanup for named-mode spill files left by a crash.

use core::time::Duration;
use std::io;
use std::path::Path;

use crate::error::FormError;

/// Whether `name` is a spill file name form-net makes:
/// `upload-` and 32 lowercase hex characters and `.part`.
fn is_spill_name(name: &[u8]) -> bool {
    let Some(rest) = name.strip_prefix(b"upload-") else {
        return false;
    };
    let Some(hex) = rest.strip_suffix(b".part") else {
        return false;
    };
    hex.len() == 32
        && hex
            .iter()
            .all(|&b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// Removes the spill files form-net names (`upload-<32 hex>.part`), and
/// nothing else, directly inside `dir`, whose modification time is older
/// than `older_than`. Returns how many it removed.
///
/// For `SpillMode::Named` stores after a crash (and the rare anonymous
/// file a crash catches between creation and unlinking on Unix): run it at
/// startup, before the server accepts connections. It never recurses and
/// never follows links: a link with a matching name is removed as a link,
/// never its target. A file whose time is in the future is kept. It is the
/// only function in form-net that reads a clock.
///
/// # Errors
///
/// `Store { NotADirectory }` when `dir` is not a directory or is a link;
/// `Io { kind }` when the directory cannot be listed. An entry that cannot
/// be read or removed (open in another process, locked by a scanner, a
/// link to a directory) is skipped, and the sweep goes on.
///
/// # Examples
///
/// ```
/// use std::time::Duration;
///
/// let dir = std::env::temp_dir().join(format!("form-net-sweep-doc-{}", std::process::id()));
/// std::fs::create_dir_all(&dir).unwrap();
/// std::fs::write(dir.join("upload-0123456789abcdef0123456789abcdef.part"), b"left").unwrap();
/// std::fs::write(dir.join("notes.txt"), b"mine").unwrap();
/// assert_eq!(form_net::sweep(&dir, Duration::ZERO)?, 1);
/// assert!(dir.join("notes.txt").exists());
/// # std::fs::remove_dir_all(&dir).unwrap();
/// # Ok::<(), form_net::FormError>(())
/// ```
pub fn sweep(dir: &Path, older_than: Duration) -> Result<usize, FormError> {
    super::temp::check_dir(dir)?;
    let io = |e: io::Error| FormError::Io { kind: e.kind() };
    let mut removed = 0usize;
    for entry in std::fs::read_dir(dir).map_err(io)? {
        let Ok(entry) = entry else { continue };
        let name = entry.file_name();
        if !is_spill_name(name.as_encoded_bytes()) {
            continue;
        }
        // `DirEntry::metadata` does not follow links.
        let Ok(meta) = entry.metadata() else { continue };
        if meta.is_dir() {
            continue;
        }
        let old = meta
            .modified()
            .ok()
            .and_then(|t| t.elapsed().ok())
            .is_some_and(|age| age >= older_than);
        if !old {
            continue;
        }
        if std::fs::remove_file(entry.path()).is_ok() {
            removed = removed.saturating_add(1);
        }
    }
    Ok(removed)
}

#[cfg(test)]
mod tests {
    use super::is_spill_name;

    #[test]
    fn test_only_form_net_names_match() {
        assert!(is_spill_name(
            b"upload-0123456789abcdef0123456789abcdef.part"
        ));
        for other in [
            &b"upload-.part"[..],
            b"upload-0123456789ABCDEF0123456789abcdef.part",
            b"upload-0123456789abcdef0123456789abcde.part",
            b"upload-0123456789abcdef0123456789abcdef.part.bak",
            b"x-upload-0123456789abcdef0123456789abcdef.part",
            b"upload-0123456789abcdef0123456789abcdeg.part",
        ] {
            assert!(!is_spill_name(other), "{other:?}");
        }
    }
}
