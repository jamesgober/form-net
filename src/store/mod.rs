//! Upload storage: files kept in memory up to a threshold and spilled to
//! private temporary files above it. No client byte is ever part of a path.
//!
//! Spilling is blocking file I/O: a server with an event loop runs a parse
//! that stores uploads where blocking is allowed, or gives the driver its
//! own [`Sink`](crate::Sink).

mod quota;
mod sweep;
mod temp;

pub use quota::{SpillBudget, SpillQuota};
pub use sweep::sweep;

use alloc::borrow::ToOwned;
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::fmt;
use core::sync::atomic::{AtomicU64, Ordering};
use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use crate::error::{ExtractError, FormError, Limit, Location};
use crate::hook::{Inspector, Sink};
use crate::limits::{Limits, Policy};
use crate::multipart::driver::{Driver, Progress};
use crate::multipart::filename::{FileName, FileNameSource};
use crate::multipart::parser::{ContentType, MultipartParser, PartHead};

/// What went wrong with storage.
///
/// # Examples
///
/// ```
/// use form_net::{FormError, Store, StoreError};
///
/// # let dir = std::env::temp_dir().join(format!("form-net-doc-store-error-{}", std::process::id()));
/// # std::fs::create_dir_all(&dir).unwrap();
/// std::fs::write(dir.join("file"), b"")?;
/// let e = Store::in_dir(dir.join("file")).unwrap_err();
/// assert!(matches!(e, FormError::Store { reason: StoreError::NotADirectory }));
/// assert_eq!(e.code(), "store.not_a_directory");
/// # std::fs::remove_dir_all(&dir).unwrap();
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StoreError {
    /// The shared spill budget is full (status hint 503).
    QuotaExceeded,
    /// `Store::in_dir` or `sweep` was given a non-directory or a link.
    NotADirectory,
    /// `persist`'s destination exists; nothing was overwritten.
    DestinationExists,
}

impl StoreError {
    #[cfg(target_has_atomic = "32")]
    pub(crate) const fn index(self) -> usize {
        match self {
            Self::QuotaExceeded => 0,
            Self::NotADirectory => 1,
            Self::DestinationExists => 2,
        }
    }

    pub(crate) const fn code(self) -> &'static str {
        match self {
            Self::QuotaExceeded => "store.quota_exceeded",
            Self::NotADirectory => "store.not_a_directory",
            Self::DestinationExists => "store.destination_exists",
        }
    }

    pub(crate) const fn describe(self) -> &'static str {
        match self {
            Self::QuotaExceeded => "the shared upload budget is full; retry later",
            Self::NotADirectory => "not a directory, or a link to one",
            Self::DestinationExists => "the destination exists; nothing was overwritten",
        }
    }
}

/// How spilled files are kept.
///
/// | | `Anonymous` (default) | `Named` |
/// |---|---|---|
/// | Unix | unlinked right after creation | keeps its random name until dropped or persisted |
/// | Windows | delete-on-close, no sharing | deleted on drop |
/// | after a crash | nothing is left | leftovers are removed by `sweep` (0.5) |
/// | `persist` | a copy | a hard link on the same file system, else a copy |
///
/// # Examples
///
/// ```
/// use form_net::{Limits, Policy, SpillMode, Store};
///
/// # let dir = std::env::temp_dir().join(format!("form-net-doc-spill-mode-{}", std::process::id()));
/// # std::fs::create_dir_all(&dir).unwrap();
/// let store = Store::in_dir(&dir)?.memory_threshold(0).mode(SpillMode::Named);
/// let body = b"--X\r\nContent-Disposition: form-data; name=f; filename=a.txt\r\n\r\nhello\r\n--X--";
/// let upload = form_net::read_multipart_with(b"multipart/form-data; boundary=X", &body[..], &Limits::DEFAULT, &Policy::DEFAULT, &store)?;
/// let path = upload.file("f").unwrap().path().unwrap().to_owned();
/// assert!(path.starts_with(&dir));
/// drop(upload); // a named file is deleted when it drops
/// assert!(!path.exists());
/// # std::fs::remove_dir_all(&dir).unwrap();
/// # Ok::<(), form_net::FormError>(())
/// ```
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum SpillMode {
    /// The file has no name the moment it exists.
    #[default]
    Anonymous,
    /// The file keeps a random name in the store's directory.
    Named,
}

/// A snapshot of what a store holds, read from atomic counters.
///
/// # Examples
///
/// ```
/// use form_net::{Limits, Policy, Store};
///
/// let store = Store::memory_only();
/// let body = b"--X\r\nContent-Disposition: form-data; name=f; filename=a.txt\r\n\r\nhello\r\n--X--";
/// let upload = form_net::read_multipart_with(b"multipart/form-data; boundary=X", &body[..], &Limits::DEFAULT, &Policy::DEFAULT, &store)?;
/// let stats = store.stats();
/// assert_eq!((stats.files_in_memory, stats.bytes_in_memory, stats.files_on_disk), (1, 5, 0));
/// drop(upload);
/// assert_eq!(store.stats().files_in_memory, 0);
/// # Ok::<(), form_net::FormError>(())
/// ```
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StoreStats {
    /// Files held in memory now.
    pub files_in_memory: u64,
    /// Files on disk now.
    pub files_on_disk: u64,
    /// Bytes held in memory now.
    pub bytes_in_memory: u64,
    /// Bytes on disk now.
    pub bytes_on_disk: u64,
    /// Files spilled to disk since the store was created.
    pub files_spilled: u64,
    /// Deletions that failed during a drop, since the store was created.
    pub cleanup_failures: u64,
}

#[derive(Default)]
struct Counters {
    files_in_memory: AtomicU64,
    files_on_disk: AtomicU64,
    bytes_in_memory: AtomicU64,
    bytes_on_disk: AtomicU64,
    files_spilled: AtomicU64,
    cleanup_failures: AtomicU64,
}

fn add(c: &AtomicU64, n: u64) {
    c.fetch_add(n, Ordering::Relaxed);
}

fn sub(c: &AtomicU64, n: u64) {
    // Saturating: a counter never wraps below zero.
    let mut v = c.load(Ordering::Relaxed);
    loop {
        match c.compare_exchange_weak(v, v.saturating_sub(n), Ordering::Relaxed, Ordering::Relaxed)
        {
            Ok(_) => return,
            Err(now) => v = now,
        }
    }
}

/// What files share with their store: the directory and the counters. A
/// store's own directory is removed (if empty) when the store and all its
/// files are gone.
struct Shared {
    dir: OnceLock<PathBuf>,
    owns_dir: bool,
    counters: Counters,
}

impl Drop for Shared {
    fn drop(&mut self) {
        if self.owns_dir {
            if let Some(dir) = self.dir.get() {
                let _ = std::fs::remove_dir(dir);
            }
        }
    }
}

impl Shared {
    fn dir(&self) -> Result<&Path, FormError> {
        if let Some(d) = self.dir.get() {
            return Ok(d);
        }
        let created = temp::create_private_dir()?;
        if let Err(lost) = self.dir.set(created) {
            // Another thread created one first; remove ours.
            let _ = std::fs::remove_dir(lost);
        }
        self.dir.get().map(PathBuf::as_path).ok_or(FormError::Io {
            kind: io::ErrorKind::Other,
        })
    }

    fn cleanup_failed(&self) {
        add(&self.counters.cleanup_failures, 1);
    }
}

/// Where uploads are kept. `Send + Sync`; create one per process or per
/// route and share it by reference.
///
/// # Examples
///
/// ```
/// use form_net::{SpillMode, Store};
///
/// // Files over 64 KiB spill to anonymous temporary files.
/// let store = Store::temp()?;
/// assert!(store.dir().is_some());
///
/// // Never touch the file system.
/// let memory = Store::memory_only().memory_threshold(1 << 20);
/// assert!(memory.dir().is_none());
/// # Ok::<(), form_net::FormError>(())
/// ```
#[derive(Clone)]
pub struct Store {
    shared: Arc<Shared>,
    memory_only: bool,
    threshold: u64,
    max_disk: Option<u64>,
    mode: SpillMode,
    budget: Option<Arc<dyn SpillBudget>>,
}

impl fmt::Debug for Store {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Store")
            .field("memory_only", &self.memory_only)
            .field("threshold", &self.threshold)
            .field("mode", &self.mode)
            .field("quota", &self.budget.is_some())
            .field("stats", &self.stats())
            .finish_non_exhaustive()
    }
}

const DEFAULT_THRESHOLD: u64 = 64 * 1024;

impl Store {
    fn with(dir: Option<PathBuf>, owns_dir: bool, memory_only: bool) -> Store {
        let lock = OnceLock::new();
        if let Some(d) = dir {
            let _ = lock.set(d);
        }
        Store {
            shared: Arc::new(Shared {
                dir: lock,
                owns_dir,
                counters: Counters::default(),
            }),
            memory_only,
            threshold: DEFAULT_THRESHOLD,
            max_disk: None,
            mode: SpillMode::Anonymous,
            budget: None,
        }
    }

    /// A store with its own private directory under the system temporary
    /// directory, created now: `form-net-<32 hex>`, owner-only on Unix,
    /// failing if the path exists. Files up to 64 KiB stay in memory;
    /// larger ones spill to anonymous temporary files.
    ///
    /// # Errors
    ///
    /// `Io { kind }` when the directory cannot be created; `Random` when the
    /// OS random source fails (no predictable fallback name).
    ///
    /// # Examples
    ///
    /// ```
    /// let store = form_net::Store::temp()?;
    /// let dir = store.dir().unwrap().to_owned();
    /// assert!(dir.is_dir());
    /// drop(store); // its own directory goes with it, once empty
    /// assert!(!dir.exists());
    /// # Ok::<(), form_net::FormError>(())
    /// ```
    pub fn temp() -> Result<Store, FormError> {
        let dir = temp::create_private_dir()?;
        Ok(Store::with(Some(dir), true, false))
    }

    /// A store whose private directory is created only when a file first
    /// spills (for one-shot parses).
    pub(crate) fn temp_lazy() -> Store {
        Store::with(None, true, false)
    }

    /// A store that never touches the file system: a file larger than
    /// `max_file_bytes` is refused, never spilled.
    ///
    /// # Examples
    ///
    /// ```
    /// use form_net::{Limits, Policy, Store};
    ///
    /// let store = Store::memory_only();
    /// let body = b"--X\r\nContent-Disposition: form-data; name=f; filename=a.txt\r\n\r\nhello\r\n--X--";
    /// let upload = form_net::read_multipart_with(b"multipart/form-data; boundary=X", &body[..], &Limits::DEFAULT, &Policy::DEFAULT, &store)?;
    /// assert!(upload.file("f").unwrap().is_in_memory());
    /// assert_eq!(store.dir(), None);
    /// # Ok::<(), form_net::FormError>(())
    /// ```
    #[must_use]
    pub fn memory_only() -> Store {
        let mut s = Store::with(None, false, true);
        s.threshold = u64::MAX;
        s
    }

    /// A store that spills into `dir`, a directory the caller made. It must
    /// exist and be a directory, not a link to one.
    ///
    /// # Errors
    ///
    /// `Store { NotADirectory }`; `Io { kind }`.
    ///
    /// # Examples
    ///
    /// ```
    /// let dir = std::env::temp_dir();
    /// let store = form_net::Store::in_dir(&dir)?;
    /// assert_eq!(store.dir(), Some(dir.as_path()));
    /// # Ok::<(), form_net::FormError>(())
    /// ```
    pub fn in_dir(dir: impl AsRef<Path>) -> Result<Store, FormError> {
        let dir = dir.as_ref();
        temp::check_dir(dir)?;
        Ok(Store::with(Some(dir.to_owned()), false, false))
    }

    /// Keeps each file in memory up to `bytes`; a file whose next write would
    /// pass it spills first. `0` spills every file at its first byte. On a
    /// [`memory_only`](Self::memory_only) store it changes nothing: that
    /// store never spills, and a file larger than `max_file_bytes` is
    /// refused (D-160).
    ///
    /// # Examples
    ///
    /// ```
    /// let dir = std::env::temp_dir();
    /// let store = form_net::Store::in_dir(&dir)?.memory_threshold(16 * 1024);
    /// assert_eq!(store.dir(), Some(dir.as_path()));
    /// # Ok::<(), form_net::FormError>(())
    /// ```
    #[must_use]
    pub fn memory_threshold(mut self, bytes: u64) -> Store {
        if !self.memory_only {
            self.threshold = bytes;
        }
        self
    }

    /// Caps the bytes one request may put on disk (`Limit { Disk }` beyond).
    /// Unset, the cap is the request's `max_body_bytes`.
    ///
    /// # Examples
    ///
    /// ```
    /// let store = form_net::Store::temp()?.max_disk_bytes(50 * 1024 * 1024);
    /// assert!(store.dir().is_some());
    /// # Ok::<(), form_net::FormError>(())
    /// ```
    #[must_use]
    pub fn max_disk_bytes(mut self, bytes: u64) -> Store {
        self.max_disk = Some(bytes);
        self
    }

    /// How spilled files are kept.
    ///
    /// # Examples
    ///
    /// ```
    /// use form_net::{Limits, Policy, SpillMode, Store};
    ///
    /// # let dir = std::env::temp_dir().join(format!("form-net-doc-mode-{}", std::process::id()));
    /// # std::fs::create_dir_all(&dir).unwrap();
    /// let store = Store::in_dir(&dir)?.memory_threshold(0).mode(SpillMode::Anonymous);
    /// let body = b"--X\r\nContent-Disposition: form-data; name=f; filename=a.txt\r\n\r\nhello\r\n--X--";
    /// let upload = form_net::read_multipart_with(b"multipart/form-data; boundary=X", &body[..], &Limits::DEFAULT, &Policy::DEFAULT, &store)?;
    /// let file = upload.file("f").unwrap();
    /// // Spilled, and nameless from the moment it was created.
    /// assert_eq!((file.is_in_memory(), file.path()), (false, None));
    /// # drop(upload);
    /// # std::fs::remove_dir_all(&dir).unwrap();
    /// # Ok::<(), form_net::FormError>(())
    /// ```
    #[must_use]
    pub fn mode(mut self, mode: SpillMode) -> Store {
        self.mode = mode;
        self
    }

    /// The directory files spill into, once it exists.
    ///
    /// # Examples
    ///
    /// ```
    /// use form_net::Store;
    ///
    /// # let dir = std::env::temp_dir().join(format!("form-net-doc-dir-{}", std::process::id()));
    /// # std::fs::create_dir_all(&dir).unwrap();
    /// assert_eq!(Store::in_dir(&dir)?.dir(), Some(dir.as_path()));
    /// assert_eq!(Store::memory_only().dir(), None);
    /// # std::fs::remove_dir_all(&dir).unwrap();
    /// # Ok::<(), form_net::FormError>(())
    /// ```
    #[must_use]
    pub fn dir(&self) -> Option<&Path> {
        self.shared.dir.get().map(PathBuf::as_path)
    }

    /// Deletions that failed during a drop (they cannot be returned).
    ///
    /// # Examples
    ///
    /// ```
    /// let store = form_net::Store::memory_only();
    /// assert_eq!(store.cleanup_failures(), 0);
    /// assert_eq!(store.cleanup_failures(), store.stats().cleanup_failures);
    /// ```
    #[must_use]
    pub fn cleanup_failures(&self) -> u64 {
        self.shared
            .counters
            .cleanup_failures
            .load(Ordering::Relaxed)
    }

    /// What the store holds now.
    ///
    /// # Examples
    ///
    /// ```
    /// use form_net::{Limits, Policy, Store};
    ///
    /// # let dir = std::env::temp_dir().join(format!("form-net-doc-stats-{}", std::process::id()));
    /// # std::fs::create_dir_all(&dir).unwrap();
    /// let store = Store::in_dir(&dir)?.memory_threshold(0);
    /// let body = b"--X\r\nContent-Disposition: form-data; name=f; filename=a.txt\r\n\r\nhello\r\n--X--";
    /// let upload = form_net::read_multipart_with(b"multipart/form-data; boundary=X", &body[..], &Limits::DEFAULT, &Policy::DEFAULT, &store)?;
    /// let stats = store.stats();
    /// assert_eq!((stats.files_on_disk, stats.bytes_on_disk, stats.files_spilled), (1, 5, 1));
    /// drop(upload);
    /// assert_eq!((store.stats().files_on_disk, store.stats().files_spilled), (0, 1));
    /// # std::fs::remove_dir_all(&dir).unwrap();
    /// # Ok::<(), form_net::FormError>(())
    /// ```
    #[must_use]
    pub fn stats(&self) -> StoreStats {
        let c = &self.shared.counters;
        StoreStats {
            files_in_memory: c.files_in_memory.load(Ordering::Relaxed),
            files_on_disk: c.files_on_disk.load(Ordering::Relaxed),
            bytes_in_memory: c.bytes_in_memory.load(Ordering::Relaxed),
            bytes_on_disk: c.bytes_on_disk.load(Ordering::Relaxed),
            files_spilled: c.files_spilled.load(Ordering::Relaxed),
            cleanup_failures: c.cleanup_failures.load(Ordering::Relaxed),
        }
    }

    /// Adds a disk budget shared across requests (and across stores given
    /// the same budget): bytes are reserved from it before every disk write,
    /// and a request that would cross it is refused with
    /// `Store { QuotaExceeded }` (status hint 503). Replaces an earlier one.
    ///
    /// # Examples
    ///
    /// ```
    /// use form_net::{Limits, NoInspector, Policy, SpillQuota, Store, Upload};
    ///
    /// let quota = SpillQuota::new(4);
    /// let store = Store::temp()?.memory_threshold(0).quota(quota.clone());
    /// let body = b"--X\r\nContent-Disposition: form-data; name=f; filename=a\r\n\r\n12345\r\n--X--";
    /// let e = form_net::read_multipart_with(b"multipart/form-data; boundary=X", &body[..], &Limits::DEFAULT, &Policy::DEFAULT, &store).unwrap_err();
    /// assert_eq!(e.status_hint(), 503);
    /// assert_eq!(quota.in_use(), 0, "the refused file gave its bytes back");
    /// # Ok::<(), form_net::FormError>(())
    /// ```
    pub fn quota(mut self, budget: impl SpillBudget) -> Store {
        self.budget = Some(Arc::new(budget));
        self
    }

    /// The worst-case bytes one request through this store can hold in
    /// memory under `limits`: [`Limits::memory_bound`] with this store's
    /// memory threshold (`u64::MAX` for a memory-only store).
    ///
    /// # Examples
    ///
    /// ```
    /// use form_net::{Limits, Store};
    ///
    /// let limits = Limits::DEFAULT;
    /// assert!(Store::temp()?.memory_bound(&limits) < Store::memory_only().memory_bound(&limits));
    /// # Ok::<(), form_net::FormError>(())
    /// ```
    #[must_use]
    pub fn memory_bound(&self, limits: &Limits) -> u64 {
        let in_memory = if self.memory_only {
            u64::MAX
        } else {
            self.threshold
        };
        limits.memory_bound(in_memory)
    }

    /// The `Sink` that stores the files of one request under `limits`.
    ///
    /// # Examples
    ///
    /// ```
    /// use form_net::{Boundary, Driver, Limits, MultipartParser, NoInspector, Policy, Progress, Store};
    ///
    /// let store = Store::memory_only();
    /// let parser = MultipartParser::new(Boundary::new(b"X")?, &Limits::DEFAULT, &Policy::DEFAULT)?;
    /// let mut driver = Driver::new(parser, store.sink(&Limits::DEFAULT));
    /// let body = b"--X\r\nContent-Disposition: form-data; name=f; filename=a.txt\r\n\r\nhello\r\n--X--";
    /// let (_, progress) = driver.push(body, true, &mut NoInspector)?;
    /// assert!(matches!(progress, Progress::File(ref f) if f.bytes() == Some(&b"hello"[..])));
    /// # Ok::<(), form_net::FormError>(())
    /// ```
    #[must_use]
    pub fn sink(&self, limits: &Limits) -> StoreSink {
        StoreSink {
            budget: self.budget.clone(),
            shared: Arc::clone(&self.shared),
            memory_only: self.memory_only,
            threshold: self.threshold,
            // A memory file never holds more than this: its buffer is
            // capped here, so `memory_bound` is exact.
            memory_cap: usize::try_from(self.threshold.min(limits.file())).unwrap_or(usize::MAX),
            mode: self.mode,
            max_disk: self.max_disk.unwrap_or(limits.max_body_bytes),
            used_disk: 0,
            current: None,
        }
    }
}

/// One file being written.
struct Current {
    name: String,
    filename: Option<(Vec<u8>, FileNameSource)>,
    content_type: Option<Vec<u8>>,
    memory: Vec<u8>,
    disk: Option<(File, Option<PathBuf>)>,
    len: u64,
    /// Bytes of this file written to disk (and added to the counters).
    on_disk: u64,
}

/// The [`Sink`] a [`Store`] hands out for one request.
///
/// # Examples
///
/// ```
/// use form_net::{Boundary, Driver, Limits, MultipartParser, NoInspector, Policy, Progress, Store};
///
/// let store = Store::memory_only();
/// let parser = MultipartParser::new(Boundary::new(b"X")?, &Limits::DEFAULT, &Policy::DEFAULT)?;
/// let mut driver = Driver::new(parser, store.sink(&Limits::DEFAULT));
/// let body = b"--X\r\nContent-Disposition: form-data; name=f; filename=a.txt\r\n\r\nhello\r\n--X--";
/// let (_, progress) = driver.push(body, true, &mut NoInspector)?;
/// // The sink's output is a `StoredFile`, counted by the store.
/// assert!(matches!(progress, Progress::File(ref f) if f.name() == "f"));
/// assert_eq!(store.stats().files_in_memory, 1);
/// # Ok::<(), form_net::FormError>(())
/// ```
pub struct StoreSink {
    budget: Option<Arc<dyn SpillBudget>>,
    shared: Arc<Shared>,
    memory_only: bool,
    threshold: u64,
    memory_cap: usize,
    mode: SpillMode,
    max_disk: u64,
    used_disk: u64,
    current: Option<Current>,
}

impl fmt::Debug for StoreSink {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("StoreSink")
            .field("used_disk", &self.used_disk)
            .field("max_disk", &self.max_disk)
            .finish_non_exhaustive()
    }
}

impl StoreSink {
    /// Bytes this request has put on disk so far.
    pub(crate) const fn used_disk(&self) -> u64 {
        self.used_disk
    }

    /// Continues a request that already put `bytes` on disk.
    pub(crate) const fn with_used_disk(mut self, bytes: u64) -> StoreSink {
        self.used_disk = bytes;
        self
    }

    fn disk_write(&mut self, bytes: &[u8]) -> Result<(), FormError> {
        let used = self.used_disk.saturating_add(bytes.len() as u64);
        if used > self.max_disk {
            return Err(FormError::limit(Limit::Disk, Location::NONE));
        }
        let Some(cur) = self.current.as_mut() else {
            return Ok(());
        };
        let Some((file, _)) = cur.disk.as_mut() else {
            return Ok(());
        };
        let n = bytes.len() as u64;
        if let Some(budget) = &self.budget {
            if !budget.reserve(n) {
                return Err(FormError::Store {
                    reason: StoreError::QuotaExceeded,
                });
            }
        }
        if let Err(e) = file.write_all(bytes) {
            // Reserved, never written: give the bytes back now. (A partial
            // write is removed with the file on abort, which releases only
            // `on_disk`.)
            if let Some(budget) = &self.budget {
                budget.release(n);
            }
            return Err(temp::io(&e));
        }
        self.used_disk = used;
        cur.on_disk = cur.on_disk.saturating_add(bytes.len() as u64);
        add(&self.shared.counters.bytes_on_disk, bytes.len() as u64);
        Ok(())
    }

    fn spill(&mut self) -> Result<(), FormError> {
        let dir = self.shared.dir()?.to_owned();
        let anonymous = self.mode == SpillMode::Anonymous;
        let (file, path) = temp::create_file(&dir, anonymous)?;
        add(&self.shared.counters.files_on_disk, 1);
        add(&self.shared.counters.files_spilled, 1);
        let prefix = match self.current.as_mut() {
            Some(cur) => {
                cur.disk = Some((file, path));
                core::mem::take(&mut cur.memory)
            }
            None => Vec::new(),
        };
        let mut prefix = prefix;
        let r = self.disk_write(&prefix);
        crate::wipe::wipe_vec(&mut prefix);
        r
    }
}

impl Sink for StoreSink {
    type Output = StoredFile;

    fn open(&mut self, head: &PartHead<'_>) -> Result<(), FormError> {
        self.abort();
        self.current = Some(Current {
            name: String::from(head.name()),
            filename: head.filename().map(|f| (f.raw().to_vec(), f.source())),
            content_type: head.content_type().map(|c| c.raw().to_vec()),
            memory: Vec::new(),
            disk: None,
            len: 0,
            on_disk: 0,
        });
        Ok(())
    }

    fn write(&mut self, chunk: &[u8]) -> Result<(), FormError> {
        let (on_disk, len) = match self.current.as_ref() {
            Some(c) => (c.disk.is_some(), c.len),
            None => return Ok(()),
        };
        let new_len = len.saturating_add(chunk.len() as u64);
        if !on_disk && !self.memory_only && new_len > self.threshold {
            self.spill()?;
        }
        let on_disk = self.current.as_ref().is_some_and(|c| c.disk.is_some());
        if on_disk {
            self.disk_write(chunk)?;
        } else if let Some(cur) = self.current.as_mut() {
            crate::wipe::append_wiping(&mut cur.memory, chunk, 4096, self.memory_cap);
        }
        if let Some(cur) = self.current.as_mut() {
            cur.len = new_len;
        }
        Ok(())
    }

    fn close(&mut self) -> Result<StoredFile, FormError> {
        let Some(mut cur) = self.current.take() else {
            return Err(FormError::Poisoned);
        };
        let on_disk = cur.disk.is_some();
        let place = match cur.disk.take() {
            Some((file, path)) => Place::Disk {
                file: Some(file),
                path,
            },
            None => {
                add(&self.shared.counters.files_in_memory, 1);
                add(&self.shared.counters.bytes_in_memory, cur.len);
                Place::Memory(core::mem::take(&mut cur.memory))
            }
        };
        Ok(StoredFile {
            name: core::mem::take(&mut cur.name),
            filename: cur.filename.take(),
            content_type: cur.content_type.take(),
            len: cur.len,
            place,
            shared: Some(Arc::clone(&self.shared)),
            budget: if on_disk { self.budget.clone() } else { None },
            reserved: cur.on_disk,
        })
    }

    fn abort(&mut self) {
        if let Some(mut cur) = self.current.take() {
            crate::wipe::wipe_vec(&mut cur.memory);
            wipe_meta(&mut cur.name, &mut cur.filename, &mut cur.content_type);
            if let Some((file, path)) = cur.disk.take() {
                // Close first, then remove the name: an open file cannot be
                // removed on Windows.
                drop(file);
                sub(&self.shared.counters.files_on_disk, 1);
                sub(&self.shared.counters.bytes_on_disk, cur.on_disk);
                let mut freed = true;
                if let Some(p) = path {
                    if std::fs::remove_file(&p).is_err() {
                        freed = false;
                        self.shared.cleanup_failed();
                    }
                }
                // As `StoredFile`'s drop: only space that is free again goes
                // back to the budget (D-101, D-143).
                if freed {
                    if let Some(budget) = &self.budget {
                        budget.release(cur.on_disk);
                    }
                }
            }
        }
    }
}

impl Drop for StoreSink {
    fn drop(&mut self) {
        self.abort();
    }
}

enum Place {
    Memory(Vec<u8>),
    Disk {
        file: Option<File>,
        path: Option<PathBuf>,
    },
    Gone,
}

/// One received file, in memory or in a temporary file.
///
/// Dropping it without [`persist`](Self::persist) deletes it (closing the
/// handle first) and wipes memory-held bytes. `Debug` never prints the file
/// name or the data.
///
/// # Examples
///
/// ```
/// use std::io::Read;
///
/// let body = b"--X\r\nContent-Disposition: form-data; name=doc; filename=a.txt\r\n\r\nhello\r\n--X--";
/// let upload = form_net::read_multipart(b"multipart/form-data; boundary=X", &body[..])?;
/// let file = upload.file("doc").unwrap();
/// let mut text = String::new();
/// file.reader()?.read_to_string(&mut text).unwrap();
/// assert_eq!((text.as_str(), file.len(), file.is_in_memory()), ("hello", 5, true));
/// # Ok::<(), form_net::FormError>(())
/// ```
#[must_use]
pub struct StoredFile {
    name: String,
    filename: Option<(Vec<u8>, FileNameSource)>,
    content_type: Option<Vec<u8>>,
    len: u64,
    place: Place,
    shared: Option<Arc<Shared>>,
    /// The budget this file's disk bytes were reserved from.
    budget: Option<Arc<dyn SpillBudget>>,
    reserved: u64,
}

impl fmt::Debug for StoredFile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("StoredFile")
            .field("name", &self.name)
            .field("len", &self.len)
            .field("in_memory", &self.is_in_memory())
            .finish_non_exhaustive()
    }
}

impl StoredFile {
    /// A memory-held file not tied to a store (from a buffered part).
    pub(crate) fn detached(
        name: &str,
        filename: Option<FileName<'_>>,
        content_type: Option<&[u8]>,
        bytes: &[u8],
    ) -> StoredFile {
        StoredFile {
            name: String::from(name),
            filename: filename.map(|f| (f.raw().to_vec(), f.source())),
            content_type: content_type.map(<[u8]>::to_vec),
            len: bytes.len() as u64,
            place: Place::Memory(bytes.to_vec()),
            shared: None,
            budget: None,
            reserved: 0,
        }
    }

    /// The form field name.
    ///
    /// # Examples
    ///
    /// ```
    /// let body = b"--X\r\nContent-Disposition: form-data; name=f; filename=a.txt\r\n\r\nhello\r\n--X--";
    /// let upload = form_net::read_multipart(b"multipart/form-data; boundary=X", &body[..])?;
    /// let file = upload.file("f").unwrap();
    /// assert_eq!(file.name(), "f");
    /// # Ok::<(), form_net::FormError>(())
    /// ```
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The client's file name: a hint, never a path.
    ///
    /// # Examples
    ///
    /// ```
    /// let body = b"--X\r\nContent-Disposition: form-data; name=f; filename=a.txt\r\n\r\nhello\r\n--X--";
    /// let upload = form_net::read_multipart(b"multipart/form-data; boundary=X", &body[..])?;
    /// let file = upload.file("f").unwrap();
    /// assert_eq!(file.filename().unwrap().as_str()?, "a.txt");
    /// # Ok::<(), form_net::FormError>(())
    /// ```
    #[must_use]
    pub fn filename(&self) -> Option<FileName<'_>> {
        self.filename
            .as_ref()
            .map(|(raw, source)| FileName::new(raw, *source))
    }

    /// The declared content type: a hint, never trusted.
    ///
    /// # Examples
    ///
    /// ```
    /// let body = b"--X\r\nContent-Disposition: form-data; name=f; filename=a.txt\r\nContent-Type: text/plain\r\n\r\nhello\r\n--X--";
    /// let upload = form_net::read_multipart(b"multipart/form-data; boundary=X", &body[..])?;
    /// let file = upload.file("f").unwrap();
    /// assert!(file.content_type().unwrap().is("text", "plain"));
    /// # Ok::<(), form_net::FormError>(())
    /// ```
    #[must_use]
    pub fn content_type(&self) -> Option<ContentType<'_>> {
        self.content_type.as_deref().and_then(ContentType::parse)
    }

    /// The length in bytes.
    ///
    /// # Examples
    ///
    /// ```
    /// let body = b"--X\r\nContent-Disposition: form-data; name=f; filename=a.txt\r\n\r\nhello\r\n--X--";
    /// let upload = form_net::read_multipart(b"multipart/form-data; boundary=X", &body[..])?;
    /// let file = upload.file("f").unwrap();
    /// assert_eq!(file.len(), 5);
    /// # Ok::<(), form_net::FormError>(())
    /// ```
    #[must_use]
    pub const fn len(&self) -> u64 {
        self.len
    }

    /// Whether the file is empty.
    ///
    /// # Examples
    ///
    /// ```
    /// let body = b"--X\r\nContent-Disposition: form-data; name=f; filename=a.txt\r\n\r\n\r\n--X--";
    /// let upload = form_net::read_multipart(b"multipart/form-data; boundary=X", &body[..])?;
    /// let file = upload.file("f").unwrap();
    /// assert!(file.is_empty());
    /// # Ok::<(), form_net::FormError>(())
    /// ```
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Whether the file is held in memory.
    ///
    /// # Examples
    ///
    /// ```
    /// let body = b"--X\r\nContent-Disposition: form-data; name=f; filename=a.txt\r\n\r\nhello\r\n--X--";
    /// let upload = form_net::read_multipart(b"multipart/form-data; boundary=X", &body[..])?;
    /// let file = upload.file("f").unwrap();
    /// // Files up to the store's memory threshold never reach the disk.
    /// assert!(file.is_in_memory());
    /// # Ok::<(), form_net::FormError>(())
    /// ```
    #[must_use]
    pub const fn is_in_memory(&self) -> bool {
        matches!(self.place, Place::Memory(_))
    }

    /// The bytes, while the file is in memory.
    ///
    /// # Examples
    ///
    /// ```
    /// let body = b"--X\r\nContent-Disposition: form-data; name=f; filename=a.txt\r\n\r\nhello\r\n--X--";
    /// let upload = form_net::read_multipart(b"multipart/form-data; boundary=X", &body[..])?;
    /// let file = upload.file("f").unwrap();
    /// assert_eq!(file.bytes(), Some(&b"hello"[..]));
    /// # Ok::<(), form_net::FormError>(())
    /// ```
    #[must_use]
    pub fn bytes(&self) -> Option<&[u8]> {
        match &self.place {
            Place::Memory(b) => Some(b),
            _ => None,
        }
    }

    /// A reader over the file, in memory or on disk. It reads with
    /// positional reads, so several readers can exist at once.
    ///
    /// # Errors
    ///
    /// `Io { kind }` when the file is gone.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::io::Read;
    ///
    /// let body = b"--X\r\nContent-Disposition: form-data; name=f; filename=a.txt\r\n\r\nhello\r\n--X--";
    /// let upload = form_net::read_multipart(b"multipart/form-data; boundary=X", &body[..])?;
    /// let file = upload.file("f").unwrap();
    /// let (mut a, mut b) = (file.reader()?, file.reader()?);
    /// let mut first = [0u8; 2];
    /// a.read_exact(&mut first)?;
    /// let mut all = Vec::new();
    /// b.read_to_end(&mut all)?;
    /// assert_eq!((&first, all.as_slice()), (b"he", &b"hello"[..]));
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn reader(&self) -> Result<StoredReader<'_>, FormError> {
        match &self.place {
            Place::Gone => Err(FormError::Io {
                kind: io::ErrorKind::NotFound,
            }),
            _ => Ok(StoredReader { file: self, pos: 0 }),
        }
    }

    /// The whole file in a new vector, refusing a file longer than `max`
    /// before allocating.
    ///
    /// # Errors
    ///
    /// `Limit { FileSize }`; `Io { kind }`.
    ///
    /// # Examples
    ///
    /// ```
    /// let body = b"--X\r\nContent-Disposition: form-data; name=f; filename=a.txt\r\n\r\nhello\r\n--X--";
    /// let upload = form_net::read_multipart(b"multipart/form-data; boundary=X", &body[..])?;
    /// let file = upload.file("f").unwrap();
    /// assert_eq!(file.read_to_vec(1024)?, b"hello");
    /// assert_eq!(file.read_to_vec(4).unwrap_err().code(), "limit.file_size");
    /// # Ok::<(), form_net::FormError>(())
    /// ```
    pub fn read_to_vec(&self, max: u64) -> Result<Vec<u8>, FormError> {
        if self.len > max {
            return Err(FormError::limit(Limit::FileSize, Location::NONE));
        }
        let mut out = Vec::with_capacity(usize::try_from(self.len).unwrap_or(0));
        self.reader()?
            .read_to_end(&mut out)
            .map_err(|e| temp::io(&e))?;
        Ok(out)
    }

    /// The temporary file's path, in `SpillMode::Named` only.
    ///
    /// # Examples
    ///
    /// ```
    /// use form_net::{Limits, Policy, SpillMode, Store};
    ///
    /// # let dir = std::env::temp_dir().join(format!("form-net-doc-path-{}", std::process::id()));
    /// # std::fs::create_dir_all(&dir).unwrap();
    /// let store = Store::in_dir(&dir)?.memory_threshold(0).mode(SpillMode::Named);
    /// let body = b"--X\r\nContent-Disposition: form-data; name=f; filename=a.txt\r\n\r\nhello\r\n--X--";
    /// let upload = form_net::read_multipart_with(b"multipart/form-data; boundary=X", &body[..], &Limits::DEFAULT, &Policy::DEFAULT, &store)?;
    /// let path = upload.file("f").unwrap().path().unwrap();
    /// assert!(path.is_file() && path.parent() == Some(dir.as_path()));
    /// # drop(upload);
    /// # std::fs::remove_dir_all(&dir).unwrap();
    /// # Ok::<(), form_net::FormError>(())
    /// ```
    #[must_use]
    pub fn path(&self) -> Option<&Path> {
        match &self.place {
            Place::Disk { path, .. } => path.as_deref(),
            _ => None,
        }
    }

    fn read_at(&self, buf: &mut [u8], pos: u64) -> io::Result<usize> {
        match &self.place {
            Place::Memory(b) => {
                let start = usize::try_from(pos).unwrap_or(usize::MAX).min(b.len());
                let src = b.get(start..).unwrap_or(&[]);
                let n = src.len().min(buf.len());
                if let (Some(dst), Some(src)) = (buf.get_mut(..n), src.get(..n)) {
                    dst.copy_from_slice(src);
                }
                Ok(n)
            }
            Place::Disk { file: Some(f), .. } => temp::read_at(f, buf, pos),
            _ => Err(io::Error::from(io::ErrorKind::NotFound)),
        }
    }

    /// Moves the finished upload to `dest`, which the caller built. It never
    /// overwrites: a named file is hard-linked when `dest` is on the same file
    /// system and copied otherwise; anonymous and memory files are copied
    /// into `dest` opened with `create_new`. Whether to fsync `dest` and its
    /// directory is the caller's choice.
    ///
    /// On Unix the persisted file is owner-only (`0600`) however it was
    /// made, as the temporary file was; widen it after `persist` if others
    /// must read it. On Windows it takes the destination directory's
    /// inherited ACL (D-155).
    ///
    /// On failure the upload is handed back in the error, untouched, so the
    /// caller can try another name; `?` into a `FormError` drops it, which
    /// deletes the temporary file (D-154).
    ///
    /// # Errors
    ///
    /// `Store { DestinationExists }`; `Io { kind }`; each with the file.
    ///
    /// # Examples
    ///
    /// ```
    /// # let dir = std::env::temp_dir().join(format!("form-net-persist-doc-{}", std::process::id()));
    /// # std::fs::create_dir_all(&dir).unwrap();
    /// let body = b"--X\r\nContent-Disposition: form-data; name=f; filename=a.txt\r\n\r\nhello\r\n--X--";
    /// let mut upload = form_net::read_multipart(b"multipart/form-data; boundary=X", &body[..])?;
    /// let file = upload.take_file("f").unwrap();
    /// std::fs::write(dir.join("a.txt"), b"taken")?;
    /// // The name is taken: the upload comes back, and another name works.
    /// let file = match file.persist(&dir.join("a.txt")) {
    ///     Ok(()) => unreachable!(),
    ///     Err(e) => {
    ///         assert_eq!(e.error().code(), "store.destination_exists");
    ///         e.into_file()
    ///     }
    /// };
    /// file.persist(&dir.join("a (2).txt"))?;
    /// assert_eq!(std::fs::read(dir.join("a (2).txt"))?, b"hello");
    /// # std::fs::remove_dir_all(&dir).unwrap();
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn persist(mut self, dest: &Path) -> Result<(), PersistError> {
        match self.persist_ref(dest) {
            Ok(()) => Ok(()),
            Err(error) => Err(PersistError {
                error,
                file: alloc::boxed::Box::new(self),
            }),
        }
    }

    /// `persist`, leaving `self` intact on failure.
    fn persist_ref(&mut self, dest: &Path) -> Result<(), FormError> {
        let exists = |e: io::Error| {
            if e.kind() == io::ErrorKind::AlreadyExists {
                FormError::Store {
                    reason: StoreError::DestinationExists,
                }
            } else {
                temp::io(&e)
            }
        };
        if let Place::Disk {
            file,
            path: Some(p),
        } = &mut self.place
        {
            // Close before linking and removing: Windows needs it.
            drop(file.take());
            match std::fs::hard_link(&*p, dest) {
                Ok(()) => {
                    let p = p.clone();
                    self.place = Place::Gone;
                    self.release();
                    self.release_budget();
                    if std::fs::remove_file(&p).is_err() {
                        if let Some(s) = &self.shared {
                            s.cleanup_failed();
                        }
                    }
                    return Ok(());
                }
                Err(e) => {
                    // Reopened either way: on `AlreadyExists` the upload goes
                    // back to the caller readable; on another file system it
                    // is copied below, from the name.
                    let f = File::open(&*p).map_err(|e| temp::io(&e))?;
                    *file = Some(f);
                    if e.kind() == io::ErrorKind::AlreadyExists {
                        return Err(exists(e));
                    }
                }
            }
        }
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            // Owner-only, as a hard link to the 0600 temporary file is (D-155).
            options.mode(0o600);
        }
        let mut out = options.open(dest).map_err(exists)?;
        let mut buf = [0u8; 64 * 1024];
        let mut hi = 0usize;
        let mut pos = 0u64;
        let copied = loop {
            let n = match self.read_at(&mut buf, pos) {
                Ok(0) => break Ok(()),
                Ok(n) => n,
                Err(e) => break Err(temp::io(&e)),
            };
            hi = hi.max(n);
            if let Err(e) = out.write_all(buf.get(..n).unwrap_or(&[])) {
                break Err(temp::io(&e));
            }
            pos = pos.saturating_add(n as u64);
        };
        crate::wipe::wipe(buf.get_mut(..hi).unwrap_or(&mut []));
        if copied.is_err() {
            // Never leave a partial copy under the caller's name.
            drop(out);
            let _ = std::fs::remove_file(dest);
        }
        copied
    }

    /// Gives the file's disk bytes back to the shared budget, once.
    fn release_budget(&mut self) {
        if let Some(budget) = self.budget.take() {
            budget.release(self.reserved);
        }
    }

    /// Takes the file off the store's counters.
    fn release(&mut self) {
        if let Some(s) = &self.shared {
            match &self.place {
                Place::Memory(_) => {
                    sub(&s.counters.files_in_memory, 1);
                    sub(&s.counters.bytes_in_memory, self.len);
                }
                _ => {
                    sub(&s.counters.files_on_disk, 1);
                    sub(&s.counters.bytes_on_disk, self.len);
                }
            }
        }
    }
}

/// Wipes a file's name, file name, and declared content type.
fn wipe_meta(
    name: &mut String,
    filename: &mut Option<(Vec<u8>, FileNameSource)>,
    content_type: &mut Option<Vec<u8>>,
) {
    crate::wipe::wipe_string(name);
    if let Some((mut f, _)) = filename.take() {
        crate::wipe::wipe_vec(&mut f);
    }
    if let Some(mut c) = content_type.take() {
        crate::wipe::wipe_vec(&mut c);
    }
}

/// A failed [`StoredFile::persist`]: the error, and the upload, untouched,
/// so the caller can try another name (D-154). Converting it into a
/// `FormError` (as `?` does) drops the file, which deletes it.
///
/// # Examples
///
/// ```
/// fn save(file: form_net::StoredFile, dest: &std::path::Path) -> Result<(), form_net::FormError> {
///     file.persist(dest)?; // on failure the file is dropped and deleted
///     Ok(())
/// }
/// ```
pub struct PersistError {
    error: FormError,
    file: alloc::boxed::Box<StoredFile>,
}

impl PersistError {
    /// Why `persist` failed.
    ///
    /// # Examples
    ///
    /// ```
    /// # let dir = std::env::temp_dir().join(format!("form-net-doc-persist-error-{}", std::process::id()));
    /// # std::fs::create_dir_all(&dir).unwrap();
    /// let body = b"--X\r\nContent-Disposition: form-data; name=f; filename=a.txt\r\n\r\nhello\r\n--X--";
    /// let mut upload = form_net::read_multipart(b"multipart/form-data; boundary=X", &body[..])?;
    /// std::fs::write(dir.join("a.txt"), b"taken")?;
    /// let e = upload.take_file("f").unwrap().persist(&dir.join("a.txt")).unwrap_err();
    /// assert_eq!(e.error().code(), "store.destination_exists");
    /// # drop(e);
    /// # std::fs::remove_dir_all(&dir).unwrap();
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[must_use]
    pub const fn error(&self) -> FormError {
        self.error
    }

    /// The upload, to persist under another name.
    ///
    /// # Examples
    ///
    /// ```
    /// # let dir = std::env::temp_dir().join(format!("form-net-doc-into-file-{}", std::process::id()));
    /// # std::fs::create_dir_all(&dir).unwrap();
    /// let body = b"--X\r\nContent-Disposition: form-data; name=f; filename=a.txt\r\n\r\nhello\r\n--X--";
    /// let mut upload = form_net::read_multipart(b"multipart/form-data; boundary=X", &body[..])?;
    /// std::fs::write(dir.join("a.txt"), b"taken")?;
    /// let e = upload.take_file("f").unwrap().persist(&dir.join("a.txt")).unwrap_err();
    /// e.into_file().persist(&dir.join("b.txt"))?;
    /// assert_eq!(std::fs::read(dir.join("b.txt"))?, b"hello");
    /// # std::fs::remove_dir_all(&dir).unwrap();
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn into_file(self) -> StoredFile {
        *self.file
    }
}

impl From<PersistError> for FormError {
    fn from(e: PersistError) -> FormError {
        e.error
    }
}

impl fmt::Debug for PersistError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PersistError")
            .field("error", &self.error)
            .field("file", &self.file)
            .finish()
    }
}

impl fmt::Display for PersistError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.error, f)
    }
}

impl std::error::Error for PersistError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.error)
    }
}

impl Drop for StoredFile {
    fn drop(&mut self) {
        wipe_meta(&mut self.name, &mut self.filename, &mut self.content_type);
        match core::mem::replace(&mut self.place, Place::Gone) {
            Place::Memory(mut b) => {
                self.place = Place::Memory(Vec::new());
                self.release();
                crate::wipe::wipe_vec(&mut b);
            }
            Place::Disk { file, path } => {
                self.place = Place::Disk {
                    file: None,
                    path: None,
                };
                self.release();
                drop(file);
                let mut freed = true;
                if let Some(p) = path {
                    if std::fs::remove_file(&p).is_err() {
                        freed = false;
                        if let Some(s) = &self.shared {
                            s.cleanup_failed();
                        }
                    }
                }
                // Only space that is free again goes back to the budget: a
                // file that could not be removed still uses its bytes.
                if freed {
                    self.release_budget();
                }
            }
            Place::Gone => {}
        }
    }
}

/// A reader over a [`StoredFile`]; `Read + Seek` with its own position.
///
/// # Examples
///
/// ```
/// use std::io::{Read, Seek, SeekFrom};
///
/// let body = b"--X\r\nContent-Disposition: form-data; name=f; filename=a.txt\r\n\r\nhello\r\n--X--";
/// let upload = form_net::read_multipart(b"multipart/form-data; boundary=X", &body[..])?;
/// let mut reader = upload.file("f").unwrap().reader()?;
/// reader.seek(SeekFrom::Start(2))?;
/// let mut rest = String::new();
/// reader.read_to_string(&mut rest)?;
/// assert_eq!(rest, "llo");
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug)]
pub struct StoredReader<'f> {
    file: &'f StoredFile,
    pos: u64,
}

impl Read for StoredReader<'_> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let n = self.file.read_at(buf, self.pos)?;
        self.pos = self.pos.saturating_add(n as u64);
        Ok(n)
    }
}

impl Seek for StoredReader<'_> {
    fn seek(&mut self, to: SeekFrom) -> io::Result<u64> {
        let target = match to {
            SeekFrom::Start(n) => Some(n),
            SeekFrom::End(d) => self.file.len.checked_add_signed(d),
            SeekFrom::Current(d) => self.pos.checked_add_signed(d),
        };
        match target {
            Some(t) => {
                self.pos = t;
                Ok(t)
            }
            None => Err(io::Error::from(io::ErrorKind::InvalidInput)),
        }
    }
}

/// A position in an upload's text arena.
#[derive(Clone, Copy)]
struct Span {
    name: (usize, usize),
    value: (usize, usize),
}

/// The result of a completed multipart parse with storage: text fields,
/// owned and wiped on drop, and files.
///
/// # Examples
///
/// ```
/// let body = b"--X\r\nContent-Disposition: form-data; name=title\r\n\r\nHoliday\r\n--X\r\nContent-Disposition: form-data; name=photo; filename=a.jpg\r\n\r\nJPEG\r\n--X--";
/// let mut upload = form_net::read_multipart(b"multipart/form-data; boundary=X", &body[..])?;
/// assert_eq!(upload.field("title"), Some("Holiday"));
/// let photo = upload.take_file("photo").unwrap();
/// assert_eq!(photo.bytes(), Some(&b"JPEG"[..]));
/// assert_eq!(upload.file_count(), 0);
/// # Ok::<(), form_net::FormError>(())
/// ```
#[must_use]
#[derive(Default)]
pub struct Upload {
    text: Vec<u8>,
    spans: Vec<Span>,
    files: Vec<StoredFile>,
    token: Option<String>,
}

impl fmt::Debug for Upload {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Upload")
            .field("fields", &self.spans.len())
            .field("text_len", &self.text_len())
            .field("files", &self.files)
            .finish_non_exhaustive()
    }
}

impl Drop for Upload {
    fn drop(&mut self) {
        crate::wipe::wipe_vec(&mut self.text);
        if let Some(t) = self.token.as_mut() {
            crate::wipe::wipe_string(t);
        }
    }
}

impl Upload {
    fn slice(&self, (start, len): (usize, usize)) -> &str {
        let b = self
            .text
            .get(start..start.saturating_add(len))
            .unwrap_or(&[]);
        // Every value was checked as text when it was stored.
        core::str::from_utf8(b).unwrap_or("")
    }

    fn push_field(&mut self, name: &str, value: &[u8], cap: usize) {
        let n0 = self.text.len();
        crate::wipe::append_wiping(&mut self.text, name.as_bytes(), 1024, cap);
        let v0 = self.text.len();
        crate::wipe::append_wiping(&mut self.text, value, 1024, cap);
        self.spans.push(Span {
            name: (n0, name.len()),
            value: (v0, value.len()),
        });
    }

    /// The first text field named `name`.
    ///
    /// # Examples
    ///
    /// ```
    /// let body = b"--X\r\nContent-Disposition: form-data; name=title\r\n\r\nHoliday\r\n--X\r\nContent-Disposition: form-data; name=tag\r\n\r\na\r\n--X\r\nContent-Disposition: form-data; name=tag\r\n\r\nb\r\n--X--";
    /// let upload = form_net::read_multipart(b"multipart/form-data; boundary=X", &body[..])?;
    /// assert_eq!(upload.field("title"), Some("Holiday"));
    /// assert_eq!(upload.field("tag"), Some("a"));
    /// assert_eq!(upload.field("missing"), None);
    /// # Ok::<(), form_net::FormError>(())
    /// ```
    #[must_use]
    pub fn field(&self, name: &str) -> Option<&str> {
        self.spans
            .iter()
            .find(|s| self.slice(s.name) == name)
            .map(|s| self.slice(s.value))
    }

    /// The text field named `name`, refusing a name given twice.
    ///
    /// # Errors
    ///
    /// `Extract { Duplicate }`.
    ///
    /// # Examples
    ///
    /// ```
    /// let body = b"--X\r\nContent-Disposition: form-data; name=title\r\n\r\nHoliday\r\n--X\r\nContent-Disposition: form-data; name=tag\r\n\r\na\r\n--X\r\nContent-Disposition: form-data; name=tag\r\n\r\nb\r\n--X--";
    /// let upload = form_net::read_multipart(b"multipart/form-data; boundary=X", &body[..])?;
    /// assert_eq!(upload.field_one("title")?, Some("Holiday"));
    /// assert_eq!(upload.field_one("tag").unwrap_err().code(), "extract.duplicate");
    /// # Ok::<(), form_net::FormError>(())
    /// ```
    pub fn field_one(&self, name: &str) -> Result<Option<&str>, FormError> {
        let mut found = None;
        for s in self.spans.iter().filter(|s| self.slice(s.name) == name) {
            if found.is_some() {
                return Err(FormError::Extract {
                    reason: ExtractError::Duplicate,
                    field: None,
                });
            }
            found = Some(self.slice(s.value));
        }
        Ok(found)
    }

    /// Every text field named `name`, in body order.
    ///
    /// # Examples
    ///
    /// ```
    /// let body = b"--X\r\nContent-Disposition: form-data; name=title\r\n\r\nHoliday\r\n--X\r\nContent-Disposition: form-data; name=tag\r\n\r\na\r\n--X\r\nContent-Disposition: form-data; name=tag\r\n\r\nb\r\n--X--";
    /// let upload = form_net::read_multipart(b"multipart/form-data; boundary=X", &body[..])?;
    /// assert_eq!(upload.fields("tag").collect::<Vec<_>>(), ["a", "b"]);
    /// # Ok::<(), form_net::FormError>(())
    /// ```
    pub fn fields<'u>(&'u self, name: &'u str) -> impl Iterator<Item = &'u str> + 'u {
        self.spans
            .iter()
            .filter(move |s| self.slice(s.name) == name)
            .map(move |s| self.slice(s.value))
    }

    /// Every text field `(name, value)`, in body order.
    ///
    /// # Examples
    ///
    /// ```
    /// let body = b"--X\r\nContent-Disposition: form-data; name=title\r\n\r\nHoliday\r\n--X\r\nContent-Disposition: form-data; name=tag\r\n\r\na\r\n--X\r\nContent-Disposition: form-data; name=tag\r\n\r\nb\r\n--X--";
    /// let upload = form_net::read_multipart(b"multipart/form-data; boundary=X", &body[..])?;
    /// let pairs: Vec<_> = upload.iter().collect();
    /// assert_eq!(pairs, [("title", "Holiday"), ("tag", "a"), ("tag", "b")]);
    /// # Ok::<(), form_net::FormError>(())
    /// ```
    pub fn iter(&self) -> impl Iterator<Item = (&str, &str)> {
        self.spans
            .iter()
            .map(|s| (self.slice(s.name), self.slice(s.value)))
    }

    /// The first file named `name`.
    ///
    /// # Examples
    ///
    /// ```
    /// let body = b"--X\r\nContent-Disposition: form-data; name=title\r\n\r\nHoliday\r\n--X\r\nContent-Disposition: form-data; name=f; filename=a.txt\r\n\r\n1\r\n--X\r\nContent-Disposition: form-data; name=f; filename=b.txt\r\n\r\n22\r\n--X--";
    /// let upload = form_net::read_multipart(b"multipart/form-data; boundary=X", &body[..])?;
    /// assert_eq!(upload.file("f").map(|f| f.len()), Some(1));
    /// assert!(upload.file("title").is_none(), "a text field is not a file");
    /// # Ok::<(), form_net::FormError>(())
    /// ```
    #[must_use]
    pub fn file(&self, name: &str) -> Option<&StoredFile> {
        self.files.iter().find(|f| f.name() == name)
    }

    /// Every file named `name`, in body order.
    ///
    /// # Examples
    ///
    /// ```
    /// let body = b"--X\r\nContent-Disposition: form-data; name=title\r\n\r\nHoliday\r\n--X\r\nContent-Disposition: form-data; name=f; filename=a.txt\r\n\r\n1\r\n--X\r\nContent-Disposition: form-data; name=f; filename=b.txt\r\n\r\n22\r\n--X--";
    /// let upload = form_net::read_multipart(b"multipart/form-data; boundary=X", &body[..])?;
    /// let lens: Vec<u64> = upload.files("f").map(|f| f.len()).collect();
    /// assert_eq!(lens, [1, 2]);
    /// # Ok::<(), form_net::FormError>(())
    /// ```
    pub fn files<'u>(&'u self, name: &'u str) -> impl Iterator<Item = &'u StoredFile> + 'u {
        self.files.iter().filter(move |f| f.name() == name)
    }

    /// Removes and returns the first file named `name`.
    ///
    /// # Examples
    ///
    /// ```
    /// let body = b"--X\r\nContent-Disposition: form-data; name=title\r\n\r\nHoliday\r\n--X\r\nContent-Disposition: form-data; name=f; filename=a.txt\r\n\r\n1\r\n--X\r\nContent-Disposition: form-data; name=f; filename=b.txt\r\n\r\n22\r\n--X--";
    /// let mut upload = form_net::read_multipart(b"multipart/form-data; boundary=X", &body[..])?;
    /// let first = upload.take_file("f").unwrap();
    /// assert_eq!(first.bytes(), Some(&b"1"[..]));
    /// assert_eq!(upload.file("f").map(|f| f.len()), Some(2));
    /// assert_eq!(upload.file_count(), 1);
    /// # Ok::<(), form_net::FormError>(())
    /// ```
    pub fn take_file(&mut self, name: &str) -> Option<StoredFile> {
        let i = self.files.iter().position(|f| f.name() == name)?;
        Some(self.files.remove(i))
    }

    /// Every file, in body order.
    ///
    /// # Examples
    ///
    /// ```
    /// let body = b"--X\r\nContent-Disposition: form-data; name=title\r\n\r\nHoliday\r\n--X\r\nContent-Disposition: form-data; name=f; filename=a.txt\r\n\r\n1\r\n--X\r\nContent-Disposition: form-data; name=f; filename=b.txt\r\n\r\n22\r\n--X--";
    /// let upload = form_net::read_multipart(b"multipart/form-data; boundary=X", &body[..])?;
    /// let files = upload.into_files();
    /// assert_eq!(files.iter().map(|f| f.len()).collect::<Vec<_>>(), [1, 2]);
    /// # Ok::<(), form_net::FormError>(())
    /// ```
    #[must_use]
    pub fn into_files(mut self) -> Vec<StoredFile> {
        core::mem::take(&mut self.files)
    }

    /// One owned type for both encodings (D-090, D-098): the fields of a
    /// urlencoded form as an upload with no files, so a server keeps one
    /// `Upload` per request whatever the `Content-Type`. The policy's token
    /// field is captured as a multipart parse would capture it, so
    /// [`captured_token`](Self::captured_token) means the same for both.
    /// The form is wiped as it drops.
    ///
    /// # Errors
    ///
    /// `Policy { DuplicateToken }` when the token field appears twice.
    ///
    /// # Examples
    ///
    /// ```
    /// use form_net::{Policy, TokenField, Upload};
    ///
    /// let policy = Policy::DEFAULT.with_token_field(TokenField::new("csrf"));
    /// let form = form_net::parse_urlencoded(b"email=a%40b.example&csrf=t0k&tag=x&tag=y")?;
    /// let upload = Upload::from_form(form, &policy)?;
    /// assert_eq!(upload.field("email"), Some("a@b.example"));
    /// assert_eq!(upload.captured_token(), Some("t0k"));
    /// assert_eq!((upload.fields("tag").count(), upload.file_count()), (2, 0));
    /// # Ok::<(), form_net::FormError>(())
    /// ```
    pub fn from_form(form: crate::Form, policy: &Policy) -> Result<Upload, FormError> {
        let mut upload = Upload::default();
        let token = policy.token_field.map(|t| t.name());
        for (name, value) in form.iter() {
            if Some(name) == token {
                if upload.token.is_some() {
                    return Err(FormError::policy(
                        crate::error::PolicyRefusal::DuplicateToken,
                        Location::NONE,
                    ));
                }
                upload.token = Some(String::from(value));
            }
            upload.push_field(name, value.as_bytes(), usize::MAX);
        }
        Ok(upload)
    }

    /// The stored files, for extraction that moves some of them out.
    pub(crate) fn files_mut(&mut self) -> &mut Vec<StoredFile> {
        &mut self.files
    }

    pub(crate) fn into_parts(mut self) -> (Upload, Vec<StoredFile>) {
        let files = core::mem::take(&mut self.files);
        (self, files)
    }

    /// The anti-forgery token field's value, when a token field was set.
    ///
    /// # Examples
    ///
    /// ```
    /// use form_net::{Limits, Policy, Store, TokenField};
    ///
    /// let store = Store::memory_only();
    /// let policy = Policy::DEFAULT.with_token_field(TokenField::new("csrf"));
    /// let body = b"--X\r\nContent-Disposition: form-data; name=csrf\r\n\r\nt0k\r\n--X\r\nContent-Disposition: form-data; name=f; filename=a.txt\r\n\r\nhello\r\n--X--";
    /// let upload = form_net::read_multipart_with(b"multipart/form-data; boundary=X", &body[..], &Limits::DEFAULT, &policy, &store)?;
    /// assert_eq!(upload.captured_token(), Some("t0k"));
    /// # Ok::<(), form_net::FormError>(())
    /// ```
    #[must_use]
    pub fn captured_token(&self) -> Option<&str> {
        self.token.as_deref()
    }

    /// The bytes of all text values together.
    ///
    /// # Examples
    ///
    /// ```
    /// let body = b"--X\r\nContent-Disposition: form-data; name=title\r\n\r\nHoliday\r\n--X\r\nContent-Disposition: form-data; name=tag\r\n\r\na\r\n--X\r\nContent-Disposition: form-data; name=tag\r\n\r\nb\r\n--X--";
    /// let upload = form_net::read_multipart(b"multipart/form-data; boundary=X", &body[..])?;
    /// assert_eq!(upload.text_len(), 9); // "Holiday", "a", "b"
    /// # Ok::<(), form_net::FormError>(())
    /// ```
    #[must_use]
    pub fn text_len(&self) -> u64 {
        self.spans.iter().map(|s| s.value.1 as u64).sum()
    }

    /// The number of files.
    ///
    /// # Examples
    ///
    /// ```
    /// let body = b"--X\r\nContent-Disposition: form-data; name=title\r\n\r\nHoliday\r\n--X\r\nContent-Disposition: form-data; name=f; filename=a.txt\r\n\r\n1\r\n--X\r\nContent-Disposition: form-data; name=f; filename=b.txt\r\n\r\n22\r\n--X--";
    /// let upload = form_net::read_multipart(b"multipart/form-data; boundary=X", &body[..])?;
    /// assert_eq!(upload.file_count(), 2);
    /// # Ok::<(), form_net::FormError>(())
    /// ```
    #[must_use]
    pub fn file_count(&self) -> usize {
        self.files.len()
    }
}

/// The push-style form for event-driven servers: hand each body chunk to
/// [`push`](Self::push) as reads complete, then [`finish`](Self::finish).
///
/// Dropping a session without `finish` deletes every file it stored.
/// [`captured_token`](Self::captured_token) is available as soon as the
/// token field ends, so the token can be verified while files are still
/// arriving.
///
/// # Examples
///
/// ```
/// use form_net::{Limits, NoInspector, Policy, Store, UploadSession};
///
/// let store = Store::temp()?;
/// let body = b"--X\r\nContent-Disposition: form-data; name=a\r\n\r\n1\r\n--X--";
/// let mut session = UploadSession::new(b"multipart/form-data; boundary=X", &Limits::DEFAULT, &Policy::DEFAULT, &store)?;
/// for chunk in body.chunks(5) {
///     session.push(chunk, &mut NoInspector)?;
/// }
/// let upload = session.finish(&mut NoInspector)?;
/// assert_eq!(upload.field("a"), Some("1"));
/// # Ok::<(), form_net::FormError>(())
/// ```
pub struct UploadSession {
    driver: Driver<Vec<u8>, StoreSink>,
    upload: Upload,
    text_cap: usize,
    ended: bool,
    failed: bool,
}

impl fmt::Debug for UploadSession {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("UploadSession")
            .field("bytes_seen", &self.bytes_seen())
            .field("upload", &self.upload)
            .finish_non_exhaustive()
    }
}

impl UploadSession {
    /// A session for one request body.
    ///
    /// # Errors
    ///
    /// `UnsupportedMediaType` and the boundary errors of
    /// [`FormKind::detect`](crate::FormKind::detect); `InvalidLimits`.
    ///
    /// # Examples
    ///
    /// ```
    /// use form_net::{Limits, Policy, Store, UploadSession};
    ///
    /// let (store, policy) = (Store::memory_only(), Policy::DEFAULT);
    /// let session = UploadSession::new(b"multipart/form-data; boundary=X", &Limits::DEFAULT, &policy, &store)?;
    /// assert_eq!(session.bytes_seen(), 0);
    /// let e = UploadSession::new(b"text/plain", &Limits::DEFAULT, &policy, &store).unwrap_err();
    /// assert_eq!(e.code(), "unsupported_media_type");
    /// # Ok::<(), form_net::FormError>(())
    /// ```
    pub fn new(
        content_type: &[u8],
        limits: &Limits,
        policy: &Policy,
        store: &Store,
    ) -> Result<Self, FormError> {
        let boundary = crate::Boundary::parse(content_type)?;
        let parser = MultipartParser::new(boundary, limits, policy)?;
        Ok(UploadSession {
            driver: Driver::new(parser, store.sink(limits)),
            upload: Upload::default(),
            text_cap: usize::try_from(limits.text_and_names()).unwrap_or(usize::MAX),
            ended: false,
            failed: false,
        })
    }

    /// Feeds one chunk; consumes all of it or fails.
    ///
    /// # Errors
    ///
    /// Every multipart error, `Refused`, `Io`, `Store`, `Limit { Disk }`;
    /// `Poisoned` after an error.
    ///
    /// # Examples
    ///
    /// ```
    /// use form_net::{Limits, NoInspector, Policy, Store, UploadSession};
    ///
    /// let (store, policy) = (Store::memory_only(), Policy::DEFAULT);
    /// let mut session = UploadSession::new(b"multipart/form-data; boundary=X", &Limits::DEFAULT, &policy, &store)?;
    /// let body = b"--X\r\nContent-Disposition: form-data; name=f; filename=a.txt\r\n\r\nhello\r\n--X--";
    /// for chunk in body.chunks(7) {
    ///     session.push(chunk, &mut NoInspector)?;
    /// }
    /// let upload = session.finish(&mut NoInspector)?;
    /// assert_eq!(upload.file("f").unwrap().bytes(), Some(&b"hello"[..]));
    /// # Ok::<(), form_net::FormError>(())
    /// ```
    pub fn push<I: Inspector>(&mut self, chunk: &[u8], inspector: &mut I) -> Result<(), FormError> {
        self.feed(chunk, false, inspector)
    }

    /// Ends the body and returns the upload.
    ///
    /// # Errors
    ///
    /// `Syntax { Truncated }` if the close delimiter was not seen; as
    /// [`push`](Self::push).
    ///
    /// # Examples
    ///
    /// ```
    /// use form_net::{Limits, NoInspector, Policy, Store, UploadSession};
    ///
    /// let (store, policy) = (Store::memory_only(), Policy::DEFAULT);
    /// let mut session = UploadSession::new(b"multipart/form-data; boundary=X", &Limits::DEFAULT, &policy, &store)?;
    /// // The body stops before its close delimiter.
    /// session.push(b"--X\r\nContent-Disposition: form-data; name=a\r\n\r\n1", &mut NoInspector)?;
    /// let e = session.finish(&mut NoInspector).unwrap_err();
    /// assert_eq!(e.code(), "syntax.truncated");
    /// # Ok::<(), form_net::FormError>(())
    /// ```
    pub fn finish<I: Inspector>(mut self, inspector: &mut I) -> Result<Upload, FormError> {
        if self.failed {
            return Err(FormError::Poisoned);
        }
        if !self.ended {
            self.feed(&[], true, inspector)?;
        }
        let mut upload = core::mem::take(&mut self.upload);
        upload.token = self.driver.parser().captured_token().map(String::from);
        Ok(upload)
    }

    /// The token field's value, once its part has ended.
    ///
    /// # Examples
    ///
    /// ```
    /// use form_net::{Limits, NoInspector, Policy, Store, TokenField, UploadSession};
    ///
    /// let store = Store::memory_only();
    /// let policy = Policy::DEFAULT.with_token_field(TokenField::new("csrf"));
    /// let mut session = UploadSession::new(b"multipart/form-data; boundary=X", &Limits::DEFAULT, &policy, &store)?;
    /// let body = b"--X\r\nContent-Disposition: form-data; name=csrf\r\n\r\nt0k\r\n--X\r\nContent-Disposition: form-data; name=f; filename=a.txt\r\n\r\nhello\r\n--X--";
    /// session.push(body, &mut NoInspector)?;
    /// // Known before `finish`, so the token can be checked first.
    /// assert_eq!(session.captured_token(), Some("t0k"));
    /// # Ok::<(), form_net::FormError>(())
    /// ```
    #[must_use]
    pub fn captured_token(&self) -> Option<&str> {
        self.driver.parser().captured_token()
    }

    /// Body bytes consumed so far.
    ///
    /// # Examples
    ///
    /// ```
    /// use form_net::{Limits, NoInspector, Policy, Store, UploadSession};
    ///
    /// let (store, policy) = (Store::memory_only(), Policy::DEFAULT);
    /// let mut session = UploadSession::new(b"multipart/form-data; boundary=X", &Limits::DEFAULT, &policy, &store)?;
    /// let body = b"--X\r\nContent-Disposition: form-data; name=f; filename=a.txt\r\n\r\nhello\r\n--X--";
    /// session.push(&body[..10], &mut NoInspector)?;
    /// assert_eq!(session.bytes_seen(), 10);
    /// # Ok::<(), form_net::FormError>(())
    /// ```
    #[must_use]
    pub fn bytes_seen(&self) -> u64 {
        self.driver.parser().bytes_seen()
    }

    fn feed<I: Inspector>(
        &mut self,
        chunk: &[u8],
        eof: bool,
        inspector: &mut I,
    ) -> Result<(), FormError> {
        if self.failed {
            return Err(FormError::Poisoned);
        }
        let r = self.feed_inner(chunk, eof, inspector);
        if let Err(e) = &r {
            self.failed = true;
            // The driver reports its own failures; a field that fails its
            // text check fails here, after the driver returned.
            if !self.driver.has_failed() {
                inspector.failed(e);
            }
            // Nothing of a refused body survives.
            self.upload.files.clear();
        }
        r
    }

    fn feed_inner<I: Inspector>(
        &mut self,
        chunk: &[u8],
        eof: bool,
        inspector: &mut I,
    ) -> Result<(), FormError> {
        let mut pos = 0usize;
        loop {
            let rest = chunk.get(pos..).unwrap_or(&[]);
            let (n, progress) = self.driver.push(rest, eof, inspector)?;
            pos = pos.saturating_add(n);
            match progress {
                Progress::Field { name, value } => {
                    // Checked as text by the driver before the inspector
                    // saw it, as the buffered parser checks it.
                    let cap = self.text_cap;
                    self.upload.push_field(name, value, cap);
                }
                Progress::File(f) => self.upload.files.push(f),
                Progress::End => {
                    self.ended = true;
                    if pos >= chunk.len() {
                        return Ok(());
                    }
                }
                _ => {
                    if pos >= chunk.len() {
                        return Ok(());
                    }
                }
            }
        }
    }
}

/// The blocking convenience: reads `body` to the end in 64 KiB chunks with
/// the default limits and policy, keeps files up to 64 KiB in memory, and
/// spills larger ones to anonymous temporary files in a private directory
/// that is created only when a file first spills and removed when the last
/// file from it is dropped. There is no process-wide state.
///
/// # Errors
///
/// Every multipart error; `Io { kind }`; `Limit { Disk }`; `Random`.
///
/// # Examples
///
/// ```
/// let body = b"--X\r\nContent-Disposition: form-data; name=title\r\n\r\nHi\r\n--X--";
/// let upload = form_net::read_multipart(b"multipart/form-data; boundary=X", &body[..])?;
/// assert_eq!(upload.field("title"), Some("Hi"));
/// # Ok::<(), form_net::FormError>(())
/// ```
pub fn read_multipart<R: Read>(content_type: &[u8], body: R) -> Result<Upload, FormError> {
    let store = Store::temp_lazy();
    read_multipart_with(
        content_type,
        body,
        &Limits::DEFAULT,
        &Policy::DEFAULT,
        &store,
    )
}

/// [`read_multipart`] with explicit limits, policy, and store: reads the
/// whole body from `body` in 64 KiB chunks (replaces `Upload::parse_with`,
/// D-148).
///
/// # Errors
///
/// Every multipart error; `Io { kind }`; `Store { .. }`; `Limit { Disk }`;
/// `InvalidLimits`.
///
/// # Examples
///
/// ```
/// use form_net::{Limits, Policy, Store, TokenField};
///
/// let store = Store::temp()?;
/// let policy = Policy::DEFAULT.with_token_field(TokenField::new("csrf_token"));
/// let body = b"--X\r\nContent-Disposition: form-data; name=csrf_token\r\n\r\ntok\r\n--X--";
/// let upload = form_net::read_multipart_with(
///     b"multipart/form-data; boundary=X", &body[..], &Limits::DEFAULT, &policy, &store,
/// )?;
/// assert_eq!(upload.captured_token(), Some("tok"));
/// # Ok::<(), form_net::FormError>(())
/// ```
pub fn read_multipart_with<R: Read>(
    content_type: &[u8],
    body: R,
    limits: &Limits,
    policy: &Policy,
    store: &Store,
) -> Result<Upload, FormError> {
    read_multipart_inspected(
        content_type,
        body,
        limits,
        policy,
        store,
        &mut crate::NoInspector,
    )
}

/// [`read_multipart_with`] with an inspector: every part passes through it
/// in the hook order, before any file reaches the store. `failed` is called
/// once on every failed read, setup errors included (D-144).
///
/// # Errors
///
/// Every multipart error; `Refused`; `Io { kind }`; `Store { .. }`;
/// `Limit { Disk }`; `InvalidLimits`.
///
/// # Examples
///
/// ```
/// use form_net::{Limits, Policy, Store, TokenCheck, TokenField};
///
/// let store = Store::temp()?;
/// let policy = Policy::DEFAULT.with_token_field(TokenField::new("csrf_token"));
/// let mut check = TokenCheck::new("csrf_token", |t| {
///     if t == "tok" { Ok(()) } else { Err(form_net::Refusal::new(403, "csrf.bad")) }
/// });
/// let body = b"--X\r\nContent-Disposition: form-data; name=csrf_token\r\n\r\ntok\r\n--X--";
/// let upload = form_net::read_multipart_inspected(
///     b"multipart/form-data; boundary=X", &body[..], &Limits::DEFAULT, &policy, &store, &mut check,
/// )?;
/// assert!(check.verified());
/// assert_eq!(upload.captured_token(), Some("tok"));
/// # Ok::<(), form_net::FormError>(())
/// ```
pub fn read_multipart_inspected<R: Read, I: Inspector>(
    content_type: &[u8],
    mut reader: R,
    limits: &Limits,
    policy: &Policy,
    store: &Store,
    inspector: &mut I,
) -> Result<Upload, FormError> {
    // `failed` on every failed parse, setup included, as the buffered
    // entry points call it (D-043, D-144).
    let mut session = match UploadSession::new(content_type, limits, policy, store) {
        Ok(s) => s,
        Err(e) => {
            inspector.failed(&e);
            return Err(e);
        }
    };
    let mut buf = crate::wipe::ReadBuf::new(64 * 1024);
    loop {
        let n = match reader.read(&mut buf.buf) {
            Ok(n) => n,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => {
                buf.failed();
                continue;
            }
            Err(e) => {
                buf.failed();
                // The client's side, not the store's (D-159).
                let err = FormError::BodyRead { kind: e.kind() };
                inspector.failed(&err);
                return Err(err);
            }
        };
        if n == 0 {
            break;
        }
        buf.filled(n);
        session.push(buf.buf.get(..n).unwrap_or(&[]), inspector)?;
    }
    drop(buf);
    session.finish(inspector)
}

impl FileName<'_> {
    /// One portable file name from the client's: the HTML-unescaped base
    /// name with every default-ignorable character (zero width spaces and
    /// joiners, soft hyphens, variation selectors, tags, bidirectional
    /// controls) replaced by `_` (D-084), then passed through
    /// `path_lib::sanitize`, which replaces separators, control characters,
    /// Windows-reserved characters and device names, and trims trailing dots
    /// and spaces. With the `unicode` feature the name is put in Unicode NFC
    /// first, so an NFD name from a macOS client sanitizes to the same bytes.
    ///
    /// The result is at most 255 bytes. A longer name loses bytes from the
    /// end of its stem, never its extension: when
    /// [`extension`](Self::extension) is `Some(e)`, the result ends with
    /// `.e`, so the name an extension check saw is the name saved (D-156). A
    /// leading `.` becomes `_`, so `.htaccess` is never kept as a dot file
    /// (D-157). It is still a name the client chose: never let it overwrite
    /// anything (`StoredFile::persist` refuses to).
    ///
    /// # Errors
    ///
    /// `Encoding { InvalidUtf8 }`.
    ///
    /// # Examples
    ///
    /// ```
    /// let body = b"--X\r\nContent-Disposition: form-data; name=f; filename=\"C:\\x\\Q3 report: final?.pdf\"\r\n\r\n\r\n--X--";
    /// let mp = form_net::parse_multipart(b"multipart/form-data; boundary=X", body)?;
    /// let name = mp.file("f").unwrap().filename().unwrap();
    /// assert_eq!(name.sanitized()?, "Q3 report_ final_.pdf");
    /// # Ok::<(), form_net::FormError>(())
    /// ```
    #[cfg_attr(docsrs, doc(cfg(feature = "fs")))]
    pub fn sanitized(&self) -> Result<String, FormError> {
        let base = FileName::new(self.basename(), self.source());
        let text = base.html_unescaped()?;
        #[cfg(feature = "unicode")]
        let text = unicode_lang::normalize(text.as_ref(), unicode_lang::Form::Nfc);
        let text: &str = text.as_ref();
        let out = if text.is_ascii() {
            path_lib::sanitize(text)
        } else {
            let visible: String = text
                .chars()
                .map(|c| if is_default_ignorable(c) { '_' } else { c })
                .collect();
            path_lib::sanitize(visible)
        };
        Ok(keep_extension(no_leading_dot(out), self.extension()))
    }
}

/// The longest name `sanitized` returns, in bytes: what common file systems
/// allow for one name, and what `path_lib::sanitize` cuts to.
const MAX_NAME_BYTES: usize = 255;

/// A leading `.` replaced by `_`: a dot file (`.htaccess`, `.user.ini`) can
/// change how a directory is served (D-157).
fn no_leading_dot(mut name: String) -> String {
    if name.starts_with('.') {
        name.replace_range(..1, "_");
    }
    name
}

/// `name`, ending with `.ext` when the client's name had that extension.
/// `path_lib::sanitize` keeps the first 255 bytes, so a long name can lose
/// its extension or end in another one (`A…A.php.jpg` saved as `A…A.php`);
/// here the stem is cut instead (D-156).
fn keep_extension(name: String, ext: Option<&str>) -> String {
    let Some(ext) = ext else {
        return name;
    };
    let tail_len = ext.len().saturating_add(1);
    let has_tail = name
        .len()
        .checked_sub(tail_len)
        .and_then(|i| name.get(i..))
        .is_some_and(|tail| tail.strip_prefix('.') == Some(ext));
    if has_tail {
        return name;
    }
    let mut keep = name.len().min(MAX_NAME_BYTES.saturating_sub(tail_len));
    while keep > 0 && !name.is_char_boundary(keep) {
        keep = keep.saturating_sub(1);
    }
    let mut out = String::with_capacity(keep.saturating_add(tail_len));
    out.push_str(name.get(..keep).unwrap_or(""));
    out.push('.');
    out.push_str(ext);
    // Once more through the sanitizer: the cut can leave a reserved name or
    // a trailing space before the dot. The result fits, so nothing is cut.
    no_leading_dot(path_lib::sanitize(out))
}

/// Unicode's `Default_Ignorable_Code_Point` property (DerivedCoreProperties):
/// characters that render as nothing, so a name can hide them.
const fn is_default_ignorable(c: char) -> bool {
    matches!(
        c,
        '\u{00AD}'
            | '\u{034F}'
            | '\u{061C}'
            | '\u{115F}'..='\u{1160}'
            | '\u{17B4}'..='\u{17B5}'
            | '\u{180B}'..='\u{180F}'
            | '\u{200B}'..='\u{200F}'
            | '\u{202A}'..='\u{202E}'
            | '\u{2060}'..='\u{206F}'
            | '\u{3164}'
            | '\u{FE00}'..='\u{FE0F}'
            | '\u{FEFF}'
            | '\u{FFA0}'
            | '\u{FFF0}'..='\u{FFF8}'
            | '\u{1BCA0}'..='\u{1BCA3}'
            | '\u{1D173}'..='\u{1D17A}'
            | '\u{E0000}'..='\u{E0FFF}'
    )
}

impl crate::FromField for StoredFile {
    /// Whether the field arrived (an empty file input counts), and the file.
    type Slot = (bool, Option<StoredFile>);

    fn accept_file(
        slot: &mut Self::Slot,
        file: crate::FileValue<'_>,
    ) -> Result<(), crate::ValueError> {
        // Before the empty-input shortcut, as for `FileData` (D-137).
        if slot.0 {
            return Err(crate::ValueError::Duplicate);
        }
        slot.0 = true;
        let stored = match file {
            crate::FileValue::Stored(f) => f,
            crate::FileValue::Buffered(part) => StoredFile::detached(
                part.name(),
                part.filename(),
                part.content_type().map(|c| c.raw()),
                part.bytes(),
            ),
        };
        if stored.is_empty() && stored.filename().is_some_and(|n| n.is_empty()) {
            // A file input left empty: as if it were absent.
            return Ok(());
        }
        slot.1 = Some(stored);
        Ok(())
    }

    fn finish(slot: Self::Slot) -> Result<Self, crate::ValueError> {
        slot.1.ok_or(crate::ValueError::Missing)
    }
}
