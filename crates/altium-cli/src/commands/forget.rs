//! `altium forget`: stop tracking a project, leaving its `.altium/` alone.

use std::env;

use altium_core::registry::Registry;
use altium_core::store::find_root;
use anyhow::{Context, Result, anyhow, bail};

use crate::display;

pub fn run(target: Option<String>) -> Result<()> {
    let target = match target {
        Some(target) => display::expand_home(&target),
        None => {
            let cwd = env::current_dir().context("dossier courant inaccessible")?;
            let root = find_root(&cwd)
                .ok_or_else(|| anyhow!("aucun projet Altium ici : précisez un chemin ou un identifiant"))?;
            root.to_string_lossy().into_owned()
        }
    };

    let Some(entry) = Registry::for_user()?.forget(&target)? else {
        bail!("aucun projet suivi ne correspond à « {target} » (voir `altium list`)");
    };
    println!(
        "Projet « {} » retiré du suivi ({}).",
        entry.name,
        display::path(&entry.path)
    );
    println!("Son dossier .altium/ n'a pas été modifié.");
    Ok(())
}
