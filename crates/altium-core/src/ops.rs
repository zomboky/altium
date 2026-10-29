//! Changes to a project, as made by Claude (MCP) or the user (dashboard):
//! each runs under the project lock and is recorded with its actor in
//! `runtime/history.jsonl`.

use std::collections::{BTreeMap, HashSet};
use std::fs::{self, OpenOptions};
use std::io::Write as _;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::actor::Actor;
use crate::error::StoreError;
use crate::graph::{self, Graph};
use crate::health;
use crate::memory::{Link, LinkTarget, LinkType, MemoryNode, NodeKind, NodeMeta};
use crate::patch::TaskPatch;
use crate::project::ProjectStatus;
use crate::store::Store;
use crate::task::{Task, TaskId};
use crate::tasks::{self, NewTask, TaskError};
use crate::time::{Clock, Timestamp};

pub const HISTORY_FILE: &str = "history.jsonl";

/// One recorded change.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Change {
    #[serde(rename = "t")]
    pub at: Timestamp,
    pub by: Actor,
    /// `T-12`, `status` or `memory:<id>`.
    pub target: String,
    /// `created`, `updated`, `deleted`…
    pub action: String,
    /// New values of the changed fields.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub fields: BTreeMap<String, Value>,
}

fn record(store: &Store, change: &Change) {
    let path = store.runtime_path(HISTORY_FILE);
    let mut line = serde_json::to_string(change).expect("changes serialize");
    line.push('\n');
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
        let _ = file.write_all(line.as_bytes());
    }
}

/// Recorded changes of `target`, oldest first.
pub fn history(store: &Store, target: &str) -> Vec<Change> {
    fs::read_to_string(store.runtime_path(HISTORY_FILE))
        .unwrap_or_default()
        .lines()
        .filter_map(|l| serde_json::from_str::<Change>(l).ok())
        .filter(|c| c.target == target)
        .collect()
}

/// All recorded changes, oldest first.
pub fn all_history(store: &Store) -> Vec<Change> {
    fs::read_to_string(store.runtime_path(HISTORY_FILE))
        .unwrap_or_default()
        .lines()
        .filter_map(|l| serde_json::from_str::<Change>(l).ok())
        .collect()
}

fn diff(before: &Task, after: &Task) -> BTreeMap<String, Value> {
    let (Value::Object(old), Value::Object(new)) = (
        serde_json::to_value(before).expect("tasks serialize"),
        serde_json::to_value(after).expect("tasks serialize"),
    ) else {
        return BTreeMap::new();
    };
    let mut fields = BTreeMap::new();
    for key in old.keys().chain(new.keys()) {
        if matches!(key.as_str(), "rev" | "ua") {
            continue;
        }
        if old.get(key) != new.get(key) {
            fields.insert(key.clone(), new.get(key).cloned().unwrap_or(Value::Null));
        }
    }
    fields
}

fn task_change(at: Timestamp, by: &Actor, task: &Task, action: &str, fields: BTreeMap<String, Value>) -> Change {
    Change {
        at,
        by: by.clone(),
        target: task.id.to_string(),
        action: action.into(),
        fields,
    }
}

pub fn create_task(store: &Store, new: NewTask, by: &Actor, clock: &Clock) -> Result<Task, TaskError> {
    let task = store.update_tasks(|list| tasks::create(list, new, clock))?;
    let fields = diff(&Task::new(task.id, "", task.created_at), &task);
    record(store, &task_change(clock.timestamp(), by, &task, "created", fields));
    Ok(task)
}

pub fn update_task(
    store: &Store,
    id: TaskId,
    patch: &TaskPatch,
    force: bool,
    by: &Actor,
    clock: &Clock,
) -> Result<Task, TaskError> {
    let (before, after) = store.update_tasks(|list| {
        let before = tasks::find(list, id)?.clone();
        let after = tasks::update(list, id, patch, force, clock)?;
        Ok::<_, TaskError>((before, after))
    })?;
    let fields = diff(&before, &after);
    if !fields.is_empty() {
        record(store, &task_change(clock.timestamp(), by, &after, "updated", fields));
    }
    Ok(after)
}

/// Deletes a task and its subtasks: the dashboard only, never Claude.
pub fn delete_task(store: &Store, id: TaskId, by: &Actor, clock: &Clock) -> Result<Vec<TaskId>, TaskError> {
    let removed = store.update_tasks(|list| tasks::delete(list, id))?;
    for task in &removed {
        record(
            store,
            &Change {
                at: clock.timestamp(),
                by: by.clone(),
                target: task.to_string(),
                action: "deleted".into(),
                fields: BTreeMap::new(),
            },
        );
    }
    Ok(removed)
}

pub fn move_task(
    store: &Store,
    id: TaskId,
    before: Option<TaskId>,
    by: &Actor,
    clock: &Clock,
) -> Result<(), TaskError> {
    store.update_tasks(|list| tasks::move_before(list, id, before, clock))?;
    let mut fields = BTreeMap::new();
    fields.insert(
        "before".into(),
        before.map_or(Value::Null, |b| Value::String(b.to_string())),
    );
    record(
        store,
        &Change {
            at: clock.timestamp(),
            by: by.clone(),
            target: id.to_string(),
            action: "moved".into(),
            fields,
        },
    );
    Ok(())
}

#[derive(Debug, thiserror::Error)]
pub enum OpError {
    #[error(transparent)]
    Store(#[from] StoreError),
    #[error("{0}")]
    Invalid(String),
    #[error("memory node `{0}` not found")]
    NodeNotFound(String),
}

pub fn set_status(
    store: &Store,
    summary: Option<String>,
    phase: Option<String>,
    by: &Actor,
    clock: &Clock,
) -> Result<ProjectStatus, OpError> {
    let config = store.project()?.config;
    let status = store.update_status(|status| {
        health::set_status(status, &config, summary.clone(), phase.clone(), by.clone(), clock)
            .map_err(OpError::Invalid)?;
        Ok::<_, OpError>(status.clone())
    })?;
    let mut fields = BTreeMap::new();
    if let Some(summary) = summary {
        fields.insert("summary".into(), Value::String(summary));
    }
    if let Some(phase) = phase {
        fields.insert("phase".into(), Value::String(phase));
    }
    record(
        store,
        &Change {
            at: clock.timestamp(),
            by: by.clone(),
            target: "status".into(),
            action: "updated".into(),
            fields,
        },
    );
    Ok(status)
}

/// A node to create or update; unset fields keep their current value.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct NodeInput {
    pub id: String,
    pub kind: Option<NodeKind>,
    pub title: Option<String>,
    #[serde(alias = "desc")]
    pub description: Option<String>,
    pub body: Option<String>,
    pub tags: Option<Vec<String>>,
    pub pinned: Option<bool>,
    /// Replaces all links when set.
    pub links: Option<Vec<Link>>,
}

/// Creates or updates a node; returns it, whether it was created, and
/// warnings such as a split suggestion.
pub fn put_node(
    store: &Store,
    input: NodeInput,
    by: &Actor,
    clock: &Clock,
) -> Result<(MemoryNode, bool, Vec<String>), OpError> {
    let existing = match store.memory_node(&input.id) {
        Ok(node) => Some(node),
        Err(StoreError::Io { source, .. }) if source.kind() == std::io::ErrorKind::NotFound => None,
        Err(StoreError::InvalidNodeId(id)) => {
            return Err(OpError::Invalid(format!(
                "invalid node id `{id}`: use lowercase letters, digits and dashes (e.g. auth-jwt)"
            )));
        }
        Err(e) => return Err(e.into()),
    };
    let created = existing.is_none();
    let now = clock.timestamp();
    let mut node = match existing {
        Some(node) => node,
        None => MemoryNode {
            id: input.id.clone(),
            meta: NodeMeta {
                kind: input
                    .kind
                    .ok_or_else(|| OpError::Invalid("a new node needs a kind".into()))?,
                title: String::new(),
                description: String::new(),
                tags: Vec::new(),
                pinned: false,
                links: Vec::new(),
                created: now,
                updated: now,
            },
            body: String::new(),
        },
    };
    if let Some(kind) = input.kind {
        node.meta.kind = kind;
    }
    if let Some(title) = input.title {
        node.meta.title = title.trim().to_owned();
    }
    if let Some(description) = input.description {
        node.meta.description = description.trim().to_owned();
    }
    if let Some(body) = input.body {
        node.body = if body.is_empty() || body.ends_with('\n') {
            body
        } else {
            format!("{body}\n")
        };
    }
    if let Some(tags) = input.tags {
        let mut clean: Vec<String> = Vec::new();
        for tag in tags {
            let tag = tag.trim().to_lowercase();
            if !tag.is_empty() && !clean.contains(&tag) {
                clean.push(tag);
            }
        }
        node.meta.tags = clean;
    }
    if let Some(pinned) = input.pinned {
        node.meta.pinned = pinned;
    }
    if let Some(links) = input.links {
        node.meta.links = links;
    }
    node.meta.updated = now;
    let warnings = graph::validate(&node).map_err(OpError::Invalid)?;
    store.save_memory_node(&node)?;
    record(
        store,
        &Change {
            at: now,
            by: by.clone(),
            target: format!("memory:{}", node.id),
            action: if created { "created" } else { "updated" }.into(),
            fields: BTreeMap::new(),
        },
    );
    Ok((node, created, warnings))
}

fn edit_node(
    store: &Store,
    id: &str,
    by: &Actor,
    clock: &Clock,
    action: &str,
    f: impl FnOnce(&mut MemoryNode) -> Result<(), OpError>,
) -> Result<MemoryNode, OpError> {
    let mut node = match store.memory_node(id) {
        Ok(node) => node,
        Err(StoreError::Io { .. }) => return Err(OpError::NodeNotFound(id.to_owned())),
        Err(StoreError::InvalidNodeId(_)) => return Err(OpError::NodeNotFound(id.to_owned())),
        Err(e) => return Err(e.into()),
    };
    f(&mut node)?;
    node.meta.updated = clock.timestamp();
    store.save_memory_node(&node)?;
    record(
        store,
        &Change {
            at: clock.timestamp(),
            by: by.clone(),
            target: format!("memory:{id}"),
            action: action.into(),
            fields: BTreeMap::new(),
        },
    );
    Ok(node)
}

/// Adds a typed link from node `from`; a node target must exist.
pub fn link(
    store: &Store,
    from: &str,
    to: LinkTarget,
    kind: LinkType,
    note: Option<String>,
    by: &Actor,
    clock: &Clock,
) -> Result<MemoryNode, OpError> {
    if let LinkTarget::Node(id) = &to {
        if id == from {
            return Err(OpError::Invalid("a node cannot link to itself".into()));
        }
        if store.memory_node(id).is_err() {
            return Err(OpError::NodeNotFound(id.clone()));
        }
    }
    edit_node(store, from, by, clock, "linked", |node| {
        node.meta.links.retain(|l| !(l.to == to && l.kind == kind));
        node.meta.links.push(Link { to, kind, note });
        Ok(())
    })
}

/// Removes links from `from` to `to` (of type `kind` only, when set).
pub fn unlink(
    store: &Store,
    from: &str,
    to: &LinkTarget,
    kind: Option<LinkType>,
    by: &Actor,
    clock: &Clock,
) -> Result<MemoryNode, OpError> {
    edit_node(store, from, by, clock, "unlinked", |node| {
        let before = node.meta.links.len();
        node.meta
            .links
            .retain(|l| !(&l.to == to && kind.is_none_or(|k| k == l.kind)));
        if node.meta.links.len() == before {
            return Err(OpError::Invalid(format!("{from} has no such link to {to}")));
        }
        Ok(())
    })
}

pub fn pin(store: &Store, id: &str, pinned: bool, by: &Actor, clock: &Clock) -> Result<MemoryNode, OpError> {
    edit_node(
        store,
        id,
        by,
        clock,
        if pinned { "pinned" } else { "unpinned" },
        |node| {
            node.meta.pinned = pinned;
            Ok(())
        },
    )
}

/// Deletes a node. With incoming links, refuses unless `force`, listing
/// the nodes that link to it.
pub fn delete_node(store: &Store, id: &str, force: bool, by: &Actor, clock: &Clock) -> Result<(), OpError> {
    let (graph, _) = store.graph()?;
    if graph.get(id).is_none() {
        return Err(OpError::NodeNotFound(id.to_owned()));
    }
    let incoming: HashSet<&str> = graph.incoming(id).iter().map(|(from, _)| from.as_str()).collect();
    if !incoming.is_empty() && !force {
        let mut list: Vec<&str> = incoming.into_iter().collect();
        list.sort();
        return Err(OpError::Invalid(format!(
            "{id} is linked from {}; confirm to delete it",
            list.join(", ")
        )));
    }
    store.delete_memory_node(id)?;
    record(
        store,
        &Change {
            at: clock.timestamp(),
            by: by.clone(),
            target: format!("memory:{id}"),
            action: "deleted".into(),
            fields: BTreeMap::new(),
        },
    );
    Ok(())
}

/// Records that nodes were loaded, for the relevance ranking.
pub fn touch_nodes(store: &Store, ids: &[String], clock: &Clock) {
    if ids.is_empty() {
        return;
    }
    let _ = store.update_runtime(graph::ACCESS_FILE, |log: &mut graph::AccessLog| {
        graph::record_access(log, ids, clock.timestamp());
    });
}

/// The memory graph and the access log of a project.
pub fn load_graph(store: &Store) -> Result<(Graph, graph::AccessLog), StoreError> {
    let (graph, _) = store.graph()?;
    Ok((graph, store.read_runtime(graph::ACCESS_FILE)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::patch::Edit;
    use crate::task::{Priority, TaskStatus};

    #[test]
    fn changes_are_attributed() {
        let root = tempfile::tempdir().unwrap();
        let store = Store::init(root.path(), "foo").unwrap();
        let clock = Clock::system();
        let claude = Actor::Claude {
            session: Some("abc".into()),
        };
        let task = create_task(
            &store,
            NewTask {
                title: "Écrire le README".into(),
                priority: Some(Priority::P1),
                ..NewTask::default()
            },
            &claude,
            &clock,
        )
        .unwrap();
        let patch = TaskPatch {
            priority: Some(Edit::set(Priority::P0)),
            ..TaskPatch::default()
        };
        update_task(&store, task.id, &patch, false, &Actor::User, &clock).unwrap();
        let done = TaskPatch {
            status: Some(Edit::set(TaskStatus::Done)),
            ..TaskPatch::default()
        };
        update_task(&store, task.id, &done, false, &claude, &clock).unwrap();

        let history = history(&store, "T-1");
        assert_eq!(history.len(), 3);
        assert_eq!(
            (history[0].by.to_string(), history[0].action.as_str()),
            ("claude:abc".into(), "created")
        );
        assert_eq!(history[1].by, Actor::User);
        assert_eq!(history[1].fields["pri"], "P0");
        assert!(history[2].fields.contains_key("da"));
    }

    #[test]
    fn memory_put_link_and_guarded_delete() {
        let root = tempfile::tempdir().unwrap();
        let store = Store::init(root.path(), "foo").unwrap();
        let clock = Clock::system();
        let claude = Actor::Claude { session: None };
        let put = |id: &str, desc: &str| {
            put_node(
                &store,
                NodeInput {
                    id: id.into(),
                    kind: Some(NodeKind::Decision),
                    title: Some(format!("Title {id}")),
                    description: Some(desc.into()),
                    body: Some("Body".into()),
                    ..NodeInput::default()
                },
                &claude,
                &clock,
            )
        };
        assert!(
            put("auth-jwt", &"x".repeat(200))
                .unwrap_err()
                .to_string()
                .contains("120")
        );
        let (_, created, _) = put("auth-jwt", "Tokens").unwrap();
        assert!(created);
        put("api-refresh-token", "Refresh").unwrap();
        link(
            &store,
            "api-refresh-token",
            LinkTarget::Node("auth-jwt".into()),
            LinkType::DependsOn,
            None,
            &claude,
            &clock,
        )
        .unwrap();
        assert!(
            link(
                &store,
                "api-refresh-token",
                LinkTarget::Node("ghost".into()),
                LinkType::DependsOn,
                None,
                &claude,
                &clock
            )
            .is_err()
        );

        let err = delete_node(&store, "auth-jwt", false, &Actor::User, &clock).unwrap_err();
        assert!(err.to_string().contains("api-refresh-token"), "{err}");
        delete_node(&store, "auth-jwt", true, &Actor::User, &clock).unwrap();
        assert_eq!(store.memory_node("api-refresh-token").unwrap().body, "Body\n");
    }
}
