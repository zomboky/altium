//! The registry of tracked projects, in `$XDG_CONFIG_HOME/altium/projects.toml`.
//!
//! The registry only knows where projects are. Everything else lives in each
//! project's `.altium/`.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::error::{Result, StoreError};
use crate::project::{Project, toml_error};
use crate::store::{Store, atomic, lock};
use crate::time::Timestamp;

const FILE_NAME: &str = "projects.toml";
const LOCK_NAME: &str = "projects.lock";
const LOCK_TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Entry {
    pub id: String,
    /// Last known name, to show projects whose directory is gone.
    pub name: String,
    /// Canonical project directory, parent of `.altium/`.
    pub path: PathBuf,
    pub added_at: Timestamp,
}

/// What [`Registry::register`] did.
#[derive(Debug, Clone, PartialEq)]
pub enum Registered {
    Added,
    Unchanged,
    /// The project was registered at another path, now updated.
    Moved {
        from: PathBuf,
    },
}

/// A registry entry and the project found at its path, `None` when the
/// directory is gone or no longer holds that project.
#[derive(Debug)]
pub struct Listed {
    pub entry: Entry,
    pub project: Option<Project>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
struct RegistryFile {
    #[serde(default, rename = "project")]
    projects: Vec<Entry>,
}

pub struct Registry {
    dir: PathBuf,
}

impl Registry {
    /// The registry of the current user, in the platform config directory.
    pub fn for_user() -> Result<Registry> {
        let base = dirs::config_dir().ok_or(StoreError::NoConfigDir)?;
        Ok(Self::at(base.join("altium")))
    }

    /// A registry stored in `dir`.
    pub fn at(dir: impl Into<PathBuf>) -> Registry {
        Registry { dir: dir.into() }
    }

    pub fn file(&self) -> PathBuf {
        self.dir.join(FILE_NAME)
    }

    pub fn entries(&self) -> Result<Vec<Entry>> {
        Ok(self.load()?.projects)
    }

    pub fn list(&self) -> Result<Vec<Listed>> {
        Ok(self
            .entries()?
            .into_iter()
            .map(|entry| Listed {
                project: find(&entry),
                entry,
            })
            .collect())
    }

    /// Adds the project found at `root`. A project with the same id already
    /// registered elsewhere has moved: its path is updated instead of adding
    /// a duplicate.
    pub fn register(&self, project: &Project, root: &Path) -> Result<Registered> {
        let path = fs::canonicalize(root).map_err(StoreError::io(root))?;
        self.update(|file| {
            // Another project registered at this path has been replaced.
            file.projects.retain(|e| e.id == project.id || e.path != path);
            let Some(entry) = file.projects.iter_mut().find(|e| e.id == project.id) else {
                file.projects.push(Entry {
                    id: project.id.clone(),
                    name: project.name.clone(),
                    path,
                    added_at: Timestamp::now(),
                });
                return Ok(Registered::Added);
            };
            entry.name.clone_from(&project.name);
            if entry.path == path {
                Ok(Registered::Unchanged)
            } else {
                let from = std::mem::replace(&mut entry.path, path);
                Ok(Registered::Moved { from })
            }
        })
    }

    /// Removes the project whose id or path is `target`, and returns it.
    pub fn forget(&self, target: &str) -> Result<Option<Entry>> {
        // The directory may be gone, so match its absolute path too.
        let paths: Vec<PathBuf> = [fs::canonicalize(target).ok(), std::path::absolute(target).ok()]
            .into_iter()
            .flatten()
            .collect();
        self.update(|file| {
            let index = file
                .projects
                .iter()
                .position(|e| e.id == target || paths.contains(&e.path));
            Ok(index.map(|i| file.projects.remove(i)))
        })
    }

    fn load(&self) -> Result<RegistryFile> {
        let file = self.file();
        match fs::read_to_string(&file) {
            Ok(text) => toml::from_str(&text).map_err(|e| toml_error(&text, &file, &e)),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(RegistryFile::default()),
            Err(e) => Err(StoreError::io(&file)(e)),
        }
    }

    fn update<T>(&self, f: impl FnOnce(&mut RegistryFile) -> Result<T>) -> Result<T> {
        fs::create_dir_all(&self.dir).map_err(StoreError::io(&self.dir))?;
        lock::with_lock(&self.dir.join(LOCK_NAME), LOCK_TIMEOUT, || {
            let mut file = self.load()?;
            let before = file.clone();
            let value = f(&mut file)?;
            if file != before {
                let text = toml::to_string(&file).expect("the registry always serializes");
                atomic::write(&self.file(), &self.dir, text.as_bytes())?;
            }
            Ok(value)
        })
    }
}

/// The project registered as `entry`, if its directory still holds it.
fn find(entry: &Entry) -> Option<Project> {
    let project = Store::open(&entry.path).ok()?.project().ok()?;
    (project.id == entry.id).then_some(project)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Env {
        _tmp: tempfile::TempDir,
        registry: Registry,
        projects: PathBuf,
    }

    fn env() -> Env {
        let tmp = tempfile::tempdir().unwrap();
        let projects = tmp.path().join("projects");
        fs::create_dir_all(&projects).unwrap();
        Env {
            registry: Registry::at(tmp.path().join("config/altium")),
            projects: fs::canonicalize(projects).unwrap(),
            _tmp: tmp,
        }
    }

    fn init(root: &Path, name: &str) -> Project {
        fs::create_dir_all(root).unwrap();
        Store::init(root, name).unwrap().project().unwrap()
    }

    #[test]
    fn registers_once() {
        let env = env();
        let foo = env.projects.join("foo");
        let project = init(&foo, "foo");

        assert_eq!(env.registry.register(&project, &foo).unwrap(), Registered::Added);
        assert_eq!(env.registry.register(&project, &foo).unwrap(), Registered::Unchanged);
        let entries = env.registry.entries().unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(
            (entries[0].id.as_str(), entries[0].path.as_path()),
            (project.id.as_str(), foo.as_path())
        );
    }

    #[test]
    fn moved_project_updates_its_path() {
        let env = env();
        let (old, new) = (env.projects.join("foo"), env.projects.join("work/foo"));
        let project = init(&old, "foo");
        env.registry.register(&project, &old).unwrap();

        fs::create_dir_all(new.parent().unwrap()).unwrap();
        fs::rename(&old, &new).unwrap();
        assert_eq!(
            env.registry.register(&project, &new).unwrap(),
            Registered::Moved { from: old.clone() }
        );
        let entries = env.registry.entries().unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].path, new);
    }

    #[test]
    fn new_project_at_a_registered_path_replaces_the_old_entry() {
        let env = env();
        let foo = env.projects.join("foo");
        let first = init(&foo, "first");
        env.registry.register(&first, &foo).unwrap();

        fs::remove_dir_all(foo.join(".altium")).unwrap();
        let second = Store::init(&foo, "second").unwrap().project().unwrap();
        env.registry.register(&second, &foo).unwrap();

        let ids: Vec<_> = env.registry.entries().unwrap().into_iter().map(|e| e.id).collect();
        assert_eq!(ids, [second.id]);
    }

    #[test]
    fn lists_missing_projects() {
        let env = env();
        let (a, b) = (env.projects.join("a"), env.projects.join("b"));
        for (root, name) in [(&a, "a"), (&b, "b")] {
            env.registry.register(&init(root, name), root).unwrap();
        }
        fs::remove_dir_all(&b).unwrap();

        let listed = env.registry.list().unwrap();
        let states: Vec<_> = listed
            .iter()
            .map(|l| (l.entry.name.as_str(), l.project.is_some()))
            .collect();
        assert_eq!(states, [("a", true), ("b", false)]);
    }

    #[test]
    fn forgets_by_id_or_by_path_even_when_gone() {
        let env = env();
        let (a, b) = (env.projects.join("a"), env.projects.join("b"));
        let project_a = init(&a, "a");
        env.registry.register(&project_a, &a).unwrap();
        env.registry.register(&init(&b, "b"), &b).unwrap();
        fs::remove_dir_all(&b).unwrap();

        assert_eq!(env.registry.forget(&project_a.id).unwrap().unwrap().name, "a");
        assert_eq!(env.registry.forget(b.to_str().unwrap()).unwrap().unwrap().name, "b");
        assert!(env.registry.forget("nope").unwrap().is_none());
        assert!(env.registry.entries().unwrap().is_empty());
        // Forgetting never touches the project directory.
        assert!(a.join(".altium").is_dir());
    }
}
