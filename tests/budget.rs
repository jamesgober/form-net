//! The shared disk budget (FR-49, T-7) and `sweep` (FR-48, T-11).

#![cfg(feature = "fs")]

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use form_net::{
    FormError, Limits, NoInspector, Policy, SpillBudget, SpillMode, SpillQuota, Store, StoreError,
    Upload,
};

const CT: &[u8] = b"multipart/form-data; boundary=X";

fn dir(tag: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!(
        "form-net-budget-{tag}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&p).unwrap();
    p
}

fn body(files: usize, size: usize) -> Vec<u8> {
    let mut b = Vec::new();
    for i in 0..files {
        b.extend_from_slice(
            format!("--X\r\nContent-Disposition: form-data; name=f{i}; filename=a\r\n\r\n")
                .as_bytes(),
        );
        b.extend(std::iter::repeat_n(b'x', size));
        b.extend_from_slice(b"\r\n");
    }
    b.extend_from_slice(b"--X--");
    b
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

/// A quota that records the highest level it ever allowed.
#[derive(Clone)]
struct Watched {
    quota: SpillQuota,
    high: Arc<AtomicU64>,
}

impl SpillBudget for Watched {
    fn reserve(&self, bytes: u64) -> bool {
        let ok = self.quota.reserve(bytes);
        if ok {
            self.high.fetch_max(self.quota.in_use(), Ordering::Relaxed);
        }
        ok
    }
    fn release(&self, bytes: u64) {
        self.quota.release(bytes);
    }
}

#[test]
fn test_attack_t7_parallel_uploads_never_exceed_the_shared_budget() {
    let d = dir("t7");
    let quota = SpillQuota::new(64 * 1024);
    let watched = Watched {
        quota: quota.clone(),
        high: Arc::new(AtomicU64::new(0)),
    };
    let store = Arc::new(
        Store::in_dir(&d)
            .unwrap()
            .memory_threshold(0)
            .quota(watched.clone()),
    );
    let b: Arc<[u8]> = body(2, 6000).into();
    let threads: Vec<_> = (0..16)
        .map(|_| {
            let (store, b) = (Arc::clone(&store), Arc::clone(&b));
            std::thread::spawn(move || {
                let mut refused = 0;
                for _ in 0..20 {
                    match upload(&store, &b) {
                        // Hold the files for a moment, then drop them.
                        Ok(u) => drop(u),
                        Err(FormError::Store {
                            reason: StoreError::QuotaExceeded,
                        }) => refused += 1,
                        Err(e) => panic!("{e:?}"),
                    }
                }
                refused
            })
        })
        .collect();
    let refused: u32 = threads.into_iter().map(|t| t.join().unwrap()).sum();
    assert!(
        watched.high.load(Ordering::Relaxed) <= 64 * 1024,
        "over the budget"
    );
    assert_eq!(quota.in_use(), 0, "every reserved byte was released");
    assert_eq!(std::fs::read_dir(&d).unwrap().count(), 0);
    // 16 threads × 12 KB in flight against 64 KB: some must have been refused.
    assert!(refused > 0, "the budget never bit");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn test_quota_errors_hint_503_and_persist_releases() {
    let d = dir("persist");
    let quota = SpillQuota::new(10);
    let store = Store::in_dir(&d)
        .unwrap()
        .memory_threshold(0)
        .mode(SpillMode::Named)
        .quota(quota.clone());
    let e = upload(&store, &body(1, 11)).unwrap_err();
    assert_eq!((e.status_hint(), e.code()), (503, "store.quota_exceeded"));
    assert_eq!(quota.in_use(), 0);
    let mut up = upload(&store, &body(1, 10)).unwrap();
    assert_eq!(quota.in_use(), 10);
    let dest = d.join("kept.bin");
    up.take_file("f0").unwrap().persist(&dest).unwrap();
    assert_eq!(
        quota.in_use(),
        0,
        "persisted bytes leave the temporary area"
    );
    assert_eq!(std::fs::read(&dest).unwrap().len(), 10);
    // Live limit change: new reservations see it.
    quota.set_limit(0);
    assert!(upload(&store, &body(1, 1)).is_err());
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn test_idea_a_budget_per_tenant() {
    // PLAN 14, 0.5: one quota per tenant; a tenant that fills its share
    // does not stop another.
    let d = dir("tenant");
    let (a, b) = (SpillQuota::new(8 * 1024), SpillQuota::new(8 * 1024));
    let store_a = Store::in_dir(&d)
        .unwrap()
        .memory_threshold(0)
        .quota(a.clone());
    let store_b = Store::in_dir(&d)
        .unwrap()
        .memory_threshold(0)
        .quota(b.clone());
    let held: Vec<Upload> = (0..2)
        .filter_map(|_| upload(&store_a, &body(1, 4000)).ok())
        .collect();
    assert_eq!(held.len(), 2);
    assert!(
        upload(&store_a, &body(1, 4000)).is_err(),
        "tenant A is full"
    );
    assert!(
        upload(&store_b, &body(1, 4000)).is_ok(),
        "tenant B is not affected"
    );
    drop(held);
    assert_eq!((a.in_use(), b.in_use()), (0, 0));
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn test_sweep_removes_only_old_spill_files() {
    let d = dir("sweep");
    let spill = |n: u8| {
        d.join(format!(
            "upload-{}.part",
            format!("{n:x}").repeat(32).get(..32).unwrap().to_owned()
        ))
    };
    std::fs::write(spill(1), b"old").unwrap();
    std::fs::write(spill(2), b"new").unwrap();
    std::fs::write(d.join("upload-x.part"), b"not ours").unwrap();
    std::fs::write(d.join("photo.png"), b"not ours").unwrap();
    std::fs::create_dir(spill(3)).unwrap();
    let hour_ago = std::time::SystemTime::now() - Duration::from_secs(3600);
    std::fs::File::options()
        .write(true)
        .open(spill(1))
        .unwrap()
        .set_modified(hour_ago)
        .unwrap();
    assert_eq!(form_net::sweep(&d, Duration::from_secs(600)).unwrap(), 1);
    assert!(!spill(1).exists() && spill(2).exists() && spill(3).is_dir());
    assert!(d.join("upload-x.part").exists() && d.join("photo.png").exists());
    assert_eq!(
        form_net::sweep(&d, Duration::ZERO).unwrap(),
        1,
        "the young one, with no minimum age"
    );
    assert!(matches!(
        form_net::sweep(&d.join("photo.png"), Duration::ZERO),
        Err(FormError::Store {
            reason: StoreError::NotADirectory
        })
    ));
    let _ = std::fs::remove_dir_all(&d);
}

#[cfg(unix)]
#[test]
fn test_sweep_never_follows_links() {
    let d = dir("links");
    let outside = dir("outside");
    let target = outside.join("upload-0123456789abcdef0123456789abcdef.part");
    std::fs::write(&target, b"theirs").unwrap();
    let link = d.join("upload-fedcba9876543210fedcba9876543210.part");
    std::os::unix::fs::symlink(&target, &link).unwrap();
    std::os::unix::fs::symlink(&outside, d.join("sub")).unwrap();
    assert_eq!(form_net::sweep(&d, Duration::ZERO).unwrap(), 1);
    assert!(
        std::fs::symlink_metadata(&link).is_err(),
        "the link is removed"
    );
    assert_eq!(
        std::fs::read(&target).unwrap(),
        b"theirs",
        "its target is not"
    );
    assert!(matches!(
        form_net::sweep(&d.join("sub"), Duration::ZERO),
        Err(FormError::Store {
            reason: StoreError::NotADirectory
        })
    ));
    let _ = std::fs::remove_dir_all(&d);
    let _ = std::fs::remove_dir_all(&outside);
}
