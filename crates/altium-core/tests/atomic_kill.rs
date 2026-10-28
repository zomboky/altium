//! A writer killed in the middle of saving `tasks.jsonl` never leaves a
//! partial or invalid file behind.
//!
//! The test binary re-runs itself as the writer: `child_writer` only does
//! something when `ALTIUM_TEST_WRITER_DIR` is set.

use std::path::Path;
use std::process::Command;
use std::thread;
use std::time::Duration;

use altium_core::Store;
use altium_core::task::{Task, TaskId};
use altium_core::time::Timestamp;

const WRITER_DIR: &str = "ALTIUM_TEST_WRITER_DIR";
const TASKS_PER_WRITE: u32 = 2000;

#[test]
fn child_writer() {
    let Some(root) = std::env::var_os(WRITER_DIR) else {
        return;
    };
    let store = Store::open(Path::new(&root)).unwrap();
    // Alternate between two full sets of tasks until killed.
    for title in ["A", "B"].into_iter().cycle() {
        store
            .update_tasks(|tasks| {
                *tasks = (1..=TASKS_PER_WRITE)
                    .map(|n| Task::new(TaskId(n), format!("{title} {}", "x".repeat(60)), Timestamp::now()))
                    .collect();
                Ok::<_, altium_core::StoreError>(())
            })
            .unwrap();
    }
}

#[test]
fn killed_writer_leaves_a_valid_file() {
    let root = tempfile::tempdir().unwrap();
    Store::init(root.path(), "kill").unwrap();

    let mut saw_full_write = false;
    for delay_ms in [30, 80, 150, 230, 310] {
        let mut child = Command::new(std::env::current_exe().unwrap())
            .args(["child_writer", "--exact", "--nocapture"])
            .env(WRITER_DIR, root.path())
            .spawn()
            .unwrap();
        thread::sleep(Duration::from_millis(delay_ms));
        child.kill().unwrap();
        child.wait().unwrap();

        let tasks = Store::open(root.path()).unwrap().tasks().unwrap();
        assert!(
            tasks.is_empty() || tasks.len() == TASKS_PER_WRITE as usize,
            "partial write: {} tasks",
            tasks.len()
        );
        // All tasks come from the same write.
        let first = tasks.first().map(|t| &t.title[..1]);
        assert!(tasks.iter().all(|t| Some(&t.title[..1]) == first));
        saw_full_write |= !tasks.is_empty();
    }
    // Otherwise the writer was always killed before its first write and the
    // test proved nothing.
    assert!(saw_full_write);
}
