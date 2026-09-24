use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use codedoc_anchor::Anchor;
use codedoc_core::RepoPath;
use codedoc_lang::{Adapter, Registry};
use codedoc_ledger::{
    AnchorRole, Assurance, Author, Body, Kind, Lifecycle, RecordContent, Role, SCHEMA_VERSION,
    Scope, Timestamp,
};

use crate::{OpsError, Outcome, writable};
use serde_json::{Value, json};
use tree_sitter::Node;
use walkdir::WalkDir;

const MINIMUM_CLAIM_LENGTH: usize = 12;
const ADJACENCY_LINES: usize = 2;

struct Harvested {
    file: String,
    line: u32,
    kind: Kind,
    claim: String,
    symbol: Option<String>,
    content: RecordContent,
}

pub fn import(
    root: &Path,
    scope: Option<Scope>,
    paths: &[String],
    write: bool,
    limit: Option<usize>,
) -> Outcome {
    let ledger = writable(root, scope)?;
    let root = ledger.root().to_path_buf();
    let root = root.as_path();
    let targets: Vec<String> = if paths.is_empty() { vec![".".to_owned()] } else { paths.to_vec() };

    let revision = crate::revision::head_revision(root);
    let mut harvested = Vec::new();
    let mut files_scanned = 0usize;

    for target in &targets {
        for entry in WalkDir::new(root.join(target)).into_iter().filter_map(Result::ok) {
            let path = entry.path();
            if !path.is_file() || is_excluded(path) {
                continue;
            }
            let Ok(relative) = path.strip_prefix(root) else {
                continue;
            };
            let Ok(repo_path) = RepoPath::parse(&relative.to_string_lossy()) else {
                continue;
            };
            let Ok(adapter) = Registry::for_path(&repo_path) else {
                continue;
            };
            files_scanned += 1;
            harvested.extend(harvest_file(&repo_path, adapter, path, revision.clone()));
            if limit.is_some_and(|cap| harvested.len() >= cap) {
                break;
            }
        }
    }

    if let Some(cap) = limit {
        harvested.truncate(cap);
    }

    let mut by_kind: BTreeMap<String, usize> = BTreeMap::new();
    for item in &harvested {
        *by_kind.entry(item.kind.as_str()).or_insert(0) += 1;
    }

    let sample: Vec<Value> = harvested
        .iter()
        .take(10)
        .map(|item| {
            json!({
                "file": item.file,
                "line": item.line,
                "kind": item.kind.as_str(),
                "symbol": item.symbol,
                "claim": item.claim.chars().take(110).collect::<String>(),
            })
        })
        .collect();

    let mut written = 0usize;
    if write && !harvested.is_empty() {
        let drafts: Vec<RecordContent> = harvested.into_iter().map(|item| item.content).collect();
        let records = ledger
            .append_batch(drafts)
            .map_err(|source| OpsError::Ledger { detail: source.to_string() })?;
        codedoc_index::Index::append_many(&ledger, &records)
            .map_err(|source| OpsError::Index { detail: source.to_string() })?;
        written = records.len();
    }

    Ok(json!({
            "command": "import",
            "files_scanned": files_scanned,
            "candidates": by_kind.values().sum::<usize>(),
            "by_kind": by_kind,
            "written": written,
            "dry_run": !write,
            "sample": sample,
            "scope": ledger.scope().as_str(),
    }))
}

fn is_excluded(path: &Path) -> bool {
    path.components().any(|component| {
        let name = component.as_os_str().to_string_lossy();
        matches!(
            name.as_ref(),
            ".git" | "target" | "node_modules" | "vendor" | "dist" | "build" | ".codedoc"
        )
    })
}

fn harvest_file(
    repo_path: &RepoPath,
    adapter: &Adapter,
    disk: &Path,
    revision: Option<codedoc_core::GitRev>,
) -> Vec<Harvested> {
    let Ok(source) = fs::read_to_string(disk) else {
        return Vec::new();
    };
    let Ok(tree) = adapter.parse(&source) else {
        return Vec::new();
    };

    let digests = codedoc_anchor::fingerprint::compute_all(tree.root_node(), adapter, &source);
    let symbols = codedoc_anchor::SymbolTable::build(tree.root_node(), adapter, &source);
    let first_declaration = first_declaration_row(tree.root_node(), adapter);
    let mut blocks: Vec<Vec<Node<'_>>> = Vec::new();
    collect_comment_blocks(tree.root_node(), adapter, &mut blocks);

    let mut harvested = Vec::new();
    for block in blocks {
        let Some(first) = block.first() else { continue };
        let Some(last) = block.last() else { continue };

        let text = block
            .iter()
            .filter_map(|node| node.utf8_text(source.as_bytes()).ok())
            .map(clean_comment)
            .collect::<Vec<_>>()
            .join("\n");
        let claim = text.trim().to_owned();
        if claim.chars().count() < MINIMUM_CLAIM_LENGTH || is_decorative(&claim) {
            continue;
        }

        let target = documented_node(*last, adapter);
        let anchor = if documents_the_file(&block, target, first_declaration, adapter, &source) {
            Anchor::capture_file_with(
                repo_path.clone(),
                adapter,
                &source,
                tree.root_node(),
                &digests,
                &symbols,
            )
        } else {
            let Some(target) = target else { continue };
            Anchor::capture_with(repo_path.clone(), adapter, &source, target, &digests, &symbols)
        };
        let kind = infer_kind(&claim);

        harvested.push(Harvested {
            file: repo_path.as_str().to_owned(),
            line: first.start_position().row as u32 + 1,
            kind,
            claim: claim.clone(),
            symbol: anchor.symbol.as_ref().map(ToString::to_string),
            content: RecordContent {
                schema: SCHEMA_VERSION,
                kind,
                anchors: vec![AnchorRole { role: Role::Subject, anchor }],
                body: Body { claim, detail: None },
                evidence: Vec::new(),
                assurance: Assurance::Inferred,
                author: Author::Analyzer { name: "comment-import".to_owned() },
                code_revision: revision.clone(),
                created: Timestamp::now(),
                lifecycle: Lifecycle::Active,
                parent: None,
                chain: None,
                unrecognised: BTreeMap::new(),
            },
        });
    }
    harvested
}

fn collect_comment_blocks<'tree>(
    node: Node<'tree>,
    adapter: &Adapter,
    blocks: &mut Vec<Vec<Node<'tree>>>,
) {
    let mut cursor = node.walk();
    let mut pending: Vec<Node<'tree>> = Vec::new();

    for child in node.named_children(&mut cursor) {
        if adapter.is_ignorable(child.kind()) {
            let contiguous = pending.last().is_none_or(|previous| {
                child.start_position().row.saturating_sub(previous.end_position().row) <= 1
            });
            if contiguous {
                pending.push(child);
            } else {
                blocks.push(std::mem::take(&mut pending));
                pending.push(child);
            }
            continue;
        }
        if !pending.is_empty() {
            blocks.push(std::mem::take(&mut pending));
        }
        collect_comment_blocks(child, adapter, blocks);
    }
    if !pending.is_empty() {
        blocks.push(pending);
    }
}

fn first_declaration_row(root: Node<'_>, adapter: &Adapter) -> Option<usize> {
    let mut earliest: Option<usize> = None;
    let mut pending = vec![root];
    while let Some(node) = pending.pop() {
        let mut cursor = node.walk();
        for child in node.named_children(&mut cursor) {
            if adapter.declares_symbol(child.kind()) {
                let row = child.start_position().row;
                earliest = Some(earliest.map_or(row, |seen: usize| seen.min(row)));
                continue;
            }
            pending.push(child);
        }
    }
    earliest
}

fn documents_the_file(
    block: &[Node<'_>],
    target: Option<Node<'_>>,
    first_declaration: Option<usize>,
    adapter: &Adapter,
    source: &str,
) -> bool {
    let Some(first) = block.first() else { return false };
    let precedes_every_declaration =
        first_declaration.is_none_or(|row| first.start_position().row < row);
    if !precedes_every_declaration {
        return false;
    }
    let inner_doc =
        block.iter().filter_map(|node| node.utf8_text(source.as_bytes()).ok()).any(|text| {
            let trimmed = text.trim_start();
            trimmed.starts_with("//!") || trimmed.starts_with("/*!")
        });
    inner_doc || !target.is_some_and(|node| adapter.declares_symbol(node.kind()))
}

fn documented_node<'tree>(comment: Node<'tree>, adapter: &Adapter) -> Option<Node<'tree>> {
    let mut candidate = comment.next_named_sibling();
    let mut first_adjacent = None;
    let mut previous_end = comment.end_position().row;

    while let Some(sibling) = candidate {
        if adapter.is_ignorable(sibling.kind()) {
            candidate = sibling.next_named_sibling();
            continue;
        }
        let gap = sibling.start_position().row.saturating_sub(previous_end);
        if gap > ADJACENCY_LINES {
            break;
        }
        if adapter.declares_symbol(sibling.kind()) {
            return Some(sibling);
        }
        if let Some(inner) = declaring_descendant(sibling, adapter) {
            return Some(inner);
        }
        if first_adjacent.is_none() {
            first_adjacent = Some(sibling);
        }
        if !is_modifier(sibling.kind()) {
            break;
        }
        previous_end = sibling.end_position().row;
        candidate = sibling.next_named_sibling();
    }
    if let Some(found) = first_adjacent {
        return Some(found);
    }
    let mut ancestor = comment.parent();
    while let Some(node) = ancestor {
        if adapter.declares_symbol(node.kind()) {
            return Some(node);
        }
        ancestor = node.parent();
    }
    None
}

fn declaring_descendant<'tree>(node: Node<'tree>, adapter: &Adapter) -> Option<Node<'tree>> {
    let mut cursor = node.walk();
    node.named_children(&mut cursor).find(|child| adapter.declares_symbol(child.kind()))
}

fn is_modifier(kind: &str) -> bool {
    matches!(kind, "attribute_item" | "attribute" | "decorator" | "annotation" | "modifiers")
}

fn clean_comment(raw: &str) -> String {
    let trimmed = raw.trim();
    let stripped = trimmed
        .strip_prefix("/**")
        .or_else(|| trimmed.strip_prefix("/*"))
        .map(|body| body.trim_end_matches("*/"))
        .unwrap_or(trimmed);
    stripped
        .lines()
        .map(|line| {
            line.trim()
                .trim_start_matches("///")
                .trim_start_matches("//!")
                .trim_start_matches("//")
                .trim_start_matches('#')
                .trim_start_matches('*')
                .trim()
        })
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_owned()
}

fn is_decorative(claim: &str) -> bool {
    claim
        .chars()
        .all(|character| !character.is_alphanumeric() || character == '-' || character == '=')
}

fn infer_kind(claim: &str) -> Kind {
    let lowered = claim.to_ascii_lowercase();
    let marker = |needle: &str| lowered.contains(needle);

    if marker("safety:") || marker("security") || marker("vulnerab") || marker("attack") {
        return Kind::Security;
    }
    if marker("todo") || marker("fixme") || marker("xxx:") {
        return Kind::Warning;
    }
    if marker("hack") || marker("workaround") || marker("kludge") || marker("for now") {
        return Kind::Workaround;
    }
    if marker("must ") || marker("invariant") || marker("never ") || marker("always ") {
        return Kind::Invariant;
    }
    if marker("because") || marker("rationale") || marker("reason") || marker("why ") {
        return Kind::Rationale;
    }
    if marker("panic") || marker("fails if") || marker("race") || marker("deadlock") {
        return Kind::KnownFailureMode;
    }
    if marker("performance") || marker("slow") || marker("o(n") || marker("allocat") {
        return Kind::Performance;
    }
    Kind::Explanation
}
