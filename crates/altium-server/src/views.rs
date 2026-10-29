//! What the dashboard shows, computed from a project's `.altium/`: JSON
//! shapes with readable field names.

use std::collections::HashSet;
use std::path::Path;

use altium_core::activity::{self, Archive, SessionView, Sessions};
use altium_core::config::GlobalConfig;
use altium_core::graph::{Graph, World, kind_name, link_name};
use altium_core::health::{self, Health, Progress};
use altium_core::memory::{LinkTarget, LinkType, MemoryNode};
use altium_core::project::{Project, ProjectStatus};
use altium_core::registry::Listed;
use altium_core::render::{priority_name, status_name};
use altium_core::store::Store;
use altium_core::task::{Task, TaskStatus};
use altium_core::tasks::{self, DeadlineState, deadline_state};
use altium_core::time::{Clock, Timestamp};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct TaskView {
    pub id: String,
    pub title: String,
    pub status: &'static str,
    pub priority: &'static str,
    pub due: Option<String>,
    pub deadline: DeadlineState,
    pub tags: Vec<String>,
    pub parent: Option<String>,
    pub position: Option<u32>,
    pub description: Option<String>,
    pub blocked_reason: Option<String>,
    pub note: Option<String>,
    pub rev: u32,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
    pub done_at: Option<Timestamp>,
    /// Done and total direct subtasks, when it has some.
    pub progress: Option<(usize, usize)>,
}

pub fn task(task: &Task, all: &[Task], clock: &Clock, near: u32) -> TaskView {
    TaskView {
        id: task.id.to_string(),
        title: task.title.clone(),
        status: status_name(task.status),
        priority: priority_name(task.priority),
        due: task.due.map(|d| d.to_string()),
        deadline: deadline_state(task, clock, near),
        tags: task.tags.clone(),
        parent: task.parent.map(|p| p.to_string()),
        position: task.position,
        description: task.description.clone(),
        blocked_reason: task.blocked_reason.clone(),
        note: task.note.clone(),
        rev: task.rev,
        created_at: task.created_at,
        updated_at: task.updated_at,
        done_at: task.done_at,
        progress: tasks::progress(all, task.id),
    }
}

/// Tasks in the common order: active, then blocked, then closed.
#[derive(Debug, Serialize)]
pub struct TaskList {
    pub tasks: Vec<TaskView>,
    pub near_hours: u32,
}

pub fn task_list(all: &[Task], clock: &Clock, near: u32) -> TaskList {
    let ordered = tasks::order(all, clock);
    let tasks = ordered
        .active
        .iter()
        .chain(&ordered.blocked)
        .chain(&ordered.closed)
        .map(|t| task(t, all, clock, near))
        .collect();
    TaskList {
        tasks,
        near_hours: near,
    }
}

/// Activity of a project: sessions, from the records and the log.
pub fn sessions(store: &Store, config: &GlobalConfig, clock: &Clock) -> Vec<SessionView> {
    let records: Sessions = store.read_runtime(activity::SESSIONS_FILE);
    activity::sessions(
        &records,
        &activity::recent_events(store),
        clock.now,
        config.idle_minutes(),
    )
}

#[derive(Debug, Serialize)]
pub struct ProjectCard {
    pub id: String,
    pub name: String,
    pub path: String,
    pub present: bool,
    pub read_only: bool,
    pub phase: Option<String>,
    pub summary: Option<String>,
    pub progress: Option<Progress>,
    pub health: Option<Health>,
    pub next_deadline: Option<NextDeadline>,
    /// The active session most recently seen, if any.
    pub session: Option<SessionView>,
    pub last_activity: Option<Timestamp>,
    pub error: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct NextDeadline {
    pub task: String,
    pub title: String,
    pub due: String,
    pub state: DeadlineState,
}

pub fn card(listed: &Listed, config: &GlobalConfig, clock: &Clock) -> ProjectCard {
    let entry = &listed.entry;
    let mut card = ProjectCard {
        id: entry.id.clone(),
        name: listed.project.as_ref().map_or(entry.name.clone(), |p| p.name.clone()),
        path: entry.path.display().to_string(),
        present: listed.project.is_some(),
        read_only: false,
        phase: None,
        summary: None,
        progress: None,
        health: None,
        next_deadline: None,
        session: None,
        last_activity: None,
        error: None,
    };
    let Some(project) = &listed.project else {
        return card;
    };
    let store = match Store::open(&entry.path) {
        Ok(store) => store,
        Err(e) => {
            card.error = Some(e.to_string());
            return card;
        }
    };
    card.read_only = store.is_read_only();
    let near = config.near_hours(&project.config);
    match store.status() {
        Ok(status) => {
            card.phase = Some(status.phase);
            card.summary = Some(status.summary);
            card.last_activity = status.updated_at;
        }
        Err(e) => card.error = Some(e.to_string()),
    }
    let sessions = sessions(&store, config, clock);
    let active = sessions.iter().find(|s| s.active).cloned();
    if let Some(last) = sessions.first() {
        card.last_activity = card.last_activity.max(Some(last.last_event));
    }
    match store.tasks() {
        Ok(all) => {
            let progress = health::progress(&all, clock, near);
            card.health = Some(Health::new(&progress, active.is_some()));
            card.progress = Some(progress);
            card.next_deadline = all
                .iter()
                .filter(|t| tasks::is_open(t.status))
                .filter_map(|t| t.due.map(|d| (clock.deadline(d), t)))
                .min_by_key(|(at, _)| *at)
                .map(|(_, t)| NextDeadline {
                    task: t.id.to_string(),
                    title: t.title.clone(),
                    due: t.due.expect("filtered").to_string(),
                    state: deadline_state(t, clock, near),
                });
            if let Some(updated) = all.iter().map(|t| t.updated_at).max() {
                card.last_activity = card.last_activity.max(Some(updated));
            }
        }
        Err(e) => card.error = card.error.take().or(Some(e.to_string())),
    }
    card.session = active;
    card
}

#[derive(Debug, Serialize)]
pub struct ProjectDetail {
    pub id: String,
    pub name: String,
    pub path: String,
    pub read_only: bool,
    pub phases: Vec<String>,
    pub memory_lang: String,
    pub near_hours: u32,
    pub status: Option<ProjectStatus>,
    pub progress: Option<Progress>,
    pub health: Option<Health>,
    pub errors: Vec<String>,
}

pub fn detail(store: &Store, project: &Project, config: &GlobalConfig, clock: &Clock) -> ProjectDetail {
    let near = config.near_hours(&project.config);
    let mut errors = Vec::new();
    let status = store.status().map_err(|e| errors.push(e.to_string())).ok();
    let active = sessions(store, config, clock).iter().any(|s| s.active);
    let (progress, health) = match store.tasks() {
        Ok(all) => {
            let progress = health::progress(&all, clock, near);
            let health = Health::new(&progress, active);
            (Some(progress), Some(health))
        }
        Err(e) => {
            errors.push(e.to_string());
            (None, None)
        }
    };
    if let Ok((_, node_errors)) = store.graph() {
        errors.extend(node_errors.iter().map(ToString::to_string));
    }
    ProjectDetail {
        id: project.id.clone(),
        name: project.name.clone(),
        path: store.root().display().to_string(),
        read_only: store.is_read_only(),
        phases: project.config.phases(),
        memory_lang: project.config.memory_lang().to_owned(),
        near_hours: near,
        status,
        progress,
        health,
        errors,
    }
}

#[derive(Debug, Serialize)]
pub struct NodeLine {
    pub id: String,
    pub kind: &'static str,
    pub title: String,
    pub description: String,
    pub tags: Vec<String>,
    pub pinned: bool,
    pub updated: Timestamp,
    pub superseded_by: Option<String>,
    pub broken_links: usize,
    pub contradicts: Vec<String>,
}

fn world_tasks(store: &Store) -> HashSet<altium_core::task::TaskId> {
    store
        .tasks()
        .map(|t| t.iter().map(|t| t.id).collect())
        .unwrap_or_default()
}

fn contradictions(graph: &Graph, node: &MemoryNode) -> Vec<String> {
    let mut list: Vec<String> = node
        .meta
        .links
        .iter()
        .filter(|l| l.kind == LinkType::Contradicts)
        .map(|l| l.to.to_string())
        .collect();
    list.extend(
        graph
            .incoming(&node.id)
            .iter()
            .filter(|(_, k)| *k == LinkType::Contradicts)
            .map(|(from, _)| from.clone()),
    );
    list
}

pub fn memory_index(store: &Store, graph: &Graph) -> Vec<NodeLine> {
    let task_ids = world_tasks(store);
    let world = World {
        tasks: &task_ids,
        root: store.root(),
    };
    graph
        .nodes()
        .map(|n| NodeLine {
            id: n.id.clone(),
            kind: kind_name(n.meta.kind),
            title: n.meta.title.clone(),
            description: n.meta.description.clone(),
            tags: n.meta.tags.clone(),
            pinned: n.meta.pinned,
            updated: n.meta.updated,
            superseded_by: graph.superseded_by(&n.id).map(str::to_owned),
            broken_links: graph.broken_links(n, &world).len(),
            contradicts: contradictions(graph, n),
        })
        .collect()
}

#[derive(Debug, Serialize)]
pub struct LinkView {
    pub kind: &'static str,
    /// `node`, `task`, `file` or `url`.
    pub target_type: &'static str,
    pub target: String,
    pub title: Option<String>,
    pub note: Option<String>,
    pub broken: bool,
}

#[derive(Debug, Serialize)]
pub struct NodeView {
    #[serde(flatten)]
    pub line: NodeLine,
    pub created: Timestamp,
    pub body: String,
    pub links: Vec<LinkView>,
    pub incoming: Vec<LinkView>,
}

pub fn node(store: &Store, graph: &Graph, node: &MemoryNode) -> NodeView {
    let task_ids = world_tasks(store);
    let world = World {
        tasks: &task_ids,
        root: store.root(),
    };
    let broken: Vec<String> = graph
        .broken_links(node, &world)
        .iter()
        .map(|l| l.to.to_string())
        .collect();
    let target_type = |t: &LinkTarget| match t {
        LinkTarget::Node(_) => "node",
        LinkTarget::Task(_) => "task",
        LinkTarget::File(_) => "file",
        LinkTarget::Url(_) => "url",
    };
    let links = node
        .meta
        .links
        .iter()
        .map(|l| LinkView {
            kind: link_name(l.kind),
            target_type: target_type(&l.to),
            target: match &l.to {
                LinkTarget::Task(id) => id.to_string(),
                LinkTarget::File(path) => path.clone(),
                other => other.to_string(),
            },
            title: match &l.to {
                LinkTarget::Node(id) => graph.get(id).map(|n| n.meta.title.clone()),
                _ => None,
            },
            note: l.note.clone(),
            broken: broken.contains(&l.to.to_string()),
        })
        .collect();
    let incoming = graph
        .incoming(&node.id)
        .iter()
        .map(|(from, kind)| LinkView {
            kind: link_name(*kind),
            target_type: "node",
            target: from.clone(),
            title: graph.get(from).map(|n| n.meta.title.clone()),
            note: None,
            broken: false,
        })
        .collect();
    let line = memory_index(store, graph)
        .into_iter()
        .find(|l| l.id == node.id)
        .expect("node of the graph");
    NodeView {
        line,
        created: node.meta.created,
        body: node.body.clone(),
        links,
        incoming,
    }
}

/// Counters of the period, from rotated logs and the active one.
pub fn stats(store: &Store, days: i64, clock: &Clock) -> activity::Stats {
    let archive: Archive = store.read_runtime(activity::STATS_FILE);
    let to = clock.now.date_naive();
    let from = to - chrono::Duration::days(days.max(1) - 1);
    activity::stats(&archive, &activity::events(store), from, to)
}

/// Whether a task status counts as closed for the dashboard.
pub fn is_closed(status: TaskStatus) -> bool {
    !tasks::is_open(status)
}

/// The project at `path` exists.
pub fn present(path: &Path) -> bool {
    path.join(altium_core::store::DIR_NAME).is_dir()
}
