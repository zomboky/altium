//! Tasks and the `tasks.jsonl` file: one task per line, sorted by id, with a
//! fixed key order and default values omitted, so that changing one task
//! changes exactly one line.

use std::collections::HashSet;
use std::fmt;
use std::path::Path;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::error::{Result, StoreError};
use crate::is_default;
use crate::time::{Deadline, Timestamp};

/// JSON Schema of one line of `tasks.jsonl`, shipped with the binary.
pub const TASK_SCHEMA: &str = include_str!("../schema/task.schema.json");

/// Short task id, unique within a project: `T-12`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TaskId(pub u32);

impl fmt::Display for TaskId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "T-{}", self.0)
    }
}

impl FromStr for TaskId {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        s.strip_prefix("T-")
            .and_then(|n| n.parse().ok())
            .filter(|n| *n > 0)
            .map(TaskId)
            .ok_or_else(|| format!("invalid task id `{s}` (expected T-<number>)"))
    }
}

serde_via_str!(TaskId);

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TaskStatus {
    #[default]
    Todo,
    Doing,
    Blocked,
    Done,
    Cancelled,
}

/// `P0` is the most urgent.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Priority {
    P0,
    P1,
    #[default]
    P2,
    P3,
}

/// One line of `tasks.jsonl`. Keys are short to keep the file compact; they
/// are documented in `docs/format.md` and `schema/task.schema.json`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Task {
    pub id: TaskId,
    pub title: String,
    #[serde(rename = "st", default, skip_serializing_if = "is_default")]
    pub status: TaskStatus,
    #[serde(rename = "pri", default, skip_serializing_if = "is_default")]
    pub priority: Priority,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub due: Option<Deadline>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<TaskId>,
    /// Manual order among tasks of the same priority.
    #[serde(rename = "pos", default, skip_serializing_if = "Option::is_none")]
    pub position: Option<u32>,
    #[serde(rename = "desc", default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Why the task is blocked; only set while its status is `blocked`.
    #[serde(rename = "why", default, skip_serializing_if = "Option::is_none")]
    pub blocked_reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    /// Incremented on every change.
    pub rev: u32,
    #[serde(rename = "ca")]
    pub created_at: Timestamp,
    #[serde(rename = "ua")]
    pub updated_at: Timestamp,
    #[serde(rename = "da", default, skip_serializing_if = "Option::is_none")]
    pub done_at: Option<Timestamp>,
}

impl Task {
    /// A `todo` task at priority P2, at revision 1.
    pub fn new(id: TaskId, title: impl Into<String>, now: Timestamp) -> Self {
        Self {
            id,
            title: title.into(),
            status: TaskStatus::default(),
            priority: Priority::default(),
            due: None,
            tags: Vec::new(),
            parent: None,
            position: None,
            description: None,
            blocked_reason: None,
            note: None,
            rev: 1,
            created_at: now,
            updated_at: now,
            done_at: None,
        }
    }
}

/// Parses `tasks.jsonl`. Blank lines are skipped; errors point at the
/// offending line of `file`.
pub fn parse_tasks(text: &str, file: &Path) -> Result<Vec<Task>> {
    let mut tasks = Vec::new();
    let mut seen = HashSet::new();
    for (index, line) in text.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let line_no = Some(index + 1);
        let task: Task = serde_json::from_str(line).map_err(|e| StoreError::invalid(file, line_no, e))?;
        if !seen.insert(task.id) {
            return Err(StoreError::invalid(
                file,
                line_no,
                format!("duplicate task id {}", task.id),
            ));
        }
        tasks.push(task);
    }
    tasks.sort_by_key(|task| task.id);
    Ok(tasks)
}

/// Renders tasks as `tasks.jsonl`, sorted by id.
pub fn render_tasks(tasks: &[Task]) -> String {
    let mut sorted: Vec<&Task> = tasks.iter().collect();
    sorted.sort_by_key(|task| task.id);
    let mut text = String::new();
    for task in sorted {
        text.push_str(&serde_json::to_string(task).expect("tasks always serialize"));
        text.push('\n');
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    const FILE: &str = "tasks.jsonl";

    fn now() -> Timestamp {
        "2026-10-28T10:00:00Z".parse().unwrap()
    }

    fn sample() -> Vec<Task> {
        (1..=12)
            .map(|n| {
                let mut task = Task::new(TaskId(n), format!("Tâche {n}"), now());
                if n % 3 == 0 {
                    task.priority = Priority::P1;
                    task.tags = vec!["api".into()];
                    task.due = Some("2026-11-02T18:00".parse().unwrap());
                }
                task
            })
            .collect()
    }

    #[test]
    fn minimal_task_omits_defaults() {
        let line = render_tasks(&[Task::new(TaskId(1), "Écrire le README", now())]);
        assert_eq!(
            line,
            "{\"id\":\"T-1\",\"title\":\"Écrire le README\",\"rev\":1,\"ca\":\"2026-10-28T10:00:00Z\",\"ua\":\"2026-10-28T10:00:00Z\"}\n"
        );
    }

    #[test]
    fn round_trips() {
        let mut tasks = sample();
        tasks[0].status = TaskStatus::Blocked;
        tasks[0].blocked_reason = Some("clé API manquante".into());
        tasks[1].parent = Some(TaskId(1));
        tasks[1].description = Some("ligne 1\nligne 2".into());
        let text = render_tasks(&tasks);
        assert_eq!(parse_tasks(&text, Path::new(FILE)).unwrap(), tasks);
    }

    #[test]
    fn sorts_by_numeric_id() {
        let mut tasks = sample();
        tasks.reverse();
        let ids: Vec<String> = parse_tasks(&render_tasks(&tasks), Path::new(FILE))
            .unwrap()
            .iter()
            .map(|t| t.id.to_string())
            .collect();
        assert_eq!(ids[..3], ["T-1", "T-2", "T-3"]);
        assert_eq!(ids.last().unwrap(), "T-12");
    }

    #[test]
    fn one_change_is_one_line_of_diff() {
        let before = sample();
        let mut after = before.clone();
        after[4].priority = Priority::P1;
        after[4].rev += 1;

        let (before, after) = (render_tasks(&before), render_tasks(&after));
        let before: Vec<&str> = before.lines().collect();
        let after: Vec<&str> = after.lines().collect();
        assert_eq!(before.len(), after.len());
        let changed: Vec<usize> = (0..before.len()).filter(|&i| before[i] != after[i]).collect();
        assert_eq!(changed, [4]);
    }

    #[test]
    fn errors_point_at_the_line() {
        let mut text = render_tasks(&sample()[..2]);
        text.push_str("{\"id\":\"T-3\",\"title\":\"x\",\"rev\":1,\"ca\":\"bad\",\"ua\":\"bad\"}\n");
        let err = parse_tasks(&text, Path::new(FILE)).unwrap_err();
        assert!(matches!(err, StoreError::Invalid { line: Some(3), .. }), "{err}");
    }

    #[test]
    fn rejects_unknown_keys_and_duplicates() {
        let unknown = "{\"id\":\"T-1\",\"title\":\"x\",\"prio\":\"P1\",\"rev\":1,\"ca\":\"2026-10-28T10:00:00Z\",\"ua\":\"2026-10-28T10:00:00Z\"}\n";
        assert!(parse_tasks(unknown, Path::new(FILE)).is_err());

        let line = render_tasks(&sample()[..1]);
        let err = parse_tasks(&format!("{line}\n{line}"), Path::new(FILE)).unwrap_err();
        assert!(matches!(err, StoreError::Invalid { line: Some(3), .. }), "{err}");
    }
}
