//! The memory graph: validation, compact index, progressive loading along
//! typed links, search and relevance ranking.

use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::fmt::Write as _;
use std::path::Path;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::memory::{Link, LinkTarget, LinkType, MemoryNode, NodeKind, is_valid_node_id};
use crate::task::TaskId;
use crate::time::Timestamp;

/// Longest node description, in characters.
pub const MAX_DESC_CHARS: usize = 120;

/// Node body size above which splitting is suggested, in estimated tokens.
pub const NODE_TOKEN_BUDGET: usize = 2000;

/// Rough token count used for every budget: one token per 3.5 characters.
pub fn estimate_tokens(text: &str) -> usize {
    (text.chars().count() * 2).div_ceil(7)
}

/// Checks a node before it is saved. Returns warnings that do not prevent
/// saving, or the reason it cannot be saved.
pub fn validate(node: &MemoryNode) -> Result<Vec<String>, String> {
    if !is_valid_node_id(&node.id) {
        return Err(format!(
            "invalid node id `{}`: use lowercase letters, digits and dashes (e.g. auth-jwt)",
            node.id
        ));
    }
    if node.meta.title.trim().is_empty() {
        return Err("a node needs a title".into());
    }
    let desc = node.meta.description.trim();
    if desc.is_empty() {
        return Err("a node needs a one-line description (desc)".into());
    }
    if desc.contains('\n') {
        return Err("desc must be a single line".into());
    }
    let chars = desc.chars().count();
    if chars > MAX_DESC_CHARS {
        return Err(format!(
            "desc is {chars} characters, the limit is {MAX_DESC_CHARS}: shorten it and move details to the body"
        ));
    }
    let mut warnings = Vec::new();
    let tokens = estimate_tokens(&node.body);
    if tokens > NODE_TOKEN_BUDGET {
        warnings.push(format!(
            "body is ~{tokens} tokens (budget {NODE_TOKEN_BUDGET}): consider splitting it into smaller nodes linked with part_of"
        ));
    }
    Ok(warnings)
}

/// Last access and load count of a node, kept in `runtime/memory-access.json`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Access {
    pub last: Timestamp,
    pub count: u32,
}

pub type AccessLog = BTreeMap<String, Access>;

/// Records that `ids` were loaded at `now`.
pub fn record_access(log: &mut AccessLog, ids: &[String], now: Timestamp) {
    for id in ids {
        let access = log.entry(id.clone()).or_insert(Access { last: now, count: 0 });
        access.last = now;
        access.count += 1;
    }
}

/// A link seen from a node: outgoing (`->`) or incoming (`<-`).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Neighbor {
    pub from: String,
    pub kind: LinkType,
    pub incoming: bool,
    pub target: LinkTarget,
}

/// The nodes of a project, with their incoming links.
#[derive(Debug, Default)]
pub struct Graph {
    nodes: BTreeMap<String, MemoryNode>,
    incoming: HashMap<String, Vec<(String, LinkType)>>,
}

/// Where a link may point, to detect broken links.
pub struct World<'a> {
    pub tasks: &'a HashSet<TaskId>,
    /// Project root, against which `file:` links resolve.
    pub root: &'a Path,
}

impl Graph {
    pub fn new(nodes: impl IntoIterator<Item = MemoryNode>) -> Graph {
        let nodes: BTreeMap<String, MemoryNode> = nodes.into_iter().map(|n| (n.id.clone(), n)).collect();
        let mut incoming: HashMap<String, Vec<(String, LinkType)>> = HashMap::new();
        for node in nodes.values() {
            for link in &node.meta.links {
                if let LinkTarget::Node(to) = &link.to {
                    incoming
                        .entry(to.clone())
                        .or_default()
                        .push((node.id.clone(), link.kind));
                }
            }
        }
        Graph { nodes, incoming }
    }

    pub fn get(&self, id: &str) -> Option<&MemoryNode> {
        self.nodes.get(id)
    }

    pub fn nodes(&self) -> impl Iterator<Item = &MemoryNode> {
        self.nodes.values()
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// Nodes linking to `id`, with the link type.
    pub fn incoming(&self, id: &str) -> &[(String, LinkType)] {
        self.incoming.get(id).map(Vec::as_slice).unwrap_or_default()
    }

    /// The node that replaces `id` through `supersedes`, if any.
    pub fn superseded_by(&self, id: &str) -> Option<&str> {
        self.incoming(id)
            .iter()
            .find(|(_, kind)| *kind == LinkType::Supersedes)
            .map(|(from, _)| from.as_str())
    }

    /// The canonical node `id` is `equivalent_to`, if any.
    pub fn canonical(&self, id: &str) -> Option<&str> {
        self.nodes.get(id)?.meta.links.iter().find_map(|link| match &link.to {
            LinkTarget::Node(to) if link.kind == LinkType::EquivalentTo && self.nodes.contains_key(to) => {
                Some(to.as_str())
            }
            _ => None,
        })
    }

    /// Nodes not replaced by another one.
    pub fn active(&self) -> impl Iterator<Item = &MemoryNode> {
        self.nodes.values().filter(|n| self.superseded_by(&n.id).is_none())
    }

    /// Links of `node` whose target does not exist.
    pub fn broken_links<'n>(&self, node: &'n MemoryNode, world: &World) -> Vec<&'n Link> {
        node.meta
            .links
            .iter()
            .filter(|link| match &link.to {
                LinkTarget::Node(id) => !self.nodes.contains_key(id),
                LinkTarget::Task(id) => !world.tasks.contains(id),
                LinkTarget::File(path) => !world.root.join(path.trim_start_matches('/')).exists(),
                LinkTarget::Url(_) => false,
            })
            .collect()
    }

    /// Active nodes, most relevant first: pinned, then by recency and
    /// frequency of use.
    pub fn ranked(&self, access: &AccessLog, now: DateTime<Utc>) -> Vec<&MemoryNode> {
        let mut nodes: Vec<&MemoryNode> = self.active().collect();
        let score = |n: &MemoryNode| relevance(n, access.get(&n.id), now);
        nodes.sort_by(|a, b| {
            b.meta
                .pinned
                .cmp(&a.meta.pinned)
                .then(score(b).total_cmp(&score(a)))
                .then(a.id.cmp(&b.id))
        });
        nodes
    }

    /// Loads `ids` with their body. `follow` link types are followed from
    /// loaded nodes up to `depth` levels, loading the bodies found on the way.
    pub fn load(&self, ids: &[String], follow: &[LinkType], depth: u32) -> Loaded<'_> {
        let mut loaded = Loaded::default();
        let mut queue: VecDeque<(String, u32)> = VecDeque::new();
        let mut seen = HashSet::new();
        for id in ids {
            let Some(node) = self.nodes.get(id) else {
                loaded.missing.push(id.clone());
                continue;
            };
            let target = match self.canonical(id) {
                Some(canonical) => {
                    loaded.notes.push(format!(
                        "{id} is equivalent_to {canonical} (canonical): showing {canonical}"
                    ));
                    canonical.to_owned()
                }
                None => node.id.clone(),
            };
            if seen.insert(target.clone()) {
                queue.push_back((target, 0));
            }
        }

        while let Some((id, level)) = queue.pop_front() {
            let node = &self.nodes[&id];
            loaded.depth = loaded.depth.max(level);
            if let Some(newer) = self.superseded_by(&id) {
                loaded
                    .notes
                    .push(format!("WARNING: {id} is obsolete, superseded by {newer}"));
            }
            loaded.nodes.push(node);
            for link in &node.meta.links {
                if let LinkTarget::Node(to) = &link.to
                    && follow.contains(&link.kind)
                    && self.nodes.contains_key(to)
                    && !seen.contains(to)
                {
                    if level < depth {
                        seen.insert(to.clone());
                        queue.push_back((to.clone(), level + 1));
                    } else {
                        loaded.beyond_depth += 1;
                    }
                }
            }
        }

        let in_load: HashSet<&str> = loaded.nodes.iter().map(|n| n.id.as_str()).collect();
        for node in &loaded.nodes {
            for link in &node.meta.links {
                if link.kind == LinkType::Contradicts {
                    loaded
                        .notes
                        .push(format!("WARNING: {} contradicts {}", node.id, link.to));
                }
                let shown = matches!(&link.to, LinkTarget::Node(to) if in_load.contains(to.as_str()));
                if !shown {
                    loaded.neighbors.push(Neighbor {
                        from: node.id.clone(),
                        kind: link.kind,
                        incoming: false,
                        target: link.to.clone(),
                    });
                }
            }
            for (from, kind) in self.incoming(&node.id) {
                if *kind == LinkType::Contradicts && !in_load.contains(from.as_str()) {
                    loaded.notes.push(format!("WARNING: {from} contradicts {}", node.id));
                }
                if !in_load.contains(from.as_str()) {
                    loaded.neighbors.push(Neighbor {
                        from: node.id.clone(),
                        kind: *kind,
                        incoming: true,
                        target: LinkTarget::Node(from.clone()),
                    });
                }
            }
        }
        loaded.notes.dedup();
        loaded
    }

    /// Nodes matching `query`, best first.
    pub fn search(&self, query: &Search, access: &AccessLog, now: DateTime<Utc>) -> Vec<&MemoryNode> {
        let terms: Vec<String> = query
            .text
            .as_deref()
            .unwrap_or_default()
            .split_whitespace()
            .map(str::to_lowercase)
            .collect();
        let path = query.path.as_deref().map(normalize_path);
        let mut found: Vec<(f64, &MemoryNode)> = Vec::new();
        for node in self.nodes.values() {
            let meta = &node.meta;
            if query.kind.is_some_and(|k| k != meta.kind)
                || query
                    .tag
                    .as_ref()
                    .is_some_and(|t| !meta.tags.contains(&t.to_lowercase()))
            {
                continue;
            }
            if let Some(path) = &path {
                let refers = meta.links.iter().any(|link| match &link.to {
                    LinkTarget::File(file) => {
                        let file = normalize_path(file);
                        path == &file || path.starts_with(&format!("{}/", file.trim_end_matches('/')))
                    }
                    _ => false,
                });
                if !refers {
                    continue;
                }
            }
            let mut score = 0.0;
            for term in &terms {
                let mut hit = 0.0;
                if node.id.contains(term.as_str()) || meta.title.to_lowercase().contains(term.as_str()) {
                    hit += 5.0;
                }
                if meta.tags.iter().any(|t| t.contains(term.as_str())) {
                    hit += 4.0;
                }
                if meta.description.to_lowercase().contains(term.as_str()) {
                    hit += 3.0;
                }
                if node.body.to_lowercase().contains(term.as_str()) {
                    hit += 1.0;
                }
                if hit == 0.0 {
                    score = -1.0;
                    break;
                }
                score += hit;
            }
            if score < 0.0 {
                continue;
            }
            if self.superseded_by(&node.id).is_some() {
                score -= 0.5;
            }
            let bonus = relevance(node, access.get(&node.id), now) / 1000.0 + if meta.pinned { 0.1 } else { 0.0 };
            found.push((score + bonus, node));
        }
        found.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.id.cmp(&b.1.id)));
        found.into_iter().map(|(_, n)| n).collect()
    }

    /// One index line: `auth-jwt [decision] Auth par JWT: description`,
    /// with `*` after the kind when pinned.
    pub fn index_line(&self, node: &MemoryNode) -> String {
        let pin = if node.meta.pinned { "*" } else { "" };
        let mut line = format!(
            "{} [{}{pin}] {}: {}",
            node.id,
            kind_name(node.meta.kind),
            node.meta.title,
            node.meta.description
        );
        if let Some(newer) = self.superseded_by(&node.id) {
            let _ = write!(line, " (superseded by {newer})");
        }
        line
    }
}

/// Criteria of [`Graph::search`]; empty criteria match every node.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Search {
    pub text: Option<String>,
    pub kind: Option<NodeKind>,
    pub tag: Option<String>,
    /// A project path; matches nodes linking to it or to a parent directory.
    pub path: Option<String>,
}

/// Result of [`Graph::load`].
#[derive(Debug, Default)]
pub struct Loaded<'a> {
    /// Nodes whose body is returned, in loading order.
    pub nodes: Vec<&'a MemoryNode>,
    /// Links of loaded nodes to things not loaded.
    pub neighbors: Vec<Neighbor>,
    /// Redirections, obsolescence and contradiction warnings.
    pub notes: Vec<String>,
    pub missing: Vec<String>,
    /// Deepest level loaded.
    pub depth: u32,
    /// Followed links not loaded because of the depth limit.
    pub beyond_depth: usize,
}

impl Loaded<'_> {
    pub fn ids(&self) -> Vec<String> {
        self.nodes.iter().map(|n| n.id.clone()).collect()
    }
}

/// Score of a node in the memory index: frequent and recent use weigh most.
fn relevance(node: &MemoryNode, access: Option<&Access>, now: DateTime<Utc>) -> f64 {
    let last = access.map_or(node.meta.updated, |a| a.last.max(node.meta.updated));
    let days = (now - last.as_datetime()).num_minutes().max(0) as f64 / 1440.0;
    let count = access.map_or(0, |a| a.count) as f64;
    (1.0 + count) / (1.0 + days)
}

pub fn kind_name(kind: NodeKind) -> &'static str {
    match kind {
        NodeKind::Decision => "decision",
        NodeKind::Fact => "fact",
        NodeKind::Convention => "convention",
        NodeKind::Component => "component",
        NodeKind::Gotcha => "gotcha",
        NodeKind::Howto => "howto",
        NodeKind::Reference => "reference",
    }
}

pub fn link_name(kind: LinkType) -> &'static str {
    match kind {
        LinkType::DependsOn => "depends_on",
        LinkType::Implies => "implies",
        LinkType::EquivalentTo => "equivalent_to",
        LinkType::PartOf => "part_of",
        LinkType::Supersedes => "supersedes",
        LinkType::Contradicts => "contradicts",
        LinkType::RelatesTo => "relates_to",
    }
}

fn normalize_path(path: &str) -> String {
    path.replace('\\', "/")
        .trim_start_matches("./")
        .trim_start_matches('/')
        .to_owned()
}

/// Default memory language when `memory.lang` is not set.
pub const DEFAULT_LANG: &str = "en";

/// The instruction telling Claude which language to write memory in.
pub fn language_instruction(lang: &str) -> String {
    let name = match lang.to_lowercase().as_str() {
        "en" => "English",
        "fr" => "French",
        "de" => "German",
        "es" => "Spanish",
        "it" => "Italian",
        "pt" => "Portuguese",
        "nl" => "Dutch",
        "ja" => "Japanese",
        "zh" => "Chinese",
        _ => lang,
    };
    format!("Write memory nodes (title, desc, body) in {name}; keep ids, kinds and link types in English.")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::NodeMeta;

    fn ts() -> Timestamp {
        "2026-10-28T10:00:00Z".parse().unwrap()
    }

    fn node(id: &str, links: &[(&str, LinkType)]) -> MemoryNode {
        MemoryNode {
            id: id.into(),
            meta: NodeMeta {
                kind: NodeKind::Decision,
                title: format!("Title {id}"),
                description: format!("About {id}"),
                tags: vec![],
                pinned: false,
                links: links
                    .iter()
                    .map(|(to, kind)| Link {
                        to: to.parse().unwrap(),
                        kind: *kind,
                        note: None,
                    })
                    .collect(),
                created: ts(),
                updated: ts(),
            },
            body: format!("Body of {id}.\n"),
        }
    }

    fn now() -> DateTime<Utc> {
        ts().as_datetime()
    }

    #[test]
    fn validation() {
        let mut n = node("auth-jwt", &[]);
        assert_eq!(validate(&n).unwrap(), Vec::<String>::new());
        n.meta.description = "x".repeat(200);
        let err = validate(&n).unwrap_err();
        assert!(err.contains("120"), "{err}");
        n.meta.description = "ok".into();
        n.body = "word ".repeat(3000);
        assert!(validate(&n).unwrap()[0].contains("part_of"));
        n.id = "Auth JWT".into();
        assert!(validate(&n).is_err());
    }

    #[test]
    fn index_skips_superseded_nodes() {
        let mut nodes: Vec<MemoryNode> = (0..80).map(|i| node(&format!("n{i}"), &[])).collect();
        for i in 0..5 {
            nodes[70 + i] = node(&format!("n{}", 70 + i), &[(&format!("n{i}"), LinkType::Supersedes)]);
        }
        let graph = Graph::new(nodes);
        let index: Vec<String> = graph
            .ranked(&AccessLog::new(), now())
            .iter()
            .map(|n| graph.index_line(n))
            .collect();
        assert_eq!(index.len(), 75);
        assert!(index.iter().all(|line| !line.contains("Body")));
    }

    #[test]
    fn incoming_links_and_broken_links() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir(root.path().join("src")).unwrap();
        let graph = Graph::new([
            node(
                "api-refresh-token",
                &[
                    ("auth-jwt", LinkType::DependsOn),
                    ("file:src/", LinkType::RelatesTo),
                    ("file:src/gone.rs", LinkType::RelatesTo),
                    ("task:T-9", LinkType::Implies),
                    ("ghost", LinkType::RelatesTo),
                ],
            ),
            node("auth-jwt", &[]),
        ]);
        assert_eq!(
            graph.incoming("auth-jwt"),
            &[("api-refresh-token".to_owned(), LinkType::DependsOn)]
        );
        let tasks = HashSet::from([TaskId(1)]);
        let world = World {
            tasks: &tasks,
            root: root.path(),
        };
        let broken: Vec<String> = graph
            .broken_links(graph.get("api-refresh-token").unwrap(), &world)
            .iter()
            .map(|l| l.to.to_string())
            .collect();
        assert_eq!(broken, ["file:src/gone.rs", "task:T-9", "ghost"]);
    }

    #[test]
    fn simple_load_returns_neighbors_as_index_lines() {
        let graph = Graph::new([
            node("api-refresh-token", &[("auth-jwt", LinkType::DependsOn)]),
            node("auth-jwt", &[("crypto", LinkType::DependsOn)]),
            node("crypto", &[]),
        ]);
        let loaded = graph.load(&["api-refresh-token".into()], &[], 0);
        assert_eq!(loaded.ids(), ["api-refresh-token"]);
        assert_eq!(loaded.neighbors[0].target.to_string(), "auth-jwt");
        assert_eq!(loaded.neighbors[0].kind, LinkType::DependsOn);

        let deep = graph.load(&["api-refresh-token".into()], &[LinkType::DependsOn], 2);
        assert_eq!(deep.ids(), ["api-refresh-token", "auth-jwt", "crypto"]);
        assert_eq!(deep.depth, 2);
        let shallow = graph.load(&["api-refresh-token".into()], &[LinkType::DependsOn], 1);
        assert_eq!((shallow.depth, shallow.beyond_depth), (1, 1));
    }

    #[test]
    fn link_semantics_on_load() {
        let graph = Graph::new([
            node("auth-jwt", &[("auth-session", LinkType::Supersedes)]),
            node("auth-session", &[]),
            node("login-flow", &[("auth-jwt", LinkType::EquivalentTo)]),
            node("cache-a", &[("cache-b", LinkType::Contradicts)]),
            node("cache-b", &[]),
        ]);
        let obsolete = graph.load(&["auth-session".into()], &[], 0);
        assert!(obsolete.notes[0].contains("auth-session is obsolete, superseded by auth-jwt"));

        let redirected = graph.load(&["login-flow".into()], &[], 0);
        assert_eq!(redirected.ids(), ["auth-jwt"]);
        assert!(redirected.notes[0].contains("equivalent_to auth-jwt"));

        let both = graph.load(&["cache-a".into(), "cache-b".into()], &[], 0);
        assert!(both.notes.iter().any(|n| n == "WARNING: cache-a contradicts cache-b"));
    }

    #[test]
    fn search_by_text_path_and_rank() {
        let mut jwt = node("auth-jwt", &[("file:src/auth/", LinkType::RelatesTo)]);
        jwt.meta.description = "Tokens with refresh rotation".into();
        let mut pinned = node("conventions-code", &[]);
        pinned.meta.pinned = true;
        let graph = Graph::new([jwt, node("db", &[]), pinned]);
        let none = AccessLog::new();

        let by_text = graph.search(
            &Search {
                text: Some("refresh".into()),
                ..Search::default()
            },
            &none,
            now(),
        );
        assert_eq!(by_text.iter().map(|n| n.id.as_str()).collect::<Vec<_>>(), ["auth-jwt"]);

        let by_path = graph.search(
            &Search {
                path: Some("src/auth/middleware.rs".into()),
                ..Search::default()
            },
            &none,
            now(),
        );
        assert_eq!(by_path[0].id, "auth-jwt");

        let mut log = AccessLog::new();
        record_access(&mut log, &["db".into(), "db".into()], ts());
        let ranked: Vec<&str> = graph.ranked(&log, now()).iter().map(|n| n.id.as_str()).collect();
        assert_eq!(ranked, ["conventions-code", "db", "auth-jwt"]);
    }

    #[test]
    fn language_instruction_follows_memory_lang() {
        assert!(language_instruction("en").contains("in English"));
        assert!(language_instruction("fr").contains("in French"));
    }
}
