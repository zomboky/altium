//! Altium local web server: REST API, SSE stream, file watcher, notifications
//! and the embedded web front. A thin adapter over `altium-core`.

pub mod api;
pub mod assets;
pub mod notifications;
pub mod security;
pub mod views;
pub mod watch;

use std::collections::HashMap;
use std::io;
use std::net::{Ipv4Addr, SocketAddr};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use altium_core::config::GlobalConfig;
use altium_core::git::GitInfo;
use altium_core::registry::Registry;
use serde::Serialize;
use tokio::net::TcpListener;
use tokio::sync::{Notify, broadcast};

/// A change pushed to the dashboard: the client refetches that part.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub struct Change {
    /// `None` for the project list itself.
    pub project: Option<String>,
    /// `projects`, `project`, `status`, `tasks`, `memory`, `activity` or `git`.
    pub scope: &'static str,
}

pub struct AppState {
    pub registry: Registry,
    pub port: u16,
    pub changes: broadcast::Sender<Change>,
    /// Signalled by `POST /api/shutdown` (`altium service uninstall`).
    pub shutdown: Notify,
    git: Mutex<HashMap<PathBuf, (Instant, Option<GitInfo>)>>,
}

impl AppState {
    pub fn new(registry: Registry, port: u16) -> Arc<AppState> {
        let (changes, _) = broadcast::channel(256);
        Arc::new(AppState {
            registry,
            port,
            changes,
            shutdown: Notify::new(),
            git: Mutex::new(HashMap::new()),
        })
    }

    pub fn config(&self) -> GlobalConfig {
        GlobalConfig::load(self.registry.dir()).unwrap_or_default()
    }

    /// Git information of `root`, cached for 2 seconds; runs `git` off the
    /// async threads.
    pub async fn git(&self, root: PathBuf) -> Option<GitInfo> {
        if let Some((at, info)) = self.git.lock().expect("git cache").get(&root)
            && at.elapsed().as_secs() < 2
        {
            return info.clone();
        }
        let key = root.clone();
        let info = tokio::task::spawn_blocking(move || altium_core::git::read(&root))
            .await
            .ok()
            .flatten();
        self.git
            .lock()
            .expect("git cache")
            .insert(key, (Instant::now(), info.clone()));
        info
    }
}

/// Why the server could not start.
#[derive(Debug, thiserror::Error)]
pub enum ServeError {
    #[error("port {0} already in use")]
    PortInUse(u16),
    #[error(transparent)]
    Io(#[from] io::Error),
}

/// Binds the loopback address only: the dashboard is never reachable from
/// another machine. Fails instead of picking another port.
pub async fn bind(port: u16) -> Result<TcpListener, ServeError> {
    let address = SocketAddr::from((Ipv4Addr::LOCALHOST, port));
    TcpListener::bind(address).await.map_err(|e| match e.kind() {
        io::ErrorKind::AddrInUse => ServeError::PortInUse(port),
        // Windows reports a port held by another program as access denied.
        io::ErrorKind::PermissionDenied if cfg!(windows) => ServeError::PortInUse(port),
        _ => ServeError::Io(e),
    })
}

/// Serves the dashboard on `listener` until Ctrl+C or `POST /api/shutdown`.
pub async fn serve(listener: TcpListener, state: Arc<AppState>, notify: bool) -> io::Result<()> {
    watch::spawn(state.clone());
    if notify {
        notifications::spawn(state.clone());
    }
    let app = api::router(state.clone());
    axum::serve(listener, app)
        .with_graceful_shutdown(async move {
            tokio::select! {
                _ = tokio::signal::ctrl_c() => {}
                _ = state.shutdown.notified() => {}
            }
        })
        .await
}
