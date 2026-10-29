//! `altium mcp`: the MCP server Claude Code talks to over stdio. It speaks
//! JSON-RPC 2.0, one message per line, and exposes the project status,
//! tasks and memory as tools with compact English text replies.

use std::env;
use std::io::{BufRead, Write};
use std::path::PathBuf;

use altium_core::activity::{self, Sessions};
use altium_core::actor::Actor;
use altium_core::context::instructions;
use altium_core::graph::{Search, World, link_name};
use altium_core::memory::{LinkTarget, LinkType, NodeKind};
use altium_core::ops::{self, NodeInput, OpError};
use altium_core::patch::{Edit, TaskPatch};
use altium_core::project::Project;
use altium_core::render;
use altium_core::store::{Store, find_root};
use altium_core::task::{Priority, TaskId, TaskStatus};
use altium_core::tasks::{self, DeadlineState, NewTask, TaskError, TaskFilter};
use altium_core::time::{Clock, Deadline};
use altium_core::{StoreError, process};
use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

/// Protocol version used when the client does not ask for one.
const PROTOCOL_VERSION: &str = "2025-06-18";

/// Error returned by every tool outside an Altium project.
pub const NO_PROJECT: &str = "no Altium project here, run altium init";

/// Serves MCP requests read from `input` until it closes.
pub fn serve(input: impl BufRead, mut output: impl Write) -> std::io::Result<()> {
    let server = Server::new();
    for line in input.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let reply = match serde_json::from_str::<Value>(&line) {
            Ok(message) => server.handle(&message),
            Err(e) => Some(error(Value::Null, -32700, &format!("parse error: {e}"))),
        };
        if let Some(reply) = reply {
            writeln!(output, "{reply}")?;
            output.flush()?;
        }
    }
    Ok(())
}

fn error(id: Value, code: i64, message: &str) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": message}})
}

struct Server {
    cwd: PathBuf,
}

impl Server {
    fn new() -> Server {
        Server {
            cwd: env::var_os("ALTIUM_PROJECT")
                .map(PathBuf::from)
                .or_else(|| env::current_dir().ok())
                .unwrap_or_default(),
        }
    }

    fn handle(&self, message: &Value) -> Option<Value> {
        let method = message.get("method").and_then(Value::as_str).unwrap_or_default();
        // Notifications have no id and get no reply.
        let id = message.get("id")?.clone();
        let params = message.get("params").cloned().unwrap_or(Value::Null);
        let result = match method {
            "initialize" => Ok(self.initialize(&params)),
            "ping" => Ok(json!({})),
            "tools/list" => Ok(json!({"tools": tool_list()})),
            "tools/call" => Ok(self.call(&params)),
            "resources/list" => Ok(json!({"resources": []})),
            "prompts/list" => Ok(json!({"prompts": []})),
            _ => Err(error(id.clone(), -32601, &format!("method not found: {method}"))),
        };
        Some(match result {
            Ok(result) => json!({"jsonrpc": "2.0", "id": id, "result": result}),
            Err(e) => e,
        })
    }

    fn initialize(&self, params: &Value) -> Value {
        let version = params
            .get("protocolVersion")
            .and_then(Value::as_str)
            .unwrap_or(PROTOCOL_VERSION);
        let lang = self
            .store()
            .ok()
            .and_then(|s| s.project().ok())
            .map(|p| p.config.memory_lang().to_owned())
            .unwrap_or_else(|| "en".into());
        json!({
            "protocolVersion": version,
            "capabilities": {"tools": {}},
            "serverInfo": {"name": "altium", "version": altium_core::VERSION},
            "instructions": format!(
                "{}\n- Keep replies small: task_list and memory_index return one line per item; load details only when needed.",
                instructions(&lang)
            ),
        })
    }

    fn store(&self) -> Result<Store, String> {
        let root = find_root(&self.cwd).ok_or(NO_PROJECT)?;
        Store::open(&root).map_err(|e| e.to_string())
    }

    fn call(&self, params: &Value) -> Value {
        let name = params.get("name").and_then(Value::as_str).unwrap_or_default();
        let args = params.get("arguments").cloned().unwrap_or_else(|| json!({}));
        let result = self.store().and_then(|store| {
            let tool = Tool {
                store: &store,
                clock: Clock::system(),
                actor: actor(&store),
            };
            tool.run(name, args)
        });
        match result {
            Ok(text) => json!({"content": [{"type": "text", "text": text}]}),
            Err(message) => json!({"content": [{"type": "text", "text": message}], "isError": true}),
        }
    }
}

/// Claude, with the session the `SessionStart` hook recorded for the same
/// `claude` process; an unknown session if that fails.
fn actor(store: &Store) -> Actor {
    let session = process::claude_ancestor().and_then(|pid| {
        let sessions: Sessions = store.read_runtime(activity::SESSIONS_FILE);
        activity::session_of_pid(&sessions, pid)
    });
    Actor::Claude { session }
}

struct Tool<'a> {
    store: &'a Store,
    clock: Clock,
    actor: Actor,
}

fn parse<T: DeserializeOwned>(args: Value) -> Result<T, String> {
    serde_json::from_value(args).map_err(|e| format!("invalid arguments: {e}"))
}

fn task_error(e: TaskError) -> String {
    match e {
        TaskError::Conflict(c) => c.to_string(),
        other => other.to_string(),
    }
}

fn op_error(e: OpError) -> String {
    e.to_string()
}

fn store_error(e: StoreError) -> String {
    e.to_string()
}

/// `null` or `""` clears an optional field.
fn optional<T: std::str::FromStr<Err = String>>(value: &Value, field: &str) -> Result<Option<T>, String> {
    match value {
        Value::Null => Ok(None),
        Value::String(s) if s.trim().is_empty() => Ok(None),
        Value::String(s) => s.trim().parse().map(Some).map_err(|e| format!("{field}: {e}")),
        other => Err(format!("{field}: expected a string, got {other}")),
    }
}

#[derive(Deserialize)]
struct StatusSet {
    summary: Option<String>,
    phase: Option<String>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct TaskList {
    status: Vec<TaskStatus>,
    priority: Vec<Priority>,
    tag: Option<String>,
    deadline: Vec<DeadlineState>,
    text: Option<String>,
    /// Include done and cancelled tasks.
    closed: bool,
    limit: Option<usize>,
}

#[derive(Deserialize)]
struct TaskGet {
    id: TaskId,
}

#[derive(Deserialize)]
struct TaskCreate {
    title: String,
    description: Option<String>,
    status: Option<TaskStatus>,
    priority: Option<Priority>,
    #[serde(default)]
    due: Value,
    #[serde(default)]
    tags: Vec<String>,
    parent: Option<TaskId>,
    blocked_reason: Option<String>,
    note: Option<String>,
}

#[derive(Deserialize)]
struct TaskUpdate {
    id: TaskId,
    title: Option<String>,
    description: Option<Value>,
    status: Option<TaskStatus>,
    priority: Option<Priority>,
    due: Option<Value>,
    tags: Option<Vec<String>>,
    parent: Option<Value>,
    blocked_reason: Option<String>,
    note: Option<Value>,
    #[serde(default)]
    force: bool,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct MemoryIndex {
    limit: Option<usize>,
}

#[derive(Deserialize)]
struct MemoryGet {
    ids: Vec<String>,
    #[serde(default)]
    follow: Vec<LinkType>,
    depth: Option<u32>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct MemorySearch {
    text: Option<String>,
    kind: Option<NodeKind>,
    tag: Option<String>,
    path: Option<String>,
    limit: Option<usize>,
}

#[derive(Deserialize)]
struct MemoryLink {
    from: String,
    to: String,
    #[serde(rename = "type")]
    kind: LinkType,
    note: Option<String>,
}

#[derive(Deserialize)]
struct MemoryUnlink {
    from: String,
    to: String,
    #[serde(rename = "type")]
    kind: Option<LinkType>,
}

impl Tool<'_> {
    fn project(&self) -> Result<Project, String> {
        self.store.project().map_err(store_error)
    }

    fn run(&self, name: &str, args: Value) -> Result<String, String> {
        match name {
            "status_get" => self.status_get(),
            "status_set" => self.status_set(parse(args)?),
            "task_list" => self.task_list(parse(args)?),
            "task_get" => self.task_get(parse(args)?),
            "task_create" => self.task_create(parse(args)?),
            "task_update" => self.task_update(parse(args)?),
            "memory_index" => self.memory_index(parse(args)?),
            "memory_get" => self.memory_get(parse(args)?),
            "memory_search" => self.memory_search(parse(args)?),
            "memory_put" => self.memory_put(parse(args)?),
            "memory_link" => self.memory_link(parse(args)?),
            "memory_unlink" => self.memory_unlink(parse(args)?),
            _ => Err(format!("unknown tool `{name}`")),
        }
    }

    fn status_get(&self) -> Result<String, String> {
        let project = self.project()?;
        let status = self.store.status().map_err(store_error)?;
        let tasks = self.store.tasks().map_err(store_error)?;
        let near = project.config.near_hours();
        let p = altium_core::health::progress(&tasks, &self.clock, near);
        let mut text = format!(
            "project: {}\nphase: {} (allowed: {})\nsummary: {}\n",
            project.name,
            status.phase,
            project.config.phases().join(", "),
            if status.summary.is_empty() {
                "(none)"
            } else {
                &status.summary
            }
        );
        if let (Some(at), Some(by)) = (status.updated_at, status.updated_by) {
            text.push_str(&format!("updated: {at} by {by}\n"));
        }
        text.push_str(&format!(
            "tasks: {}/{} done ({}%), {} blocked, {} overdue, {} near deadline",
            p.done, p.total, p.percent, p.blocked, p.overdue, p.near
        ));
        Ok(text)
    }

    fn status_set(&self, args: StatusSet) -> Result<String, String> {
        if args.summary.is_none() && args.phase.is_none() {
            return Err("pass summary and/or phase".into());
        }
        let status =
            ops::set_status(self.store, args.summary, args.phase, &self.actor, &self.clock).map_err(op_error)?;
        Ok(format!(
            "ok: phase {}, summary {} chars",
            status.phase,
            status.summary.chars().count()
        ))
    }

    fn task_list(&self, args: TaskList) -> Result<String, String> {
        let project = self.project()?;
        let near = project.config.near_hours();
        let all = self.store.tasks().map_err(store_error)?;
        let filter = TaskFilter {
            tags: args.tag.into_iter().collect(),
            status: args.status,
            priority: args.priority,
            deadline: args.deadline,
            text: args.text,
        };
        let ordered = tasks::order(&all, &self.clock);
        let keep = |t: &&altium_core::task::Task| filter.matches(t, &self.clock, near);
        let mut lines: Vec<String> = Vec::new();
        let mut section = |title: &str, list: Vec<&altium_core::task::Task>| {
            let list: Vec<_> = list.into_iter().filter(keep).collect();
            if !list.is_empty() {
                lines.push(format!("{title}:"));
                lines.extend(list.iter().map(|t| render::task_line(t, &all, &self.clock, near)));
            }
        };
        section("open", ordered.active.clone());
        section("blocked", ordered.blocked.clone());
        if args.closed || filter.status.iter().any(|s| !tasks::is_open(*s)) {
            section("closed", ordered.closed.clone());
        }
        if lines.is_empty() {
            return Ok("no matching tasks".into());
        }
        let limit = args.limit.unwrap_or(50);
        let total = lines.len();
        if total > limit {
            lines.truncate(limit);
            lines.push(format!("+{} lines, narrow the filter or raise limit", total - limit));
        }
        Ok(lines.join("\n"))
    }

    fn task_get(&self, args: TaskGet) -> Result<String, String> {
        let project = self.project()?;
        let all = self.store.tasks().map_err(store_error)?;
        let task = tasks::find(&all, args.id).map_err(task_error)?;
        let mut text = render::task_detail(task, &all, &self.clock, project.config.near_hours());
        for change in ops::history(self.store, &args.id.to_string()) {
            let fields: Vec<&str> = change.fields.keys().map(String::as_str).collect();
            text.push_str(&format!(
                "\n{} {} by {} {}",
                change.at,
                change.action,
                change.by,
                fields.join(",")
            ));
        }
        Ok(text)
    }

    fn task_create(&self, args: TaskCreate) -> Result<String, String> {
        let new = NewTask {
            title: args.title,
            description: args.description,
            status: args.status,
            priority: args.priority,
            due: optional::<Deadline>(&args.due, "due")?,
            tags: args.tags,
            parent: args.parent,
            blocked_reason: args.blocked_reason,
            note: args.note,
        };
        let task = ops::create_task(self.store, new, &self.actor, &self.clock).map_err(task_error)?;
        let all = self.store.tasks().map_err(store_error)?;
        let near = self.project()?.config.near_hours();
        Ok(format!("created {}", render::task_line(&task, &all, &self.clock, near)))
    }

    fn task_update(&self, args: TaskUpdate) -> Result<String, String> {
        let text_field = |value: &Option<Value>, name: &str| -> Result<Option<Edit<Option<String>>>, String> {
            match value {
                None => Ok(None),
                Some(Value::Null) => Ok(Some(Edit::set(None))),
                Some(Value::String(s)) => Ok(Some(Edit::set((!s.trim().is_empty()).then(|| s.clone())))),
                Some(other) => Err(format!("{name}: expected a string, got {other}")),
            }
        };
        let patch = TaskPatch {
            title: args.title.map(Edit::set),
            description: text_field(&args.description, "description")?,
            status: args.status.map(Edit::set),
            priority: args.priority.map(Edit::set),
            due: match &args.due {
                None => None,
                Some(value) => Some(Edit::set(optional::<Deadline>(value, "due")?)),
            },
            tags: args.tags.map(Edit::set),
            parent: match &args.parent {
                None => None,
                Some(value) => Some(Edit::set(optional::<TaskId>(value, "parent")?)),
            },
            position: None,
            blocked_reason: args.blocked_reason.map(|r| Edit::set(Some(r))),
            note: text_field(&args.note, "note")?,
        };
        let task =
            ops::update_task(self.store, args.id, &patch, args.force, &self.actor, &self.clock).map_err(task_error)?;
        let all = self.store.tasks().map_err(store_error)?;
        let near = self.project()?.config.near_hours();
        Ok(format!("updated {}", render::task_line(&task, &all, &self.clock, near)))
    }

    fn memory_index(&self, args: MemoryIndex) -> Result<String, String> {
        let (graph, access) = ops::load_graph(self.store).map_err(store_error)?;
        let ranked = graph.ranked(&access, self.clock.now);
        if ranked.is_empty() {
            return Ok("memory is empty; save knowledge with memory_put".into());
        }
        let limit = args.limit.unwrap_or(200);
        let mut lines: Vec<String> = ranked.iter().take(limit).map(|n| graph.index_line(n)).collect();
        if ranked.len() > limit {
            lines.push(format!("+{} nodes, see memory_search", ranked.len() - limit));
        }
        Ok(lines.join("\n"))
    }

    fn memory_get(&self, args: MemoryGet) -> Result<String, String> {
        if args.ids.is_empty() {
            return Err("pass at least one id".into());
        }
        let (graph, _) = ops::load_graph(self.store).map_err(store_error)?;
        let depth = if args.follow.is_empty() {
            0
        } else {
            args.depth.unwrap_or(1)
        };
        let loaded = graph.load(&args.ids, &args.follow, depth);
        let task_ids = self
            .store
            .tasks()
            .map(|t| t.iter().map(|t| t.id).collect())
            .unwrap_or_default();
        let world = World {
            tasks: &task_ids,
            root: self.store.root(),
        };
        let text = render::loaded(&loaded, &graph, |node| {
            graph
                .broken_links(node, &world)
                .iter()
                .map(|l| l.to.to_string())
                .collect()
        });
        ops::touch_nodes(self.store, &loaded.ids(), &self.clock);
        Ok(text)
    }

    fn memory_search(&self, args: MemorySearch) -> Result<String, String> {
        let (graph, access) = ops::load_graph(self.store).map_err(store_error)?;
        let search = Search {
            text: args.text,
            kind: args.kind,
            tag: args.tag,
            path: args.path,
        };
        let found = graph.search(&search, &access, self.clock.now);
        if found.is_empty() {
            return Ok("no matching nodes".into());
        }
        let limit = args.limit.unwrap_or(20);
        let mut lines: Vec<String> = found.iter().take(limit).map(|n| graph.index_line(n)).collect();
        if found.len() > limit {
            lines.push(format!("+{} more", found.len() - limit));
        }
        Ok(lines.join("\n"))
    }

    fn memory_put(&self, args: NodeInput) -> Result<String, String> {
        let (node, created, warnings) = ops::put_node(self.store, args, &self.actor, &self.clock).map_err(op_error)?;
        let (graph, _) = ops::load_graph(self.store).map_err(store_error)?;
        let mut text = format!(
            "{} {}",
            if created { "created" } else { "updated" },
            graph.index_line(graph.get(&node.id).unwrap_or(&node))
        );
        for warning in warnings {
            text.push_str(&format!("\nnote: {warning}"));
        }
        Ok(text)
    }

    fn memory_link(&self, args: MemoryLink) -> Result<String, String> {
        let to: LinkTarget = args.to.parse()?;
        ops::link(
            self.store,
            &args.from,
            to.clone(),
            args.kind,
            args.note,
            &self.actor,
            &self.clock,
        )
        .map_err(op_error)?;
        Ok(format!("linked {} -{}-> {to}", args.from, link_name(args.kind)))
    }

    fn memory_unlink(&self, args: MemoryUnlink) -> Result<String, String> {
        let to: LinkTarget = args.to.parse()?;
        ops::unlink(self.store, &args.from, &to, args.kind, &self.actor, &self.clock).map_err(op_error)?;
        Ok(format!("unlinked {} -> {to}", args.from))
    }
}

/// Tool definitions: names, short English descriptions and input schemas.
fn tool_list() -> Value {
    let status = json!({"type": "string", "enum": ["todo", "doing", "blocked", "done", "cancelled"]});
    let priority = json!({"type": "string", "enum": ["P0", "P1", "P2", "P3"]});
    let kind = json!({"type": "string", "enum": ["decision", "fact", "convention", "component", "gotcha", "howto", "reference"]});
    let link_type = json!({"type": "string", "enum": ["depends_on", "implies", "equivalent_to", "part_of", "supersedes", "contradicts", "relates_to"]});
    let task_id = json!({"type": "string", "description": "Task id, e.g. T-12"});
    let due = json!({"type": ["string", "null"], "description": "YYYY-MM-DD or YYYY-MM-DDTHH:MM in the user's local time; empty or null clears it"});
    let link_target =
        json!({"type": "string", "description": "A node id, task:T-12, file:path/in/project, or an http(s) URL"});
    let tools = vec![
        tool(
            "status_get",
            "Project name, phase, summary and task progress.",
            json!({}),
            &[],
        ),
        tool(
            "status_set",
            "Update the project summary (max 600 chars: where things stand, next step, blockers) and/or phase.",
            json!({"summary": {"type": "string"}, "phase": {"type": "string"}}),
            &[],
        ),
        tool(
            "task_list",
            "Tasks in priority order, one line each. Open and blocked by default.",
            json!({
                "status": {"type": "array", "items": status},
                "priority": {"type": "array", "items": priority},
                "tag": {"type": "string"},
                "deadline": {"type": "array", "items": {"type": "string", "enum": ["overdue", "near", "later", "none"]}},
                "text": {"type": "string"},
                "closed": {"type": "boolean", "description": "Also list done and cancelled tasks"},
                "limit": {"type": "integer"}
            }),
            &[],
        ),
        tool(
            "task_get",
            "One task in full, with its subtasks and change history.",
            json!({"id": task_id}),
            &["id"],
        ),
        tool(
            "task_create",
            "Create a task (todo, P2 by default). Returns its line.",
            json!({
                "title": {"type": "string"}, "description": {"type": "string"}, "status": status,
                "priority": priority, "due": due, "tags": {"type": "array", "items": {"type": "string"}},
                "parent": task_id, "blocked_reason": {"type": "string"}, "note": {"type": "string"}
            }),
            &["title"],
        ),
        tool(
            "task_update",
            "Change fields of a task. blocked needs blocked_reason; done fails while subtasks are open unless force. Tasks cannot be deleted: set status cancelled with a note.",
            json!({
                "id": task_id, "title": {"type": "string"}, "description": {"type": ["string", "null"]},
                "status": status, "priority": priority, "due": due,
                "tags": {"type": "array", "items": {"type": "string"}}, "parent": {"type": ["string", "null"]},
                "blocked_reason": {"type": "string"}, "note": {"type": ["string", "null"]},
                "force": {"type": "boolean", "description": "Close even with open subtasks"}
            }),
            &["id"],
        ),
        tool(
            "memory_index",
            "One line per active memory node (id [kind] title: desc), most relevant first; * marks pinned.",
            json!({"limit": {"type": "integer"}}),
            &[],
        ),
        tool(
            "memory_get",
            "Load nodes with their body and links; neighbors come as index lines. follow loads linked bodies too, up to depth (default 1).",
            json!({
                "ids": {"type": "array", "items": {"type": "string"}},
                "follow": {"type": "array", "items": link_type},
                "depth": {"type": "integer"}
            }),
            &["ids"],
        ),
        tool(
            "memory_search",
            "Find nodes by text, kind, tag, or a project path they reference. Returns index lines.",
            json!({"text": {"type": "string"}, "kind": kind, "tag": {"type": "string"}, "path": {"type": "string"}, "limit": {"type": "integer"}}),
            &[],
        ),
        tool(
            "memory_put",
            "Create or update a node. id: lowercase slug. desc: one line, max 120 chars. Keep bodies short; split big topics into nodes linked with part_of. Unset fields keep their value; links replaces all links.",
            json!({
                "id": {"type": "string"}, "kind": kind, "title": {"type": "string"}, "desc": {"type": "string"},
                "body": {"type": "string", "description": "Markdown"}, "tags": {"type": "array", "items": {"type": "string"}},
                "pinned": {"type": "boolean"},
                "links": {"type": "array", "items": {"type": "object", "properties": {"to": link_target, "type": link_type, "note": {"type": "string"}}, "required": ["to", "type"]}}
            }),
            &["id"],
        ),
        tool(
            "memory_link",
            "Add a typed link from a node to a node, task, file or URL.",
            json!({"from": {"type": "string"}, "to": link_target, "type": link_type, "note": {"type": "string"}}),
            &["from", "to", "type"],
        ),
        tool(
            "memory_unlink",
            "Remove links from a node to a target (only of the given type, when set).",
            json!({"from": {"type": "string"}, "to": link_target, "type": link_type}),
            &["from", "to"],
        ),
    ];
    Value::Array(tools)
}

fn tool(name: &str, description: &str, properties: Value, required: &[&str]) -> Value {
    json!({
        "name": name,
        "description": description,
        "inputSchema": {"type": "object", "properties": properties, "required": required},
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_tool_has_an_english_description_and_a_schema() {
        for tool in tool_list().as_array().unwrap() {
            let description = tool["description"].as_str().unwrap();
            assert!(description.is_ascii(), "{description}");
            assert_eq!(tool["inputSchema"]["type"], "object");
        }
    }
}
