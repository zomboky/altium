//! `.altium/` layout, concurrent updates, schema versions and invalid files.

use std::fs;
use std::path::Path;
use std::thread;

use altium_core::memory::{MemoryNode, NodeKind, NodeMeta};
use altium_core::project::CURRENT_SCHEMA;
use altium_core::store::{DIR_NAME, Migrator};
use altium_core::task::{Task, TaskId};
use altium_core::time::Timestamp;
use altium_core::{Store, StoreError};

fn add_task(store: &Store, title: &str) -> TaskId {
    store
        .update_tasks(|tasks| {
            let id = TaskId(tasks.iter().map(|t| t.id.0).max().unwrap_or(0) + 1);
            tasks.push(Task::new(id, title, Timestamp::now()));
            Ok::<_, StoreError>(id)
        })
        .unwrap()
}

#[test]
fn init_creates_the_layout() {
    let root = tempfile::tempdir().unwrap();
    let store = Store::init(root.path(), "foo").unwrap();

    let dir = root.path().join(DIR_NAME);
    for name in [
        "project.toml",
        "status.json",
        "tasks.jsonl",
        "memory",
        ".gitignore",
        "runtime",
    ] {
        assert!(dir.join(name).exists(), "{name} missing");
    }
    assert_eq!(fs::read_to_string(dir.join(".gitignore")).unwrap(), "runtime/\n");
    let project = store.project().unwrap();
    assert_eq!((project.name.as_str(), project.schema), ("foo", CURRENT_SCHEMA));
    assert!(store.tasks().unwrap().is_empty());
    assert!(store.memory_nodes().unwrap().is_empty());

    assert!(matches!(
        Store::init(root.path(), "foo"),
        Err(StoreError::AlreadyExists(_))
    ));
}

#[test]
fn open_requires_a_project() {
    let root = tempfile::tempdir().unwrap();
    assert!(matches!(Store::open(root.path()), Err(StoreError::NotAProject(_))));
}

#[test]
fn open_recreates_untracked_dirs_after_a_clone() {
    let root = tempfile::tempdir().unwrap();
    Store::init(root.path(), "foo").unwrap();
    let dir = root.path().join(DIR_NAME);
    fs::remove_dir_all(dir.join("runtime")).unwrap();
    fs::remove_dir_all(dir.join("memory")).unwrap();

    let store = Store::open(root.path()).unwrap();
    add_task(&store, "after clone");
    assert!(dir.join("runtime").is_dir() && dir.join("memory").is_dir());
}

#[test]
fn concurrent_writers_lose_nothing() {
    let root = tempfile::tempdir().unwrap();
    Store::init(root.path(), "foo").unwrap();

    let writers: Vec<_> = (0..2)
        .map(|writer| {
            let root = root.path().to_path_buf();
            thread::spawn(move || {
                // Each writer opens its own store, as separate processes would.
                let store = Store::open(&root).unwrap();
                for n in 0..50 {
                    add_task(&store, &format!("writer {writer} #{n}"));
                }
            })
        })
        .collect();
    for writer in writers {
        writer.join().unwrap();
    }

    let tasks = Store::open(root.path()).unwrap().tasks().unwrap();
    assert_eq!(tasks.len(), 100);
    assert!(tasks.iter().enumerate().all(|(i, t)| t.id == TaskId(i as u32 + 1)));
    for writer in 0..2 {
        let count = tasks
            .iter()
            .filter(|t| t.title.starts_with(&format!("writer {writer} ")))
            .count();
        assert_eq!(count, 50);
    }
}

#[test]
fn unchanged_update_does_not_rewrite() {
    let root = tempfile::tempdir().unwrap();
    let store = Store::init(root.path(), "foo").unwrap();
    add_task(&store, "one");
    let file = root.path().join(DIR_NAME).join("tasks.jsonl");
    let before = fs::metadata(&file).unwrap().modified().unwrap();
    thread::sleep(std::time::Duration::from_millis(20));

    store.update_tasks(|_| Ok::<_, StoreError>(())).unwrap();
    assert_eq!(fs::metadata(&file).unwrap().modified().unwrap(), before);
}

#[test]
fn memory_ids_cannot_escape_the_memory_dir() {
    let root = tempfile::tempdir().unwrap();
    let store = Store::init(root.path(), "foo").unwrap();
    assert!(matches!(
        store.memory_node("../project"),
        Err(StoreError::InvalidNodeId(_))
    ));
}

fn set_schema(root: &Path, schema: u32) {
    let file = root.join(DIR_NAME).join("project.toml");
    let text = fs::read_to_string(&file).unwrap();
    let text = text.replace(&format!("schema = {CURRENT_SCHEMA}"), &format!("schema = {schema}"));
    fs::write(file, text).unwrap();
}

#[test]
fn newer_schema_opens_read_only() {
    let root = tempfile::tempdir().unwrap();
    let store = Store::init(root.path(), "foo").unwrap();
    add_task(&store, "existing");
    set_schema(root.path(), 99);
    let dir = root.path().join(DIR_NAME);
    let tasks_before = fs::read(dir.join("tasks.jsonl")).unwrap();

    let store = Store::open(root.path()).unwrap();
    assert!(store.is_read_only());
    assert_eq!(store.tasks().unwrap().len(), 1);

    let refused = |result: Result<(), StoreError>| {
        assert!(matches!(
            result,
            Err(StoreError::ReadOnly {
                found: 99,
                supported: CURRENT_SCHEMA
            })
        ));
    };
    refused(store.update_tasks(|_| Ok(())));
    refused(store.update_status(|status| {
        status.summary = "x".into();
        Ok(())
    }));
    assert_eq!(fs::read(dir.join("tasks.jsonl")).unwrap(), tasks_before);
    assert!(
        fs::read_to_string(dir.join("project.toml"))
            .unwrap()
            .contains("schema = 99")
    );
}

/// Fake v0 → v1 step: v0 called the project name `title`.
fn rename_title(dir: &Path) -> altium_core::Result<()> {
    let file = dir.join("project.toml");
    let text = fs::read_to_string(&file).unwrap();
    fs::write(&file, text.replace("title =", "name =")).unwrap();
    Ok(())
}

fn fake_v0_project(root: &Path) {
    let dir = root.join(DIR_NAME);
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("project.toml"),
        "id = \"01JBDZ4Q6V0M2T8YHC9N3K5R7W\"\ntitle = \"legacy\"\ncreated_at = \"2026-01-01T00:00:00Z\"\n",
    )
    .unwrap();
}

#[test]
fn older_schema_is_migrated_after_a_backup() {
    let root = tempfile::tempdir().unwrap();
    fake_v0_project(root.path());
    let migrator = Migrator {
        current: 1,
        steps: vec![(0, rename_title)],
    };

    let store = Store::open_with(root.path(), &migrator).unwrap();
    assert_eq!(store.schema(), 1);
    let project = store.project().unwrap();
    assert_eq!((project.schema, project.name.as_str()), (1, "legacy"));

    let backups: Vec<_> = fs::read_dir(root.path().join(DIR_NAME).join("runtime/backups"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    assert_eq!(backups.len(), 1);
    assert!(
        backups[0]
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .starts_with("schema-v0-")
    );
    let saved = fs::read_to_string(backups[0].join("project.toml")).unwrap();
    assert!(saved.contains("title = \"legacy\""));

    // Already migrated: opening again changes nothing.
    Store::open_with(root.path(), &migrator).unwrap();
    assert_eq!(
        fs::read_dir(root.path().join(DIR_NAME).join("runtime/backups"))
            .unwrap()
            .count(),
        1
    );
}

#[test]
fn missing_migration_changes_nothing() {
    let root = tempfile::tempdir().unwrap();
    fake_v0_project(root.path());
    let migrator = Migrator {
        current: 1,
        steps: Vec::new(),
    };

    assert!(matches!(
        Store::open_with(root.path(), &migrator),
        Err(StoreError::NoMigration(0))
    ));
    let dir = root.path().join(DIR_NAME);
    assert!(!dir.join("runtime/backups").exists());
    assert!(fs::read_to_string(dir.join("project.toml")).unwrap().contains("title"));
}

#[test]
fn invalid_tasks_file_only_breaks_its_section() {
    let root = tempfile::tempdir().unwrap();
    let store = Store::init(root.path(), "foo").unwrap();
    add_task(&store, "one");
    store
        .update_status(|status| {
            status.summary = "still visible".into();
            Ok::<_, StoreError>(())
        })
        .unwrap();
    store.save_memory_node(&node("auth-jwt")).unwrap();

    // A hand edit breaks line 2.
    let file = root.path().join(DIR_NAME).join("tasks.jsonl");
    let mut text = fs::read_to_string(&file).unwrap();
    text.push_str("{\"id\":\"T-2\",\"title\":\"oops\",\n");
    fs::write(&file, &text).unwrap();

    let snapshot = store.snapshot().unwrap();
    match &snapshot.tasks {
        Err(StoreError::Invalid { file: f, line, .. }) => {
            assert_eq!((f.as_path(), *line), (file.as_path(), Some(2)));
        }
        other => panic!("expected an invalid tasks file, got {other:?}"),
    }
    assert_eq!(snapshot.status.unwrap().summary, "still visible");
    assert_eq!(snapshot.memory.unwrap().len(), 1);

    // Updates refuse to run on the broken file and leave it untouched.
    let result = store.update_tasks(|tasks| {
        tasks.clear();
        Ok::<_, StoreError>(())
    });
    assert!(matches!(result, Err(StoreError::Invalid { line: Some(2), .. })));
    assert_eq!(fs::read_to_string(&file).unwrap(), text);
}

#[test]
fn invalid_memory_node_does_not_hide_the_others() {
    let root = tempfile::tempdir().unwrap();
    let store = Store::init(root.path(), "foo").unwrap();
    store.save_memory_node(&node("auth-jwt")).unwrap();
    store.save_memory_node(&node("db-schema")).unwrap();
    fs::write(root.path().join(DIR_NAME).join("memory/broken.md"), "no front matter\n").unwrap();

    let nodes = store.memory_nodes().unwrap();
    let ids: Vec<_> = nodes
        .iter()
        .filter_map(|n| n.as_ref().ok())
        .map(|n| n.id.as_str())
        .collect();
    assert_eq!(ids, ["auth-jwt", "db-schema"]);
    assert_eq!(nodes.iter().filter(|n| n.is_err()).count(), 1);
    assert_eq!(store.memory_node("auth-jwt").unwrap(), node("auth-jwt"));
}

fn node(id: &str) -> MemoryNode {
    let now = Timestamp::now();
    MemoryNode {
        id: id.into(),
        meta: NodeMeta {
            kind: NodeKind::Fact,
            title: id.into(),
            description: format!("About {id}."),
            tags: Vec::new(),
            pinned: false,
            links: Vec::new(),
            created: now,
            updated: now,
        },
        body: "Body.\n".into(),
    }
}

#[test]
fn find_root_walks_up_to_the_project() {
    let root = tempfile::tempdir().unwrap();
    let project = root.path().join("foo");
    let nested = project.join("src/lib");
    fs::create_dir_all(&nested).unwrap();
    Store::init(&project, "foo").unwrap();

    assert_eq!(altium_core::store::find_root(&nested), Some(project.clone()));
    assert_eq!(altium_core::store::find_root(&project), Some(project));
    assert_eq!(altium_core::store::find_root(&root.path().join("elsewhere")), None);
}
