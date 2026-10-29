//! Watches the registry and every project's `.altium/`, and pushes debounced
//! changes to the dashboard. Hooks and MCP never talk to the server: the
//! files are the only channel.

use std::collections::hash_map::Entry;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use altium_core::activity;
use altium_core::store::DIR_NAME;
use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher};
use tokio::sync::mpsc;

use crate::{AppState, Change};

const DEBOUNCE: Duration = Duration::from_millis(150);

/// Git info of displayed projects is refreshed at least this often.
const GIT_REFRESH: Duration = Duration::from_secs(30);

/// Tools whose end can change the git repository.
const GIT_TOOLS: [&str; 6] = ["Bash", "PowerShell", "Edit", "Write", "MultiEdit", "NotebookEdit"];

/// The scope a changed file of `.altium/` belongs to, if any.
pub fn scope(relative: &Path) -> Option<&'static str> {
    let parts: Vec<String> = relative
        .components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect();
    match parts.iter().map(String::as_str).collect::<Vec<_>>().as_slice() {
        ["tasks.jsonl"] => Some("tasks"),
        ["status.json"] => Some("status"),
        ["project.toml"] => Some("project"),
        ["memory", ..] => Some("memory"),
        ["runtime", name] if name.starts_with("events") || *name == activity::SESSIONS_FILE => Some("activity"),
        ["runtime", "memory-access.json"] => Some("memory"),
        _ => None,
    }
}

pub fn spawn(state: Arc<AppState>) {
    let (sender, mut receiver) = mpsc::unbounded_channel::<notify::Result<Event>>();
    let watcher = notify::recommended_watcher(move |event| {
        let _ = sender.send(event);
    });
    let mut watcher = match watcher {
        Ok(watcher) => watcher,
        Err(e) => {
            altium_core::log::write("serve", &format!("file watcher unavailable: {e}"));
            return;
        }
    };

    tokio::spawn(async move {
        let mut projects = HashMap::new();
        let _ = std::fs::create_dir_all(state.registry.dir());
        if let Err(e) = watcher.watch(state.registry.dir(), RecursiveMode::NonRecursive) {
            altium_core::log::write("serve", &format!("cannot watch the registry: {e}"));
        }
        sync_projects(&state, &mut watcher, &mut projects);
        let mut git_timer = tokio::time::interval(GIT_REFRESH);
        let mut pending: HashSet<Change> = HashSet::new();

        loop {
            let event = if pending.is_empty() {
                tokio::select! {
                    event = receiver.recv() => event,
                    _ = git_timer.tick() => {
                        refresh_git(&state, &projects);
                        continue;
                    }
                }
            } else {
                match tokio::time::timeout(DEBOUNCE, receiver.recv()).await {
                    Ok(event) => event,
                    Err(_) => {
                        // Quiet for the debounce delay: flush.
                        let registry_changed = pending.iter().any(|c| c.scope == "projects");
                        if registry_changed {
                            sync_projects(&state, &mut watcher, &mut projects);
                        }
                        for change in pending.drain() {
                            let _ = state.changes.send(change);
                        }
                        continue;
                    }
                }
            };
            let Some(event) = event else { break };
            let Ok(event) = event else { continue };
            for path in &event.paths {
                for change in classify(&state, &projects, path) {
                    pending.insert(change);
                }
            }
        }
    });
}

/// Changes caused by a modified `path`.
fn classify(state: &AppState, projects: &HashMap<PathBuf, String>, path: &Path) -> Vec<Change> {
    if path.parent() == Some(state.registry.dir()) {
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or_default();
        if name == "projects.toml" || name == altium_core::config::FILE_NAME {
            return vec![Change {
                project: None,
                scope: "projects",
            }];
        }
        return Vec::new();
    }
    for (dir, id) in projects {
        if let Ok(relative) = path.strip_prefix(dir)
            && let Some(scope) = scope(relative)
        {
            let mut changes = vec![
                Change {
                    project: Some(id.clone()),
                    scope,
                },
                // Cards of the overview show the same data.
                Change {
                    project: None,
                    scope: "projects",
                },
            ];
            if scope == "activity" && last_tool_may_touch_git(dir) {
                changes.push(Change {
                    project: Some(id.clone()),
                    scope: "git",
                });
            }
            return changes;
        }
    }
    Vec::new()
}

/// Whether the last logged event ends a tool call that may change the repository.
fn last_tool_may_touch_git(altium_dir: &Path) -> bool {
    let path = altium_dir.join("runtime").join(activity::EVENTS_FILE);
    let Ok(text) = std::fs::read_to_string(path) else {
        return false;
    };
    text.lines()
        .last()
        .and_then(|line| serde_json::from_str::<activity::Event>(line).ok())
        .is_some_and(|e| {
            matches!(e.kind, activity::EventKind::Post | activity::EventKind::Fail)
                && e.tool.as_deref().is_some_and(|t| GIT_TOOLS.contains(&t))
        })
}

/// Watches the `.altium/` of every registered project, adding and removing
/// watches as the registry changes.
fn sync_projects(state: &AppState, watcher: &mut RecommendedWatcher, projects: &mut HashMap<PathBuf, String>) {
    let wanted: HashMap<PathBuf, String> = state
        .registry
        .entries()
        .unwrap_or_default()
        .into_iter()
        .map(|e| (e.path.join(DIR_NAME), e.id))
        .filter(|(dir, _)| dir.is_dir())
        .collect();
    for dir in projects.keys().filter(|d| !wanted.contains_key(*d)) {
        let _ = watcher.unwatch(dir);
    }
    projects.retain(|dir, _| wanted.contains_key(dir));
    for (dir, id) in wanted {
        if let Entry::Vacant(entry) = projects.entry(dir) {
            if let Err(e) = watcher.watch(entry.key(), RecursiveMode::Recursive) {
                altium_core::log::write("serve", &format!("cannot watch {}: {e}", entry.key().display()));
                continue;
            }
            entry.insert(id);
        }
    }
}

/// Asks clients to refetch git info, when someone is listening.
fn refresh_git(state: &AppState, projects: &HashMap<PathBuf, String>) {
    if state.changes.receiver_count() == 0 {
        return;
    }
    for id in projects.values() {
        let _ = state.changes.send(Change {
            project: Some(id.clone()),
            scope: "git",
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scopes_of_files() {
        assert_eq!(scope(Path::new("tasks.jsonl")), Some("tasks"));
        assert_eq!(scope(Path::new("memory/auth-jwt.md")), Some("memory"));
        assert_eq!(scope(Path::new("runtime/events.jsonl")), Some("activity"));
        assert_eq!(scope(Path::new("runtime/tmp/x")), None);
        assert_eq!(scope(Path::new("runtime/lock")), None);
    }
}
