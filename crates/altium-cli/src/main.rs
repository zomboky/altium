mod commands;
mod display;

use std::process::ExitCode;

use clap::{Parser, Subcommand};

/// Altium : tableau de bord de projets alimenté par Claude Code.
#[derive(Parser)]
#[command(name = "altium", version = altium_core::VERSION, about)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Initialise un projet Altium dans le dossier courant et le référence
    Init {
        /// Nom affiché du projet (par défaut : nom du dossier)
        #[arg(long)]
        name: Option<String>,
        /// Crée le projet même dans un sous-dossier d'un projet existant
        #[arg(long)]
        force: bool,
    },
    /// Liste les projets suivis
    List,
    /// Retire un projet du suivi, sans toucher à son dossier .altium/
    Forget {
        /// Chemin ou identifiant du projet (par défaut : le projet courant)
        target: Option<String>,
    },
}

fn main() -> ExitCode {
    let result = match Cli::parse().command {
        Command::Init { name, force } => commands::init::run(name, force),
        Command::List => commands::list::run(),
        Command::Forget { target } => commands::forget::run(target),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("erreur : {error:#}");
            ExitCode::FAILURE
        }
    }
}
