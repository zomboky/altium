//! Operations on a project's task list, shared by the dashboard and MCP:
//! creation, lifecycle rules, hierarchy, deadlines, filters and the common
//! priority order.

use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};

use crate::error::StoreError;
use crate::patch::{Conflict, Edit, TaskPatch};
use crate::task::{Priority, Task, TaskId, TaskStatus};
use crate::time::{Clock, Deadline};

/// Hours before its deadline when an open task counts as near it.
pub const DEFAULT_NEAR_HOURS: u32 = 48;

/// Longest blocking reason, in characters.
pub const MAX_REASON_CHARS: usize = 200;

#[derive(Debug, thiserror::Error)]
pub enum TaskError {
    #[error(transparent)]
    Store(#[from] StoreError),
    #[error(transparent)]
    Conflict(#[from] Conflict),
    #[error("task {0} not found")]
    NotFound(TaskId),
    #[error("a blocked task needs a short reason (blocked_reason)")]
    BlockedWithoutReason,
    #[error("{task} cannot be a subtask of {parent}: {parent} is {task} or one of its subtasks")]
    Cycle { task: TaskId, parent: TaskId },
    #[error("{task} has open subtasks: {}; close them first or pass force", ids(.open))]
    OpenSubtasks { task: TaskId, open: Vec<TaskId> },
    #[error("{0}")]
    Invalid(String),
}

fn ids(list: &[TaskId]) -> String {
    list.iter().map(ToString::to_string).collect::<Vec<_>>().join(", ")
}

/// Fields of a task to create; everything but the title is optional.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct NewTask {
    pub title: String,
    pub description: Option<String>,
    pub status: Option<TaskStatus>,
    pub priority: Option<Priority>,
    pub due: Option<Deadline>,
    pub tags: Vec<String>,
    pub parent: Option<TaskId>,
    pub blocked_reason: Option<String>,
    pub note: Option<String>,
}

/// Creates a task with the next free id and returns it.
pub fn create(tasks: &mut Vec<Task>, new: NewTask, clock: &Clock) -> Result<Task, TaskError> {
    let id = TaskId(tasks.iter().map(|t| t.id.0).max().unwrap_or(0) + 1);
    let mut task = Task::new(id, "", clock.timestamp());
    if let Some(parent) = new.parent {
        find(tasks, parent)?;
    }
    let patch = TaskPatch {
        title: Some(Edit::set(new.title)),
        description: new.description.map(|d| Edit::set(Some(d))),
        status: new.status.map(Edit::set),
        priority: new.priority.map(Edit::set),
        due: new.due.map(|d| Edit::set(Some(d))),
        tags: Some(Edit::set(new.tags)),
        parent: new.parent.map(|p| Edit::set(Some(p))),
        blocked_reason: new.blocked_reason.map(|r| Edit::set(Some(r))),
        note: new.note.map(|n| Edit::set(Some(n))),
        position: None,
    };
    patch.apply(&mut task, clock.timestamp())?;
    task.rev = 1;
    normalize(&mut task, None, clock)?;
    tasks.push(task.clone());
    Ok(task)
}

/// Applies `patch` to task `id` and enforces the lifecycle rules. With
/// `force`, a task with open subtasks can still be closed. Nothing changes
/// on error.
pub fn update(
    tasks: &mut [Task],
    id: TaskId,
    patch: &TaskPatch,
    force: bool,
    clock: &Clock,
) -> Result<Task, TaskError> {
    let index = tasks.iter().position(|t| t.id == id).ok_or(TaskError::NotFound(id))?;
    let before = tasks[index].clone();
    let mut task = before.clone();
    patch.apply(&mut task, clock.timestamp())?;

    if task.parent != before.parent
        && let Some(parent) = task.parent
    {
        find(tasks, parent)?;
        if parent == id || descendants(tasks, id).contains(&parent) {
            return Err(TaskError::Cycle { task: id, parent });
        }
    }
    if task.status == TaskStatus::Done && before.status != TaskStatus::Done && !force {
        let open: Vec<TaskId> = descendants(tasks, id)
            .into_iter()
            .filter(|child| find(tasks, *child).is_ok_and(|t| is_open(t.status)))
            .collect();
        if !open.is_empty() {
            return Err(TaskError::OpenSubtasks { task: id, open });
        }
    }
    normalize(&mut task, Some(&before), clock)?;
    tasks[index] = task.clone();
    Ok(task)
}

/// Checks and completes a task after a change: clean title and tags, a
/// reason exactly while blocked, and an end date exactly while closed.
fn normalize(task: &mut Task, before: Option<&Task>, clock: &Clock) -> Result<(), TaskError> {
    task.title = task.title.trim().to_owned();
    if task.title.is_empty() {
        return Err(TaskError::Invalid("a task needs a title".into()));
    }
    let mut tags: Vec<String> = Vec::new();
    for tag in &task.tags {
        let tag = tag.trim().to_lowercase();
        if !tag.is_empty() && !tags.contains(&tag) {
            tags.push(tag);
        }
    }
    task.tags = tags;
    for field in [&mut task.description, &mut task.note, &mut task.blocked_reason] {
        if field.as_deref().is_some_and(|text| text.trim().is_empty()) {
            *field = None;
        }
    }

    if task.status == TaskStatus::Blocked {
        match &task.blocked_reason {
            None => return Err(TaskError::BlockedWithoutReason),
            Some(reason) if reason.chars().count() > MAX_REASON_CHARS => {
                return Err(TaskError::Invalid(format!(
                    "blocked_reason is limited to {MAX_REASON_CHARS} characters"
                )));
            }
            Some(_) => {}
        }
    } else {
        task.blocked_reason = None;
    }

    let closed = !is_open(task.status);
    let was_closed = before.is_some_and(|b| !is_open(b.status));
    if !closed {
        task.done_at = None;
    } else if !was_closed || task.done_at.is_none() {
        task.done_at = Some(clock.timestamp());
    }
    if before.is_some_and(|b| b != task) && before.is_some_and(|b| b.rev == task.rev) {
        task.rev += 1;
        task.updated_at = clock.timestamp();
    }
    Ok(())
}

/// Removes task `id` and all its subtasks; returns the removed ids.
pub fn delete(tasks: &mut Vec<Task>, id: TaskId) -> Result<Vec<TaskId>, TaskError> {
    find(tasks, id)?;
    let mut removed = vec![id];
    removed.extend(descendants(tasks, id));
    tasks.retain(|t| !removed.contains(&t.id));
    Ok(removed)
}

/// Moves open task `id` just before task `before` (or last with `None`)
/// among the open tasks of its priority, and renumbers their positions.
pub fn move_before(tasks: &mut [Task], id: TaskId, before: Option<TaskId>, clock: &Clock) -> Result<(), TaskError> {
    let priority = find(tasks, id)?.priority;
    if let Some(target) = before {
        let target = find(tasks, target)?;
        if target.priority != priority {
            return Err(TaskError::Invalid(format!(
                "{} is {:?} and {id} is {priority:?}: only tasks of the same priority can be reordered",
                target.id, target.priority
            )));
        }
    }
    let mut group: Vec<TaskId> = order(tasks, clock)
        .active
        .iter()
        .chain(order(tasks, clock).blocked.iter())
        .filter(|t| t.priority == priority && t.id != id)
        .map(|t| t.id)
        .collect();
    let at = before
        .and_then(|b| group.iter().position(|g| *g == b))
        .unwrap_or(group.len());
    group.insert(at, id);
    let now = clock.timestamp();
    for (index, task_id) in group.iter().enumerate() {
        let task = tasks.iter_mut().find(|t| t.id == *task_id).expect("task of the group");
        let position = Some(index as u32 + 1);
        if task.position != position {
            task.position = position;
            task.rev += 1;
            task.updated_at = now;
        }
    }
    Ok(())
}

pub fn find(tasks: &[Task], id: TaskId) -> Result<&Task, TaskError> {
    tasks.iter().find(|t| t.id == id).ok_or(TaskError::NotFound(id))
}

/// Whether a task in this status still has work to do.
pub fn is_open(status: TaskStatus) -> bool {
    !matches!(status, TaskStatus::Done | TaskStatus::Cancelled)
}

/// All subtasks of `id`, at any depth.
pub fn descendants(tasks: &[Task], id: TaskId) -> Vec<TaskId> {
    let mut children: HashMap<TaskId, Vec<TaskId>> = HashMap::new();
    for task in tasks {
        if let Some(parent) = task.parent {
            children.entry(parent).or_default().push(task.id);
        }
    }
    let mut found = Vec::new();
    let mut seen = HashSet::from([id]);
    let mut stack = vec![id];
    while let Some(current) = stack.pop() {
        for child in children.get(&current).into_iter().flatten() {
            if seen.insert(*child) {
                found.push(*child);
                stack.push(*child);
            }
        }
    }
    found.sort();
    found
}

/// Progress of a parent task: its direct subtasks done, out of those not
/// cancelled. `None` without subtasks.
pub fn progress(tasks: &[Task], id: TaskId) -> Option<(usize, usize)> {
    let children: Vec<&Task> = tasks
        .iter()
        .filter(|t| t.parent == Some(id) && t.status != TaskStatus::Cancelled)
        .collect();
    if children.is_empty() {
        return None;
    }
    let done = children.iter().filter(|t| t.status == TaskStatus::Done).count();
    Some((done, children.len()))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeadlineState {
    /// Open, deadline passed.
    Overdue,
    /// Open, deadline within the near window.
    Near,
    /// Open, deadline further away.
    Later,
    /// No deadline, or the task is closed.
    #[serde(rename = "none")]
    NoDeadline,
}

pub fn deadline_state(task: &Task, clock: &Clock, near_hours: u32) -> DeadlineState {
    match task.due {
        Some(due) if is_open(task.status) => {
            let at = clock.deadline(due);
            if at < clock.now {
                DeadlineState::Overdue
            } else if at - clock.now <= Duration::hours(near_hours.into()) {
                DeadlineState::Near
            } else {
                DeadlineState::Later
            }
        }
        _ => DeadlineState::NoDeadline,
    }
}

/// Which tasks to keep; empty criteria keep everything.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TaskFilter {
    /// Tasks carrying all these tags.
    pub tags: Vec<String>,
    pub status: Vec<TaskStatus>,
    pub priority: Vec<Priority>,
    pub deadline: Vec<DeadlineState>,
    /// Case-insensitive text in the title, description or note.
    pub text: Option<String>,
}

impl TaskFilter {
    pub fn matches(&self, task: &Task, clock: &Clock, near_hours: u32) -> bool {
        let text = self.text.as_deref().map(str::to_lowercase).filter(|t| !t.is_empty());
        self.tags.iter().all(|tag| task.tags.contains(&tag.to_lowercase()))
            && (self.status.is_empty() || self.status.contains(&task.status))
            && (self.priority.is_empty() || self.priority.contains(&task.priority))
            && (self.deadline.is_empty() || self.deadline.contains(&deadline_state(task, clock, near_hours)))
            && text.is_none_or(|text| {
                [Some(&task.title), task.description.as_ref(), task.note.as_ref()]
                    .into_iter()
                    .flatten()
                    .any(|field| field.to_lowercase().contains(&text))
            })
    }
}

/// Tasks in the common priority order.
#[derive(Debug, Default)]
pub struct Ordered<'a> {
    /// `doing` and `todo` tasks, next to work on first.
    pub active: Vec<&'a Task>,
    /// Blocked tasks, listed apart, in the same order.
    pub blocked: Vec<&'a Task>,
    /// Done and cancelled tasks, most recently closed first.
    pub closed: Vec<&'a Task>,
}

/// Sorts tasks the same way for the user and for Claude: `doing` first, then
/// by priority, nearest deadline, manual position and age.
pub fn order<'a>(tasks: &'a [Task], clock: &Clock) -> Ordered<'a> {
    let mut ordered = Ordered::default();
    for task in tasks {
        match task.status {
            TaskStatus::Todo | TaskStatus::Doing => ordered.active.push(task),
            TaskStatus::Blocked => ordered.blocked.push(task),
            TaskStatus::Done | TaskStatus::Cancelled => ordered.closed.push(task),
        }
    }
    ordered.active.sort_by(|a, b| compare(a, b, clock));
    ordered.blocked.sort_by(|a, b| compare(a, b, clock));
    ordered
        .closed
        .sort_by(|a, b| b.done_at.cmp(&a.done_at).then(b.id.cmp(&a.id)));
    ordered
}

fn compare(a: &Task, b: &Task, clock: &Clock) -> Ordering {
    let due = |t: &Task| -> Option<DateTime<Utc>> { t.due.map(|d| clock.deadline(d)) };
    (a.status != TaskStatus::Doing)
        .cmp(&(b.status != TaskStatus::Doing))
        .then(a.priority.cmp(&b.priority))
        .then(none_last(due(a), due(b)))
        .then(none_last(a.position, b.position))
        .then(a.created_at.cmp(&b.created_at))
        .then(a.id.cmp(&b.id))
}

fn none_last<T: Ord>(a: Option<T>, b: Option<T>) -> Ordering {
    match (a, b) {
        (Some(a), Some(b)) => a.cmp(&b),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::time::Timestamp;

    fn clock() -> Clock {
        Clock::fixed("2026-10-28T12:00:00Z".parse::<Timestamp>().unwrap().as_datetime())
    }

    fn new(title: &str) -> NewTask {
        NewTask {
            title: title.into(),
            ..NewTask::default()
        }
    }

    fn status(value: TaskStatus) -> TaskPatch {
        TaskPatch {
            status: Some(Edit::set(value)),
            ..TaskPatch::default()
        }
    }

    #[test]
    fn minimal_task() {
        let mut tasks = Vec::new();
        create(&mut tasks, new("Avant"), &clock()).unwrap();
        let task = create(&mut tasks, new("  Écrire le README "), &clock()).unwrap();
        assert_eq!(task.id.to_string(), "T-2");
        assert_eq!(task.title, "Écrire le README");
        assert_eq!((task.status, task.priority), (TaskStatus::Todo, Priority::P2));
        assert!(task.due.is_none() && task.tags.is_empty());
        assert!(create(&mut tasks, new("  "), &clock()).is_err());
    }

    #[test]
    fn blocking_needs_a_reason_cleared_on_unblock() {
        let mut tasks = Vec::new();
        let id = create(&mut tasks, new("Intégrer Stripe"), &clock()).unwrap().id;
        let err = update(&mut tasks, id, &status(TaskStatus::Blocked), false, &clock()).unwrap_err();
        assert!(matches!(err, TaskError::BlockedWithoutReason), "{err}");
        assert_eq!(tasks[0].status, TaskStatus::Todo);

        let block = TaskPatch {
            blocked_reason: Some(Edit::set(Some("clé API manquante".into()))),
            ..status(TaskStatus::Blocked)
        };
        update(&mut tasks, id, &block, false, &clock()).unwrap();
        let task = update(&mut tasks, id, &status(TaskStatus::Doing), false, &clock()).unwrap();
        assert_eq!(task.blocked_reason, None);
    }

    #[test]
    fn closing_sets_and_reopening_clears_the_end_date() {
        let mut tasks = Vec::new();
        let id = create(&mut tasks, new("x"), &clock()).unwrap().id;
        let done = update(&mut tasks, id, &status(TaskStatus::Done), false, &clock()).unwrap();
        assert_eq!(done.done_at, Some(clock().timestamp()));
        assert_eq!(done.rev, 2);
        let reopened = update(&mut tasks, id, &status(TaskStatus::Todo), false, &clock()).unwrap();
        assert_eq!(reopened.done_at, None);
    }

    #[test]
    fn hierarchy_progress_cycles_and_closing() {
        let c = clock();
        let mut tasks = Vec::new();
        let parent = create(&mut tasks, new("parent"), &c).unwrap().id;
        let children: Vec<TaskId> = (0..4)
            .map(|n| {
                let child = NewTask {
                    parent: Some(parent),
                    ..new(&format!("enfant {n}"))
                };
                create(&mut tasks, child, &c).unwrap().id
            })
            .collect();
        for child in &children[..3] {
            update(&mut tasks, *child, &status(TaskStatus::Done), false, &c).unwrap();
        }
        assert_eq!(progress(&tasks, parent), Some((3, 4)));

        let grandchild = create(
            &mut tasks,
            NewTask {
                parent: Some(children[3]),
                ..new("petit-enfant")
            },
            &c,
        )
        .unwrap()
        .id;
        let cycle = TaskPatch {
            parent: Some(Edit::set(Some(grandchild))),
            ..TaskPatch::default()
        };
        assert!(matches!(
            update(&mut tasks, parent, &cycle, false, &c),
            Err(TaskError::Cycle { .. })
        ));

        let err = update(&mut tasks, parent, &status(TaskStatus::Done), false, &c).unwrap_err();
        match &err {
            TaskError::OpenSubtasks { open, .. } => assert_eq!(open, &[children[3], grandchild]),
            other => panic!("{other}"),
        }
        assert!(err.to_string().contains("T-5, T-6"), "{err}");
        update(&mut tasks, parent, &status(TaskStatus::Done), true, &c).unwrap();

        assert_eq!(delete(&mut tasks, children[3]).unwrap(), [children[3], grandchild]);
        assert_eq!(tasks.len(), 4);
    }

    #[test]
    fn deadlines_and_filters() {
        let c = clock();
        let mut tasks = Vec::new();
        let due = |s: &str| Some(s.parse().unwrap());
        for (title, deadline) in [
            ("hier", due("2026-10-27")),
            ("demain", due("2026-10-29T18:00")),
            ("plus tard", due("2026-11-15")),
            ("sans", None),
        ] {
            let task = NewTask {
                due: deadline,
                tags: vec!["Backend".into()],
                ..new(title)
            };
            create(&mut tasks, task, &c).unwrap();
        }
        let states: Vec<DeadlineState> = tasks.iter().map(|t| deadline_state(t, &c, 48)).collect();
        use DeadlineState::*;
        assert_eq!(states, [Overdue, Near, Later, NoDeadline]);

        update(&mut tasks, TaskId(1), &status(TaskStatus::Done), false, &c).unwrap();
        assert_eq!(deadline_state(&tasks[0], &c, 48), NoDeadline);

        let filter = TaskFilter {
            tags: vec!["backend".into()],
            deadline: vec![Near, Later],
            ..TaskFilter::default()
        };
        let kept: Vec<&str> = tasks
            .iter()
            .filter(|t| filter.matches(t, &c, 48))
            .map(|t| t.title.as_str())
            .collect();
        assert_eq!(kept, ["demain", "plus tard"]);
    }

    #[test]
    fn common_priority_order() {
        let c = clock();
        let mut tasks = Vec::new();
        let mut add = |title: &str, priority: Priority, due: Option<&str>| {
            let task = NewTask {
                priority: Some(priority),
                due: due.map(|d| d.parse().unwrap()),
                ..new(title)
            };
            create(&mut tasks, task, &c).unwrap().id
        };
        let old_p1 = add("P1 ancienne", Priority::P1, None);
        let p0 = add("P0", Priority::P0, None);
        let p1_due = add("P1 demain", Priority::P1, Some("2026-10-29"));
        let doing = add("P3 en cours", Priority::P3, None);
        let new_p1 = add("P1 récente", Priority::P1, None);
        let blocked = add("bloquée", Priority::P0, None);
        update(&mut tasks, doing, &status(TaskStatus::Doing), false, &c).unwrap();
        let block = TaskPatch {
            blocked_reason: Some(Edit::set(Some("attente".into()))),
            ..status(TaskStatus::Blocked)
        };
        update(&mut tasks, blocked, &block, false, &c).unwrap();
        tasks.iter_mut().find(|t| t.id == new_p1).unwrap().created_at = "2026-10-28T13:00:00Z".parse().unwrap();

        let ids = |tasks: &[Task]| order(tasks, &c).active.iter().map(|t| t.id).collect::<Vec<_>>();
        assert_eq!(ids(&tasks), [doing, p0, p1_due, old_p1, new_p1]);
        assert_eq!(order(&tasks, &c).blocked[0].id, blocked);

        // The user drags the recent P1 above the older one.
        move_before(&mut tasks, new_p1, Some(old_p1), &c).unwrap();
        assert_eq!(ids(&tasks), [doing, p0, p1_due, new_p1, old_p1]);
        assert!(move_before(&mut tasks, new_p1, Some(p0), &c).is_err());
    }
}
