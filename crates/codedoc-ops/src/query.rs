use std::path::Path;

use codedoc_context::Target as ContextTarget;
use codedoc_core::RepoPath;
use codedoc_graph::Graph;
use codedoc_index::Index;
use codedoc_verify::{Status, Verifier};
use serde_json::{Value, json};

use crate::lifecycle::find;
use crate::{OpsError, Outcome, workspace};

pub fn context(
    root: &Path,
    file: &str,
    line: Option<u32>,
    symbol: Option<&str>,
    depth: u8,
    budget: Option<usize>,
) -> Outcome {
    let found = workspace(root)?;
    let normalised = RepoPath::parse(file)
        .map(|path| path.as_str().to_owned())
        .unwrap_or_else(|_| file.to_owned());

    let mut records = Vec::new();
    for ledger in found.ledgers() {
        let index = Index::current(ledger)
            .map_err(|source| OpsError::Index { detail: source.to_string() })?;
        let batch = match (symbol, line) {
            (Some(wanted), _) => index.active_for_symbol(wanted),
            (None, Some(wanted)) => index.active_covering_line(&normalised, wanted),
            (None, None) => index.active_in_file(&normalised),
        }
        .map_err(|source| OpsError::Index { detail: source.to_string() })?;
        records.extend(batch);
    }

    if depth > 0 {
        let mut symbols: Vec<String> = records
            .iter()
            .flat_map(|record| record.content().anchors.iter())
            .filter_map(|entry| entry.anchor.symbol.as_ref().map(ToString::to_string))
            .collect();
        if let Some(requested) = symbol {
            symbols.push(requested.to_owned());
        }
        for ledger in found.ledgers() {
            if let Ok(index) = Index::current(ledger)
                && let Ok(batch) = index.active_relations_touching(&symbols)
            {
                records.extend(batch);
            }
        }
    }

    records.sort_by_key(|record| record.id().to_string());
    records.dedup_by_key(|record| record.id().to_string());

    let graph = Graph::from_records(records);
    let mut pack = codedoc_context::assemble(
        &graph,
        ContextTarget { file: normalised, line, symbol: symbol.map(str::to_owned) },
        depth,
    );
    pack.deduplicate();
    if let Some(limit) = budget {
        pack.fit_within(limit);
    }

    Ok(json!({
        "command": "context",
        "pack": serde_json::to_value(&pack).unwrap_or(Value::Null),
        "claims": pack.claim_count(),
        "empty": pack.is_empty(),
        "scopes": found.scopes().iter().map(|scope| scope.as_str()).collect::<Vec<_>>(),
    }))
}

pub fn verify(root: &Path) -> Result<(Value, i32), OpsError> {
    let found = workspace(root)?;
    let report = Verifier::new(found.root())
        .run_across(&found)
        .map_err(|source| OpsError::Ledger { detail: source.to_string() })?;
    let code = report.exit_code();
    let payload = json!({
        "command": "verify",
        "records": report.records,
        "integrity_intact": report.integrity_intact,
        "orphaned_records": report
            .orphaned_records
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>(),
        "counts": report.counts(),
        "findings": serde_json::to_value(&report.findings).unwrap_or(Value::Null),
    });
    Ok((payload, code))
}

pub fn detached(root: &Path) -> Result<(Value, i32), OpsError> {
    let found = workspace(root)?;
    let report = Verifier::new(found.root())
        .run_across(&found)
        .map_err(|source| OpsError::Ledger { detail: source.to_string() })?;
    let rows: Vec<Value> = report
        .findings
        .iter()
        .filter(|finding| finding.status == Status::Detached)
        .map(|finding| {
            json!({
                "record": finding.record.to_string(),
                "kind": finding.kind,
                "claim": finding.claim,
                "file": finding.file,
                "symbol": finding.symbol,
                "recorded_range": finding.recorded_range.to_string(),
                "resolution": serde_json::to_value(&finding.resolution).unwrap_or(Value::Null),
            })
        })
        .collect();
    let code = i32::from(!rows.is_empty()) * 2;
    Ok((json!({"command": "detached", "count": rows.len(), "records": rows}), code))
}

pub fn list(root: &Path, file: Option<&str>, symbol: Option<&str>) -> Outcome {
    let found = workspace(root)?;
    let graph = Graph::across(&found)?;
    let records = match (file, symbol) {
        (_, Some(wanted)) => graph.for_symbol(wanted),
        (Some(wanted), None) => graph.in_file(wanted),
        (None, None) => graph.active(),
    };
    let rows: Vec<Value> = records
        .iter()
        .map(|record| {
            json!({
                "record": record.id().to_string(),
                "kind": record.kind().as_str(),
                "claim": record.content().body.claim,
                "file": record.subject().map(|anchor| anchor.file.as_str().to_owned()),
                "symbol": record
                    .subject()
                    .and_then(|anchor| anchor.symbol.as_ref().map(ToString::to_string)),
                "created": record.content().created.to_rfc3339(),
            })
        })
        .collect();
    Ok(json!({"command": "list", "count": rows.len(), "records": rows}))
}

pub fn history(root: &Path, reference: &str) -> Outcome {
    let original = find(root, reference)?;
    let found = workspace(root)?;
    let graph = Graph::across(&found)?;
    let chain = graph.supersession_chain(original.id());
    let rows: Vec<Value> = chain
        .iter()
        .map(|entry| {
            json!({
                "record": entry.id().to_string(),
                "kind": entry.kind().as_str(),
                "claim": entry.content().body.claim,
                "created": entry.content().created.to_rfc3339(),
                "code_revision": entry.content().code_revision.as_ref().map(ToString::to_string),
            })
        })
        .collect();
    Ok(json!({"command": "history", "revisions": rows.len(), "chain": rows}))
}

pub fn render(root: &Path, format: &str, title: &str) -> Outcome {
    let found = workspace(root)?;
    let graph = Graph::across(&found)?;
    let active = graph.active();

    let rendered = match format.trim().to_ascii_lowercase().as_str() {
        "markdown" | "md" => codedoc_render::overview_markdown(&active, title),
        "mermaid" | "graph" => {
            let relations = graph.relations();
            codedoc_render::mermaid_relations(&relations)
        }
        other => {
            return Err(OpsError::UnknownKind {
                found: other.to_owned(),
                vocabulary: "markdown, mermaid".to_owned(),
            });
        }
    };

    Ok(json!({
        "command": "render",
        "format": format,
        "records": active.len(),
        "output": rendered,
    }))
}

pub fn conflicts(root: &Path) -> Result<(Value, i32), OpsError> {
    let found = workspace(root)?;
    let graph = Graph::across(&found)?;
    let findings = graph.conflicts();
    let rows: Vec<Value> = findings
        .iter()
        .map(|finding| {
            json!({
                "kind": finding.kind.as_str(),
                "describes": finding.kind.describes(),
                "left": finding.left.to_string(),
                "right": finding.right.to_string(),
                "anchor": finding.anchor,
                "left_claim": finding.left_claim,
                "right_claim": finding.right_claim,
                "similarity": finding.similarity,
            })
        })
        .collect();
    let code = i32::from(!rows.is_empty());
    Ok((json!({"command": "conflicts", "count": rows.len(), "findings": rows}), code))
}

pub fn stats(root: &Path) -> Outcome {
    let found = workspace(root)?;
    let graph = Graph::across(&found)?;
    let verification = found.verify()?;
    Ok(json!({
        "command": "stats",
        "total_records": graph.all().len(),
        "active_records": graph.active().len(),
        "by_kind": graph.counts_by_kind(),
        "relations": graph.relations().len(),
        "tips": verification.tips.iter().map(ToString::to_string).collect::<Vec<_>>(),
        "integrity_intact": verification.is_intact(),
        "scopes": found.scopes().iter().map(|scope| scope.as_str()).collect::<Vec<_>>(),
    }))
}
