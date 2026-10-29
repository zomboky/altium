//! Keeping `.altium/` in git. A project's `.gitignore` written as an allow
//! list (`*`, then `!` exceptions) hides `.altium/` along with everything
//! else; [`ensure_tracked`] adds the missing exceptions.

use std::fs;
use std::path::Path;

use ignore::Match;
use ignore::gitignore::{Gitignore, GitignoreBuilder};

use crate::error::{Result, StoreError};
use crate::store::{DIR_NAME, PROJECT_FILE};

const FILE_NAME: &str = ".gitignore";

/// Appended to a `.gitignore` that hides `.altium/`. `.altium/.gitignore`
/// still keeps `runtime/` out.
const EXCEPTIONS: [&str; 3] = [
    "# Altium : garder .altium/ dans git (sauf .altium/runtime/)",
    "!/.altium/",
    "!/.altium/**",
];

/// What [`ensure_tracked`] found or did.
#[derive(Debug, Clone, PartialEq)]
pub enum Tracking {
    /// No `.gitignore` hides `.altium/`.
    Tracked,
    /// `rule`, a broad rule, hid `.altium/`: exceptions were added.
    Fixed { rule: String },
    /// `rule` hides `.altium/` and was left alone: it names `.altium`
    /// itself, so it is deliberate.
    Ignored { rule: String },
}

/// Makes sure the `.gitignore` at `root` does not hide `.altium/`, adding
/// exceptions after a broad rule such as `*`. Only that file is read: a rule
/// elsewhere (`.git/info/exclude`, global excludes) is not seen.
pub fn ensure_tracked(root: &Path) -> Result<Tracking> {
    let file = root.join(FILE_NAME);
    let Some(rule) = hiding_rule(root, &file)? else {
        return Ok(Tracking::Tracked);
    };
    if rule.contains(DIR_NAME) {
        return Ok(Tracking::Ignored { rule });
    }

    let text = fs::read_to_string(&file).map_err(StoreError::io(&file))?;
    let newline = if text.contains("\r\n") { "\r\n" } else { "\n" };
    let mut fixed = text;
    if !fixed.is_empty() && !fixed.ends_with('\n') {
        fixed.push_str(newline);
    }
    for line in EXCEPTIONS {
        fixed.push_str(line);
        fixed.push_str(newline);
    }
    fs::write(&file, fixed).map_err(StoreError::io(&file))?;
    Ok(Tracking::Fixed { rule })
}

/// The rule of `file` that hides `.altium/`, if any.
fn hiding_rule(root: &Path, file: &Path) -> Result<Option<String>> {
    if !file.is_file() {
        return Ok(None);
    }
    let mut builder = GitignoreBuilder::new(root);
    // Like git, skip the lines that do not parse.
    let _ = builder.add(file);
    let gitignore = builder.build().map_err(|e| StoreError::invalid(file, None, e))?;

    // Git does not look inside an ignored directory: check `.altium/`
    // before a file it holds.
    let dir = root.join(DIR_NAME);
    Ok(ignored_by(&gitignore, &dir, true).or_else(|| ignored_by(&gitignore, &dir.join(PROJECT_FILE), false)))
}

fn ignored_by(gitignore: &Gitignore, path: &Path, is_dir: bool) -> Option<String> {
    match gitignore.matched(path, is_dir) {
        Match::Ignore(glob) => Some(glob.original().to_owned()),
        Match::None | Match::Whitelist(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn project(gitignore: Option<&str>) -> tempfile::TempDir {
        let root = tempfile::tempdir().unwrap();
        if let Some(text) = gitignore {
            fs::write(root.path().join(FILE_NAME), text).unwrap();
        }
        root
    }

    fn gitignore(root: &tempfile::TempDir) -> String {
        fs::read_to_string(root.path().join(FILE_NAME)).unwrap()
    }

    #[test]
    fn no_gitignore_or_unrelated_rules_change_nothing() {
        let root = project(None);
        assert_eq!(ensure_tracked(root.path()).unwrap(), Tracking::Tracked);
        assert!(!root.path().join(FILE_NAME).exists());

        let root = project(Some("target/\n*.log\n"));
        assert_eq!(ensure_tracked(root.path()).unwrap(), Tracking::Tracked);
        assert_eq!(gitignore(&root), "target/\n*.log\n");
    }

    #[test]
    fn an_allow_list_gets_exceptions_once() {
        let root = project(Some("*\r\n!/.gitignore\r\n!/src/**"));
        assert_eq!(
            ensure_tracked(root.path()).unwrap(),
            Tracking::Fixed { rule: "*".to_owned() }
        );
        assert_eq!(
            gitignore(&root),
            "*\r\n!/.gitignore\r\n!/src/**\r\n\
             # Altium : garder .altium/ dans git (sauf .altium/runtime/)\r\n\
             !/.altium/\r\n!/.altium/**\r\n"
        );
        assert_eq!(ensure_tracked(root.path()).unwrap(), Tracking::Tracked);
    }

    #[test]
    fn hidden_dotfiles_get_exceptions_too() {
        let root = project(Some(".*\n"));
        assert_eq!(
            ensure_tracked(root.path()).unwrap(),
            Tracking::Fixed { rule: ".*".to_owned() }
        );
    }

    #[test]
    fn a_rule_naming_altium_is_left_alone() {
        let root = project(Some("node_modules/\n.altium/\n"));
        assert_eq!(
            ensure_tracked(root.path()).unwrap(),
            Tracking::Ignored {
                rule: ".altium/".to_owned()
            }
        );
        assert_eq!(gitignore(&root), "node_modules/\n.altium/\n");
    }
}
