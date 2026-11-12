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

/// Whether `name` is a private directory `Store::temp` makes:
/// `form-net-` and 32 lowercase hex characters.
fn is_private_dir_name(name: &[u8]) -> bool {
    name.strip_prefix(b"form-net-").is_some_and(|hex| {
        hex.len() == 32
            && hex
                .iter()
                .all(|&b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    })
}

/// Removes the spill files form-net names (`upload-<32 hex>.part`), and
/// nothing else, whose modification time is older than `older_than`:
/// directly inside `dir`, and directly inside each private directory
/// `Store::temp` makes there (`form-net-<32 hex>`), so
/// `sweep(&std::env::temp_dir(), ..)` reaches what a crashed process left
/// under `Store::temp` or `read_multipart` (D-158). Returns how many it
/// removed.
///
/// For `SpillMode::Named` stores after a crash (and the rare anonymous
/// file a crash catches between creation and unlinking on Unix): run it at
/// startup, before the server accepts connections, with `older_than`
/// longer than any live process keeps a named upload. It goes one level
/// down and never further, and never follows links: a link with a matching
/// name is removed as a link, never its target, and a link to a directory
/// is not entered. The private directories themselves are left in place,
/// empty: a live store may still own one. A file whose time is in the
/// future is kept. It is the only function in form-net that reads a clock.
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
        // `DirEntry::metadata` does not follow links: a link is never a dir.
        let Ok(meta) = entry.metadata() else { continue };
        if meta.is_dir() && is_private_dir_name(name.as_encoded_bytes()) {
            let inner = sweep_files(&entry.path(), older_than).unwrap_or(0);
            removed = removed.saturating_add(inner);
            continue;
        }
        if !is_spill_name(name.as_encoded_bytes()) || meta.is_dir() {
            continue;
        }
        if is_old(&meta, older_than) && std::fs::remove_file(entry.path()).is_ok() {
            removed = removed.saturating_add(1);
        }
    }
    Ok(removed)
}

/// Whether an entry's modification time is at least `older_than` ago.
fn is_old(meta: &std::fs::Metadata, older_than: Duration) -> bool {
    meta.modified()
        .ok()
        .and_then(|t| t.elapsed().ok())
        .is_some_and(|age| age >= older_than)
}

/// The spill files directly inside a private directory; no deeper.
fn sweep_files(dir: &Path, older_than: Duration) -> Result<usize, FormError> {
    super::temp::check_dir(dir)?;
    let mut removed = 0usize;
    for entry in std::fs::read_dir(dir).map_err(|e| FormError::Io { kind: e.kind() })? {
        let Ok(entry) = entry else { continue };
        let name = entry.file_name();
        if !is_spill_name(name.as_encoded_bytes()) {
            continue;
        }
        let Ok(meta) = entry.metadata() else { continue };
        if meta.is_dir() {
            continue;
        }
        if is_old(&meta, older_than) && std::fs::remove_file(entry.path()).is_ok() {
            removed = removed.saturating_add(1);
        }
    }
    Ok(removed)
}

#[cfg(test)]
mod tests {
    use super::{is_private_dir_name, is_spill_name};

    #[test]
    fn test_only_private_directory_names_match() {
        assert!(is_private_dir_name(
            b"form-net-0123456789abcdef0123456789abcdef"
        ));
        for other in [
            &b"form-net-"[..],
            b"form-net-0123456789ABCDEF0123456789abcdef",
            b"form-net-0123456789abcdef0123456789abcde",
            b"form-net-0123456789abcdef0123456789abcdef0",
            b"form-net-sweep-doc-1",
            b"x-form-net-0123456789abcdef0123456789abcdef",
        ] {
            assert!(!is_private_dir_name(other), "{other:?}");
        }
    }

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
