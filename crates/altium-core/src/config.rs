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
    /// Language of memory nodes when the project does not set one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memory_lang: Option<String>,
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

    /// Memory language of a project: its own setting, else the global one.
    pub fn memory_lang<'a>(&'a self, project: &'a crate::project::ProjectConfig) -> &'a str {
        project
            .memory
            .lang
            .as_deref()
            .or(self.memory_lang.as_deref())
            .unwrap_or(crate::graph::DEFAULT_LANG)
    }

    /// The user's config, from `config.toml` of their registry; the default
    /// one when it cannot be read.
    pub fn for_user() -> GlobalConfig {
        crate::registry::Registry::for_user()
            .ok()
            .and_then(|r| GlobalConfig::load(r.dir()).ok())
            .unwrap_or_default()
    }

    /// Memory language outside any project.
    pub fn default_memory_lang(&self) -> &str {
        self.memory_lang.as_deref().unwrap_or(crate::graph::DEFAULT_LANG)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::ProjectConfig;

    #[test]
    fn memory_language_of_the_project_then_of_the_user() {
        let mut project = ProjectConfig::default();
        let mut config = GlobalConfig::default();
        assert_eq!(config.memory_lang(&project), "en");
        config.memory_lang = Some("fr".into());
        assert_eq!(config.memory_lang(&project), "fr");
        assert_eq!(config.default_memory_lang(), "fr");
        project.memory.lang = Some("de".into());
        assert_eq!(config.memory_lang(&project), "de");
    }
}
