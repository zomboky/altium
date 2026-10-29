//! `altium hook <event>`: called by Claude Code for every hook event. Logs
//! the event in the project's `.altium/runtime/`, and at session start prints
//! the project context. Always exits successfully and silently on error:
//! errors go to the Altium log, never to Claude.

use std::env;
use std::io::{self, Read, Write};
use std::path::PathBuf;

use altium_core::activity::{self, ROTATE_BYTES, SESSIONS_FILE, Sessions};
use altium_core::context::{self, Input};
use altium_core::graph::AccessLog;
use altium_core::store::{Store, find_root};
use altium_core::time::{Clock, Timestamp};
use altium_core::{log, process};
use anyhow::{Context, Result};
use serde_json::{Value, json};

/// Largest payload read; tool responses can be big and are not needed.
const MAX_INPUT: u64 = 16 * 1024 * 1024;

pub fn run(event: &str) {
    let mut input = Vec::new();
    let _ = io::stdin().take(MAX_INPUT).read_to_end(&mut input);
    match handle(event, &input) {
        Ok(Some(output)) => {
            let mut stdout = io::stdout().lock();
            let _ = stdout.write_all(output.as_bytes());
            let _ = stdout.flush();
        }
        Ok(None) => {}
        Err(error) => log::write(&format!("hook {event}"), &format!("{error:#}")),
    }
}

fn handle(event: &str, input: &[u8]) -> Result<Option<String>> {
    let payload: Value = serde_json::from_slice(input).context("invalid hook payload")?;
    let cwd = match payload.get("cwd").and_then(Value::as_str) {
        Some(cwd) => PathBuf::from(cwd),
        None => env::current_dir()?,
    };
    let Some(root) = find_root(&cwd) else {
        return Ok(None);
    };
    let store = Store::open(&root)?;
    let now = Timestamp::now();
    if let Some(event) = activity::event_from_hook(event, &payload, store.root(), now) {
        activity::append(&store, &event)?;
    }
    let session = payload.get("session_id").and_then(Value::as_str).unwrap_or("unknown");
    match event {
        "SessionStart" => {
            let model = payload.get("model").and_then(|m| m.as_str().or(m.get("id")?.as_str()));
            let pid = process::claude_ancestor();
            store.update_runtime(SESSIONS_FILE, |sessions: &mut Sessions| {
                activity::session_started(sessions, session, now, pid, model.map(str::to_owned));
            })?;
            activity::rotate_if_needed(&store, ROTATE_BYTES, now.as_datetime())?;
            Ok(Some(session_context(&store)?))
        }
        "SessionEnd" => {
            store.update_runtime(SESSIONS_FILE, |sessions: &mut Sessions| {
                activity::session_ended(sessions, session, now);
            })?;
            activity::rotate_if_needed(&store, ROTATE_BYTES, now.as_datetime())?;
            Ok(None)
        }
        "Stop" => {
            activity::rotate_if_needed(&store, ROTATE_BYTES, now.as_datetime())?;
            Ok(None)
        }
        _ => Ok(None),
    }
}

/// The `SessionStart` output carrying the project context.
fn session_context(store: &Store) -> Result<String> {
    let project = store.project()?;
    let status = store.status()?;
    let tasks = store.tasks()?;
    let (graph, errors) = store.graph()?;
    for error in errors {
        log::write("hook SessionStart", &error.to_string());
    }
    let access: AccessLog = store.read_runtime(altium_core::graph::ACCESS_FILE);
    let clock = Clock::system();
    let input = Input {
        project: &project,
        status: &status,
        tasks: &tasks,
        graph: &graph,
        access: &access,
        clock: &clock,
    };
    let text = context::build(&input, project.config.context_budget());
    let output = json!({
        "hookSpecificOutput": {
            "hookEventName": "SessionStart",
            "additionalContext": text,
        }
    });
    Ok(output.to_string())
}
