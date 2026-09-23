use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use anyhow::{Context, Result, anyhow, bail};
use codedoc_anchor::{Anchor, FileIndex, Resolution, locate};
use codedoc_core::RecordId;
use codedoc_graph::Graph;
use codedoc_index::Index;
use codedoc_lang::Registry;
use codedoc_ledger::{
    AnchorRole, Body, Kind, Ledger, Lifecycle, Record, RecordContent, Role, SCHEMA_VERSION,
    Timestamp,
};
use serde_json::{Value, json};

use crate::git;

pub struct Relocation<'a> {
    pub symbol: Option<&'a str>,
    pub line: Option<u32>,
    pub file: Option<&'a str>,
}

fn find(graph: &Graph, reference: &str) -> Result<Record> {
    let exact = reference.parse::<RecordId>().ok().and_then(|id| graph.find(id).cloned());
    if let Some(record) = exact {
        return Ok(record);
    }

    let matches: Vec<&Record> = graph
        .all()
        .iter()
        .filter(|record| record.id().to_string().starts_with(reference))
        .collect();
    match matches.as_slice() {
        [single] => Ok((*single).clone()),
        [] => bail!("no record in this ledger begins with {reference:?}"),
        many => {
            bail!("{reference:?} is ambiguous across {} records; use more characters", many.len())
        }
    }
}

fn capture_at(root: &Path, file: &str, relocation: &Relocation<'_>) -> Result<Anchor> {
    let repo_path = codedoc_core::RepoPath::parse(file)
        .context("the target path must sit inside the repository")?;
    let adapter = Registry::for_path(&repo_path)
        .with_context(|| format!("no language adapter handles {file}"))?;
    let source = fs::read_to_string(root.join(repo_path.as_str()))
        .with_context(|| format!("reading {file}"))?;
    let tree = adapter.parse(&source).context("parsing the target file")?;

    let node = match (relocation.symbol, relocation.line) {
        (Some(symbol), _) => locate::by_symbol(&tree, &source, adapter, symbol)
            .ok_or_else(|| anyhow!("no symbol {symbol:?} found in {file}"))?,
        (None, Some(line)) => locate::by_line(&tree, adapter, line)
            .ok_or_else(|| anyhow!("line {line} covers no node in {file}"))?,
        (None, None) => bail!("relocating requires --to-symbol or --to-line"),
    };
    Ok(Anchor::capture(repo_path, adapter, &source, node))
}

fn current_position(root: &Path, anchor: &Anchor) -> Result<Anchor> {
    let adapter = Registry::for_path(&anchor.file)
        .with_context(|| format!("no language adapter handles {}", anchor.file))?;
    let source = fs::read_to_string(root.join(anchor.file.as_str()))
        .with_context(|| format!("reading {}", anchor.file))?;
    let tree = adapter.parse(&source).context("parsing the anchored file")?;
    let index = FileIndex::build(adapter, &source, &tree);

    match index.resolve(anchor) {
        Resolution::Located(located) => {
            let node = located
                .node_path()
                .descend(tree.root_node())
                .ok_or_else(|| anyhow!("resolved position could not be re-read"))?;
            Ok(Anchor::capture(anchor.file.clone(), adapter, &source, node))
        }
        Resolution::Detached(reason) => bail!(
            "this record's anchor is detached ({reason:?}); use `codedoc resolve` to place it \
             explicitly rather than superseding blindly"
        ),
        _ => bail!("resolution produced an unrecognised outcome"),
    }
}

fn emit(
    ledger: &Ledger,
    original: &Record,
    anchor: Anchor,
    kind: Kind,
    body: Body,
    root: &Path,
) -> Result<Record> {
    let content = RecordContent {
        schema: SCHEMA_VERSION,
        kind,
        anchors: vec![AnchorRole { role: Role::Subject, anchor }],
        body,
        evidence: original.content().evidence.clone(),
        assurance: original.content().assurance,
        author: original.content().author.clone(),
        code_revision: git::head_revision(root),
        created: Timestamp::now(),
        lifecycle: Lifecycle::Active,
        parent: Some(original.id()),
        chain: None,
        unrecognised: BTreeMap::new(),
    };
    let record = ledger.append(content).context("appending the superseding record")?;
    Index::append(ledger, &record).context("refreshing the index")?;
    Ok(record)
}

pub fn supersede(
    root: &Path,
    reference: &str,
    claim: Option<&str>,
    detail: Option<&str>,
    kind: Option<&str>,
) -> Result<(Value, i32)> {
    let ledger = Ledger::discover(root).context("opening the ledger")?;
    let graph = Graph::load(&ledger).context("loading the knowledge graph")?;
    let original = find(&graph, reference)?;
    let anchor =
        original.subject().ok_or_else(|| anyhow!("record {reference} has no subject anchor"))?;

    let relocated = current_position(ledger.root(), anchor)?;
    let kind = match kind {
        Some(name) => Kind::parse(name)
            .ok_or_else(|| anyhow!("unknown kind {name:?}; run `codedoc kinds`"))?,
        None => original.kind(),
    };
    let body = Body {
        claim: claim.unwrap_or(&original.content().body.claim).to_owned(),
        detail: detail.map(str::to_owned).or_else(|| original.content().body.detail.clone()),
    };

    let record = emit(&ledger, &original, relocated.clone(), kind, body, ledger.root())?;
    Ok((
        json!({
            "command": "supersede",
            "record": record.id().to_string(),
            "supersedes": original.id().to_string(),
            "kind": record.kind().as_str(),
            "range": relocated.range.to_string(),
            "claim": record.content().body.claim,
        }),
        0,
    ))
}

pub fn resolve(root: &Path, reference: &str, relocation: &Relocation<'_>) -> Result<(Value, i32)> {
    let ledger = Ledger::discover(root).context("opening the ledger")?;
    let graph = Graph::load(&ledger).context("loading the knowledge graph")?;
    let original = find(&graph, reference)?;
    let anchor =
        original.subject().ok_or_else(|| anyhow!("record {reference} has no subject anchor"))?;

    let file = relocation.file.unwrap_or(anchor.file.as_str()).to_owned();
    let placed = capture_at(ledger.root(), &file, relocation)?;
    let body = original.content().body.clone();
    let kind = original.kind();
    let record = emit(&ledger, &original, placed.clone(), kind, body, ledger.root())?;

    Ok((
        json!({
            "command": "resolve",
            "record": record.id().to_string(),
            "readopts": original.id().to_string(),
            "file": placed.file.as_str(),
            "symbol": placed.symbol.as_ref().map(ToString::to_string),
            "range": placed.range.to_string(),
        }),
        0,
    ))
}

pub fn retract(root: &Path, reference: &str, reason: Option<&str>) -> Result<(Value, i32)> {
    let ledger = Ledger::discover(root).context("opening the ledger")?;
    let graph = Graph::load(&ledger).context("loading the knowledge graph")?;
    let original = find(&graph, reference)?;
    let anchor = original
        .subject()
        .ok_or_else(|| anyhow!("record {reference} has no subject anchor"))?
        .clone();

    let body = Body {
        claim: reason.unwrap_or("Retracted.").to_owned(),
        detail: Some(format!("Retracts: {}", original.content().body.claim)),
    };
    let record = emit(&ledger, &original, anchor, Kind::Tombstone, body, ledger.root())?;

    Ok((
        json!({
            "command": "retract",
            "record": record.id().to_string(),
            "retracts": original.id().to_string(),
            "was": original.content().body.claim,
        }),
        0,
    ))
}
