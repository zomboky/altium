//! Memory nodes: one Markdown file per node in `memory/<id>.md`, with YAML
//! front matter holding the metadata and links, followed by the body.

use std::fmt;
use std::path::Path;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::error::{Result, StoreError};
use crate::is_default;
use crate::task::TaskId;
use crate::time::Timestamp;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeKind {
    Decision,
    Fact,
    Convention,
    Component,
    Gotcha,
    Howto,
    Reference,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LinkType {
    DependsOn,
    Implies,
    EquivalentTo,
    PartOf,
    Supersedes,
    Contradicts,
    RelatesTo,
}

/// What a link points at. Stored as `auth-jwt` (a node), `task:T-12`,
/// `file:src/auth/` or an `http(s)://` URL.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum LinkTarget {
    Node(String),
    Task(TaskId),
    File(String),
    Url(String),
}

impl fmt::Display for LinkTarget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LinkTarget::Node(id) => f.write_str(id),
            LinkTarget::Task(id) => write!(f, "task:{id}"),
            LinkTarget::File(path) => write!(f, "file:{path}"),
            LinkTarget::Url(url) => f.write_str(url),
        }
    }
}

impl FromStr for LinkTarget {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.starts_with("https://") || s.starts_with("http://") {
            Ok(LinkTarget::Url(s.to_owned()))
        } else if let Some(id) = s.strip_prefix("task:") {
            id.parse().map(LinkTarget::Task)
        } else if let Some(path) = s.strip_prefix("file:") {
            match path {
                "" => Err("empty file link".to_owned()),
                _ => Ok(LinkTarget::File(path.to_owned())),
            }
        } else if is_valid_node_id(s) {
            Ok(LinkTarget::Node(s.to_owned()))
        } else {
            Err(format!("invalid link target `{s}`"))
        }
    }
}

serde_via_str!(LinkTarget);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Link {
    pub to: LinkTarget,
    #[serde(rename = "type")]
    pub kind: LinkType,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// Front matter of a node file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NodeMeta {
    pub kind: NodeKind,
    pub title: String,
    /// One-line description shown in the memory index.
    #[serde(rename = "desc")]
    pub description: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    #[serde(default, skip_serializing_if = "is_default")]
    pub pinned: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub links: Vec<Link>,
    pub created: Timestamp,
    pub updated: Timestamp,
}

/// A node; its id is the file name without `.md`.
#[derive(Debug, Clone, PartialEq)]
pub struct MemoryNode {
    pub id: String,
    pub meta: NodeMeta,
    pub body: String,
}

/// Node ids are slugs: lowercase letters, digits and inner dashes.
pub fn is_valid_node_id(id: &str) -> bool {
    !id.is_empty()
        && !id.starts_with('-')
        && !id.ends_with('-')
        && id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

pub fn parse_node(id: &str, text: &str, file: &Path) -> Result<MemoryNode> {
    let rest = text
        .strip_prefix("---\n")
        .or_else(|| text.strip_prefix("---\r\n"))
        .ok_or_else(|| StoreError::invalid(file, Some(1), "missing front matter (expected `---` on the first line)"))?;

    let mut offset = 0;
    let mut fence = None;
    for line in rest.split_inclusive('\n') {
        if line.trim_end() == "---" {
            fence = Some((offset, offset + line.len()));
            break;
        }
        offset += line.len();
    }
    let (yaml_end, body_start) = fence.ok_or_else(|| StoreError::invalid(file, None, "unterminated front matter"))?;

    // Line numbers are shifted by one for the opening `---`.
    let meta: NodeMeta = serde_yaml_ng::from_str(&rest[..yaml_end])
        .map_err(|e| StoreError::invalid(file, e.location().map(|l| l.line() + 1), e))?;
    let body = &rest[body_start..];
    let body = body
        .strip_prefix('\n')
        .or_else(|| body.strip_prefix("\r\n"))
        .unwrap_or(body);

    Ok(MemoryNode {
        id: id.to_owned(),
        meta,
        body: body.to_owned(),
    })
}

pub fn render_node(node: &MemoryNode) -> String {
    let yaml = serde_yaml_ng::to_string(&node.meta).expect("node metadata always serializes");
    format!("---\n{yaml}---\n\n{}", node.body)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node() -> MemoryNode {
        MemoryNode {
            id: "api-refresh-token".into(),
            meta: NodeMeta {
                kind: NodeKind::Decision,
                title: "Refresh tokens".into(),
                description: "Refresh tokens rotate on every use and live 30 days.".into(),
                tags: vec!["auth".into()],
                pinned: false,
                links: vec![
                    Link {
                        to: LinkTarget::Node("auth-jwt".into()),
                        kind: LinkType::DependsOn,
                        note: None,
                    },
                    Link {
                        to: LinkTarget::File("src/auth/".into()),
                        kind: LinkType::RelatesTo,
                        note: Some("middleware".into()),
                    },
                    Link {
                        to: LinkTarget::Task(TaskId(12)),
                        kind: LinkType::Implies,
                        note: None,
                    },
                ],
                created: "2026-10-28T10:00:00Z".parse().unwrap(),
                updated: "2026-10-28T11:00:00Z".parse().unwrap(),
            },
            body: "Rotation happens in `refresh()`.\n\n- Revoke the old token.\n".into(),
        }
    }

    #[test]
    fn round_trips() {
        let text = render_node(&node());
        assert!(text.starts_with("---\nkind: decision\ntitle: Refresh tokens\n"));
        assert!(text.contains("- to: auth-jwt\n  type: depends_on\n"));
        assert!(text.contains("- to: task:T-12\n"));
        assert!(!text.contains("pinned"));
        assert_eq!(
            parse_node("api-refresh-token", &text, Path::new("x.md")).unwrap(),
            node()
        );
    }

    #[test]
    fn empty_body_round_trips() {
        let mut n = node();
        n.body.clear();
        assert_eq!(parse_node(&n.id, &render_node(&n), Path::new("x.md")).unwrap(), n);
    }

    #[test]
    fn link_targets_parse() {
        assert_eq!(
            "https://docs.rs".parse::<LinkTarget>().unwrap(),
            LinkTarget::Url("https://docs.rs".into())
        );
        assert!("task:12".parse::<LinkTarget>().is_err());
        assert!("Auth JWT".parse::<LinkTarget>().is_err());
    }

    #[test]
    fn errors_point_at_the_file_line() {
        let file = Path::new("x.md");
        assert!(matches!(
            parse_node("x", "no front matter", file),
            Err(StoreError::Invalid { line: Some(1), .. })
        ));

        let text = "---\nkind: decision\ntitle: X\ndesc: Y\ncolor: red\ncreated: 2026-10-28T10:00:00Z\nupdated: 2026-10-28T10:00:00Z\n---\n";
        let err = parse_node("x", text, file).unwrap_err();
        assert!(matches!(err, StoreError::Invalid { line: Some(5), .. }), "{err}");
    }

    #[test]
    fn node_ids_are_slugs() {
        assert!(is_valid_node_id("auth-jwt2"));
        for bad in ["", "-auth", "auth-", "Auth", "auth_jwt", "../etc"] {
            assert!(!is_valid_node_id(bad), "{bad}");
        }
    }
}
