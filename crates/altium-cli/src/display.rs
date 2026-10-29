//! Formatting shared by the commands.

use std::path::{MAIN_SEPARATOR, Path};

/// Dashboard page of a project, on the configured port.
pub fn dashboard_url(project_id: &str) -> String {
    let port = altium_core::registry::Registry::for_user()
        .ok()
        .and_then(|r| altium_core::config::GlobalConfig::load(r.dir()).ok())
        .map(|c| c.port())
        .unwrap_or(altium_core::config::DEFAULT_PORT);
    format!("http://127.0.0.1:{port}/projects/{project_id}")
}

/// `path` with the home directory shortened to `~`.
pub fn path(path: &Path) -> String {
    match dirs::home_dir().and_then(|home| path.strip_prefix(home).ok().map(Path::to_path_buf)) {
        Some(rest) if rest.as_os_str().is_empty() => "~".to_owned(),
        Some(rest) => format!("~{MAIN_SEPARATOR}{}", rest.display()),
        None => path.display().to_string(),
    }
}

/// `target` with a leading `~` expanded to the home directory, so that the
/// paths shown by [`path`] can be typed back: shells on Windows leave `~` as is.
pub fn expand_home(target: &str) -> String {
    match (target.strip_prefix('~'), dirs::home_dir()) {
        (Some(rest), Some(home)) if rest.is_empty() || rest.starts_with(std::path::is_separator) => {
            format!("{}{rest}", home.display())
        }
        _ => target.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    #[test]
    fn home_is_shortened_then_expanded_back() {
        let home = dirs::home_dir().unwrap();
        let project = home.join("projects").join("foo");
        let shown = path(&project);
        assert_eq!(shown, format!("~{MAIN_SEPARATOR}projects{MAIN_SEPARATOR}foo"));
        assert_eq!(PathBuf::from(expand_home(&shown)), project);

        assert_eq!(path(&home), "~");
        assert_eq!(PathBuf::from(expand_home("~")), home);
    }

    #[test]
    fn only_a_leading_lone_tilde_is_expanded() {
        for target in ["~alice/foo", "foo/~", "01JBDZ4Q6V0M2T8YHC9N3K5R7W"] {
            assert_eq!(expand_home(target), target);
        }
    }
}
