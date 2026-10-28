//! `altium list`: show the tracked projects and whether they are still there.

use altium_core::registry::Registry;
use anyhow::Result;

use crate::display;

pub fn run() -> Result<()> {
    let listed = Registry::for_user()?.list()?;
    if listed.is_empty() {
        println!("Aucun projet suivi. Lancez `altium init` dans le dossier d'un projet.");
        return Ok(());
    }

    let rows: Vec<[String; 3]> = listed
        .iter()
        .map(|item| {
            let (name, status) = match &item.project {
                Some(project) => (project.name.clone(), "présent"),
                None => (item.entry.name.clone(), "introuvable"),
            };
            [name, display::path(&item.entry.path), status.to_owned()]
        })
        .collect();

    let header = ["NOM", "CHEMIN", "STATUT"].map(str::to_owned);
    let width = |column: usize| {
        rows.iter()
            .chain([&header])
            .map(|row| row[column].chars().count())
            .max()
            .unwrap_or(0)
    };
    let (name_width, path_width) = (width(0), width(1));
    for row in [&header].into_iter().chain(&rows) {
        println!("{:name_width$}  {:path_width$}  {}", row[0], row[1], row[2]);
    }

    if listed.iter().any(|item| item.project.is_none()) {
        println!("\nUn projet introuvable peut être retiré avec `altium forget <chemin|id>`.");
    }
    Ok(())
}
