//! `altium init`: create `.altium/` in the current directory and register it.

use std::env;
use std::path::Path;

use altium_core::Store;
use altium_core::gitignore::{self, Tracking};
use altium_core::project::Project;
use altium_core::registry::{Registered, Registry};
use altium_core::store::{DIR_NAME, PROJECT_FILE, find_root};
use altium_core::survey::{self, SurveyState};
use anyhow::{Context, Result, bail};

use crate::display;

pub fn run(name: Option<String>, force: bool) -> Result<()> {
    let cwd = env::current_dir().context("dossier courant inaccessible")?;
    let registry = Registry::for_user()?;
    let name = match name {
        Some(name) if name.trim().is_empty() => bail!("le nom du projet ne peut pas être vide"),
        name => name.map(|name| name.trim().to_owned()),
    };

    if cwd.join(DIR_NAME).is_dir() {
        complete(&cwd, &registry, name)?;
    } else {
        create(&cwd, &registry, name, force)?;
    }
    track_in_git(&cwd);
    offer_survey(&cwd)
}

/// Offers a survey by Claude when the folder already holds a project that
/// was never surveyed. Never launches anything without a yes.
fn offer_survey(root: &Path) -> Result<()> {
    if !super::survey::should_offer(root) {
        return Ok(());
    }
    let surveyed = Store::open(root)
        .map(|store| store.read_runtime::<SurveyState>(survey::STATE_FILE).last.is_some())
        .unwrap_or(true);
    if surveyed {
        return Ok(());
    }
    println!();
    if super::survey::ask("Ce dossier contient déjà des fichiers. Lancer un état des lieux avec Claude ?")? {
        super::survey::run()
    } else {
        println!("Tu pourras le lancer plus tard avec `altium survey`.");
        Ok(())
    }
}

fn create(cwd: &Path, registry: &Registry, name: Option<String>, force: bool) -> Result<()> {
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

    let store = Store::init(cwd, &name.unwrap_or_else(|| dir_name(cwd)))?;
    let project = store.project()?;
    registry.register(&project, cwd)?;

    println!(
        "Projet « {} » initialisé dans {}",
        project.name,
        display::path(store.dir())
    );
    print_details(&project);
    Ok(())
}

/// Already a project, maybe half-made: create only what `.altium/` lacks,
/// never touch its data, and make sure the project is tracked.
fn complete(cwd: &Path, registry: &Registry, name: Option<String>) -> Result<()> {
    // Without `project.toml`, keep the identity the registry knows.
    let known = registry.find_path(cwd)?;
    let (store, created) = Store::complete(cwd, || match known {
        Some(entry) => Project {
            id: entry.id,
            name: name.clone().unwrap_or(entry.name),
            ..Project::new("")
        },
        None => Project::new(&name.clone().unwrap_or_else(|| dir_name(cwd))),
    })?;
    let project = store.project()?;
    let registered = registry.register(&project, cwd)?;

    if created.contains(&PROJECT_FILE) {
        println!(
            "Projet « {} » complété dans {} : les fichiers déjà présents n'ont pas été modifiés.",
            project.name,
            display::path(store.dir())
        );
    } else if created.is_empty() {
        println!(
            "Le projet « {} » est déjà initialisé ici : son dossier .altium/ n'a pas été modifié.",
            project.name
        );
    } else {
        println!(
            "Le projet « {} » est déjà initialisé ici : seuls les fichiers manquants de .altium/ ont été créés.",
            project.name
        );
    }
    if !created.contains(&PROJECT_FILE) && name.is_some_and(|name| name != project.name) {
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
    if !created.is_empty() {
        println!("  créés      {}", created.join(", "));
    }
    print_details(&project);
    Ok(())
}

/// A `.gitignore` that hides everything but an allow list also hides
/// `.altium/`, which is meant to be versioned.
fn track_in_git(root: &Path) {
    match gitignore::ensure_tracked(root) {
        Ok(Tracking::Tracked) => {}
        Ok(Tracking::Fixed { rule }) => println!(
            "Le .gitignore excluait .altium/ (règle « {rule} ») : des exceptions ont été ajoutées à la fin pour le versionner."
        ),
        Ok(Tracking::Ignored { rule }) => println!(
            "Le .gitignore exclut .altium/ (règle « {rule} ») : il est laissé tel quel, le suivi du projet ne sera pas versionné."
        ),
        Err(error) => eprintln!("attention : le .gitignore n'a pas pu être vérifié : {error:#}"),
    }
}

fn dir_name(dir: &Path) -> String {
    dir.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "projet".to_owned())
}

fn print_details(project: &Project) {
    println!("  id         {}", project.id);
    println!("  dashboard  {}", display::dashboard_url(&project.id));
}
