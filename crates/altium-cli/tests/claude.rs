//! The Claude Code integration: `altium hook` replaying real hook payloads,
//! `altium setup` and `altium doctor` against a throwaway Claude config
//! (`CLAUDE_CONFIG_DIR`, since `HOME` moves nothing on Windows).

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use altium_core::Store;
use altium_core::activity::{self, EventKind};
use altium_core::registry::canonical_path;
use serde_json::{Value, json};

const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/hooks/windows-2.1.310");

struct Env {
    tmp: tempfile::TempDir,
}

impl Env {
    fn new() -> Env {
        Env {
            tmp: tempfile::tempdir().unwrap(),
        }
    }

    fn path(&self, relative: &str) -> PathBuf {
        let path = canonical_path(self.tmp.path()).unwrap().join(relative);
        fs::create_dir_all(&path).unwrap();
        path
    }

    fn claude_dir(&self) -> PathBuf {
        self.path("claude-config")
    }

    fn altium(&self, cwd: &Path, args: &[&str]) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_altium"));
        command
            .args(args)
            .current_dir(cwd)
            .env("ALTIUM_CONFIG_DIR", self.path("config"))
            .env("CLAUDE_CONFIG_DIR", self.claude_dir())
            // No real `claude`: setup writes .claude.json itself.
            .env("ALTIUM_CLAUDE", self.tmp.path().join("no-claude"))
            .env("HOME", self.path("home"));
        command
    }

    fn hook(&self, event: &str, payload: &str, cwd: &Path) -> Output {
        let mut child = self
            .altium(cwd, &["hook", event])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child.stdin.take().unwrap().write_all(payload.as_bytes()).unwrap();
        child.wait_with_output().unwrap()
    }

    fn project(&self, name: &str) -> PathBuf {
        let root = self.path(name);
        Store::init(&root, name).unwrap();
        root
    }

    fn settings(&self) -> Value {
        serde_json::from_str(&fs::read_to_string(self.claude_dir().join("settings.json")).unwrap()).unwrap()
    }
}

/// The captured payloads, in order, pointed at `root`.
fn fixtures(root: &Path) -> Vec<(String, String)> {
    let escaped = serde_json::to_string(&root.display().to_string()).unwrap();
    let escaped = &escaped[1..escaped.len() - 1];
    let mut files: Vec<PathBuf> = fs::read_dir(FIXTURES).unwrap().map(|e| e.unwrap().path()).collect();
    files.sort();
    files
        .iter()
        .map(|file| {
            let name = file.file_stem().unwrap().to_str().unwrap();
            let event = name.split_once('-').unwrap().1.to_owned();
            let text = fs::read_to_string(file).unwrap().replace("{{PROJECT}}", escaped);
            (event, text)
        })
        .collect()
}

#[test]
fn every_registered_event_has_a_fixture() {
    let events: Vec<String> = fixtures(Path::new("/x")).into_iter().map(|(e, _)| e).collect();
    for event in [
        "SessionStart",
        "UserPromptSubmit",
        "PreToolUse",
        "PostToolUse",
        "PostToolUseFailure",
        "Stop",
        "SessionEnd",
    ] {
        assert!(events.iter().any(|e| e == event), "no fixture for {event}");
    }
}

#[test]
fn replaying_a_session_logs_its_activity() {
    let env = Env::new();
    let root = env.project("demo");
    let mut context = None;
    for (event, payload) in fixtures(&root) {
        let output = env.hook(&event, &payload, &root);
        assert!(output.status.success());
        assert!(output.stderr.is_empty(), "{}", String::from_utf8_lossy(&output.stderr));
        if event == "SessionStart" {
            context = Some(String::from_utf8(output.stdout).unwrap());
        } else {
            assert!(output.stdout.is_empty(), "{event} printed something");
        }
    }

    let context: Value = serde_json::from_str(&context.unwrap()).unwrap();
    assert_eq!(context["hookSpecificOutput"]["hookEventName"], "SessionStart");
    let text = context["hookSpecificOutput"]["additionalContext"].as_str().unwrap();
    assert!(text.starts_with("# Altium project: demo (phase: idea)"), "{text}");
    assert!(text.contains("mcp__altium__"));

    let store = Store::open(&root).unwrap();
    let events = activity::events(&store);
    assert_eq!(events.len(), 14);
    let calls = activity::tool_calls(&events);
    let summary: Vec<(String, Option<String>, Option<bool>)> = calls
        .iter()
        .rev()
        .map(|c| (c.tool.clone(), c.target.clone(), c.ok))
        .collect();
    assert_eq!(
        summary,
        [
            ("Read".into(), Some("notes.txt".into()), Some(true)),
            ("Bash".into(), Some("echo ok".into()), Some(true)),
            ("Bash".into(), Some("exit 3".into()), Some(false)),
            ("Write".into(), Some("out.txt".into()), Some(true)),
            ("Grep".into(), Some("hello".into()), Some(true)),
        ]
    );
    assert_eq!(calls.last().unwrap().ms, Some(41));
    assert!(
        events
            .iter()
            .any(|e| e.kind == EventKind::End && e.why.as_deref() == Some("other"))
    );

    let sessions: activity::Sessions = store.read_runtime(activity::SESSIONS_FILE);
    let session = &sessions["11111111-2222-3333-4444-555555555555"];
    assert!(session.end.is_some());
}

#[test]
fn hook_outside_a_project_writes_nothing() {
    let env = Env::new();
    let dir = env.path("downloads");
    for (event, payload) in fixtures(&dir) {
        let output = env.hook(&event, &payload, &dir);
        assert!(output.status.success());
        assert!(output.stdout.is_empty() && output.stderr.is_empty());
    }
    assert_eq!(fs::read_dir(&dir).unwrap().count(), 0);
}

#[test]
fn hook_on_a_corrupt_project_is_silent_and_logs() {
    let env = Env::new();
    let root = env.project("broken");
    fs::write(root.join(".altium/status.json"), "{ not json").unwrap();
    let (_, payload) = fixtures(&root).into_iter().next().unwrap();
    let output = env.hook("SessionStart", &payload, &root);
    assert!(output.status.success());
    assert!(output.stdout.is_empty() && output.stderr.is_empty());
    let log = fs::read_to_string(env.path("config").join("altium.log")).unwrap();
    assert!(
        log.contains("hook SessionStart") && log.contains("status.json"),
        "{log}"
    );

    let output = env.hook("SessionStart", "garbage", &root);
    assert!(output.status.success() && output.stdout.is_empty());
}

#[test]
fn parallel_hooks_do_not_mix_lines() {
    let env = Env::new();
    let root = env.project("busy");
    let payload = fixtures(&root).into_iter().find(|(e, _)| e == "PreToolUse").unwrap().1;
    std::thread::scope(|scope| {
        for _ in 0..4 {
            scope.spawn(|| {
                for _ in 0..25 {
                    assert!(env.hook("PreToolUse", &payload, &root).status.success());
                }
            });
        }
    });
    let text = fs::read_to_string(root.join(".altium/runtime/events.jsonl")).unwrap();
    assert_eq!(text.lines().count(), 100);
    assert!(text.lines().all(|l| serde_json::from_str::<Value>(l).is_ok()));
}

#[test]
fn setup_is_idempotent_and_keeps_user_hooks() {
    let env = Env::new();
    let home = env.path("home");
    let user_hook = json!({"matcher": "Edit", "hooks": [{"type": "command", "command": "prettier --write"}]});
    let original = json!({"model": "opus", "hooks": {"PostToolUse": [user_hook.clone()]}});
    fs::write(env.claude_dir().join("settings.json"), original.to_string()).unwrap();

    for _ in 0..2 {
        let output = env.altium(&home, &["setup"]).output().unwrap();
        assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    }
    let settings = env.settings();
    for event in ["SessionStart", "PreToolUse", "PostToolUse", "SessionEnd"] {
        let altium: Vec<&Value> = settings["hooks"][event]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|g| g["hooks"].as_array().unwrap())
            .filter(|h| h["args"][0] == "hook")
            .collect();
        assert_eq!(altium.len(), 1, "{event}");
        assert!(Path::new(altium[0]["command"].as_str().unwrap()).is_absolute());
    }
    assert_eq!(settings["hooks"]["PostToolUse"][0], user_hook);
    let state: Value =
        serde_json::from_str(&fs::read_to_string(env.claude_dir().join(".claude.json")).unwrap()).unwrap();
    assert_eq!(state["mcpServers"]["altium"]["args"], json!(["mcp"]));
    let backups = fs::read_dir(env.claude_dir())
        .unwrap()
        .filter(|e| e.as_ref().unwrap().file_name().to_string_lossy().contains("altium-bak"))
        .count();
    assert!(backups >= 2);

    let output = env.altium(&home, &["setup", "--uninstall"]).output().unwrap();
    assert!(output.status.success());
    assert_eq!(env.settings(), original);
    let state: Value =
        serde_json::from_str(&fs::read_to_string(env.claude_dir().join(".claude.json")).unwrap()).unwrap();
    assert!(state["mcpServers"].get("altium").is_none());
}

#[test]
fn doctor_reports_a_missing_hook() {
    let env = Env::new();
    let root = env.project("doc");
    assert!(env.altium(&root, &["setup"]).output().unwrap().status.success());
    let mut settings = env.settings();
    settings["hooks"].as_object_mut().unwrap().remove("PostToolUse");
    fs::write(env.claude_dir().join("settings.json"), settings.to_string()).unwrap();

    let output = env.altium(&root, &["doctor"]).output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(!output.status.success());
    assert!(stdout.contains("hooks manquants : PostToolUse"), "{stdout}");
    assert!(stdout.contains("altium setup"));
}

#[test]
fn init_offers_no_survey_outside_a_terminal() {
    let env = Env::new();
    let root = env.path("meudon");
    fs::write(root.join("README.md"), "# Meudon\n").unwrap();
    let output = env.altium(&root, &["init"]).output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(!stdout.contains("état des lieux"), "{stdout}");
}

#[test]
fn survey_launches_nothing_without_claude_or_setup() {
    let env = Env::new();
    let root = env.project("meudon");
    let output = env.altium(&root, &["survey"]).output().unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("commande claude introuvable"));

    // A `claude` that exists, but Altium is not set up and nobody can answer.
    let output = env
        .altium(&root, &["survey"])
        .env("ALTIUM_CLAUDE", env!("CARGO_BIN_EXE_altium"))
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("état des lieux annulé"));
    assert!(!root.join(".altium/runtime/survey/sessions.md").exists());
}
