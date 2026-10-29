mod claude;
mod commands;
mod display;
mod french;
mod http;

use std::process::ExitCode;

use clap::{Parser, Subcommand};

/// Altium : tableau de bord de projets alimenté par Claude Code.
#[derive(Parser)]
// Without `bin_name`, clap names the program after its file: `altium.exe` on Windows.
#[command(name = "altium", bin_name = "altium", version = altium_core::VERSION, about)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Initialise un projet Altium dans le dossier courant et le référence
    Init {
        /// Nom affiché du projet (par défaut : nom du dossier)
        #[arg(long, value_name = "NOM")]
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
        #[arg(value_name = "PROJET")]
        target: Option<String>,
    },
    /// Installe les hooks et le serveur MCP d'Altium dans Claude Code
    Setup {
        /// Retire ce qu'Altium a ajouté à la configuration de Claude Code
        #[arg(long)]
        uninstall: bool,
    },
    /// Vérifie l'intégration avec Claude Code, le dashboard et le projet courant
    Doctor {
        /// Renumérote les tasks dont l'identifiant est en double
        #[arg(long)]
        renumber: bool,
    },
    /// Point d'entrée des hooks de Claude Code
    #[command(hide = true)]
    Hook {
        #[arg(value_name = "ÉVÉNEMENT")]
        event: String,
    },
}

fn main() -> ExitCode {
    let result = match french::parse::<Cli>().command {
        Command::Init { name, force } => commands::init::run(name, force),
        Command::List => commands::list::run(),
        Command::Forget { target } => commands::forget::run(target),
        Command::Setup { uninstall } => commands::setup::run(uninstall),
        Command::Doctor { renumber } => commands::doctor::run(renumber),
        Command::Hook { event } => {
            commands::hook::run(&event);
            Ok(())
        }
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("erreur : {error:#}");
            ExitCode::FAILURE
        }
    }
}
