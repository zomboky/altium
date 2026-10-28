//! Formatting shared by the commands.

use std::path::Path;

/// Default port of `altium serve`.
pub const DEFAULT_PORT: u16 = 7420;

/// Dashboard page of a project.
pub fn dashboard_url(project_id: &str) -> String {
    format!("http://127.0.0.1:{DEFAULT_PORT}/projects/{project_id}")
}

/// `path` with the home directory shortened to `~`.
pub fn path(path: &Path) -> String {
    match dirs::home_dir().and_then(|home| path.strip_prefix(home).ok().map(Path::to_path_buf)) {
        Some(rest) if rest.as_os_str().is_empty() => "~".to_owned(),
        Some(rest) => format!("~/{}", rest.display()),
        None => path.display().to_string(),
    }
}
