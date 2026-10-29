//! `altium setup [--uninstall]`: installs (or removes) the Altium hooks and
//! MCP server in the user configuration of Claude Code, once per machine.

use std::env;

use anyhow::{Context, Result};

use crate::claude;
use crate::display;

pub fn run(uninstall: bool) -> Result<()> {
    if uninstall {
        return remove();
    }
    let exe = dunce::canonicalize(env::current_exe()?).context("chemin du binaire altium introuvable")?;
    let file = claude::settings_file()?;
    let mut settings = claude::read_json(&file)?;
    claude::remove_hooks(&mut settings);
    claude::add_hooks(&mut settings, &exe)?;
    let backup = claude::write_json_with_backup(&file, &settings)?;
    let how = claude::register_mcp(&exe)?;

    println!("Altium est installé dans Claude Code.");
    println!(
        "  hooks      {} ({} événements)",
        display::path(&file),
        claude::EVENTS.len()
    );
    if let Some(backup) = backup {
        println!("  sauvegarde {}", display::path(&backup));
    }
    println!(
        "  MCP        serveur « {} » au niveau utilisateur (via {how})",
        claude::MCP_NAME
    );
    println!("  binaire    {}", display::path(&exe));
    println!();
    println!("Redémarre tes sessions Claude Code pour qu'elles le prennent en compte.");
    Ok(())
}

fn remove() -> Result<()> {
    let file = claude::settings_file()?;
    let mut settings = claude::read_json(&file)?;
    let removed = claude::remove_hooks(&mut settings);
    if removed > 0 {
        claude::write_json_with_backup(&file, &settings)?;
    }
    let mcp = claude::unregister_mcp()?;
    if removed == 0 && !mcp {
        println!("Altium n'était pas installé dans Claude Code : rien à retirer.");
    } else {
        println!("Altium est retiré de Claude Code.");
        println!("  hooks retirés  {removed}");
        println!("  MCP            {}", if mcp { "retiré" } else { "absent" });
        println!("Les autres hooks et serveurs MCP n'ont pas été modifiés.");
    }
    Ok(())
}
