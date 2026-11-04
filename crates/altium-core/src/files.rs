//! Project files the dashboard may show: task deliverables and the files
//! memory links point to. A file is given by its path relative to the project
//! root, with `/` separators; it is shown only if it really lies inside the
//! project, outside hidden folders such as `.git` or `.env`.

use std::path::{Component, Path, PathBuf};

/// Whether `text` is a web address rather than a project file.
pub fn is_url(text: &str) -> bool {
    text.starts_with("https://") || text.starts_with("http://")
}

/// Checks a deliverable as typed by the user or Claude and returns its stored
/// form: a URL as is, or a clean relative path (`rapport/fiche.pdf`).
pub fn normalize(text: &str) -> Result<String, String> {
    let text = text.trim();
    if is_url(text) {
        return Ok(text.to_owned());
    }
    let path = text.strip_prefix("file:").unwrap_or(text).replace('\\', "/");
    let path = path.trim_start_matches("./");
    if path.is_empty() {
        return Err("a deliverable needs a path or a URL".into());
    }
    if path.starts_with('/') || Path::new(path).has_root() || path.contains(':') {
        return Err(format!(
            "deliverable `{path}`: give a path relative to the project root, or an http(s) URL"
        ));
    }
    let parts: Vec<&str> = path.split('/').filter(|p| !p.is_empty() && *p != ".").collect();
    if parts.contains(&"..") {
        return Err(format!("deliverable `{path}` points outside the project"));
    }
    if parts.iter().any(|p| p.starts_with('.')) {
        return Err(format!(
            "deliverable `{path}` is in a hidden folder, which the dashboard does not show"
        ));
    }
    Ok(parts.join("/"))
}

/// The file at `relative` inside `root`, if it exists, is a regular file, and
/// stays inside the project once symbolic links are followed.
pub fn resolve(root: &Path, relative: &str) -> Option<PathBuf> {
    let relative = Path::new(relative);
    let clean = relative.components().all(|c| match c {
        Component::Normal(part) => !part.to_string_lossy().starts_with('.'),
        _ => false,
    });
    if !clean {
        return None;
    }
    let root = dunce::canonicalize(root).ok()?;
    let file = dunce::canonicalize(root.join(relative)).ok()?;
    (file.starts_with(&root) && file.is_file()).then_some(file)
}

/// Whether a deliverable can be opened: a URL always, a file if it exists.
pub fn available(root: &Path, deliverable: &str) -> bool {
    is_url(deliverable) || resolve(root, deliverable).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_paths_and_urls() {
        assert_eq!(normalize(" https://exemple.fr/x ").unwrap(), "https://exemple.fr/x");
        assert_eq!(normalize("rapport\\14\\fiche.pdf").unwrap(), "rapport/14/fiche.pdf");
        assert_eq!(normalize("./rapport//fiche.pdf").unwrap(), "rapport/fiche.pdf");
        assert_eq!(normalize("file:out/video.mp4").unwrap(), "out/video.mp4");
        for bad in [
            "",
            "  ",
            "/etc/passwd",
            "C:/Users/x.pdf",
            "../voisin/secret.txt",
            "a/../../b",
            ".env",
            "x/.git/config",
        ] {
            assert!(normalize(bad).is_err(), "{bad} should be refused");
        }
    }

    #[test]
    fn resolves_only_files_inside_the_project() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("projet");
        std::fs::create_dir_all(root.join("rapport")).unwrap();
        std::fs::create_dir_all(root.join(".git")).unwrap();
        std::fs::write(root.join("rapport/fiche.pdf"), "%PDF").unwrap();
        std::fs::write(root.join(".git/config"), "secret").unwrap();
        std::fs::write(dir.path().join("dehors.txt"), "secret").unwrap();

        assert!(resolve(&root, "rapport/fiche.pdf").is_some());
        assert!(resolve(&root, "rapport").is_none(), "a folder is not a deliverable");
        assert!(resolve(&root, "rapport/absent.pdf").is_none());
        assert!(resolve(&root, ".git/config").is_none());
        assert!(resolve(&root, "../dehors.txt").is_none());
        assert!(available(&root, "https://exemple.fr"));
        assert!(!available(&root, "rapport/absent.pdf"));
    }
}
