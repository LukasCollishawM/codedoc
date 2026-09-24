use std::collections::BTreeMap;
use std::fmt;

use codedoc_core::{
    AnchorId, Canonical, ContentFingerprint, ContextFingerprint, RepoPath, StructuralFingerprint,
    SymbolPath,
};
use codedoc_lang::Adapter;
use serde::{Deserialize, Serialize};
use tree_sitter::Node;

use crate::fingerprint;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceRange {
    pub start_line: u32,
    pub start_column: u32,
    pub end_line: u32,
    pub end_column: u32,
}

impl SourceRange {
    pub fn of(node: Node<'_>) -> Self {
        let start = node.start_position();
        let end = node.end_position();
        SourceRange {
            start_line: start.row as u32 + 1,
            start_column: start.column as u32 + 1,
            end_line: end.row as u32 + 1,
            end_column: end.column as u32 + 1,
        }
    }

    pub fn contains_line(&self, line: u32) -> bool {
        (self.start_line..=self.end_line).contains(&line)
    }
}

impl fmt::Display for SourceRange {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{}:{}-{}:{}",
            self.start_line, self.start_column, self.end_line, self.end_column
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeStep {
    pub kind: String,
    pub ordinal: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct NodePath(Vec<NodeStep>);

impl NodePath {
    pub fn of(node: Node<'_>, stop_at: Option<Node<'_>>) -> Self {
        let mut steps = Vec::new();
        let mut current = node;
        loop {
            if let Some(boundary) = stop_at
                && current.id() == boundary.id()
            {
                break;
            }
            let Some(parent) = current.parent() else { break };
            let mut ordinal = 0u32;
            let mut cursor = parent.walk();
            for sibling in parent.named_children(&mut cursor) {
                if sibling.id() == current.id() {
                    break;
                }
                if sibling.kind() == current.kind() {
                    ordinal += 1;
                }
            }
            steps.push(NodeStep { kind: current.kind().to_owned(), ordinal });
            current = parent;
        }
        steps.reverse();
        NodePath(steps)
    }

    pub fn steps(&self) -> &[NodeStep] {
        &self.0
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn descend<'tree>(&self, root: Node<'tree>) -> Option<Node<'tree>> {
        let mut current = root;
        for step in &self.0 {
            let mut seen = 0u32;
            let mut matched = None;
            let mut cursor = current.walk();
            for child in current.named_children(&mut cursor) {
                if child.kind() == step.kind {
                    if seen == step.ordinal {
                        matched = Some(child);
                        break;
                    }
                    seen += 1;
                }
            }
            current = matched?;
        }
        Some(current)
    }
}

impl fmt::Display for NodePath {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let rendered: Vec<String> =
            self.0.iter().map(|step| format!("{}[{}]", step.kind, step.ordinal)).collect();
        formatter.write_str(&rendered.join("/"))
    }
}

#[derive(Debug, Clone, Default)]
pub struct SymbolTable {
    declarations: BTreeMap<(String, String), Vec<usize>>,
}

impl SymbolTable {
    pub fn build(root: Node<'_>, adapter: &Adapter, source: &str) -> Self {
        let mut declarations: BTreeMap<(String, String), Vec<usize>> = BTreeMap::new();
        let mut cursor = root.walk();
        let mut descending = true;
        loop {
            if descending {
                let node = cursor.node();
                if adapter.declares_symbol(node.kind())
                    && let Some(path) = symbol_path_of(node, adapter, source)
                {
                    declarations
                        .entry((path.to_string(), node.kind().to_owned()))
                        .or_default()
                        .push(node.id());
                }
                if cursor.goto_first_child() {
                    continue;
                }
                descending = false;
            }
            if cursor.goto_next_sibling() {
                descending = true;
                continue;
            }
            if !cursor.goto_parent() {
                break;
            }
        }
        SymbolTable { declarations }
    }

    pub fn cardinality(&self, symbol: &str, kind: &str) -> u32 {
        self.declarations
            .get(&(symbol.to_owned(), kind.to_owned()))
            .map(|found| found.len() as u32)
            .unwrap_or(0)
    }

    pub fn ordinal_of(&self, symbol: &str, kind: &str, node: Node<'_>) -> u32 {
        let key = (symbol.to_owned(), kind.to_owned());
        let mut current = Some(node);
        while let Some(candidate) = current {
            if let Some(found) = self.declarations.get(&key)
                && let Some(position) =
                    found.iter().position(|identity| *identity == candidate.id())
            {
                return position as u32;
            }
            current = candidate.parent();
        }
        0
    }

    pub fn owning_kind(&self, symbol: &str, node: Node<'_>) -> Option<String> {
        let mut current = Some(node);
        while let Some(candidate) = current {
            for ((named, kind), members) in &self.declarations {
                if named == symbol && members.contains(&candidate.id()) {
                    return Some(kind.clone());
                }
            }
            current = candidate.parent();
        }
        None
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Anchor {
    pub file: RepoPath,
    pub language: String,
    pub node_kind: String,
    pub symbol: Option<SymbolPath>,
    pub node_path: NodePath,
    pub structural: StructuralFingerprint,
    pub content: ContentFingerprint,
    pub preceding: ContextFingerprint,
    pub following: ContextFingerprint,
    pub shape: BTreeMap<String, u32>,
    #[serde(default)]
    pub context_siblings: u32,
    #[serde(default)]
    pub symbol_kind: String,
    #[serde(default = "one_declaration")]
    pub symbol_cardinality: u32,
    #[serde(default)]
    pub symbol_ordinal: u32,
    pub range: SourceRange,
}

impl Anchor {
    pub fn capture(file: RepoPath, adapter: &Adapter, source: &str, node: Node<'_>) -> Self {
        let mut root = node;
        while let Some(parent) = root.parent() {
            root = parent;
        }
        let digests = fingerprint::compute_all(root, adapter, source);
        let symbols = SymbolTable::build(root, adapter, source);
        Anchor::capture_with(file, adapter, source, node, &digests, &symbols)
    }

    pub fn capture_with(
        file: RepoPath,
        adapter: &Adapter,
        source: &str,
        node: Node<'_>,
        digests: &fingerprint::Digests,
        symbols: &SymbolTable,
    ) -> Self {
        let symbol = symbol_path_of(node, adapter, source);
        let rendered = symbol.as_ref().map(ToString::to_string).unwrap_or_default();
        let owning = symbols.owning_kind(&rendered, node).unwrap_or_default();
        let symbol_root = symbol.as_ref().and_then(|_| enclosing_declaration(node, adapter));
        Anchor {
            file,
            language: adapter.name().to_owned(),
            node_kind: node.kind().to_owned(),
            symbol,
            node_path: NodePath::of(node, symbol_root),
            structural: fingerprint::structural_of(node, digests),
            content: fingerprint::content_of(node, digests),
            preceding: fingerprint::preceding_context_with(node, adapter, digests),
            following: fingerprint::following_context_with(node, adapter, digests),
            shape: fingerprint::shape_histogram(node, adapter),
            context_siblings: fingerprint::context_siblings(node, adapter),
            symbol_kind: owning.clone(),
            symbol_cardinality: symbols.cardinality(&rendered, &owning),
            symbol_ordinal: symbols.ordinal_of(&rendered, &owning, node),
            range: SourceRange::of(node),
        }
    }

    pub fn id(&self) -> AnchorId {
        let canonical =
            Canonical::from_serializable(self).map(|value| value.encode()).unwrap_or_default();
        AnchorId::of(&canonical)
    }

    pub fn is_whole_declaration(&self) -> bool {
        self.node_path.is_empty()
    }
}

fn one_declaration() -> u32 {
    1
}

fn enclosing_declaration<'tree>(node: Node<'tree>, adapter: &Adapter) -> Option<Node<'tree>> {
    let mut current = Some(node);
    while let Some(candidate) = current {
        if adapter.declares_symbol(candidate.kind()) {
            return Some(candidate);
        }
        current = candidate.parent();
    }
    None
}

pub fn symbol_path_of(node: Node<'_>, adapter: &Adapter, source: &str) -> Option<SymbolPath> {
    let mut names = Vec::new();
    let mut current = Some(node);
    while let Some(candidate) = current {
        if let Some(name) = adapter.declaration_name(candidate, source) {
            names.push(name.into_owned());
        }
        current = candidate.parent();
    }
    if names.is_empty() {
        return None;
    }
    names.reverse();
    SymbolPath::parse(&format!("{}://{}", adapter.name(), names.join("/"))).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use codedoc_lang::Registry;

    fn capture_first_function(source: &str) -> Anchor {
        let adapter = Registry::by_name("rust").unwrap();
        let tree = adapter.parse(source).unwrap();
        let node = tree.root_node().child(0).unwrap();
        Anchor::capture(RepoPath::parse("src/lib.rs").unwrap(), adapter, source, node)
    }

    #[test]
    fn capture_records_symbol_and_range() {
        let anchor = capture_first_function("fn authorize() -> bool { true }");
        assert_eq!(anchor.symbol.as_ref().unwrap().to_string(), "rust://authorize");
        assert_eq!(anchor.range.start_line, 1);
        assert_eq!(anchor.node_kind, "function_item");
    }

    #[test]
    fn nested_symbols_build_a_dotted_path() {
        let adapter = Registry::by_name("rust").unwrap();
        let source = "impl Ledger { fn append(&self) {} }";
        let tree = adapter.parse(source).unwrap();
        let outer = tree.root_node().child(0).unwrap();
        let body = outer.child_by_field_name("body").unwrap();
        let method = body.named_child(0).unwrap();
        let path = symbol_path_of(method, adapter, source).unwrap();
        assert_eq!(path.to_string(), "rust://Ledger/append");
    }

    #[test]
    fn anchor_identity_is_stable_across_captures() {
        let left = capture_first_function("fn authorize() -> bool { true }");
        let right = capture_first_function("fn authorize() -> bool { true }");
        assert_eq!(left.id(), right.id());
    }

    #[test]
    fn node_path_descends_back_to_the_same_node() {
        let adapter = Registry::by_name("rust").unwrap();
        let source = "fn a() { if x { y(); } if z { w(); } }";
        let tree = adapter.parse(source).unwrap();
        let function = tree.root_node().child(0).unwrap();
        let block = function.child_by_field_name("body").unwrap();
        let second_if = block.named_child(1).unwrap();
        let path = NodePath::of(second_if, Some(function));
        assert_eq!(path.descend(function).unwrap().id(), second_if.id());
    }

    #[test]
    fn source_range_is_one_indexed() {
        let anchor = capture_first_function("fn a() {}");
        assert_eq!(anchor.range.start_line, 1);
        assert_eq!(anchor.range.start_column, 1);
    }
}
