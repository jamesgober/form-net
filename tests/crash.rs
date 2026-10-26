//! A killed process leaves no upload behind in the default anonymous mode
//! (FR-48, T-11). The test runs itself as a child that spills a file and
//! waits; the parent kills it and counts what is left.

#![cfg(feature = "fs")]

use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Command, Stdio};

const CHILD: &str = "FORM_NET_CRASH_CHILD_DIR";
const MODE: &str = "FORM_NET_CRASH_CHILD_MODE";

#[test]
fn test_crash_child() {
    // The child half: only runs when spawned by the parent test.
    let Ok(dir) = std::env::var(CHILD) else {
        return;
    };
    let named = std::env::var(MODE).is_ok_and(|m| m == "named");
    let mode = if named {
        form_net::SpillMode::Named
    } else {
        form_net::SpillMode::Anonymous
    };
    let store = form_net::Store::in_dir(&dir)
        .unwrap()
        .memory_threshold(0)
        .mode(mode);
    let body = b"--X\r\nContent-Disposition: form-data; name=f; filename=a\r\n\r\nspilled bytes";
    let mut s = form_net::UploadSession::new(
        b"multipart/form-data; boundary=X",
        &form_net::Limits::DEFAULT,
        &form_net::Policy::DEFAULT,
        &store,
    )
    .unwrap();
    s.push(body, &mut form_net::NoInspector).unwrap();
    println!("ready");
    std::thread::sleep(std::time::Duration::from_secs(120));
}

fn crash_with(mode: &str) -> usize {
    let dir: PathBuf = std::env::temp_dir().join(format!(
        "form-net-crash-{mode}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "test_crash_child",
            "--nocapture",
            "--test-threads=1",
        ])
        .env(CHILD, &dir)
        .env(MODE, mode)
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let out = child.stdout.take().unwrap();
    let mut ready = false;
    for line in BufReader::new(out).lines() {
        if line.unwrap().contains("ready") {
            ready = true;
            break;
        }
    }
    assert!(ready, "the child did not reach the spill");
    child.kill().unwrap();
    let _ = child.wait();
    let left = std::fs::read_dir(&dir).unwrap().count();
    // What a restarted server does first: sweep the leftovers.
    let swept = form_net::sweep(&dir, std::time::Duration::ZERO).unwrap();
    assert_eq!(swept, left, "sweep removes every leftover spill file");
    assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 0);
    let _ = std::fs::remove_dir_all(&dir);
    left
}

#[test]
fn test_anonymous_files_do_not_survive_a_crash() {
    assert_eq!(crash_with("anonymous"), 0);
}

#[test]
fn test_attack_t11_named_files_survive_a_crash_until_sweep() {
    // Named mode leaves the file; `sweep` at startup removes it.
    assert_eq!(crash_with("named"), 1);
}
