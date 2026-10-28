use clap::Parser;

/// Altium: a project dashboard fed by Claude Code.
#[derive(Parser)]
#[command(name = "altium", version = altium_core::VERSION, about)]
struct Cli {}

fn main() {
    let _cli = Cli::parse();
}
