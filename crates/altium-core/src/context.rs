//! The compact context injected at session start: project, tasks, memory
//! index and usage instructions, within a token budget.

use std::fmt::Write as _;

use crate::graph::{AccessLog, Graph, estimate_tokens, language_instruction};
use crate::project::{Project, ProjectStatus};
use crate::render::task_line;
use crate::task::Task;
use crate::tasks::{DeadlineState, deadline_state, order};
use crate::time::Clock;

/// Open tasks shown at most.
pub const TOP_TASKS: usize = 5;

/// What the context is built from.
pub struct Input<'a> {
    pub project: &'a Project,
    pub status: &'a ProjectStatus,
    pub tasks: &'a [Task],
    pub graph: &'a Graph,
    pub access: &'a AccessLog,
    pub clock: &'a Clock,
}

/// Usage instructions for the Altium MCP tools, shared with the MCP server.
pub fn instructions(memory_lang: &str) -> String {
    format!(
        "Altium keeps this project's tasks, status and memory in .altium/; use the mcp__altium__ tools, not the files.\n\
         - Tasks: set status doing when you start, done when finished, blocked with a reason; create tasks for new work; never delete, cancel instead.\n\
         - Status: after meaningful progress, update the summary (where things stand, next step, blockers) with status_set.\n\
         - Memory: load nodes with memory_get (follow depends_on for prerequisites); save durable knowledge as short nodes linked to others with memory_put and memory_link.\n\
         - {}",
        language_instruction(memory_lang)
    )
}

/// Builds the context within `budget` estimated tokens: the memory index is
/// reduced first, then the task lists, each with a `+N …` line.
pub fn build(input: &Input, budget: usize) -> String {
    let near = input.project.config.near_hours();
    let header = header(input);
    let usage = format!(
        "## Altium usage\n{}\n",
        instructions(input.project.config.memory_lang())
    );

    let ordered = order(input.tasks, input.clock);
    let line = |t: &Task| task_line(t, input.tasks, input.clock, near);
    let next: Vec<String> = ordered.active.iter().map(|t| line(t)).collect();
    let blocked: Vec<String> = ordered.blocked.iter().map(|t| line(t)).collect();
    // Deadlines past or near that are not already among the next tasks.
    let shown: Vec<_> = ordered.active.iter().take(TOP_TASKS).map(|t| t.id).collect();
    let deadlines: Vec<String> = ordered
        .active
        .iter()
        .chain(&ordered.blocked)
        .filter(|t| !shown.contains(&t.id) && !ordered.blocked.iter().any(|b| b.id == t.id))
        .filter(|t| {
            matches!(
                deadline_state(t, input.clock, near),
                DeadlineState::Overdue | DeadlineState::Near
            )
        })
        .map(|t| line(t))
        .collect();
    let open = ordered.active.len() + ordered.blocked.len();

    let index: Vec<String> = input
        .graph
        .ranked(input.access, input.clock.now)
        .iter()
        .map(|n| input.graph.index_line(n))
        .collect();

    let mut limits = Limits {
        next: TOP_TASKS,
        blocked: TOP_TASKS,
        deadlines: TOP_TASKS,
        memory: index.len(),
    };
    let render = |limits: &Limits| {
        let mut text = header.clone();
        if open > 0 {
            let _ = writeln!(text, "\n## Tasks ({open} open, {} blocked)", ordered.blocked.len());
            section(&mut text, "Next:", &next, limits.next, "tasks, see task_list");
            section(
                &mut text,
                "Blocked:",
                &blocked,
                limits.blocked,
                "blocked tasks, see task_list",
            );
            section(
                &mut text,
                "Deadlines:",
                &deadlines,
                limits.deadlines,
                "deadlines, see task_list",
            );
        }
        if !index.is_empty() {
            let _ = writeln!(text, "\n## Memory index ({} nodes)", index.len());
            section(&mut text, "", &index, limits.memory, "nodes, see memory_search");
        }
        text.push('\n');
        text.push_str(&usage);
        text
    };

    let mut text = render(&limits);
    // Memory first, down to pinned nodes only, then tasks.
    let pinned = input.graph.active().filter(|n| n.meta.pinned).count();
    while estimate_tokens(&text) > budget {
        if limits.memory > pinned {
            limits.memory = (limits.memory * 3 / 4).max(pinned);
        } else if limits.deadlines > 1 {
            limits.deadlines -= 1;
        } else if limits.blocked > 1 {
            limits.blocked -= 1;
        } else if limits.next > 1 {
            limits.next -= 1;
        } else if limits.memory > 0 {
            limits.memory = 0;
        } else {
            break;
        }
        text = render(&limits);
    }
    text
}

struct Limits {
    next: usize,
    blocked: usize,
    deadlines: usize,
    memory: usize,
}

fn header(input: &Input) -> String {
    let mut text = format!(
        "# Altium project: {} (phase: {})\n",
        input.project.name, input.status.phase
    );
    if input.status.summary.is_empty() {
        text.push_str("Summary: (none yet; write one with status_set)\n");
    } else {
        let _ = writeln!(text, "Summary: {}", input.status.summary);
        if let (Some(at), Some(by)) = (input.status.updated_at, &input.status.updated_by) {
            let _ = writeln!(text, "(updated {at} by {by})");
        }
    }
    text
}

fn section(text: &mut String, title: &str, lines: &[String], limit: usize, more: &str) {
    if lines.is_empty() {
        return;
    }
    if !title.is_empty() {
        let _ = writeln!(text, "{title}");
    }
    for line in lines.iter().take(limit) {
        let _ = writeln!(text, "{line}");
    }
    if lines.len() > limit {
        let _ = writeln!(text, "+{} {more}", lines.len() - limit);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::{MemoryNode, NodeKind, NodeMeta};
    use crate::project::ProjectConfig;
    use crate::tasks::{NewTask, create};
    use crate::time::Timestamp;

    fn clock() -> Clock {
        Clock::fixed("2026-10-28T12:00:00Z".parse::<Timestamp>().unwrap().as_datetime())
    }

    fn project(lang: Option<&str>) -> Project {
        let mut p = Project::new("foo");
        p.config = ProjectConfig::default();
        p.config.memory.lang = lang.map(Into::into);
        p
    }

    fn nodes(count: usize) -> Vec<MemoryNode> {
        (0..count)
            .map(|i| MemoryNode {
                id: format!("node-{i:02}"),
                meta: NodeMeta {
                    kind: NodeKind::Fact,
                    title: format!("Fait numéro {i}"),
                    description: "A one line description of what this node holds for the project".into(),
                    tags: vec![],
                    pinned: i == 79,
                    links: vec![],
                    created: clock().timestamp(),
                    updated: clock().timestamp(),
                },
                body: "Detailed body that must never be injected.".into(),
            })
            .collect()
    }

    fn tasks(count: usize) -> Vec<Task> {
        let mut tasks = Vec::new();
        for i in 0..count {
            let task = NewTask {
                title: format!("Tâche en français {i}"),
                ..NewTask::default()
            };
            create(&mut tasks, task, &clock()).unwrap();
        }
        tasks
    }

    #[test]
    fn small_project() {
        let tasks = tasks(3);
        let graph = Graph::new(nodes(2));
        let status = ProjectStatus {
            summary: "API en place, reste l'auth.".into(),
            ..ProjectStatus::default()
        };
        let p = project(None);
        let input = Input {
            project: &p,
            status: &status,
            tasks: &tasks,
            graph: &graph,
            access: &AccessLog::new(),
            clock: &clock(),
        };
        let text = build(&input, 2000);
        assert!(text.starts_with("# Altium project: foo (phase: idea)\nSummary: API en place, reste l'auth.\n"));
        assert!(text.contains("## Tasks (3 open, 0 blocked)\nNext:\nT-1 P2 todo \"Tâche en français 0\""));
        assert!(text.contains("## Memory index (2 nodes)\nnode-00 [fact] Fait numéro 0:"));
        assert!(text.contains("in English"));
        assert!(!text.contains("Detailed body"));
    }

    #[test]
    fn big_project_fits_the_budget() {
        let tasks = tasks(30);
        let graph = Graph::new(nodes(80));
        let p = project(Some("fr"));
        let status = ProjectStatus::default();
        let input = Input {
            project: &p,
            status: &status,
            tasks: &tasks,
            graph: &graph,
            access: &AccessLog::new(),
            clock: &clock(),
        };
        let text = build(&input, 2000);
        assert!(estimate_tokens(&text) <= 2000, "{}", estimate_tokens(&text));
        assert!(text.contains("+25 tasks, see task_list"));
        assert!(text.contains("nodes, see memory_search"));
        assert!(text.contains("node-79 [fact*]"), "pinned node kept");
        assert!(text.contains("in French"));
        assert!(!text.contains("Detailed body"));
    }
}
