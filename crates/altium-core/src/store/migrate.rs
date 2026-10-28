//! Schema migrations of `.altium/`.

use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use super::{PROJECT_FILE, RUNTIME_DIR, TMP_DIR, atomic};
use crate::error::{Result, StoreError};
use crate::project::{self, CURRENT_SCHEMA, toml_error};
use crate::time::Timestamp;

/// Migrates a `.altium/` directory from one schema version to the next.
pub type MigrationStep = fn(&Path) -> Result<()>;

/// The schema version a binary writes, and how to reach it from older ones.
pub struct Migrator {
    /// Schema version written by this binary.
    pub current: u32,
    /// `(from, step)` pairs: `step` migrates schema `from` to `from + 1`.
    pub steps: Vec<(u32, MigrationStep)>,
}

impl Default for Migrator {
    fn default() -> Self {
        // v1 is the first schema: nothing to migrate from yet.
        Self {
            current: CURRENT_SCHEMA,
            steps: Vec::new(),
        }
    }
}

/// Reads the schema version declared in `project.toml`. A missing `schema`
/// key means v0.
pub(super) fn read_schema(dir: &Path) -> Result<u32> {
    #[derive(Deserialize)]
    struct Probe {
        #[serde(default)]
        schema: u32,
    }

    let file = dir.join(PROJECT_FILE);
    let text = fs::read_to_string(&file).map_err(StoreError::io(&file))?;
    toml::from_str::<Probe>(&text)
        .map(|probe| probe.schema)
        .map_err(|e| toml_error(&text, &file, &e))
}

/// Backs up the tracked files, applies every step from `from` up to
/// `migrator.current`, then records the new schema version.
pub(super) fn run(dir: &Path, from: u32, migrator: &Migrator) -> Result<PathBuf> {
    let steps = (from..migrator.current)
        .map(|version| {
            migrator
                .steps
                .iter()
                .find(|(v, _)| *v == version)
                .map(|(_, step)| *step)
                .ok_or(StoreError::NoMigration(version))
        })
        .collect::<Result<Vec<_>>>()?;

    let backup = backup(dir, from)?;
    for step in steps {
        step(dir)?;
    }

    let file = dir.join(PROJECT_FILE);
    let text = fs::read_to_string(&file).map_err(StoreError::io(&file))?;
    let mut project = project::parse_project(&text, &file)?;
    project.schema = migrator.current;
    atomic::write(&file, &dir.join(TMP_DIR), project::render_project(&project).as_bytes())?;
    Ok(backup)
}

/// Copies everything but `runtime/` to `runtime/backups/schema-v<from>-<time>/`.
fn backup(dir: &Path, from: u32) -> Result<PathBuf> {
    let stamp = Timestamp::now().as_datetime().format("%Y%m%dT%H%M%SZ");
    let target = dir
        .join(RUNTIME_DIR)
        .join("backups")
        .join(format!("schema-v{from}-{stamp}"));
    fs::create_dir_all(&target).map_err(StoreError::io(&target))?;
    for entry in fs::read_dir(dir).map_err(StoreError::io(dir))? {
        let entry = entry.map_err(StoreError::io(dir))?;
        if entry.file_name() != RUNTIME_DIR {
            copy_tree(&entry.path(), &target.join(entry.file_name()))?;
        }
    }
    Ok(target)
}

fn copy_tree(from: &Path, to: &Path) -> Result<()> {
    if from.is_dir() {
        fs::create_dir_all(to).map_err(StoreError::io(to))?;
        for entry in fs::read_dir(from).map_err(StoreError::io(from))? {
            let entry = entry.map_err(StoreError::io(from))?;
            copy_tree(&entry.path(), &to.join(entry.file_name()))?;
        }
    } else {
        fs::copy(from, to).map_err(StoreError::io(from))?;
    }
    Ok(())
}
