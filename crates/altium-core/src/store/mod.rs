//! The `.altium/` directory of a project: layout, loading and safe updates.
//!
//! Every write is atomic (temporary file, then rename) and every
//! read-modify-write runs under an exclusive lock on `runtime/lock`, so Claude
//! sessions, hooks and the dashboard can write at the same time without losing
//! changes.

pub(crate) mod atomic;
pub(crate) mod lock;
mod migrate;

pub use migrate::{MigrationStep, Migrator};

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::error::{Result, StoreError};
use crate::memory::{self, MemoryNode};
use crate::project::{self, CURRENT_SCHEMA, Project, ProjectStatus};
use crate::task::{self, Task};

/// Name of the per-project directory.
pub const DIR_NAME: &str = ".altium";

pub const PROJECT_FILE: &str = "project.toml";
pub const STATUS_FILE: &str = "status.json";
pub const TASKS_FILE: &str = "tasks.jsonl";
pub const MEMORY_DIR: &str = "memory";
const GITIGNORE: &str = ".gitignore";
pub const RUNTIME_DIR: &str = "runtime";
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
        let (store, _) = Self::complete(root, || Project::new(name))?;
        Ok(store)
    }

    /// Creates the missing files of the existing `.altium/` directory of
    /// `root`, never touching the others, and returns their names. `project`
    /// is only called when `project.toml` itself is missing.
    pub fn complete(root: &Path, project: impl FnOnce() -> Project) -> Result<(Store, Vec<&'static str>)> {
        let dir = root.join(DIR_NAME);
        if !dir.is_dir() {
            return Err(StoreError::NotAProject(root.to_path_buf()));
        }
        let has_project = dir.join(PROJECT_FILE).is_file();
        let store = if has_project {
            let store = Self::open(root)?;
            if store.is_read_only() {
                return Ok((store, Vec::new()));
            }
            store
        } else {
            let store = Store {
                dir,
                schema: CURRENT_SCHEMA,
                supported: CURRENT_SCHEMA,
            };
            store.ensure_dirs()?;
            store
        };

        let mut created = Vec::new();
        let defaults = [
            (GITIGNORE, format!("{RUNTIME_DIR}/\n")),
            (STATUS_FILE, project::render_status(&ProjectStatus::default())),
            (TASKS_FILE, String::new()),
        ];
        for (name, contents) in defaults {
            if !store.dir.join(name).exists() {
                store.write(name, contents.as_bytes())?;
                created.push(name);
            }
        }
        // Last, so that a half-completed directory is not a valid project.
        if !has_project {
            store.write(PROJECT_FILE, project::render_project(&project()).as_bytes())?;
            created.push(PROJECT_FILE);
        }
        Ok((store, created))
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

    /// The memory graph of valid nodes, and the errors of invalid ones.
    pub fn graph(&self) -> Result<(crate::graph::Graph, Vec<StoreError>)> {
        let mut errors = Vec::new();
        let nodes: Vec<MemoryNode> = self
            .memory_nodes()?
            .into_iter()
            .filter_map(|node| node.map_err(|e| errors.push(e)).ok())
            .collect();
        Ok((crate::graph::Graph::new(nodes), errors))
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

    /// Deletes a memory node; a missing node is not an error.
    pub fn delete_memory_node(&self, id: &str) -> Result<()> {
        self.ensure_writable()?;
        let path = self.dir.join(memory_file(id)?);
        self.with_lock(|| match fs::remove_file(&path) {
            Err(e) if e.kind() != io::ErrorKind::NotFound => Err(StoreError::io(&path)(e)),
            _ => Ok(()),
        })
    }

    /// Like [`Store::update_tasks`], for `project.toml`.
    pub fn update_project<T, E: From<StoreError>>(&self, f: impl FnOnce(&mut Project) -> Result<T, E>) -> Result<T, E> {
        self.ensure_writable()?;
        self.with_lock(|| {
            let mut project = self.project()?;
            let before = project.clone();
            let value = f(&mut project)?;
            if project != before {
                self.write(PROJECT_FILE, project::render_project(&project).as_bytes())?;
            }
            Ok(value)
        })
    }

    /// A file of `runtime/`, which holds local data that is not versioned.
    pub fn runtime_path(&self, name: &str) -> PathBuf {
        self.dir.join(RUNTIME_DIR).join(name)
    }

    /// Reads a JSON file of `runtime/`. A missing or invalid file reads as
    /// the default value: runtime data can always be rebuilt.
    pub fn read_runtime<T: DeserializeOwned + Default>(&self, name: &str) -> T {
        fs::read_to_string(self.runtime_path(name))
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default()
    }

    /// Updates a JSON file of `runtime/` under the project lock. Runtime data
    /// is not part of the schema, so this works on read-only projects too.
    pub fn update_runtime<T, R>(&self, name: &str, f: impl FnOnce(&mut T) -> R) -> Result<R>
    where
        T: DeserializeOwned + Serialize + Default,
    {
        self.with_lock(|| {
            let mut value: T = self.read_runtime(name);
            let result = f(&mut value);
            let text = serde_json::to_string(&value).expect("runtime data always serializes");
            let path = self.runtime_path(name);
            if fs::read_to_string(&path).ok().as_deref() != Some(text.as_str()) {
                if let Some(parent) = path.parent() {
                    fs::create_dir_all(parent).map_err(StoreError::io(parent))?;
                }
                atomic::write(&path, &self.dir.join(TMP_DIR), text.as_bytes())?;
            }
            Ok(result)
        })
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

/// The project directory containing `start`: `start` itself or its closest
/// ancestor that has a `.altium/` directory. `start` should be absolute.
pub fn find_root(start: &Path) -> Option<PathBuf> {
    start
        .ancestors()
        .find(|dir| dir.join(DIR_NAME).is_dir())
        .map(Path::to_path_buf)
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
