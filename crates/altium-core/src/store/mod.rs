//! The `.altium/` directory of a project: layout, loading and safe updates.
//!
//! Every write is atomic (temporary file, then rename) and every
//! read-modify-write runs under an exclusive lock on `runtime/lock`, so Claude
//! sessions, hooks and the dashboard can write at the same time without losing
//! changes.

mod atomic;
mod lock;
mod migrate;

pub use migrate::{MigrationStep, Migrator};

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::error::{Result, StoreError};
use crate::memory::{self, MemoryNode};
use crate::project::{self, CURRENT_SCHEMA, Project, ProjectConfig, ProjectStatus};
use crate::task::{self, Task};
use crate::time::Timestamp;

/// Name of the per-project directory.
pub const DIR_NAME: &str = ".altium";

const PROJECT_FILE: &str = "project.toml";
const STATUS_FILE: &str = "status.json";
const TASKS_FILE: &str = "tasks.jsonl";
const MEMORY_DIR: &str = "memory";
const GITIGNORE: &str = ".gitignore";
const RUNTIME_DIR: &str = "runtime";
const LOCK_FILE: &str = "runtime/lock";
const TMP_DIR: &str = "runtime/tmp";

const LOCK_TIMEOUT: Duration = Duration::from_secs(2);

/// An opened `.altium/` directory.
#[derive(Debug, Clone)]
pub struct Store {
    dir: PathBuf,
    schema: u32,
    supported: u32,
}

/// Everything a project holds, loaded section by section: an invalid file
/// only makes its own section fail.
#[derive(Debug)]
pub struct Snapshot {
    pub project: Project,
    pub read_only: bool,
    pub status: Result<ProjectStatus>,
    pub tasks: Result<Vec<Task>>,
    pub memory: Result<Vec<Result<MemoryNode>>>,
}

impl Store {
    /// Creates `.altium/` in `root` for a new project named `name`.
    pub fn init(root: &Path, name: &str) -> Result<Store> {
        let dir = root.join(DIR_NAME);
        if dir.exists() {
            return Err(StoreError::AlreadyExists(dir));
        }
        fs::create_dir_all(&dir).map_err(StoreError::io(&dir))?;
        let store = Store {
            dir,
            schema: CURRENT_SCHEMA,
            supported: CURRENT_SCHEMA,
        };
        store.ensure_dirs()?;

        let project = Project {
            schema: CURRENT_SCHEMA,
            id: ulid::Ulid::generate().to_string(),
            name: name.to_owned(),
            created_at: Timestamp::now(),
            config: ProjectConfig::default(),
        };
        store.write(GITIGNORE, format!("{RUNTIME_DIR}/\n").as_bytes())?;
        store.write(
            STATUS_FILE,
            project::render_status(&ProjectStatus::default()).as_bytes(),
        )?;
        store.write(TASKS_FILE, b"")?;
        // Last, so that a half-initialized directory is not a valid project.
        store.write(PROJECT_FILE, project::render_project(&project).as_bytes())?;
        Ok(store)
    }

    /// Opens the `.altium/` directory of `root`, migrating an older schema.
    /// A newer schema opens read-only and nothing is written.
    pub fn open(root: &Path) -> Result<Store> {
        Self::open_with(root, &Migrator::default())
    }

    /// Like [`Store::open`], with an explicit set of migrations.
    pub fn open_with(root: &Path, migrator: &Migrator) -> Result<Store> {
        let dir = root.join(DIR_NAME);
        if !dir.is_dir() {
            return Err(StoreError::NotAProject(root.to_path_buf()));
        }
        let mut store = Store {
            schema: migrate::read_schema(&dir)?,
            dir,
            supported: migrator.current,
        };
        if store.is_read_only() {
            return Ok(store);
        }

        // `runtime/` and empty `memory/` are not in git: recreate them after a clone.
        store.ensure_dirs()?;
        if store.schema < migrator.current {
            store.with_lock(|| {
                // Another process may have migrated while we waited.
                let schema = migrate::read_schema(&store.dir)?;
                if schema < migrator.current {
                    migrate::run(&store.dir, schema, migrator)?;
                }
                Ok::<_, StoreError>(())
            })?;
            store.schema = migrator.current;
        }
        Ok(store)
    }

    /// The `.altium/` directory.
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// The project directory, parent of `.altium/`.
    pub fn root(&self) -> &Path {
        self.dir.parent().expect(".altium/ always has a parent")
    }

    pub fn schema(&self) -> u32 {
        self.schema
    }

    /// Whether `.altium/` uses a newer schema than this binary supports, in
    /// which case every write is refused.
    pub fn is_read_only(&self) -> bool {
        self.schema > self.supported
    }

    pub fn project(&self) -> Result<Project> {
        let (text, file) = self.read(PROJECT_FILE)?;
        project::parse_project(&text, &file)
    }

    /// The project status; a missing `status.json` reads as the default.
    pub fn status(&self) -> Result<ProjectStatus> {
        match self.read(STATUS_FILE) {
            Ok((text, file)) => project::parse_status(&text, &file),
            Err(e) if is_not_found(&e) => Ok(ProjectStatus::default()),
            Err(e) => Err(e),
        }
    }

    /// All tasks, sorted by id; a missing `tasks.jsonl` reads as empty.
    pub fn tasks(&self) -> Result<Vec<Task>> {
        let (text, file) = self.read_tasks()?;
        task::parse_tasks(&text, &file)
    }

    pub fn memory_node(&self, id: &str) -> Result<MemoryNode> {
        let (text, file) = self.read(&memory_file(id)?)?;
        memory::parse_node(id, &text, &file)
    }

    /// Every node of `memory/`, sorted by id. Each node loads on its own, so
    /// one invalid file does not hide the others.
    pub fn memory_nodes(&self) -> Result<Vec<Result<MemoryNode>>> {
        let dir = self.dir.join(MEMORY_DIR);
        let entries = match fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(StoreError::io(&dir)(e)),
        };
        let mut ids = Vec::new();
        for entry in entries {
            let path = entry.map_err(StoreError::io(&dir))?.path();
            if path.extension().is_some_and(|ext| ext == "md")
                && let Some(id) = path.file_stem().and_then(|stem| stem.to_str())
            {
                ids.push(id.to_owned());
            }
        }
        ids.sort();
        Ok(ids.iter().map(|id| self.memory_node(id)).collect())
    }

    pub fn snapshot(&self) -> Result<Snapshot> {
        Ok(Snapshot {
            project: self.project()?,
            read_only: self.is_read_only(),
            status: self.status(),
            tasks: self.tasks(),
            memory: self.memory_nodes(),
        })
    }

    /// Runs `f` on the current tasks under the project lock and saves them if
    /// they changed. Nothing is written if `f` fails or if `tasks.jsonl` is
    /// invalid.
    pub fn update_tasks<T, E: From<StoreError>>(&self, f: impl FnOnce(&mut Vec<Task>) -> Result<T, E>) -> Result<T, E> {
        self.ensure_writable()?;
        self.with_lock(|| {
            let (text, file) = self.read_tasks()?;
            let mut tasks = task::parse_tasks(&text, &file)?;
            let value = f(&mut tasks)?;
            let rendered = task::render_tasks(&tasks);
            if rendered != text {
                self.write(TASKS_FILE, rendered.as_bytes())?;
            }
            Ok(value)
        })
    }

    /// Like [`Store::update_tasks`], for the project status.
    pub fn update_status<T, E: From<StoreError>>(
        &self,
        f: impl FnOnce(&mut ProjectStatus) -> Result<T, E>,
    ) -> Result<T, E> {
        self.ensure_writable()?;
        self.with_lock(|| {
            let mut status = self.status()?;
            let before = status.clone();
            let value = f(&mut status)?;
            if status != before {
                self.write(STATUS_FILE, project::render_status(&status).as_bytes())?;
            }
            Ok(value)
        })
    }

    /// Creates or replaces a memory node.
    pub fn save_memory_node(&self, node: &MemoryNode) -> Result<()> {
        self.ensure_writable()?;
        let name = memory_file(&node.id)?;
        self.with_lock(|| self.write(&name, memory::render_node(node).as_bytes()))
    }

    fn ensure_writable(&self) -> Result<()> {
        if self.is_read_only() {
            return Err(StoreError::ReadOnly {
                found: self.schema,
                supported: self.supported,
            });
        }
        Ok(())
    }

    fn ensure_dirs(&self) -> Result<()> {
        for name in [MEMORY_DIR, RUNTIME_DIR, TMP_DIR] {
            let path = self.dir.join(name);
            fs::create_dir_all(&path).map_err(StoreError::io(&path))?;
        }
        Ok(())
    }

    fn with_lock<T, E: From<StoreError>>(&self, f: impl FnOnce() -> Result<T, E>) -> Result<T, E> {
        lock::with_lock(&self.dir.join(LOCK_FILE), LOCK_TIMEOUT, f)
    }

    fn read(&self, name: &str) -> Result<(String, PathBuf)> {
        let file = self.dir.join(name);
        let text = fs::read_to_string(&file).map_err(StoreError::io(&file))?;
        Ok((text, file))
    }

    fn read_tasks(&self) -> Result<(String, PathBuf)> {
        match self.read(TASKS_FILE) {
            Err(e) if is_not_found(&e) => Ok((String::new(), self.dir.join(TASKS_FILE))),
            other => other,
        }
    }

    fn write(&self, name: &str, contents: &[u8]) -> Result<()> {
        atomic::write(&self.dir.join(name), &self.dir.join(TMP_DIR), contents)
    }
}

fn memory_file(id: &str) -> Result<String> {
    if !memory::is_valid_node_id(id) {
        return Err(StoreError::InvalidNodeId(id.to_owned()));
    }
    Ok(format!("{MEMORY_DIR}/{id}.md"))
}

fn is_not_found(error: &StoreError) -> bool {
    matches!(error, StoreError::Io { source, .. } if source.kind() == io::ErrorKind::NotFound)
}
