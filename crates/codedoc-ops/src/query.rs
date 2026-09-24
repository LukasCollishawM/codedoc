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
            (Some(wanted), _) => index.active_for_symbol(wanted).and_then(|mut found| {
                found.extend(index.active_in_file(&normalised)?.into_iter().filter(is_file_scoped));
                Ok(found)
            }),
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
    pack.rank();
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

fn is_file_scoped(record: &codedoc_ledger::Record) -> bool {
    record
        .content()
        .anchors
        .iter()
        .any(|entry| matches!(entry.anchor.subject, codedoc_anchor::Subject::File))
}

pub fn verify(root: &Path) -> Result<(Value, i32), OpsError> {
    verify_scoped(root, &[], None)
}

pub fn verify_scoped(
    root: &Path,
    files: &[String],
    since: Option<&str>,
) -> Result<(Value, i32), OpsError> {
    let found = workspace(root)?;
    let mut targets: Vec<String> = files.to_vec();
    if let Some(revision) = since {
        let changed =
            codedoc_verify::history::changed_since(found.root(), revision).ok_or_else(|| {
                OpsError::Ledger { detail: format!("could not read what changed since {revision}") }
            })?;
        targets.extend(changed);
    }
    targets.sort();
    targets.dedup();

    let report = if targets.is_empty() {
        Verifier::new(found.root())
            .run_across(&found)
            .map_err(|source| OpsError::Ledger { detail: source.to_string() })?
    } else {
        let mut records = Vec::new();
        for ledger in found.ledgers() {
            let index = Index::current(ledger)
                .map_err(|source| OpsError::Index { detail: source.to_string() })?;
            for file in &targets {
                records.extend(
                    index
                        .active_in_file(file)
                        .map_err(|source| OpsError::Index { detail: source.to_string() })?,
                );
            }
        }
        records.sort_by_key(|record| record.id().to_string());
        records.dedup_by_key(|record| record.id().to_string());
        Verifier::new(found.root())
            .run_records(records, &targets)
            .map_err(|source| OpsError::Ledger { detail: source.to_string() })?
    };
    let code = report.exit_code();
    let payload = json!({
        "command": "verify",
        "scoped_to": targets,
        "records": report.records,
        "integrity_checked": report.integrity_checked,
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

fn suggestions_for(root: &Path, finding: &codedoc_verify::Finding, graph: &Graph) -> Vec<Value> {
    let Some(record) =
        graph.all().iter().find(|entry| entry.id().to_string() == finding.record.to_string())
    else {
        return Vec::new();
    };
    let Some(anchor) = record.subject() else {
        return Vec::new();
    };
    let Ok(adapter) = codedoc_lang::Registry::for_path(&anchor.file) else {
        return Vec::new();
    };
    let Ok(source) = std::fs::read_to_string(root.join(anchor.file.as_str())) else {
        return Vec::new();
    };
    let Ok(tree) = adapter.parse(&source) else {
        return Vec::new();
    };
    codedoc_anchor::FileIndex::build(adapter, &source, &tree)
        .candidates_like(anchor, 3)
        .into_iter()
        .map(|(symbol, range, likeness)| {
            json!({"symbol": symbol, "range": range.to_string(), "likeness": likeness})
        })
        .collect()
}

pub fn detached(root: &Path) -> Result<(Value, i32), OpsError> {
    let found = workspace(root)?;
    let report = Verifier::new(found.root())
        .run_across(&found)
        .map_err(|source| OpsError::Ledger { detail: source.to_string() })?;
    let graph = Graph::across(&found)?;
    let rows: Vec<Value> = report
        .findings
        .iter()
        .filter(|finding| finding.status == Status::Detached)
        .map(|finding| {
            json!({
                "suggestions": suggestions_for(found.root(), finding, &graph),
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

pub fn review(root: &Path, base: &str) -> Result<(Value, i32), OpsError> {
    let found = workspace(root)?;
    let changed = codedoc_verify::history::changed_since(found.root(), base).ok_or_else(|| {
        OpsError::Ledger { detail: format!("could not read what changed since {base}") }
    })?;

    let (payload, code) = verify_scoped(root, &changed, None)?;
    let findings: Vec<codedoc_verify::Finding> =
        serde_json::from_value(payload["findings"].clone()).unwrap_or_default();

    let review_graph = Graph::across(&found)?;
    let mut stale = Vec::new();
    let mut detached = Vec::new();
    let mut unchanged = 0usize;
    for finding in &findings {
        let symbol = finding.symbol.clone().unwrap_or_else(|| finding.file.clone());
        match finding.status {
            codedoc_verify::Status::Stale => {
                stale.push((finding.file.clone(), symbol, finding.claim.clone(), finding.drift))
            }
            codedoc_verify::Status::Detached => {
                let hint = suggestions_for(found.root(), finding, &review_graph).first().and_then(
                    |value| value.get("symbol").and_then(|name| name.as_str()).map(str::to_owned),
                );
                detached.push((finding.file.clone(), symbol, finding.claim.clone(), hint))
            }
            _ => unchanged += 1,
        }
    }

    let (touched, undocumented) =
        crate::coverage::touched_declarations(found.root(), &changed).unwrap_or((0, 0));

    let rendered = codedoc_render::review_markdown(&codedoc_render::ReviewInput {
        base,
        files: &changed,
        stale,
        detached,
        unchanged,
        touched_declarations: touched,
        undocumented_declarations: undocumented,
    });

    Ok((
        json!({
            "command": "review",
            "base": base,
            "files_changed": changed.len(),
            "output": rendered,
            "touched_declarations": touched,
            "undocumented_declarations": undocumented,
            "stale": payload["counts"]["stale"],
            "detached": payload["counts"]["detached"],
        }),
        code,
    ))
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
