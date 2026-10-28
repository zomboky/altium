//! Web front assets, embedded from `web/dist` at build time.
//!
//! When the front has not been built, `index.html` falls back to a small
//! placeholder page so the binary still compiles and serves something useful.

use std::borrow::Cow;

use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "../../web/dist"]
#[allow_missing = true]
struct WebDist;

const PLACEHOLDER_INDEX: &str = include_str!("placeholder.html");

/// Returns the embedded file at `path` (relative to `web/dist`), if any.
pub fn get(path: &str) -> Option<Cow<'static, [u8]>> {
    WebDist::get(path).map(|file| file.data)
}

/// Returns the front entry page, or the placeholder when the front is missing.
pub fn index_html() -> Cow<'static, [u8]> {
    get("index.html").unwrap_or(Cow::Borrowed(PLACEHOLDER_INDEX.as_bytes()))
}

/// Whether the real front was embedded (as opposed to the placeholder).
pub fn has_front() -> bool {
    WebDist::get("index.html").is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn index_is_never_empty() {
        assert!(!index_html().is_empty());
    }

    #[test]
    fn front_is_embedded_when_built() {
        let built = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../web/dist/index.html")
            .exists();
        assert_eq!(has_front(), built);

        let index = String::from_utf8(index_html().into_owned()).unwrap();
        if built {
            assert!(index.contains(r#"<div id="app">"#));
        } else {
            assert_eq!(index, PLACEHOLDER_INDEX);
        }
    }
}
