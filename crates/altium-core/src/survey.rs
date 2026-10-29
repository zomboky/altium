//! Digest of past Claude Code conversations in a project, for the project
//! survey: one dated section per conversation with the user's messages and
//! Claude's last reply, truncated and masked, plus Claude's auto memory.

use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::activity::{mask, truncate};
use crate::time::Timestamp;

/// Folder of `runtime/` holding the digest.
pub const DIR: &str = "survey";
pub const DIGEST_FILE: &str = "sessions.md";
/// The survey instructions, read by Claude at the start of the session.
pub const PROMPT_FILE: &str = "prompt.md";
pub const STATE_FILE: &str = "survey/state.json";

const MAX_USER_CHARS: usize = 600;
const MAX_REPLY_CHARS: usize = 1500;
const MAX_MESSAGES: usize = 30;
const MAX_MEMORY_CHARS: usize = 20_000;

/// When the last survey ran, kept in `runtime/survey/state.json`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SurveyState {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last: Option<Timestamp>,
}

/// The folder name Claude Code gives a project: every character that is
/// not an ASCII letter or digit becomes `-`.
pub fn encode_path(path: &Path) -> String {
    path.to_string_lossy()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect()
}

/// Claude Code's folder for the project at `root`, if it exists. Matched
/// without case on Windows, where the drive letter case varies.
pub fn conversations_dir(claude_config: &Path, root: &Path) -> Option<PathBuf> {
    let projects = claude_config.join("projects");
    let wanted = encode_path(root);
    let exact = projects.join(&wanted);
    if exact.is_dir() {
        return Some(exact);
    }
    if cfg!(windows) {
        return fs::read_dir(&projects)
            .ok()?
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .find(|p| {
                p.is_dir()
                    && p.file_name()
                        .and_then(|n| n.to_str())
                        .is_some_and(|n| n.eq_ignore_ascii_case(&wanted))
            });
    }
    None
}

/// One conversation, reduced.
#[derive(Debug, Clone, PartialEq)]
pub struct Conversation {
    pub id: String,
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
    pub user: Vec<String>,
    pub last_reply: Option<String>,
}

/// Reads a conversation file, skipping what is not a main-thread user or
/// assistant message. `None` when it holds no user message.
pub fn read_conversation(path: &Path) -> Option<Conversation> {
    let text = fs::read_to_string(path).ok()?;
    let mut start: Option<DateTime<Utc>> = None;
    let mut end: Option<DateTime<Utc>> = None;
    let mut user = Vec::new();
    let mut last_reply = None;
    for line in text.lines() {
        let Ok(entry) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        let kind = entry.get("type").and_then(Value::as_str).unwrap_or_default();
        if !matches!(kind, "user" | "assistant")
            || entry.get("isSidechain").and_then(Value::as_bool) == Some(true)
            || entry.get("isMeta").and_then(Value::as_bool) == Some(true)
        {
            continue;
        }
        let at = entry
            .get("timestamp")
            .and_then(Value::as_str)
            .and_then(|t| DateTime::parse_from_rfc3339(t).ok())
            .map(|t| t.with_timezone(&Utc));
        let content = entry.get("message").and_then(|m| m.get("content"));
        let texts: Vec<&str> = match content {
            Some(Value::String(s)) => vec![s.as_str()],
            Some(Value::Array(items)) => items
                .iter()
                .filter(|i| i.get("type").and_then(Value::as_str) == Some("text"))
                .filter_map(|i| i.get("text").and_then(Value::as_str))
                .collect(),
            _ => Vec::new(),
        };
        let joined = texts.join("\n");
        let joined = joined.trim();
        // Tool results, command echoes and system reminders are not messages.
        if joined.is_empty() || joined.starts_with('<') {
            continue;
        }
        if let Some(at) = at {
            start = Some(start.map_or(at, |s| s.min(at)));
            end = Some(end.map_or(at, |e| e.max(at)));
        }
        if kind == "user" {
            user.push(truncate(&mask(joined), MAX_USER_CHARS));
        } else {
            last_reply = Some(truncate(&mask(joined), MAX_REPLY_CHARS));
        }
    }
    if user.is_empty() {
        return None;
    }
    Some(Conversation {
        id: path.file_stem()?.to_string_lossy().into_owned(),
        start: start?,
        end: end?,
        user,
        last_reply,
    })
}

/// Builds the digest of conversations that ended after `since`, oldest
/// first. Returns it with the number of conversations it covers.
pub fn digest(claude_config: &Path, root: &Path, since: Option<Timestamp>) -> (String, usize) {
    let mut text = format!(
        "# Past Claude Code conversations in {}\n\nGenerated by `altium survey`. Times are UTC. Messages are truncated and secrets masked.\n",
        root.display()
    );
    let Some(dir) = conversations_dir(claude_config, root) else {
        text.push_str("\nNo past conversations found for this folder.\n");
        return (text, 0);
    };
    let mut conversations: Vec<Conversation> = fs::read_dir(&dir)
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "jsonl"))
        .filter_map(|p| read_conversation(&p))
        .filter(|c| since.is_none_or(|s| c.end > s.as_datetime()))
        .collect();
    conversations.sort_by_key(|c| c.start);
    if let Some(since) = since {
        let _ = writeln!(text, "Only conversations after the previous survey ({since}).");
    }
    if conversations.is_empty() {
        text.push_str("\nNo new conversations.\n");
    }
    for c in &conversations {
        let _ = write!(
            text,
            "\n## {} to {} (conversation {})\n\nUser messages:\n",
            c.start.format("%Y-%m-%d %H:%M"),
            c.end.format("%Y-%m-%d %H:%M"),
            &c.id[..c.id.len().min(8)]
        );
        let skipped = c.user.len().saturating_sub(MAX_MESSAGES);
        if skipped > 0 {
            let _ = writeln!(text, "- ({skipped} earlier messages omitted)");
        }
        for message in c.user.iter().skip(skipped) {
            let _ = writeln!(text, "- {}", message.replace('\n', "\n  "));
        }
        if let Some(reply) = &c.last_reply {
            let _ = writeln!(text, "\nClaude's last reply:\n\n> {}", reply.replace('\n', "\n> "));
        }
    }

    let memory = dir.join("memory");
    let mut files: Vec<PathBuf> = fs::read_dir(&memory)
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "md"))
        .collect();
    files.sort();
    if !files.is_empty() {
        text.push_str("\n# Claude auto memory for this folder\n");
        let mut budget = MAX_MEMORY_CHARS;
        for file in files {
            let Ok(content) = fs::read_to_string(&file) else {
                continue;
            };
            let content = truncate(&mask(&content), budget.max(200));
            budget = budget.saturating_sub(content.chars().count());
            let modified = fs::metadata(&file)
                .and_then(|m| m.modified())
                .map(|t| DateTime::<Utc>::from(t).format("%Y-%m-%d").to_string())
                .unwrap_or_default();
            let name = file.file_name().unwrap_or_default().to_string_lossy();
            let _ = write!(text, "\n## {name} (modified {modified})\n\n{}\n", content.trim_end());
        }
    }
    (text, conversations.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/conversations/session.jsonl"
    );

    #[test]
    fn encodes_paths_like_claude_code() {
        assert_eq!(
            encode_path(Path::new(r"H:\claude-projects\altium")),
            "H--claude-projects-altium"
        );
        assert_eq!(encode_path(Path::new("/home/me/my_app.v2")), "-home-me-my-app-v2");
    }

    #[test]
    fn reads_a_real_conversation() {
        let c = read_conversation(Path::new(FIXTURE)).unwrap();
        assert_eq!(c.user.len(), 2);
        assert!(c.user[0].starts_with("Do exactly these steps"));
        assert!(c.user[1].contains("Bearer ***"), "{}", c.user[1]);
        assert_eq!(c.last_reply.as_deref(), Some("fin"));
        assert_eq!(c.start.to_rfc3339(), "2026-10-29T09:28:09.413+00:00");
    }

    #[test]
    fn digest_has_dated_sections_and_skips_old_ones() {
        let config = tempfile::tempdir().unwrap();
        let root = Path::new("/home/me/meudon");
        let dir = config.path().join("projects").join(encode_path(root));
        fs::create_dir_all(dir.join("memory")).unwrap();
        fs::copy(FIXTURE, dir.join("abcdef12-0000.jsonl")).unwrap();
        fs::write(dir.join("memory").join("MEMORY.md"), "- user prefers French\n").unwrap();

        let (text, count) = digest(config.path(), root, None);
        assert_eq!(count, 1);
        assert!(
            text.contains("## 2026-10-29 09:28 to 2026-10-29 09:28 (conversation abcdef12)"),
            "{text}"
        );
        assert!(text.contains("> fin"));
        assert!(text.contains("## MEMORY.md (modified"));

        let after: Timestamp = "2026-10-30T00:00:00Z".parse().unwrap();
        let (text, count) = digest(config.path(), root, Some(after));
        assert_eq!(count, 0);
        assert!(text.contains("No new conversations."));
    }
}
