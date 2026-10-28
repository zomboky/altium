//! Field-level task edits with conflict detection.
//!
//! The dashboard sends only the fields it changed, each with the value it
//! based the change on. The patch applies if every edited field still has that
//! value, so edits of different fields by Claude and the user merge, and an
//! edit of a field someone else changed in the meantime is reported as a
//! conflict instead of silently overwriting it.

use crate::task::{Priority, Task, TaskId, TaskStatus};
use crate::time::{Deadline, Timestamp};

/// A change to one field. With `expect`, it only applies if the field still
/// holds that value; without it, it always applies (e.g. from MCP, which
/// works on the latest version).
#[derive(Debug, Clone, PartialEq)]
pub struct Edit<T> {
    pub expect: Option<T>,
    pub set: T,
}

impl<T> Edit<T> {
    pub fn set(value: T) -> Self {
        Self {
            expect: None,
            set: value,
        }
    }

    pub fn from(base: T, value: T) -> Self {
        Self {
            expect: Some(base),
            set: value,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct TaskPatch {
    pub title: Option<Edit<String>>,
    pub description: Option<Edit<Option<String>>>,
    pub status: Option<Edit<TaskStatus>>,
    pub priority: Option<Edit<Priority>>,
    pub due: Option<Edit<Option<Deadline>>>,
    pub tags: Option<Edit<Vec<String>>>,
    pub parent: Option<Edit<Option<TaskId>>>,
    pub position: Option<Edit<Option<u32>>>,
    pub blocked_reason: Option<Edit<Option<String>>>,
    pub note: Option<Edit<Option<String>>>,
}

/// The patch was based on values that changed since. `task` is the current
/// version, for the editor to show.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
#[error("{} changed since it was read: {}", task.id, fields.join(", "))]
pub struct Conflict {
    pub fields: Vec<&'static str>,
    pub task: Box<Task>,
}

/// Calls `$m!(field, "name")` for every patchable field.
macro_rules! for_each_field {
    ($m:ident) => {
        $m!(title, "title");
        $m!(description, "description");
        $m!(status, "status");
        $m!(priority, "priority");
        $m!(due, "due");
        $m!(tags, "tags");
        $m!(parent, "parent");
        $m!(position, "position");
        $m!(blocked_reason, "blocked_reason");
        $m!(note, "note");
    };
}

impl TaskPatch {
    /// Applies the patch to `task`, or changes nothing and returns a
    /// [`Conflict`]. Returns whether the task changed; if so, its revision and
    /// update time are bumped.
    pub fn apply(&self, task: &mut Task, now: Timestamp) -> Result<bool, Conflict> {
        let mut conflicts = Vec::new();
        macro_rules! check {
            ($field:ident, $name:literal) => {
                if let Some(Edit {
                    expect: Some(base), ..
                }) = &self.$field
                    && *base != task.$field
                {
                    conflicts.push($name);
                }
            };
        }
        for_each_field!(check);
        if !conflicts.is_empty() {
            return Err(Conflict {
                fields: conflicts,
                task: Box::new(task.clone()),
            });
        }

        let before = task.clone();
        macro_rules! assign {
            ($field:ident, $name:literal) => {
                if let Some(edit) = &self.$field {
                    task.$field = edit.set.clone();
                }
            };
        }
        for_each_field!(assign);

        let changed = *task != before;
        if changed {
            task.rev += 1;
            task.updated_at = now;
        }
        Ok(changed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(minute: u32) -> Timestamp {
        format!("2026-10-28T10:{minute:02}:00Z").parse().unwrap()
    }

    fn task() -> Task {
        Task::new(TaskId(1), "Écrire le README", at(0))
    }

    #[test]
    fn edits_of_different_fields_merge() {
        let read_by_user = task();
        let mut current = task();

        // Claude changes the status, from MCP.
        let claude = TaskPatch {
            status: Some(Edit::set(TaskStatus::Doing)),
            ..TaskPatch::default()
        };
        assert!(claude.apply(&mut current, at(1)).unwrap());

        // The user, still on the old version, changes the deadline.
        let due = Some("2026-11-02".parse().unwrap());
        let user = TaskPatch {
            due: Some(Edit::from(read_by_user.due, due)),
            ..TaskPatch::default()
        };
        assert!(user.apply(&mut current, at(2)).unwrap());

        assert_eq!((current.status, current.due), (TaskStatus::Doing, due));
        assert_eq!((current.rev, current.updated_at), (3, at(2)));
    }

    #[test]
    fn edit_of_a_changed_field_conflicts() {
        let read_by_user = task();
        let mut current = task();
        current.title = "Écrire la doc".into();
        current.rev = 2;

        let user = TaskPatch {
            title: Some(Edit::from(read_by_user.title.clone(), "README complet".into())),
            priority: Some(Edit::from(read_by_user.priority, Priority::P0)),
            ..TaskPatch::default()
        };
        let conflict = user.apply(&mut current, at(3)).unwrap_err();
        assert_eq!(conflict.fields, ["title"]);
        assert_eq!(conflict.task.title, "Écrire la doc");
        // Nothing applied, not even the non-conflicting priority.
        assert_eq!((current.priority, current.rev), (Priority::P2, 2));
    }

    #[test]
    fn no_op_patch_keeps_the_revision() {
        let mut current = task();
        let patch = TaskPatch {
            priority: Some(Edit::from(Priority::P2, Priority::P2)),
            ..TaskPatch::default()
        };
        assert!(!patch.apply(&mut current, at(4)).unwrap());
        assert_eq!((current.rev, current.updated_at), (1, at(0)));
    }
}
