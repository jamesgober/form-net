//! `FileData`: an owned file for buffered bodies.

use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;

use super::{FileValue, FromField, ValueError};

/// An owned file: its bytes, and the file name and content type the client
/// sent, when they are text. The output of [`MemorySink`](crate::MemorySink)
/// and a field type for typed extraction from buffered bodies.
///
/// The bytes are wiped on drop. `Debug` prints the length only.
///
/// # Examples
///
/// ```
/// form_net::form! {
///     pub struct Avatar {
///         pub image: form_net::FileData,
///     }
/// }
/// use form_net::FromForm;
///
/// let body = b"--X\r\nContent-Disposition: form-data; name=image; filename=\"me.png\"\r\nContent-Type: image/png\r\n\r\nPNG\r\n--X--";
/// let a = Avatar::from_multipart(b"multipart/form-data; boundary=X", body)?;
/// assert_eq!((a.image.bytes(), a.image.filename()), (&b"PNG"[..], Some("me.png")));
/// # Ok::<(), form_net::FormError>(())
/// ```
#[derive(Default)]
pub struct FileData {
    bytes: Vec<u8>,
    filename: Option<String>,
    content_type: Option<String>,
}

impl fmt::Debug for FileData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "FileData({} bytes)", self.bytes.len())
    }
}

impl FileData {
    pub(crate) fn new(
        bytes: Vec<u8>,
        filename: Option<String>,
        content_type: Option<String>,
    ) -> Self {
        FileData {
            bytes,
            filename,
            content_type,
        }
    }

    pub(crate) fn append(&mut self, chunk: &[u8]) {
        crate::wipe::append_wiping(&mut self.bytes, chunk, 64 * 1024, usize::MAX);
    }

    /// The file's bytes.
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// The file's bytes, owned. They are no longer wiped by form-net.
    #[must_use]
    pub fn into_bytes(mut self) -> Vec<u8> {
        core::mem::take(&mut self.bytes)
    }

    /// The length in bytes.
    #[must_use]
    pub fn len(&self) -> usize {
        self.bytes.len()
    }

    /// Whether the file is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }

    /// The client's file name, as sent: never sanitized, never a path.
    #[must_use]
    pub fn filename(&self) -> Option<&str> {
        self.filename.as_deref()
    }

    /// The declared content type: a hint, never trusted.
    #[must_use]
    pub fn content_type(&self) -> Option<&str> {
        self.content_type.as_deref()
    }
}

impl Drop for FileData {
    fn drop(&mut self) {
        crate::wipe::wipe_vec(&mut self.bytes);
        crate::wipe::wipe_opt_string(&mut self.filename);
        crate::wipe::wipe_opt_string(&mut self.content_type);
    }
}

impl FromField for FileData {
    type Slot = Option<FileData>;

    fn accept_file(slot: &mut Self::Slot, file: FileValue<'_>) -> Result<(), ValueError> {
        let data = match file {
            FileValue::Buffered(part) => {
                let name = part.filename();
                if name.is_some_and(|n| n.is_empty()) && part.is_empty() {
                    // A file input left empty: as if it were absent.
                    return Ok(());
                }
                let filename = match name {
                    Some(n) => {
                        Some(String::from(n.as_str().map_err(|_| {
                            ValueError::Invalid("file name is not UTF-8")
                        })?))
                    }
                    None => None,
                };
                let content_type = part
                    .content_type()
                    .and_then(|c| core::str::from_utf8(c.raw()).ok().map(String::from));
                FileData::new(part.bytes().to_vec(), filename, content_type)
            }
            #[cfg(feature = "fs")]
            FileValue::Stored(stored) => {
                if stored.is_empty() && stored.filename().is_some_and(|n| n.is_empty()) {
                    return Ok(());
                }
                let filename = match stored.filename() {
                    Some(n) => {
                        Some(String::from(n.as_str().map_err(|_| {
                            ValueError::Invalid("file name is not UTF-8")
                        })?))
                    }
                    None => None,
                };
                let content_type = stored
                    .content_type()
                    .and_then(|c| core::str::from_utf8(c.raw()).ok().map(String::from));
                let bytes = stored
                    .read_to_vec(stored.len())
                    .map_err(|_| ValueError::Invalid("the stored file could not be read"))?;
                FileData::new(bytes, filename, content_type)
            }
        };
        if slot.is_some() {
            return Err(ValueError::Duplicate);
        }
        *slot = Some(data);
        Ok(())
    }

    fn finish(slot: Self::Slot) -> Result<Self, ValueError> {
        slot.ok_or(ValueError::Missing)
    }
}
