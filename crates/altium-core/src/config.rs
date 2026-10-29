//! Global settings, `config.toml` next to the registry. Every key is
//! optional; per-project settings in `project.toml` take precedence.

use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::project::toml_error;

pub const FILE_NAME: &str = "config.toml";

/// Default port of the dashboard.
pub const DEFAULT_PORT: u16 = 7420;

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct GlobalConfig {
    /// Port of `altium serve`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub port: Option<u16>,
    /// Desktop notifications, for every project.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notifications: Option<bool>,
    /// Hours before its deadline when a task counts as near it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub near_deadline_hours: Option<u32>,
    /// Minutes without events after which a session is inactive.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub idle_minutes: Option<u32>,
}

impl GlobalConfig {
    /// Reads `config.toml` in `dir`; a missing file is the default config.
    pub fn load(dir: &Path) -> Result<GlobalConfig> {
        let file = dir.join(FILE_NAME);
        match fs::read_to_string(&file) {
            Ok(text) => toml::from_str(&text).map_err(|e| toml_error(&text, &file, &e)),
            Err(_) => Ok(GlobalConfig::default()),
        }
    }

    pub fn port(&self) -> u16 {
        self.port.unwrap_or(DEFAULT_PORT)
    }

    pub fn notifications(&self) -> bool {
        self.notifications.unwrap_or(true)
    }

    pub fn idle_minutes(&self) -> i64 {
        self.idle_minutes.map_or(crate::activity::IDLE_MINUTES, i64::from)
    }

    /// Near-deadline window of a project: its own setting, else the global one.
    pub fn near_hours(&self, project: &crate::project::ProjectConfig) -> u32 {
        project
            .near_deadline_hours
            .or(self.near_deadline_hours)
            .unwrap_or(crate::tasks::DEFAULT_NEAR_HOURS)
    }
}
