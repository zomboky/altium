//! The user configuration of Claude Code that `altium setup` changes: hooks
//! in `settings.json`, and the user-scope MCP server.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use altium_core::time::Timestamp;
use anyhow::{Context, Result, bail};
use serde_json::{Map, Value, json};

/// Hook events Altium listens to.
pub const EVENTS: [&str; 7] = [
    "SessionStart",
    "UserPromptSubmit",
    "PreToolUse",
    "PostToolUse",
    "PostToolUseFailure",
    "Stop",
    "SessionEnd",
];

/// Name of the MCP server, hence the `mcp__altium__` tool prefix.
pub const MCP_NAME: &str = "altium";

/// Overrides the `claude` command (tests, unusual installs).
pub const CLAUDE_BIN_VAR: &str = "ALTIUM_CLAUDE";

/// `$CLAUDE_CONFIG_DIR`, else `~/.claude`.
pub fn config_dir() -> Result<PathBuf> {
    if let Some(dir) = env::var_os("CLAUDE_CONFIG_DIR").filter(|d| !d.is_empty()) {
        return Ok(PathBuf::from(dir));
    }
    Ok(dirs::home_dir()
        .context("dossier personnel introuvable")?
        .join(".claude"))
}

pub fn settings_file() -> Result<PathBuf> {
    Ok(config_dir()?.join("settings.json"))
}

/// The file holding user-scope MCP servers: `.claude.json` in
/// `$CLAUDE_CONFIG_DIR`, else in the home directory.
pub fn user_state_file() -> Result<PathBuf> {
    if let Some(dir) = env::var_os("CLAUDE_CONFIG_DIR").filter(|d| !d.is_empty()) {
        return Ok(PathBuf::from(dir).join(".claude.json"));
    }
    Ok(dirs::home_dir()
        .context("dossier personnel introuvable")?
        .join(".claude.json"))
}

/// The `claude` command, if it can be found.
pub fn claude_bin() -> Option<PathBuf> {
    if let Some(bin) = env::var_os(CLAUDE_BIN_VAR) {
        let bin = PathBuf::from(bin);
        return bin.is_file().then_some(bin);
    }
    let names: &[&str] = if cfg!(windows) {
        &["claude.exe", "claude.cmd"]
    } else {
        &["claude"]
    };
    let mut dirs: Vec<PathBuf> = env::var_os("PATH")
        .map(|p| env::split_paths(&p).collect())
        .unwrap_or_default();
    if let Some(home) = dirs::home_dir() {
        dirs.push(home.join(".local").join("bin"));
        dirs.push(home.join(".claude").join("local"));
    }
    dirs.iter()
        .flat_map(|dir| names.iter().map(move |name| dir.join(name)))
        .find(|path| path.is_file())
}

pub fn read_json(path: &Path) -> Result<Value> {
    match fs::read_to_string(path) {
        Ok(text) if text.trim().is_empty() => Ok(json!({})),
        Ok(text) => serde_json::from_str(&text).with_context(|| format!("{} n'est pas un JSON valide", path.display())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(json!({})),
        Err(e) => Err(e).with_context(|| format!("lecture de {}", path.display())),
    }
}

/// Writes `value` to `path`, after saving the previous version next to it.
pub fn write_json_with_backup(path: &Path, value: &Value) -> Result<Option<PathBuf>> {
    let backup = if path.exists() {
        let stamp = Timestamp::now().to_string().replace(':', "");
        let backup = PathBuf::from(format!("{}.altium-bak-{stamp}", path.display()));
        fs::copy(path, &backup).with_context(|| format!("sauvegarde de {}", path.display()))?;
        Some(backup)
    } else {
        None
    };
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let mut text = serde_json::to_string_pretty(value)?;
    text.push('\n');
    fs::write(path, text).with_context(|| format!("écriture de {}", path.display()))?;
    Ok(backup)
}

/// Whether a hook command is Altium's: the `altium` binary called with
/// `hook`, in exec form or shell form.
pub fn is_altium_hook(hook: &Value) -> bool {
    let command = hook.get("command").and_then(Value::as_str).unwrap_or_default();
    let is_altium = |program: &str| {
        let program = program.trim_matches('"').replace('\\', "/");
        let name = program.rsplit('/').next().unwrap_or_default().to_lowercase();
        name == "altium" || name == "altium.exe"
    };
    match hook.get("args").and_then(Value::as_array) {
        Some(args) => is_altium(command) && args.first().and_then(Value::as_str) == Some("hook"),
        None => {
            let words: Vec<&str> = command.split_whitespace().collect();
            words.windows(2).any(|w| is_altium(w[0]) && w[1] == "hook")
        }
    }
}

/// Removes Altium's hooks from `settings`; returns how many were removed.
pub fn remove_hooks(settings: &mut Value) -> usize {
    let Some(hooks) = settings.get_mut("hooks").and_then(Value::as_object_mut) else {
        return 0;
    };
    let mut removed = 0;
    for groups in hooks.values_mut() {
        let Some(groups) = groups.as_array_mut() else { continue };
        for group in groups.iter_mut() {
            if let Some(list) = group.get_mut("hooks").and_then(Value::as_array_mut) {
                let before = list.len();
                list.retain(|hook| !is_altium_hook(hook));
                removed += before - list.len();
            }
        }
        groups.retain(|group| {
            group
                .get("hooks")
                .and_then(Value::as_array)
                .is_none_or(|list| !list.is_empty())
        });
    }
    hooks.retain(|_, groups| groups.as_array().is_none_or(|g| !g.is_empty()));
    if hooks.is_empty()
        && let Some(root) = settings.as_object_mut()
    {
        root.remove("hooks");
    }
    removed
}

/// Adds Altium's hooks, calling `exe` in exec form: no shell, so a path with
/// spaces works with Git Bash, PowerShell or sh alike.
pub fn add_hooks(settings: &mut Value, exe: &Path) -> Result<()> {
    let Some(root) = settings.as_object_mut() else {
        bail!("settings.json ne contient pas un objet JSON");
    };
    let hooks = root.entry("hooks").or_insert_with(|| Value::Object(Map::new()));
    let Some(hooks) = hooks.as_object_mut() else {
        bail!("la clé hooks de settings.json n'est pas un objet");
    };
    for event in EVENTS {
        let group = json!({
            "hooks": [{
                "type": "command",
                "command": exe.display().to_string(),
                "args": ["hook", event],
                "timeout": 10,
            }]
        });
        match hooks.entry(event).or_insert_with(|| json!([])).as_array_mut() {
            Some(groups) => groups.push(group),
            None => bail!("hooks.{event} de settings.json n'est pas une liste"),
        }
    }
    Ok(())
}

/// Events whose Altium hook is present, with the program they call.
pub fn installed_hooks(settings: &Value) -> Vec<(String, String)> {
    let mut found = Vec::new();
    if let Some(hooks) = settings.get("hooks").and_then(Value::as_object) {
        for (event, groups) in hooks {
            for group in groups.as_array().into_iter().flatten() {
                for hook in group.get("hooks").and_then(Value::as_array).into_iter().flatten() {
                    if is_altium_hook(hook) {
                        let command = hook.get("command").and_then(Value::as_str).unwrap_or_default();
                        found.push((event.clone(), command.to_owned()));
                    }
                }
            }
        }
    }
    found
}

/// The command of the user-scope `altium` MCP server, if registered.
pub fn mcp_command() -> Result<Option<String>> {
    let state = read_json(&user_state_file()?)?;
    Ok(state
        .get("mcpServers")
        .and_then(|s| s.get(MCP_NAME))
        .and_then(|s| s.get("command"))
        .and_then(Value::as_str)
        .map(str::to_owned))
}

/// Registers the MCP server at user scope, with the `claude` CLI when
/// available, else in `.claude.json` directly. Returns how.
pub fn register_mcp(exe: &Path) -> Result<&'static str> {
    if let Some(claude) = claude_bin() {
        // Removing first makes setup idempotent and updates a moved binary.
        let _ = quiet(Command::new(&claude).args(["mcp", "remove", "--scope", "user", MCP_NAME]));
        let added = quiet(
            Command::new(&claude)
                .args(["mcp", "add", "--scope", "user", MCP_NAME, "--"])
                .arg(exe)
                .arg("mcp"),
        );
        if added {
            return Ok("claude mcp add");
        }
    }
    let file = user_state_file()?;
    let mut state = read_json(&file)?;
    let Some(root) = state.as_object_mut() else {
        bail!("{} ne contient pas un objet JSON", file.display());
    };
    let servers = root.entry("mcpServers").or_insert_with(|| json!({}));
    servers[MCP_NAME] = json!({
        "type": "stdio",
        "command": exe.display().to_string(),
        "args": ["mcp"],
        "env": {},
    });
    write_json_with_backup(&file, &state)?;
    Ok(".claude.json")
}

/// Removes the MCP server; returns whether it was registered.
pub fn unregister_mcp() -> Result<bool> {
    if mcp_command()?.is_none() {
        return Ok(false);
    }
    if let Some(claude) = claude_bin()
        && quiet(Command::new(&claude).args(["mcp", "remove", "--scope", "user", MCP_NAME]))
        && mcp_command()?.is_none()
    {
        return Ok(true);
    }
    let file = user_state_file()?;
    let mut state = read_json(&file)?;
    if let Some(servers) = state.get_mut("mcpServers").and_then(Value::as_object_mut) {
        servers.remove(MCP_NAME);
    }
    write_json_with_backup(&file, &state)?;
    Ok(true)
}

fn quiet(command: &mut Command) -> bool {
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_altium_hooks_in_both_forms() {
        assert!(is_altium_hook(
            &json!({"command": "C:\\Program Files\\Altium\\altium.exe", "args": ["hook", "Stop"]})
        ));
        assert!(is_altium_hook(
            &json!({"command": "/home/me/.local/bin/altium hook Stop"})
        ));
        assert!(!is_altium_hook(&json!({"command": "altium", "args": ["serve"]})));
        assert!(!is_altium_hook(&json!({"command": "prettier --write"})));
    }

    #[test]
    fn add_then_remove_keeps_user_hooks() {
        let user = json!({"matcher": "Edit", "hooks": [{"type": "command", "command": "prettier --write"}]});
        let mut settings = json!({"model": "opus", "hooks": {"PostToolUse": [user.clone()]}});
        let original = settings.clone();
        add_hooks(&mut settings, Path::new("/opt/my tools/altium")).unwrap();
        assert_eq!(installed_hooks(&settings).len(), EVENTS.len());
        assert_eq!(settings["hooks"]["PostToolUse"][0], user);

        assert_eq!(remove_hooks(&mut settings), EVENTS.len());
        assert_eq!(settings, original);
    }
}
