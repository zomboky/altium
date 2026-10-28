//! Altium core: models, `.altium/` store, registry, task ordering, memory graph
//! and session context. No network or UI dependencies, so it can back the web
//! server today and a Tauri app later.

/// Implements `Serialize` and `Deserialize` through `Display` and `FromStr`.
macro_rules! serde_via_str {
    ($ty:ty) => {
        impl serde::Serialize for $ty {
            fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                serializer.collect_str(self)
            }
        }

        impl<'de> serde::Deserialize<'de> for $ty {
            fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                let text = String::deserialize(deserializer)?;
                text.parse().map_err(serde::de::Error::custom)
            }
        }
    };
}

pub mod actor;
pub mod error;
pub mod memory;
pub mod patch;
pub mod project;
pub mod store;
pub mod task;
pub mod time;

pub use error::{Result, StoreError};
pub use store::Store;

/// Version of the Altium binary, shared by every crate.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// `skip_serializing_if` helper: values equal to their default are omitted.
pub(crate) fn is_default<T: Default + PartialEq>(value: &T) -> bool {
    *value == T::default()
}
