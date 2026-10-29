//! Compact text renderings for Claude (session context and MCP replies):
//! fewer tokens than JSON, always in English.

use std::fmt::Write as _;

use crate::graph::{Graph, Loaded, kind_name, link_name};
use crate::memory::{LinkTarget, MemoryNode};
use crate::task::{Priority, Task, TaskStatus};
use crate::tasks::{DeadlineState, deadline_state};
use crate::time::Clock;

pub fn status_name(status: TaskStatus) -> &'static str {
    match status {
        TaskStatus::Todo => "todo",
        TaskStatus::Doing => "doing",
        TaskStatus::Blocked => "blocked",
        TaskStatus::Done => "done",
        TaskStatus::Cancelled => "cancelled",
    }
}

pub fn priority_name(priority: Priority) -> &'static str {
    match priority {
        Priority::P0 => "P0",
        Priority::P1 => "P1",
        Priority::P2 => "P2",
        Priority::P3 => "P3",
    }
}

/// One task on one line:
/// `T-12 P1 todo due:2026-11-02(near) "Titre" #api ^T-3 [3/4]`.
pub fn task_line(task: &Task, tasks: &[Task], clock: &Clock, near_hours: u32) -> String {
    let mut line = format!(
        "{} {} {}",
        task.id,
        priority_name(task.priority),
        status_name(task.status)
    );
    if let Some(due) = task.due {
        let _ = write!(line, " due:{due}");
        match deadline_state(task, clock, near_hours) {
            DeadlineState::Overdue => line.push_str("(OVERDUE)"),
            DeadlineState::Near => line.push_str("(near)"),
            _ => {}
        }
    }
    let _ = write!(line, " \"{}\"", task.title);
    for tag in &task.tags {
        let _ = write!(line, " #{tag}");
    }
    if let Some(parent) = task.parent {
        let _ = write!(line, " ^{parent}");
    }
    if let Some((done, total)) = crate::tasks::progress(tasks, task.id) {
        let _ = write!(line, " [{done}/{total}]");
    }
    if let Some(reason) = &task.blocked_reason {
        let _ = write!(line, " why:\"{reason}\"");
    }
    line
}

/// Full task: its line, then description and note.
pub fn task_detail(task: &Task, tasks: &[Task], clock: &Clock, near_hours: u32) -> String {
    let mut text = task_line(task, tasks, clock, near_hours);
    if let Some(description) = &task.description {
        let _ = write!(text, "\ndesc: {description}");
    }
    if let Some(note) = &task.note {
        let _ = write!(text, "\nnote: {note}");
    }
    let children: Vec<String> = tasks
        .iter()
        .filter(|t| t.parent == Some(task.id))
        .map(|t| format!("{} {}", t.id, status_name(t.status)))
        .collect();
    if !children.is_empty() {
        let _ = write!(text, "\nsubtasks: {}", children.join(", "));
    }
    let _ = write!(
        text,
        "\ncreated {} updated {} rev {}",
        task.created_at, task.updated_at, task.rev
    );
    if let Some(done) = task.done_at {
        let _ = write!(text, " closed {done}");
    }
    text
}

/// A loaded node: reduced front matter, then its body.
pub fn node(node: &MemoryNode, graph: &Graph, broken: &[String]) -> String {
    let meta = &node.meta;
    let mut text = format!("## {} [{}] {}\n", node.id, kind_name(meta.kind), meta.title);
    let _ = writeln!(text, "desc: {}", meta.description);
    if !meta.tags.is_empty() {
        let _ = writeln!(text, "tags: {}", meta.tags.join(", "));
    }
    if meta.pinned {
        text.push_str("pinned: true\n");
    }
    let _ = writeln!(text, "updated: {}", meta.updated);
    if !meta.links.is_empty() {
        let links: Vec<String> = meta
            .links
            .iter()
            .map(|link| {
                let mut item = format!("{} {}", link_name(link.kind), link.to);
                if broken.contains(&link.to.to_string()) {
                    item.push_str(" (BROKEN)");
                }
                if let Some(note) = &link.note {
                    let _ = write!(item, " ({note})");
                }
                item
            })
            .collect();
        let _ = writeln!(text, "links: {}", links.join("; "));
    }
    let incoming: Vec<String> = graph
        .incoming(&node.id)
        .iter()
        .map(|(from, kind)| format!("{from} {}", link_name(*kind)))
        .collect();
    if !incoming.is_empty() {
        let _ = writeln!(text, "linked from: {}", incoming.join("; "));
    }
    text.push('\n');
    text.push_str(node.body.trim_end());
    text.push('\n');
    text
}

/// A [`Loaded`] set: warnings, node bodies, then neighbors as index lines.
pub fn loaded(loaded: &Loaded, graph: &Graph, broken: impl Fn(&MemoryNode) -> Vec<String>) -> String {
    let mut text = String::new();
    for note in &loaded.notes {
        let _ = writeln!(text, "{note}");
    }
    for missing in &loaded.missing {
        let _ = writeln!(text, "not found: {missing} (see memory_index or memory_search)");
    }
    if !text.is_empty() {
        text.push('\n');
    }
    for n in &loaded.nodes {
        text.push_str(&node(n, graph, &broken(n)));
        text.push('\n');
    }
    if !loaded.neighbors.is_empty() {
        text.push_str("Neighbors (not loaded):\n");
        for neighbor in &loaded.neighbors {
            let arrow = if neighbor.incoming { "<-" } else { "->" };
            let target = match &neighbor.target {
                LinkTarget::Node(id) => match graph.get(id) {
                    Some(n) => graph.index_line(n),
                    None => format!("{id} (BROKEN: no such node)"),
                },
                other => other.to_string(),
            };
            let _ = writeln!(text, "{} {arrow}{} {target}", neighbor.from, link_name(neighbor.kind));
        }
    }
    if loaded.nodes.len() > 1 || loaded.depth > 0 {
        let _ = write!(text, "depth reached: {}", loaded.depth);
        if loaded.beyond_depth > 0 {
            let _ = write!(text, " (+{} linked nodes beyond depth)", loaded.beyond_depth);
        }
        text.push('\n');
    }
    text
}
