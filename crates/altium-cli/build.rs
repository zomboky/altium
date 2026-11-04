//! Embeds the git commit the binary is built from: `altium update` compares
//! it with the latest one of the repository.

use std::process::Command;

fn git(args: &[&str]) -> Option<String> {
    let output = Command::new("git").args(args).output().ok()?;
    let text = String::from_utf8(output.stdout).ok()?.trim().to_owned();
    (output.status.success() && !text.is_empty()).then_some(text)
}

fn main() {
    let commit = git(&["rev-parse", "HEAD"]).unwrap_or_default();
    let short = commit.get(..7).unwrap_or("inconnu");
    println!("cargo:rustc-env=ALTIUM_COMMIT={commit}");
    println!(
        "cargo:rustc-env=ALTIUM_LONG_VERSION={} ({short})",
        std::env::var("CARGO_PKG_VERSION").unwrap_or_default()
    );
    // Rebuild when HEAD moves: a commit, a checkout or a pull.
    if let Some(dir) = git(&["rev-parse", "--absolute-git-dir"]) {
        println!("cargo:rerun-if-changed={dir}/HEAD");
    }
    // A missing path would make cargo rerun this script on every build.
    if let Some(dir) = git(&["rev-parse", "--path-format=absolute", "--git-common-dir"]) {
        for path in [format!("{dir}/refs/heads"), format!("{dir}/packed-refs")] {
            if std::path::Path::new(&path).exists() {
                println!("cargo:rerun-if-changed={path}");
            }
        }
    }
}
