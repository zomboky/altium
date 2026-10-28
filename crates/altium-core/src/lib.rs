//! Altium core: models, `.altium/` store, registry, task ordering, memory graph
//! and session context. No network or UI dependencies, so it can back the web
//! server today and a Tauri app later.

/// Version of the Altium binary, shared by every crate.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
