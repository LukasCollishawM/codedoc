use std::collections::{BTreeMap, BTreeSet, HashMap};

use codedoc_core::{ContentFingerprint, ContextFingerprint, StructuralFingerprint};
use codedoc_lang::Adapter;
use serde::{Deserialize, Serialize};
use tree_sitter::{Node, Tree};

use crate::anchor::{Anchor, NodePath, SourceRange};
use crate::fingerprint;

pub const SIMILARITY_FLOOR: f64 = 0.75;
pub const SIMILARITY_MARGIN: f64 = 0.15;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum Confidence {
    Low,
    Medium,
    High,
    Exact,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum Rung {
    ContentIdentity,
    StructuralIdentity,
    SymbolAndNodePath,
    ContextBracket,
    GitMigration,
    Similarity,
}

impl Rung {
    pub fn confidence(self) -> Confidence {
        match self {
            Rung::ContentIdentity | Rung::StructuralIdentity => Confidence::Exact,
            Rung::SymbolAndNodePath => Confidence::High,
            Rung::ContextBracket | Rung::GitMigration => Confidence::Medium,
            Rung::Similarity => Confidence::Low,
        }
    }

    pub fn position(self) -> u8 {
        match self {
            Rung::ContentIdentity => 1,
            Rung::StructuralIdentity => 2,
            Rung::SymbolAndNodePath => 3,
            Rung::ContextBracket => 4,
            Rung::GitMigration => 5,
            Rung::Similarity => 6,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "reason")]
#[non_exhaustive]
pub enum DetachReason {
    NoCandidate,
    Ambiguous { rung: Rung, candidates: u32 },
    BelowThreshold { reached: Confidence, required: Confidence },
    FileMissing,
    LanguageUnsupported,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Located {
    rung: Rung,
    confidence: Confidence,
    range: SourceRange,
    node_path: NodePath,
    node_kind: String,
}

impl Located {
    fn new(rung: Rung, range: SourceRange, node_path: NodePath, node_kind: String) -> Self {
        Located { rung, confidence: rung.confidence(), range, node_path, node_kind }
    }

    pub fn rung(&self) -> Rung {
        self.rung
    }

    pub fn confidence(&self) -> Confidence {
        self.confidence
    }

    pub fn range(&self) -> SourceRange {
        self.range
    }

    pub fn node_path(&self) -> &NodePath {
        &self.node_path
    }

    pub fn node_kind(&self) -> &str {
        &self.node_kind
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "outcome")]
#[non_exhaustive]
pub enum Resolution {
    Located(Located),
    Detached(DetachReason),
}

impl Resolution {
    pub fn located(&self) -> Option<&Located> {
        match self {
            Resolution::Located(located) => Some(located),
            Resolution::Detached(_) => None,
        }
    }

    pub fn is_detached(&self) -> bool {
        matches!(self, Resolution::Detached(_))
    }

    pub fn require(self, required: Confidence) -> Resolution {
        match self {
            Resolution::Located(located) if located.confidence < required => {
                Resolution::Detached(DetachReason::BelowThreshold {
                    reached: located.confidence,
                    required,
                })
            }
            other => other,
        }
    }
}

struct Candidate<'tree> {
    node: Node<'tree>,
    kind: String,
    content: ContentFingerprint,
    structural: StructuralFingerprint,
    preceding: ContextFingerprint,
    following: ContextFingerprint,
    symbol: Option<String>,
    declares: bool,
}

pub struct FileIndex<'tree, 'adapter> {
    adapter: &'adapter Adapter,
    root: Node<'tree>,
    candidates: Vec<Candidate<'tree>>,
    by_content: HashMap<ContentFingerprint, Vec<usize>>,
    by_structural: HashMap<StructuralFingerprint, Vec<usize>>,
    by_context: HashMap<(ContextFingerprint, ContextFingerprint), Vec<usize>>,
    by_symbol: HashMap<String, Vec<usize>>,
}

impl<'tree, 'adapter> FileIndex<'tree, 'adapter> {
    pub fn build(adapter: &'adapter Adapter, source: &str, tree: &'tree Tree) -> Self {
        let root = tree.root_node();
        let digests = fingerprint::compute_all(root, adapter, source);
        let mut candidates = Vec::new();
        let mut segments: Vec<String> = Vec::new();
        collect(root, adapter, source, &digests, &mut segments, &mut candidates);

        let mut by_content: HashMap<ContentFingerprint, Vec<usize>> = HashMap::new();
        let mut by_structural: HashMap<StructuralFingerprint, Vec<usize>> = HashMap::new();
        let mut by_context: HashMap<(ContextFingerprint, ContextFingerprint), Vec<usize>> =
            HashMap::new();
        let mut by_symbol: HashMap<String, Vec<usize>> = HashMap::new();

        for (position, candidate) in candidates.iter().enumerate() {
            by_content.entry(candidate.content).or_default().push(position);
            by_structural.entry(candidate.structural).or_default().push(position);
            by_context
                .entry((candidate.preceding, candidate.following))
                .or_default()
                .push(position);
            if let Some(symbol) = &candidate.symbol {
                by_symbol.entry(symbol.clone()).or_default().push(position);
            }
        }

        FileIndex { adapter, root, candidates, by_content, by_structural, by_context, by_symbol }
    }

    pub fn resolve(&self, anchor: &Anchor) -> Resolution {
        let mut first_ambiguity: Option<(Rung, u32)> = None;
        let note = |rung: Rung, count: usize, slot: &mut Option<(Rung, u32)>| {
            if slot.is_none() {
                *slot = Some((rung, count as u32));
            }
        };

        for rung in [
            Rung::ContentIdentity,
            Rung::StructuralIdentity,
            Rung::SymbolAndNodePath,
            Rung::ContextBracket,
        ] {
            let matches = self.matches_for(anchor, rung);
            match matches.len() {
                1 => return self.locate(rung, matches[0]),
                0 => continue,
                count => note(rung, count, &mut first_ambiguity),
            }
        }

        if let Some(resolution) = self.by_similarity(anchor) {
            return resolution;
        }

        match first_ambiguity {
            Some((rung, candidates)) => {
                Resolution::Detached(DetachReason::Ambiguous { rung, candidates })
            }
            None => Resolution::Detached(DetachReason::NoCandidate),
        }
    }

    fn matches_for(&self, anchor: &Anchor, rung: Rung) -> Vec<usize> {
        match rung {
            Rung::ContentIdentity => self
                .by_content
                .get(&anchor.content)
                .map(|positions| self.filter_kind(positions, anchor))
                .unwrap_or_default(),
            Rung::StructuralIdentity => self
                .by_structural
                .get(&anchor.structural)
                .map(|positions| {
                    positions
                        .iter()
                        .copied()
                        .filter(|position| {
                            let candidate = &self.candidates[*position];
                            candidate.kind == anchor.node_kind
                                && candidate.symbol
                                    == anchor.symbol.as_ref().map(ToString::to_string)
                        })
                        .collect()
                })
                .unwrap_or_default(),
            Rung::SymbolAndNodePath => self.by_symbol_and_path(anchor),
            Rung::ContextBracket => self
                .by_context
                .get(&(anchor.preceding, anchor.following))
                .map(|positions| self.filter_kind(positions, anchor))
                .unwrap_or_default(),
            _ => Vec::new(),
        }
    }

    fn filter_kind(&self, positions: &[usize], anchor: &Anchor) -> Vec<usize> {
        positions
            .iter()
            .copied()
            .filter(|position| self.candidates[*position].kind == anchor.node_kind)
            .collect()
    }

    fn by_symbol_and_path(&self, anchor: &Anchor) -> Vec<usize> {
        let Some(symbol) = anchor.symbol.as_ref().map(ToString::to_string) else {
            return Vec::new();
        };
        let Some(positions) = self.by_symbol.get(&symbol) else {
            return Vec::new();
        };
        let declarations: Vec<usize> = positions
            .iter()
            .copied()
            .filter(|position| self.candidates[*position].declares)
            .collect();
        let [declaration] = declarations.as_slice() else {
            return Vec::new();
        };
        let Some(target) = anchor.node_path.descend(self.candidates[*declaration].node) else {
            return Vec::new();
        };
        if target.kind() != anchor.node_kind {
            return Vec::new();
        }
        self.candidates
            .iter()
            .position(|candidate| candidate.node.id() == target.id())
            .into_iter()
            .collect()
    }

    fn by_similarity(&self, anchor: &Anchor) -> Option<Resolution> {
        let symbol = anchor.symbol.as_ref().map(ToString::to_string)?;
        let positions = self.by_symbol.get(&symbol)?;
        let mut scored: Vec<(f64, usize)> = positions
            .iter()
            .copied()
            .filter(|position| self.candidates[*position].kind == anchor.node_kind)
            .map(|position| {
                let score =
                    shape_similarity(&anchor.shape, self.candidates[position].node, self.adapter);
                (score, position)
            })
            .filter(|(score, _)| *score >= SIMILARITY_FLOOR)
            .collect();
        scored.sort_by(|left, right| {
            right.0.partial_cmp(&left.0).unwrap_or(std::cmp::Ordering::Equal)
        });
        let (best_score, best) = *scored.first()?;
        if let Some((runner_up, _)) = scored.get(1)
            && best_score - runner_up < SIMILARITY_MARGIN
        {
            return Some(Resolution::Detached(DetachReason::Ambiguous {
                rung: Rung::Similarity,
                candidates: scored.len() as u32,
            }));
        }
        Some(self.locate(Rung::Similarity, best))
    }

    fn locate(&self, rung: Rung, position: usize) -> Resolution {
        let candidate = &self.candidates[position];
        Resolution::Located(Located::new(
            rung,
            SourceRange::of(candidate.node),
            NodePath::of(candidate.node, None),
            candidate.kind.clone(),
        ))
    }

    pub fn resolve_after_migration(&self, anchor: &Anchor) -> Resolution {
        match self.resolve(anchor) {
            Resolution::Located(located) => Resolution::Located(Located::new(
                Rung::GitMigration,
                located.range,
                located.node_path,
                located.node_kind,
            )),
            other => other,
        }
    }

    pub fn root(&self) -> Node<'tree> {
        self.root
    }
}

fn collect<'tree>(
    node: Node<'tree>,
    adapter: &Adapter,
    source: &str,
    digests: &fingerprint::Digests,
    segments: &mut Vec<String>,
    out: &mut Vec<Candidate<'tree>>,
) {
    if adapter.is_ignorable(node.kind()) {
        return;
    }
    let declares = adapter.declares_symbol(node.kind());
    let named_here =
        declares.then(|| adapter.declaration_name(node, source)).flatten().map(str::to_owned);
    if let Some(name) = &named_here {
        segments.push(name.clone());
    }

    if node.is_named() {
        let symbol =
            (!segments.is_empty()).then(|| format!("{}://{}", adapter.name(), segments.join("/")));
        out.push(Candidate {
            node,
            kind: node.kind().to_owned(),
            content: fingerprint::content_of(node, digests),
            structural: fingerprint::structural_of(node, digests),
            preceding: fingerprint::preceding_context_with(node, adapter, digests),
            following: fingerprint::following_context_with(node, adapter, digests),
            symbol,
            declares,
        });
    }

    let mut cursor = node.walk();
    if cursor.goto_first_child() {
        loop {
            collect(cursor.node(), adapter, source, digests, segments, out);
            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }

    if named_here.is_some() {
        segments.pop();
    }
}

pub struct Resolver<'adapter> {
    adapter: &'adapter Adapter,
}

impl<'adapter> Resolver<'adapter> {
    pub fn new(adapter: &'adapter Adapter) -> Self {
        Resolver { adapter }
    }

    pub fn resolve(&self, anchor: &Anchor, source: &str, tree: &Tree) -> Resolution {
        FileIndex::build(self.adapter, source, tree).resolve(anchor)
    }
}

fn shape_similarity(reference: &BTreeMap<String, u32>, node: Node<'_>, adapter: &Adapter) -> f64 {
    let candidate = fingerprint::shape_histogram(node, adapter);
    if reference.is_empty() && candidate.is_empty() {
        return 1.0;
    }
    let mut shared = 0u32;
    let mut union = 0u32;
    for kind in reference.keys().chain(candidate.keys()).collect::<BTreeSet<_>>() {
        let left = reference.get(kind).copied().unwrap_or(0);
        let right = candidate.get(kind).copied().unwrap_or(0);
        shared += left.min(right);
        union += left.max(right);
    }
    if union == 0 { 0.0 } else { f64::from(shared) / f64::from(union) }
}
