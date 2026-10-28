//! `altium init`: create `.altium/` in the current directory and register it.

use std::env;

use altium_core::Store;
use altium_core::project::Project;
use altium_core::registry::{Registered, Registry};
use altium_core::store::{DIR_NAME, find_root};
use anyhow::{Context, Result, bail};

use crate::display;

pub fn run(name: Option<String>, force: bool) -> Result<()> {
    let cwd = env::current_dir().context("dossier courant inaccessible")?;
    let registry = Registry::for_user()?;

    // Already a project: never touch its data, only make sure it is tracked.
    if cwd.join(DIR_NAME).is_dir() {
        let project = Store::open(&cwd)?.project()?;
        let registered = registry.register(&project, &cwd)?;
        println!(
            "Le projet « {} » est déjà initialisé ici : son dossier .altium/ n'a pas été modifié.",
            project.name
        );
        if name.is_some_and(|name| name != project.name) {
            println!("L'option --name est ignorée pour un projet existant.");
        }
        match registered {
            Registered::Added => println!("Il est de nouveau suivi par le dashboard."),
            Registered::Moved { from } => {
                println!(
                    "Son chemin a été mis à jour dans le registre (avant : {}).",
                    display::path(&from)
                )
            }
            Registered::Unchanged => {}
        }
        print_details(&project);
        return Ok(());
    }

    if !force && let Some(parent) = cwd.parent().and_then(find_root) {
        let parent_name = Store::open(&parent)
            .and_then(|store| store.project())
            .map(|project| project.name)
            .unwrap_or_else(|_| "?".to_owned());
        bail!(
            "ce dossier fait partie du projet « {parent_name} » ({}). Lancez `altium init` à sa racine, \
             ou ajoutez --force pour créer un projet séparé ici.",
            display::path(&parent)
        );
    }

    let name = match name {
        Some(name) if name.trim().is_empty() => bail!("le nom du projet ne peut pas être vide"),
        Some(name) => name.trim().to_owned(),
        None => cwd
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "projet".to_owned()),
    };
    let store = Store::init(&cwd, &name)?;
    let project = store.project()?;
    registry.register(&project, &cwd)?;

    println!(
        "Projet « {} » initialisé dans {}.",
        project.name,
        display::path(store.dir())
    );
    print_details(&project);
    Ok(())
}

fn print_details(project: &Project) {
    println!("  id         {}", project.id);
    println!("  dashboard  {}", display::dashboard_url(&project.id));
}
