//! Project metadata (`project.toml`) and project status (`status.json`).

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::actor::Actor;
use crate::error::{Result, StoreError};
use crate::is_default;
use crate::time::Timestamp;

/// Latest `.altium/` schema version this binary reads and writes.
pub const CURRENT_SCHEMA: u32 = 1;

/// Phase of a freshly initialized project.
pub const DEFAULT_PHASE: &str = "idea";

/// `project.toml`: identity and per-project configuration.
///
/// Unknown keys are ignored rather than rejected, so that a project written by
/// a newer Altium can still be opened read-only.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Project {
    /// A missing key means schema v0.
    #[serde(default)]
    pub schema: u32,
    /// ULID, stable across renames and moves.
    pub id: String,
    pub name: String,
    pub created_at: Timestamp,
    #[serde(default, skip_serializing_if = "is_default")]
    pub config: ProjectConfig,
}

impl Project {
    /// A new project named `name`, with a fresh id.
    pub fn new(name: &str) -> Project {
        Project {
            schema: CURRENT_SCHEMA,
            id: ulid::Ulid::generate().to_string(),
            name: name.to_owned(),
            created_at: Timestamp::now(),
            config: ProjectConfig::default(),
        }
    }
}

/// Per-project settings. Unset values fall back to the global defaults.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ProjectConfig {
    /// Token budget of the context injected at session start.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_budget: Option<u32>,
    /// Hours before its deadline when a task counts as near its deadline.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub near_deadline_hours: Option<u32>,
    /// Allowed project phases, replacing the default list.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub phases: Option<Vec<String>>,
    /// Desktop notifications for this project.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notifications: Option<bool>,
    #[serde(default, skip_serializing_if = "is_default")]
    pub memory: MemoryConfig,
}

/// `[config.memory]` of `project.toml`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct MemoryConfig {
    /// Language Claude writes memory nodes in (`en` when unset).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lang: Option<String>,
}

/// Default project phases, in their usual order.
pub const DEFAULT_PHASES: [&str; 7] = [
    "idea",
    "exploration",
    "development",
    "testing",
    "maintenance",
    "paused",
    "done",
];

/// Longest project summary, in characters.
pub const MAX_SUMMARY_CHARS: usize = 600;

impl ProjectConfig {
    pub fn memory_lang(&self) -> &str {
        self.memory.lang.as_deref().unwrap_or(crate::graph::DEFAULT_LANG)
    }

    pub fn phases(&self) -> Vec<String> {
        match &self.phases {
            Some(phases) if !phases.is_empty() => phases.clone(),
            _ => DEFAULT_PHASES.iter().map(|p| (*p).to_owned()).collect(),
        }
    }

    pub fn context_budget(&self) -> usize {
        self.context_budget.map_or(2000, |b| b as usize)
    }

    pub fn near_hours(&self) -> u32 {
        self.near_deadline_hours.unwrap_or(crate::tasks::DEFAULT_NEAR_HOURS)
    }
}

/// `status.json`: where the project stands, kept up to date by Claude and the user.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectStatus {
    pub phase: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub summary: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<Timestamp>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_by: Option<Actor>,
}

impl Default for ProjectStatus {
    fn default() -> Self {
        Self {
            phase: DEFAULT_PHASE.to_owned(),
            summary: String::new(),
            updated_at: None,
            updated_by: None,
        }
    }
}

pub fn parse_project(text: &str, file: &Path) -> Result<Project> {
    toml::from_str(text).map_err(|e| toml_error(text, file, &e))
}

pub fn render_project(project: &Project) -> String {
    toml::to_string(project).expect("project metadata always serializes")
}

pub fn parse_status(text: &str, file: &Path) -> Result<ProjectStatus> {
    serde_json::from_str(text).map_err(|e| StoreError::invalid(file, Some(e.line()), e))
}

pub fn render_status(status: &ProjectStatus) -> String {
    let mut text = serde_json::to_string_pretty(status).expect("project status always serializes");
    text.push('\n');
    text
}

pub(crate) fn toml_error(text: &str, file: &Path, error: &toml::de::Error) -> StoreError {
    let line = error.span().map(|span| text[..span.start].matches('\n').count() + 1);
    StoreError::invalid(file, line, error.message())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn project() -> Project {
        Project {
            schema: CURRENT_SCHEMA,
            id: "01JBDZ4Q6V0M2T8YHC9N3K5R7W".into(),
            name: "foo".into(),
            created_at: "2026-10-28T10:00:00Z".parse().unwrap(),
            config: ProjectConfig::default(),
        }
    }

    #[test]
    fn project_round_trips_and_omits_empty_config() {
        let text = render_project(&project());
        assert!(!text.contains("[config]"));
        assert_eq!(parse_project(&text, Path::new("project.toml")).unwrap(), project());
    }

    #[test]
    fn project_keeps_config() {
        let mut p = project();
        p.config.context_budget = Some(1500);
        let text = render_project(&p);
        assert!(text.contains("[config]\ncontext_budget = 1500"));
        assert_eq!(parse_project(&text, Path::new("project.toml")).unwrap(), p);
    }

    #[test]
    fn project_error_points_at_line() {
        let text = "schema = 1\nid = \"x\"\nname = \nsummary = 3\n";
        let err = parse_project(text, Path::new("project.toml")).unwrap_err();
        assert!(matches!(err, StoreError::Invalid { line: Some(3), .. }), "{err}");
    }

    #[test]
    fn memory_lang_lives_under_config() {
        let mut p = project();
        assert_eq!(p.config.memory_lang(), "en");
        p.config.memory.lang = Some("fr".into());
        let text = render_project(&p);
        assert!(
            text.contains(
                "[config.memory]
lang = \"fr\""
            ),
            "{text}"
        );
        let dotted = text.replace(
            "[config.memory]
lang",
            "[config]
memory.lang",
        );
        assert_eq!(
            parse_project(&dotted, Path::new("project.toml"))
                .unwrap()
                .config
                .memory_lang(),
            "fr"
        );
    }

    #[test]
    fn status_round_trips() {
        let status = ProjectStatus {
            summary: "API en place, reste l'auth.".into(),
            updated_at: Some("2026-10-28T10:00:00Z".parse().unwrap()),
            updated_by: Some(Actor::Claude {
                session: Some("abc".into()),
            }),
            ..ProjectStatus::default()
        };
        let text = render_status(&status);
        assert!(text.contains(r#""updated_by": "claude:abc""#));
        assert_eq!(parse_status(&text, Path::new("status.json")).unwrap(), status);
    }

    #[test]
    fn default_status_is_minimal() {
        assert_eq!(
            render_status(&ProjectStatus::default()),
            "{\n  \"phase\": \"idea\"\n}\n"
        );
    }
}
