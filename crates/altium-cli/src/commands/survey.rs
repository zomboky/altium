//! `altium survey`: launches an interactive Claude Code session that surveys
//! an existing project (files, past conversations, Claude's memory), fills in
//! the summary, phase and memory, and asks the user about each task it
//! proposes.

use std::env;
use std::fs;
use std::io::{self, BufRead, IsTerminal, Write};
use std::path::Path;
use std::process::Command;

use altium_core::store::{Store, find_root};
use altium_core::survey::{self, SurveyState};
use altium_core::time::Timestamp;
use anyhow::{Context, Result, bail};

use crate::claude;
use crate::display;

/// The survey instructions given to Claude, in English (D11).
fn prompt(digest: &Path, conversations: usize) -> String {
    let digest = digest.display();
    format!(
        "Survey this existing project for Altium, the project tracker installed here. Work in these steps.

1. Call mcp__altium__status_get, mcp__altium__task_list and mcp__altium__memory_index first: never duplicate what already exists.
2. Explore the project files. Respect .gitignore (prefer `git ls-files` when it is a git repository); skip binaries, build outputs and large data files. Read the README, docs, manifests, TODO lists and the main source layout.
3. Read the digest of past Claude Code conversations in this folder: {digest} ({conversations} conversations, dated, oldest first). It ends with Claude's auto memory for this folder, if any.
4. Write what you learned with the Altium MCP tools only (never edit .altium/ files directly):
   - status_set: a summary (max 600 characters: where the project stands, next step, blockers) and the phase.
   - memory_put: short nodes (decision, fact, convention, component, gotcha, howto, reference) linked with memory_link. A node drawn from a conversation states the conversation date in its body and cites it as its source (\"Source: conversation of YYYY-MM-DD\"). When a later conversation contradicts an earlier fact, the newer node supersedes the older one.
5. Then propose tasks ONE AT A TIME. For each, ask the user with your question tool (AskUserQuestion) whether to add it as \"To do\", \"To explore\" or \"Ignore\", with the task title and one line of why. Wait for the answer before the next proposal:
   - To do: task_create with status todo.
   - To explore: task_create with status todo and the tag explore.
   - Ignore: create nothing.
6. Finish with a short recap of what you wrote.

Reply to the user in their language."
    )
}

pub fn run() -> Result<()> {
    let cwd = env::current_dir()?;
    let root = find_root(&cwd).context("aucun projet Altium ici : lance d'abord `altium init`")?;
    let store = Store::open(&root)?;
    let claude_bin = claude::claude_bin()
        .context("commande claude introuvable : installe Claude Code (https://claude.com/claude-code) puis relance")?;

    let settings = claude::read_json(&claude::settings_file()?)?;
    if claude::installed_hooks(&settings).is_empty() || claude::mcp_command()?.is_none() {
        println!("Altium n'est pas encore installé dans Claude Code : l'état des lieux a besoin de ses outils MCP.");
        if !ask("Lancer `altium setup` maintenant ?")? {
            bail!("état des lieux annulé : lance `altium setup` puis `altium survey`");
        }
        super::setup::run(false)?;
        println!();
    }

    let state: SurveyState = store.read_runtime(survey::STATE_FILE);
    let (digest, count) = survey::digest(&claude::config_dir()?, store.root(), state.last);
    let dir = store.runtime_path(survey::DIR);
    fs::create_dir_all(&dir)?;
    let digest_path = dir.join(survey::DIGEST_FILE);
    fs::write(&digest_path, digest)?;
    let started = Timestamp::now();

    println!("État des lieux de {} avec Claude Code.", display::path(store.root()));
    println!(
        "  conversations passées  {count} (condensé dans {})",
        display::path(&digest_path)
    );
    println!("  Claude te posera une question pour chaque task qu'il propose.");
    println!();

    let prompt_path = dir.join(survey::PROMPT_FILE);
    fs::write(&prompt_path, prompt(&digest_path, count))?;
    let status = command(&claude_bin, store.root(), &prompt_path)
        .status()
        .with_context(|| format!("lancement de {}", claude_bin.display()))?;
    if !status.success() {
        bail!("Claude Code s'est arrêté avec une erreur ({status})");
    }
    store.update_runtime(survey::STATE_FILE, |state: &mut SurveyState| {
        state.last = Some(started);
    })?;
    Ok(())
}

/// Whether `altium init` should offer a survey: an interactive terminal, a
/// folder with something besides `.altium/` and `.git/`, and Claude Code
/// installed.
pub fn should_offer(root: &Path) -> bool {
    io::stdin().is_terminal()
        && io::stdout().is_terminal()
        && claude::claude_bin().is_some()
        && fs::read_dir(root).is_ok_and(|entries| {
            entries
                .filter_map(|e| e.ok())
                .any(|e| !matches!(e.file_name().to_str(), Some(".altium" | ".git")))
        })
}

/// Asks a yes/no question, yes by default.
/// The interactive Claude Code session of the survey. Its only argument is
/// a one-line prompt pointing at the instructions file: an option taking
/// several values, such as `--add-dir`, would swallow a prompt placed after
/// it, and `cmd.exe` cuts a multi-line argument given to `claude.cmd`.
fn command(claude_bin: &Path, root: &Path, prompt_path: &Path) -> Command {
    let mut command = Command::new(claude_bin);
    command.current_dir(root).arg(format!(
        "Read {} and follow its instructions to survey this project for Altium.",
        prompt_path.display()
    ));
    command
}

pub fn ask(question: &str) -> Result<bool> {
    if !io::stdin().is_terminal() {
        return Ok(false);
    }
    print!("{question} [O/n] ");
    io::stdout().flush()?;
    let mut answer = String::new();
    io::stdin().lock().read_line(&mut answer)?;
    let answer = answer.trim().to_lowercase();
    Ok(answer.is_empty() || matches!(answer.as_str(), "o" | "oui" | "y" | "yes"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_prompt_is_the_only_argument() {
        let root = Path::new("/projets/meudon");
        let prompt = root.join(".altium/runtime/survey/prompt.md");
        let command = command(Path::new("claude"), root, &prompt);
        let args: Vec<String> = command.get_args().map(|a| a.to_string_lossy().into_owned()).collect();
        assert_eq!(args.len(), 1, "{args:?}");
        assert!(!args[0].starts_with('-'));
        assert!(!args[0].contains('\n'));
        assert!(args[0].contains("prompt.md"));
    }
}
