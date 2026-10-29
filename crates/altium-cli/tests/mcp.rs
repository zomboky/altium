//! `altium mcp` driven over stdio like Claude Code does: one JSON-RPC
//! message per line.

use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use altium_core::Store;
use altium_core::ops;
use altium_core::registry::canonical_path;
use serde_json::{Value, json};

struct Client {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    next: u64,
}

impl Client {
    fn start(cwd: &Path, config: &Path) -> Client {
        let mut child = Command::new(env!("CARGO_BIN_EXE_altium"))
            .arg("mcp")
            .current_dir(cwd)
            .env("ALTIUM_CONFIG_DIR", config)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        let stdin = child.stdin.take().unwrap();
        let stdout = BufReader::new(child.stdout.take().unwrap());
        let mut client = Client {
            child,
            stdin,
            stdout,
            next: 1,
        };
        let init = client.request("initialize", json!({"protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": "test", "version": "0"}}));
        assert_eq!(init["result"]["serverInfo"]["name"], "altium");
        writeln!(
            client.stdin,
            "{}",
            json!({"jsonrpc": "2.0", "method": "notifications/initialized"})
        )
        .unwrap();
        client
    }

    fn request(&mut self, method: &str, params: Value) -> Value {
        let id = self.next;
        self.next += 1;
        writeln!(
            self.stdin,
            "{}",
            json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params})
        )
        .unwrap();
        let mut line = String::new();
        self.stdout.read_line(&mut line).unwrap();
        let reply: Value = serde_json::from_str(&line).unwrap();
        assert_eq!(reply["id"], id);
        reply
    }

    /// Calls a tool; returns its text and whether it is an error.
    fn call(&mut self, name: &str, arguments: Value) -> (String, bool) {
        let reply = self.request("tools/call", json!({"name": name, "arguments": arguments}));
        let result = &reply["result"];
        (
            result["content"][0]["text"].as_str().unwrap().to_owned(),
            result["isError"].as_bool().unwrap_or(false),
        )
    }

    fn ok(&mut self, name: &str, arguments: Value) -> String {
        let (text, error) = self.call(name, arguments);
        assert!(!error, "{name}: {text}");
        text
    }
}

impl Drop for Client {
    fn drop(&mut self) {
        let _ = self.child.kill();
    }
}

fn project() -> (tempfile::TempDir, std::path::PathBuf) {
    let tmp = tempfile::tempdir().unwrap();
    let root = canonical_path(tmp.path()).unwrap().join("foo");
    fs::create_dir_all(&root).unwrap();
    Store::init(&root, "foo").unwrap();
    (tmp, root)
}

#[test]
fn lists_tools_in_english_with_instructions() {
    let (tmp, root) = project();
    let mut client = Client::start(&root, tmp.path());
    let tools = client.request("tools/list", json!({}));
    let names: Vec<&str> = tools["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap())
        .collect();
    for name in [
        "status_get",
        "status_set",
        "task_list",
        "task_create",
        "task_update",
        "memory_get",
        "memory_put",
    ] {
        assert!(names.contains(&name), "{name}");
    }
    assert!(!names.iter().any(|n| n.contains("delete")));
    let unknown = client.request("nope", json!({}));
    assert_eq!(unknown["error"]["code"], -32601);
}

#[test]
fn outside_a_project_tools_fail_in_english() {
    let tmp = tempfile::tempdir().unwrap();
    let downloads = tmp.path().join("Downloads");
    fs::create_dir_all(&downloads).unwrap();
    let mut client = Client::start(&downloads, tmp.path());
    let (text, error) = client.call("task_list", json!({}));
    assert!(error);
    assert_eq!(text, "no Altium project here, run altium init");
    assert_eq!(fs::read_dir(&downloads).unwrap().count(), 0);
}

#[test]
fn tasks_through_mcp() {
    let (tmp, root) = project();
    let mut client = Client::start(&root, tmp.path());
    let created = client.ok(
        "task_create",
        json!({"title": "Intégrer Stripe", "priority": "P1", "tags": ["Backend"]}),
    );
    assert_eq!(created, "created T-1 P1 todo \"Intégrer Stripe\" #backend");
    client.ok("task_create", json!({"title": "Clés API", "parent": "T-1"}));
    client.ok(
        "task_create",
        json!({"title": "Webhooks", "parent": "T-1", "due": "2020-01-01"}),
    );

    let (text, error) = client.call("task_update", json!({"id": "T-1", "status": "done"}));
    assert!(error);
    assert!(text.contains("T-2, T-3"), "{text}");
    let (text, error) = client.call("task_update", json!({"id": "T-2", "status": "blocked"}));
    assert!(error && text.contains("reason"), "{text}");

    client.ok(
        "task_update",
        json!({"id": "T-2", "status": "blocked", "blocked_reason": "clé API manquante"}),
    );
    let list = client.ok("task_list", json!({}));
    assert!(
        list.contains("T-3 P2 todo due:2020-01-01(OVERDUE) \"Webhooks\" ^T-1"),
        "{list}"
    );
    assert!(
        list.contains("blocked:\nT-2 P2 blocked \"Clés API\" ^T-1 why:\"clé API manquante\""),
        "{list}"
    );

    let done = client.ok("task_update", json!({"id": "T-1", "status": "done", "force": true}));
    assert!(done.starts_with("updated T-1 P1 done"));
    let detail = client.ok("task_get", json!({"id": "T-1"}));
    assert!(
        detail.contains("created by claude") && detail.contains("updated by claude"),
        "{detail}"
    );

    let store = Store::open(&root).unwrap();
    assert_eq!(store.tasks().unwrap().len(), 3);
    assert_eq!(ops::history(&store, "T-1")[0].by.to_string(), "claude");
}

#[test]
fn memory_through_mcp() {
    let (tmp, root) = project();
    let mut client = Client::start(&root, tmp.path());
    let (text, error) = client.call(
        "memory_put",
        json!({"id": "auth-jwt", "kind": "decision", "title": "Auth par JWT", "desc": "x".repeat(200)}),
    );
    assert!(error && text.contains("120"), "{text}");
    for (id, title) in [
        ("crypto", "Crypto"),
        ("auth-jwt", "Auth par JWT"),
        ("api-refresh-token", "Refresh tokens"),
    ] {
        client.ok(
            "memory_put",
            json!({"id": id, "kind": "decision", "title": title, "desc": format!("About {id}"), "body": format!("Body of {id}.")}),
        );
    }
    client.ok(
        "memory_link",
        json!({"from": "auth-jwt", "to": "crypto", "type": "depends_on"}),
    );
    client.ok(
        "memory_link",
        json!({"from": "api-refresh-token", "to": "auth-jwt", "type": "depends_on"}),
    );
    client.ok(
        "memory_link",
        json!({"from": "auth-jwt", "to": "file:src/auth/", "type": "relates_to"}),
    );

    let simple = client.ok("memory_get", json!({"ids": ["api-refresh-token"]}));
    assert!(simple.contains("Body of api-refresh-token."));
    assert!(
        simple.contains("api-refresh-token ->depends_on auth-jwt [decision] Auth par JWT: About auth-jwt"),
        "{simple}"
    );
    assert!(!simple.contains("Body of auth-jwt"));

    let deep = client.ok(
        "memory_get",
        json!({"ids": ["api-refresh-token"], "follow": ["depends_on"], "depth": 2}),
    );
    assert!(
        deep.contains("Body of auth-jwt.") && deep.contains("Body of crypto."),
        "{deep}"
    );
    assert!(deep.contains("depth reached: 2"));
    assert!(deep.contains("relates_to file:src/auth/ (BROKEN)"), "{deep}");

    let found = client.ok("memory_search", json!({"path": "src/auth/middleware.rs"}));
    assert!(found.starts_with("auth-jwt [decision]"), "{found}");
    let index = client.ok("memory_index", json!({}));
    assert_eq!(index.lines().count(), 3);
}
