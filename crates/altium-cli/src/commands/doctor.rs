//! `altium doctor`: checks the Claude Code integration, the dashboard and
//! the current project, and says how to fix what is wrong.

use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::Path;

use altium_core::config::GlobalConfig;
use altium_core::registry::Registry;
use altium_core::store::{DIR_NAME, Store, TASKS_FILE, find_root};
use altium_core::task::{Task, TaskId};
use altium_core::time::Timestamp;
use anyhow::{Result, bail};

use crate::claude;
use crate::display;
use crate::http;

struct Report {
    problems: usize,
}

impl Report {
    fn ok(&self, label: &str, detail: impl AsRef<str>) {
        println!("  ok      {label:<12}{}", detail.as_ref());
    }

    fn warn(&self, label: &str, detail: impl AsRef<str>) {
        println!("  attention {label:<10}{}", detail.as_ref());
    }

    fn fail(&mut self, label: &str, detail: impl AsRef<str>) {
        self.problems += 1;
        println!("  erreur  {label:<12}{}", detail.as_ref());
    }
}

pub fn run(renumber: bool) -> Result<()> {
    let mut report = Report { problems: 0 };
    let exe = dunce::canonicalize(env::current_exe()?)?;
    println!("Altium {} ({})", altium_core::VERSION, display::path(&exe));
    println!();
    println!("Claude Code");

    let settings_file = claude::settings_file()?;
    match claude::read_json(&settings_file) {
        Ok(settings) => {
            let hooks = claude::installed_hooks(&settings);
            let missing: Vec<&str> = claude::EVENTS
                .iter()
                .filter(|event| !hooks.iter().any(|(e, _)| e == *event))
                .copied()
                .collect();
            if hooks.is_empty() {
                report.fail("hooks", "aucun hook Altium : lance `altium setup`");
            } else if !missing.is_empty() {
                report.fail(
                    "hooks",
                    format!("hooks manquants : {} ; lance `altium setup`", missing.join(", ")),
                );
            } else {
                report.ok("hooks", format!("{} événements", claude::EVENTS.len()));
            }
            for program in hooks.iter().map(|(_, p)| p).collect::<std::collections::BTreeSet<_>>() {
                check_binary(&mut report, "binaire", program, &exe);
            }
        }
        Err(error) => report.fail("hooks", format!("{error:#}")),
    }
    match claude::mcp_command() {
        Ok(Some(command)) => {
            report.ok("MCP", format!("serveur « {} » enregistré", claude::MCP_NAME));
            check_binary(&mut report, "binaire MCP", &command, &exe);
        }
        Ok(None) => report.fail("MCP", "serveur MCP absent : lance `altium setup`"),
        Err(error) => report.fail("MCP", format!("{error:#}")),
    }
    match claude::claude_bin() {
        Some(bin) => report.ok("claude", display::path(&bin)),
        None => report.warn("claude", "commande claude introuvable dans le PATH"),
    }

    println!();
    println!("Dashboard");
    let registry = Registry::for_user()?;
    let port = GlobalConfig::load(registry.dir())?.port();
    match http::get("127.0.0.1", port, "/api/health") {
        Ok((200, _)) => report.ok("serveur", format!("http://127.0.0.1:{port}")),
        Ok((status, _)) => report.fail(
            "serveur",
            format!("le port {port} répond {status} : un autre programme ?"),
        ),
        Err(_) => report.warn(
            "serveur",
            "arrêté ; lance `altium serve` ou `altium service install`",
        ),
    }

    println!();
    println!("Projet courant");
    let cwd = env::current_dir()?;
    match find_root(&cwd) {
        None => report.warn("projet", "aucun projet ici ; `altium init` en crée un"),
        Some(root) => check_project(&mut report, &root, &registry, renumber)?,
    }

    println!();
    if report.problems == 0 {
        println!("Tout est en ordre.");
        Ok(())
    } else {
        bail!("{} problème(s) détecté(s)", report.problems)
    }
}

fn check_binary(report: &mut Report, label: &str, program: &str, exe: &Path) {
    let path = Path::new(program);
    if !path.is_file() {
        report.fail(label, format!("{program} introuvable : relance `altium setup`"));
    } else if dunce::canonicalize(path).ok().as_deref() != Some(exe) {
        report.warn(
            label,
            format!("{program} n'est pas ce binaire ; `altium setup` le remplace"),
        );
    }
}

fn check_project(report: &mut Report, root: &Path, registry: &Registry, renumber: bool) -> Result<()> {
    let store = match Store::open(root) {
        Ok(store) => store,
        Err(error) => {
            report.fail("projet", error.to_string());
            return Ok(());
        }
    };
    match store.project() {
        Ok(project) => {
            report.ok("projet", format!("« {} » ({})", project.name, display::path(root)));
            if registry.find_path(root)?.is_none_or(|e| e.id != project.id) {
                report.warn("registre", "projet absent du registre ; `altium init` l'y ajoute");
            }
        }
        Err(error) => report.fail("projet", error.to_string()),
    }
    if let Err(error) = store.status() {
        report.fail("statut", error.to_string());
    }

    let tasks_path = root.join(DIR_NAME).join(TASKS_FILE);
    let text = fs::read_to_string(&tasks_path).unwrap_or_default();
    let mut counts: HashMap<String, usize> = HashMap::new();
    for line in text.lines() {
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(line)
            && let Some(id) = value.get("id").and_then(|v| v.as_str())
        {
            *counts.entry(id.to_owned()).or_default() += 1;
        }
    }
    let mut duplicates: Vec<&String> = counts.iter().filter(|(_, n)| **n > 1).map(|(id, _)| id).collect();
    duplicates.sort();
    if !duplicates.is_empty() {
        if renumber {
            let renamed = renumber_duplicates(&store, &tasks_path, &text)?;
            report.ok("tasks", format!("ids en double renumérotés : {renamed}"));
        } else {
            let list: Vec<&str> = duplicates.iter().map(|s| s.as_str()).collect();
            report.fail(
                "tasks",
                format!(
                    "ids en double ({}), souvent après une fusion git ; `altium doctor --renumber` les renumérote",
                    list.join(", ")
                ),
            );
        }
    } else {
        match store.tasks() {
            Ok(tasks) => report.ok("tasks", format!("{} tasks", tasks.len())),
            Err(error) => report.fail("tasks", error.to_string()),
        }
    }
    match store.graph() {
        Ok((graph, errors)) => {
            report.ok("mémoire", format!("{} nœuds", graph.len()));
            for error in errors {
                report.fail("mémoire", error.to_string());
            }
        }
        Err(error) => report.fail("mémoire", error.to_string()),
    }
    Ok(())
}

/// Gives a new id to every task whose id is already used by an earlier line.
fn renumber_duplicates(store: &Store, path: &Path, text: &str) -> Result<String> {
    let mut tasks: Vec<Task> = Vec::new();
    for line in text.lines().filter(|l| !l.trim().is_empty()) {
        tasks.push(serde_json::from_str(line)?);
    }
    let mut next = tasks.iter().map(|t| t.id.0).max().unwrap_or(0);
    let mut seen = std::collections::HashSet::new();
    let mut renamed = Vec::new();
    for task in &mut tasks {
        if !seen.insert(task.id) {
            next += 1;
            renamed.push(format!("{} → T-{next}", task.id));
            task.id = TaskId(next);
            task.rev += 1;
            task.updated_at = Timestamp::now();
        }
    }
    let backup = path.with_extension(format!("jsonl.bak-{}", Timestamp::now().to_string().replace(':', "")));
    fs::copy(path, &backup)?;
    fs::write(path, altium_core::task::render_tasks(&tasks))?;
    // Reading back checks the result.
    store.tasks()?;
    Ok(renamed.join(", "))
}
