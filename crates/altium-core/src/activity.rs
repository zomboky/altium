//! Claude's activity in a project, captured by the hooks: the append-only
//! event log `runtime/events.jsonl`, known sessions, statistics that survive
//! log rotation, and the "tool running" state.

use std::collections::{BTreeMap, HashMap};
use std::fs::{self, OpenOptions};
use std::io::Write as _;
use std::path::Path;
use std::sync::LazyLock;

use chrono::{DateTime, Duration, NaiveDate, Utc};
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::{Result, StoreError};
use crate::store::Store;
use crate::time::Timestamp;

pub const EVENTS_FILE: &str = "events.jsonl";
pub const SESSIONS_FILE: &str = "sessions.json";
pub const STATS_FILE: &str = "stats.json";

/// Longest target kept in an event, in characters.
pub const MAX_TARGET_CHARS: usize = 120;

/// Size of `events.jsonl` above which it is rotated.
pub const ROTATE_BYTES: u64 = 4 * 1024 * 1024;

/// Rotated logs kept besides the active one.
pub const KEEP_ROTATED: usize = 2;

/// A session without an end nor any event for this long is inactive.
pub const IDLE_MINUTES: i64 = 30;

/// Lines stay below the size the system writes atomically in append mode.
const MAX_LINE_BYTES: usize = 4000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EventKind {
    Start,
    Prompt,
    Pre,
    Post,
    Fail,
    Stop,
    End,
}

/// One line of `events.jsonl`. Keys are short: the log grows fast.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Event {
    #[serde(rename = "t")]
    pub at: Timestamp,
    #[serde(rename = "s")]
    pub session: String,
    #[serde(rename = "e")]
    pub kind: EventKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool: Option<String>,
    /// Short, truncated and masked: a path, a command or a pattern.
    #[serde(rename = "tgt", default, skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    /// `tool_use_id`, pairing a `pre` with its `post` or `fail`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// Start source (`startup`, `resume`…) or end reason.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub why: Option<String>,
}

/// Builds the event for a hook payload; `None` for events that are not logged.
pub fn event_from_hook(event_name: &str, payload: &Value, root: &Path, at: Timestamp) -> Option<Event> {
    let kind = match event_name {
        "SessionStart" => EventKind::Start,
        "UserPromptSubmit" => EventKind::Prompt,
        "PreToolUse" => EventKind::Pre,
        "PostToolUse" => EventKind::Post,
        "PostToolUseFailure" => EventKind::Fail,
        "Stop" => EventKind::Stop,
        "SessionEnd" => EventKind::End,
        _ => return None,
    };
    let text = |key: &str| payload.get(key).and_then(Value::as_str).map(str::to_owned);
    let tool = text("tool_name");
    let mut event = Event {
        at,
        session: text("session_id").unwrap_or_else(|| "unknown".into()),
        kind,
        target: tool
            .as_deref()
            .and_then(|tool| target(tool, payload.get("tool_input").unwrap_or(&Value::Null), root)),
        tool,
        id: text("tool_use_id"),
        ms: payload.get("duration_ms").and_then(Value::as_u64),
        model: match payload.get("model") {
            Some(Value::String(model)) => Some(model.clone()),
            Some(model) => model.get("id").and_then(Value::as_str).map(str::to_owned),
            None => None,
        },
        why: text("source").or_else(|| text("reason")),
    };
    // A PostToolUse can still describe a failure, e.g. an interrupted command.
    if kind == EventKind::Post
        && let Some(response) = payload.get("tool_response")
        && (response.get("is_error").and_then(Value::as_bool) == Some(true)
            || response.get("interrupted").and_then(Value::as_bool) == Some(true))
    {
        event.kind = EventKind::Fail;
    }
    Some(event)
}

/// The short target of a tool call: a project-relative path, a truncated
/// command, a search pattern, a URL…, always masked.
pub fn target(tool: &str, input: &Value, root: &Path) -> Option<String> {
    let text = |key: &str| input.get(key).and_then(Value::as_str);
    let raw = match tool {
        "Bash" | "PowerShell" => text("command").map(|c| c.split_whitespace().collect::<Vec<_>>().join(" ")),
        "Grep" | "Glob" => text("pattern").map(str::to_owned),
        "WebFetch" => text("url").map(str::to_owned),
        "WebSearch" => text("query").map(str::to_owned),
        "Task" | "Agent" => text("description").or(text("subagent_type")).map(str::to_owned),
        _ if tool.starts_with("mcp__") => Some(tool.to_owned()),
        _ => text("file_path")
            .or(text("notebook_path"))
            .or(text("path"))
            .map(|path| relative(path, root)),
    }?;
    Some(truncate(&mask(&raw), MAX_TARGET_CHARS))
}

fn relative(path: &str, root: &Path) -> String {
    let root = root.to_string_lossy().replace('\\', "/");
    let path = path.replace('\\', "/");
    let lower = path.to_lowercase();
    let prefix = format!("{}/", root.trim_end_matches('/'));
    let same = if cfg!(windows) {
        lower.starts_with(&prefix.to_lowercase())
    } else {
        path.starts_with(&prefix)
    };
    if same { path[prefix.len()..].to_owned() } else { path }
}

pub fn truncate(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_owned();
    }
    let mut cut: String = text.chars().take(max - 1).collect();
    cut.push('…');
    cut
}

static SECRETS: LazyLock<Vec<(Regex, &'static str)>> = LazyLock::new(|| {
    [
        (r"(?i)(\bbearer\s+)[A-Za-z0-9._~+/=-]+", "${1}***"),
        (r"(?i)(\bbasic\s+)[A-Za-z0-9+/=]{8,}", "${1}***"),
        (r"\b(sk|pk|rk)-[A-Za-z0-9_-]{6,}", "***"),
        (r"\b(ghp|gho|ghu|ghs|ghr)_[A-Za-z0-9]{10,}", "***"),
        (r"\bgithub_pat_[A-Za-z0-9_]{10,}", "***"),
        (r"\bxox[abpr]-[A-Za-z0-9-]{10,}", "***"),
        (r"\bAKIA[0-9A-Z]{16}\b", "***"),
        (r"(://[^/\s:@]+:)[^@\s/]+@", "${1}***@"),
        (
            r"(?i)(--?(?:password|passwd|pwd|pass|token|secret|api[-_]?key|auth)(?:=|\s+))\S+",
            "${1}***",
        ),
        (
            r"(?i)\b([A-Z0-9_]*(?:KEY|TOKEN|SECRET|PASSWORD|PASSWD|PWD|CREDENTIALS?)[A-Z0-9_]*\s*=\s*)[^\s;&|]+",
            "${1}***",
        ),
    ]
    .into_iter()
    .map(|(pattern, with)| (Regex::new(pattern).expect("valid secret pattern"), with))
    .collect()
});

/// Replaces values that look like secrets with `***`. Heuristic: it does not
/// replace a real protection, but keeps tokens out of the activity log.
pub fn mask(text: &str) -> String {
    let mut masked = text.to_owned();
    for (pattern, with) in SECRETS.iter() {
        masked = pattern.replace_all(&masked, *with).into_owned();
    }
    masked
}

/// Appends `event` to the log of `store` in a single write, without taking
/// the project lock.
pub fn append(store: &Store, event: &Event) -> Result<()> {
    let mut event = event.clone();
    let mut line = serde_json::to_string(&event).expect("events always serialize");
    if line.len() >= MAX_LINE_BYTES {
        event.target = event.target.map(|t| truncate(&t, 40));
        event.model = None;
        line = serde_json::to_string(&event).expect("events always serialize");
    }
    line.push('\n');
    let path = store.runtime_path(EVENTS_FILE);
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(StoreError::io(&path))?;
    file.write_all(line.as_bytes()).map_err(StoreError::io(&path))
}

/// Reads a log, skipping invalid lines.
pub fn read_events(path: &Path) -> Vec<Event> {
    fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .filter_map(|line| serde_json::from_str(line).ok())
        .collect()
}

/// The active log of `store`.
pub fn events(store: &Store) -> Vec<Event> {
    read_events(&store.runtime_path(EVENTS_FILE))
}

/// A session as recorded by the `SessionStart` and `SessionEnd` hooks.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionRecord {
    pub start: Timestamp,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end: Option<Timestamp>,
    /// PID of the `claude` process running the session, to attribute MCP
    /// changes to it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pid: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
}

pub type Sessions = BTreeMap<String, SessionRecord>;

/// Sessions kept in `sessions.json`.
const KEEP_SESSIONS: usize = 200;

/// Records the start (or resumption) of a session.
pub fn session_started(sessions: &mut Sessions, id: &str, at: Timestamp, pid: Option<u32>, model: Option<String>) {
    let record = sessions.entry(id.to_owned()).or_insert(SessionRecord {
        start: at,
        end: None,
        pid,
        model: None,
    });
    record.end = None;
    record.pid = pid.or(record.pid);
    record.model = model.or(record.model.take());
    // Another session that ran in the same process is over.
    if let Some(pid) = pid {
        for (other, record) in sessions.iter_mut() {
            if other != id && record.pid == Some(pid) && record.end.is_none() {
                record.end = Some(at);
            }
        }
    }
    while sessions.len() > KEEP_SESSIONS {
        let oldest = sessions
            .iter()
            .min_by_key(|(_, r)| r.end.unwrap_or(r.start))
            .map(|(id, _)| id.clone())
            .expect("not empty");
        sessions.remove(&oldest);
    }
}

pub fn session_ended(sessions: &mut Sessions, id: &str, at: Timestamp) {
    if let Some(record) = sessions.get_mut(id) {
        record.end = Some(at);
    }
}

/// The session id recorded for a `claude` process, if any.
pub fn session_of_pid(sessions: &Sessions, pid: u32) -> Option<String> {
    sessions
        .iter()
        .filter(|(_, r)| r.pid == Some(pid))
        .max_by_key(|(_, r)| r.start)
        .map(|(id, _)| id.clone())
}

/// A tool call that has started but not finished.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Running {
    pub tool: String,
    pub target: Option<String>,
    pub since: Timestamp,
}

/// A session as shown in the dashboard.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SessionView {
    pub id: String,
    pub start: Timestamp,
    pub end: Option<Timestamp>,
    pub last_event: Timestamp,
    pub model: Option<String>,
    pub active: bool,
    pub running: Option<Running>,
    pub tool_calls: usize,
}

/// Sessions seen in the records and the log, most recent first.
pub fn sessions(records: &Sessions, events: &[Event], now: DateTime<Utc>, idle_minutes: i64) -> Vec<SessionView> {
    let mut views: HashMap<String, SessionView> = HashMap::new();
    for (id, record) in records {
        views.insert(
            id.clone(),
            SessionView {
                id: id.clone(),
                start: record.start,
                end: record.end,
                last_event: record.end.unwrap_or(record.start),
                model: record.model.clone(),
                active: false,
                running: None,
                tool_calls: 0,
            },
        );
    }
    let mut open_calls: HashMap<String, HashMap<String, Running>> = HashMap::new();
    for event in events {
        let view = views.entry(event.session.clone()).or_insert_with(|| SessionView {
            id: event.session.clone(),
            start: event.at,
            end: None,
            last_event: event.at,
            model: None,
            active: false,
            running: None,
            tool_calls: 0,
        });
        view.last_event = view.last_event.max(event.at);
        view.start = view.start.min(event.at);
        if view.model.is_none() {
            view.model = event.model.clone();
        }
        let calls = open_calls.entry(event.session.clone()).or_default();
        match event.kind {
            EventKind::Pre => {
                view.tool_calls += 1;
                if let Some(id) = &event.id {
                    calls.insert(
                        id.clone(),
                        Running {
                            tool: event.tool.clone().unwrap_or_default(),
                            target: event.target.clone(),
                            since: event.at,
                        },
                    );
                }
            }
            EventKind::Post | EventKind::Fail => {
                if let Some(id) = &event.id {
                    calls.remove(id);
                }
            }
            EventKind::Stop | EventKind::Prompt => calls.clear(),
            EventKind::End => {
                calls.clear();
                view.end = Some(view.end.map_or(event.at, |end| end.max(event.at)));
            }
            EventKind::Start => {
                if view.end.is_some_and(|end| end < event.at) {
                    view.end = None;
                }
            }
        }
    }
    let idle = Duration::minutes(idle_minutes);
    let mut list: Vec<SessionView> = views
        .into_values()
        .map(|mut view| {
            view.active = view.end.is_none() && now - view.last_event.as_datetime() < idle;
            if view.active {
                view.running = open_calls
                    .remove(&view.id)
                    .and_then(|calls| calls.into_values().max_by_key(|r| r.since));
            }
            view
        })
        .collect();
    list.sort_by(|a, b| b.last_event.cmp(&a.last_event).then(a.id.cmp(&b.id)));
    list
}

/// A finished tool call, Pre and Post paired.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ToolCall {
    pub at: Timestamp,
    pub session: String,
    pub tool: String,
    pub target: Option<String>,
    /// `None` while running.
    pub ok: Option<bool>,
    pub ms: Option<u64>,
}

/// Tool calls of the log, most recent first.
pub fn tool_calls(events: &[Event]) -> Vec<ToolCall> {
    let mut calls: Vec<ToolCall> = Vec::new();
    let mut by_id: HashMap<&str, usize> = HashMap::new();
    for event in events {
        match event.kind {
            EventKind::Pre => {
                if let Some(id) = &event.id {
                    by_id.insert(id, calls.len());
                }
                calls.push(ToolCall {
                    at: event.at,
                    session: event.session.clone(),
                    tool: event.tool.clone().unwrap_or_default(),
                    target: event.target.clone(),
                    ok: None,
                    ms: None,
                });
            }
            EventKind::Post | EventKind::Fail => {
                let ok = event.kind == EventKind::Post;
                let ms = event.ms.or_else(|| {
                    let index = event.id.as_deref().and_then(|id| by_id.get(id))?;
                    let started = calls[*index].at.as_datetime();
                    u64::try_from((event.at.as_datetime() - started).num_milliseconds()).ok()
                });
                match event.id.as_deref().and_then(|id| by_id.get(id)) {
                    Some(&index) => {
                        calls[index].ok = Some(ok);
                        calls[index].ms = ms;
                    }
                    None => calls.push(ToolCall {
                        at: event.at,
                        session: event.session.clone(),
                        tool: event.tool.clone().unwrap_or_default(),
                        target: event.target.clone(),
                        ok: Some(ok),
                        ms,
                    }),
                }
            }
            _ => {}
        }
    }
    calls.reverse();
    calls
}

/// Per-day usage counters, kept in `stats.json` for rotated logs.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct DayStats {
    #[serde(default)]
    pub tools: BTreeMap<String, ToolCount>,
    /// Files changed by Edit, Write and the like.
    #[serde(default)]
    pub files: BTreeMap<String, u32>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct ToolCount {
    pub calls: u32,
    pub fails: u32,
}

/// `stats.json`: counters of rotated logs, by day.
pub type Archive = BTreeMap<NaiveDate, DayStats>;

const EDIT_TOOLS: [&str; 4] = ["Edit", "Write", "MultiEdit", "NotebookEdit"];

/// Adds the finished calls of `events` to `archive`.
pub fn aggregate(archive: &mut Archive, events: &[Event]) {
    for call in tool_calls(events) {
        let Some(ok) = call.ok else { continue };
        let day = archive.entry(call.at.as_datetime().date_naive()).or_default();
        let count = day.tools.entry(call.tool.clone()).or_default();
        count.calls += 1;
        if !ok {
            count.fails += 1;
        }
        if ok
            && EDIT_TOOLS.contains(&call.tool.as_str())
            && let Some(target) = call.target
        {
            *day.files.entry(target).or_default() += 1;
        }
    }
}

/// Usage over a period, from the archive and the active log.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Stats {
    /// Most used first.
    pub tools: Vec<(String, ToolCount)>,
    pub calls: u32,
    pub fails: u32,
    /// The 10 most changed files.
    pub files: Vec<(String, u32)>,
}

/// Usage from `from` (included) to `to` (included).
pub fn stats(archive: &Archive, events: &[Event], from: NaiveDate, to: NaiveDate) -> Stats {
    let mut all = archive.clone();
    aggregate(&mut all, events);
    let mut tools: BTreeMap<String, ToolCount> = BTreeMap::new();
    let mut files: BTreeMap<String, u32> = BTreeMap::new();
    for (_, day) in all.range(from..=to) {
        for (tool, count) in &day.tools {
            let total = tools.entry(tool.clone()).or_default();
            total.calls += count.calls;
            total.fails += count.fails;
        }
        for (file, count) in &day.files {
            *files.entry(file.clone()).or_default() += count;
        }
    }
    let mut stats = Stats {
        calls: tools.values().map(|c| c.calls).sum(),
        fails: tools.values().map(|c| c.fails).sum(),
        tools: tools.into_iter().collect(),
        files: files.into_iter().collect(),
    };
    stats
        .tools
        .sort_by(|a, b| b.1.calls.cmp(&a.1.calls).then(a.0.cmp(&b.0)));
    stats.files.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    stats.files.truncate(10);
    stats
}

/// Rotates the log of `store` when it exceeds `max_bytes`: its counters go
/// to `stats.json`, it is renamed `events-<date>.jsonl`, and only the
/// [`KEEP_ROTATED`] latest rotated logs are kept. Returns whether it rotated.
pub fn rotate_if_needed(store: &Store, max_bytes: u64, now: DateTime<Utc>) -> Result<bool> {
    let path = store.runtime_path(EVENTS_FILE);
    if fs::metadata(&path).map_or(true, |m| m.len() <= max_bytes) {
        return Ok(false);
    }
    store.update_runtime(STATS_FILE, |archive: &mut Archive| -> Result<bool> {
        // Checked again under the lock: another hook may have rotated it.
        if fs::metadata(&path).map_or(true, |m| m.len() <= max_bytes) {
            return Ok(false);
        }
        let rotated = store.runtime_path(&format!("events-{}.jsonl", now.format("%Y%m%d-%H%M%S")));
        fs::rename(&path, &rotated).map_err(StoreError::io(&path))?;
        // Lines appended while renaming stay in the rotated file and are counted.
        aggregate(archive, &read_events(&rotated));

        let mut old: Vec<_> = fs::read_dir(store.runtime_path(""))
            .map_err(StoreError::io(&store.runtime_path("")))?
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| {
                p.file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.starts_with("events-") && n.ends_with(".jsonl"))
            })
            .collect();
        old.sort();
        while old.len() > KEEP_ROTATED {
            let _ = fs::remove_file(old.remove(0));
        }
        Ok(true)
    })?
}

/// Events of the log not yet counted in `stats.json`, plus those of the
/// rotated logs; used for the activity feed.
pub fn recent_events(store: &Store) -> Vec<Event> {
    let mut rotated: Vec<_> = fs::read_dir(store.runtime_path(""))
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("events-") && n.ends_with(".jsonl"))
        })
        .collect();
    rotated.sort();
    let mut events: Vec<Event> = rotated.last().map(|p| read_events(p)).unwrap_or_default();
    events.extend(self::events(store));
    events
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn at(minute: u32) -> Timestamp {
        format!("2026-10-28T10:{minute:02}:00Z").parse().unwrap()
    }

    #[test]
    fn masks_secrets() {
        let cases = [
            (
                r#"curl -H "Authorization: Bearer sk-abc123def456" https://api.x"#,
                r#"curl -H "Authorization: Bearer ***" https://api.x"#,
            ),
            ("export OPENAI=sk-proj-abcdef123456", "export OPENAI=***"),
            (
                "git push https://user:hunter2@github.com/x",
                "git push https://user:***@github.com/x",
            ),
            (
                "gh auth login --with-token ghp_abcdefghij1234567890",
                "gh auth login --with-token ***",
            ),
            ("API_KEY=abc123 npm start", "API_KEY=*** npm start"),
            ("mysql --password=s3cret -u root", "mysql --password=*** -u root"),
            ("psql --password s3cret", "psql --password ***"),
        ];
        for (raw, masked) in cases {
            assert_eq!(mask(raw), masked, "{raw}");
        }
        assert_eq!(mask("cargo build --release"), "cargo build --release");
    }

    #[test]
    fn targets_are_short_and_relative() {
        let root = Path::new("/home/me/foo");
        let edit = json!({"file_path": "/home/me/foo/src/main.rs"});
        assert_eq!(target("Edit", &edit, root).unwrap(), "src/main.rs");
        let long = json!({"command": format!("echo {}", "x".repeat(300))});
        assert_eq!(target("Bash", &long, root).unwrap().chars().count(), MAX_TARGET_CHARS);
        assert_eq!(target("Grep", &json!({"pattern": "TODO"}), root).unwrap(), "TODO");
        assert_eq!(
            target("mcp__altium__task_list", &json!({}), root).unwrap(),
            "mcp__altium__task_list"
        );
    }

    fn event(minute: u32, session: &str, kind: EventKind, tool: Option<&str>, id: Option<&str>) -> Event {
        Event {
            at: at(minute),
            session: session.into(),
            kind,
            tool: tool.map(Into::into),
            target: Some("x".into()),
            id: id.map(Into::into),
            ms: None,
            model: None,
            why: None,
        }
    }

    #[test]
    fn sessions_running_tools_and_idle() {
        let events = vec![
            event(0, "a", EventKind::Start, None, None),
            event(1, "a", EventKind::Pre, Some("Bash"), Some("t1")),
            event(2, "a", EventKind::Post, Some("Bash"), Some("t1")),
            event(3, "a", EventKind::Pre, Some("Bash"), Some("t2")),
            event(0, "b", EventKind::Start, None, None),
            event(4, "b", EventKind::End, None, None),
        ];
        let now = at(5).as_datetime();
        let views = sessions(&Sessions::new(), &events, now, IDLE_MINUTES);
        let a = views.iter().find(|v| v.id == "a").unwrap();
        assert!(a.active);
        assert_eq!(a.running.as_ref().unwrap().since, at(3));
        let b = views.iter().find(|v| v.id == "b").unwrap();
        assert!(!b.active && b.end == Some(at(4)));

        let later = now + Duration::minutes(31);
        assert!(!sessions(&Sessions::new(), &events, later, IDLE_MINUTES)[0].active);

        let calls = tool_calls(&events);
        assert_eq!(calls.len(), 2);
        assert_eq!((calls[1].ok, calls[1].ms), (Some(true), Some(60_000)));
        assert_eq!(calls[0].ok, None);
    }

    #[test]
    fn stats_survive_rotation() {
        let root = tempfile::tempdir().unwrap();
        let store = Store::init(root.path(), "foo").unwrap();
        let mut written = 0;
        for n in 0..60 {
            let id = format!("t{n}");
            let tool = if n % 3 == 0 { "Edit" } else { "Bash" };
            append(&store, &event(n % 60, "a", EventKind::Pre, Some(tool), Some(&id))).unwrap();
            let kind = if n % 10 == 0 { EventKind::Fail } else { EventKind::Post };
            append(&store, &event(n % 60, "a", kind, Some(tool), Some(&id))).unwrap();
            written += 1;
            if n == 29 {
                assert!(rotate_if_needed(&store, 100, at(30).as_datetime()).unwrap());
            }
        }
        assert_eq!(written, 60);
        let archive: Archive = store.read_runtime(STATS_FILE);
        let day = at(0).as_datetime().date_naive();
        let s = stats(&archive, &events(&store), day, day);
        assert_eq!(s.calls, 60);
        assert_eq!(s.fails, 6);
        assert_eq!(s.tools[0], ("Bash".to_owned(), ToolCount { calls: 40, fails: 4 }));
        assert_eq!(s.files, [("x".to_owned(), 18)]);
    }
}
