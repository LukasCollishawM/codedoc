use std::path::Path;

use codedoc_context::Target as ContextTarget;
use codedoc_core::RepoPath;
use codedoc_graph::Graph;
use codedoc_index::Index;
use codedoc_verify::{Status, Verifier};
use serde_json::{Value, json};

use codedoc_ledger::Scope;

use crate::lifecycle::find;
use crate::{OpsError, Outcome, workspace, workspace_in};

pub fn context(
    root: &Path,
    file: &str,
    line: Option<u32>,
    symbol: Option<&str>,
    depth: u8,
    budget: Option<usize>,
    as_of: Option<&str>,
) -> Outcome {
    let found = workspace(root)?;
    let file = crate::repo_relative(&found, file, root);
    let normalised = RepoPath::parse(&file)
        .map(|path| path.as_str().to_owned())
        .unwrap_or_else(|_| file.clone());

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

    let graph = at_moment(Graph::from_records(records), as_of)?;
    let mut pack = codedoc_context::assemble(
        &graph,
        ContextTarget { file: normalised, line, symbol: symbol.map(str::to_owned) },
        depth,
    );
    pack.deduplicate();
    pack.rank();
    pack.fit_within(budget.unwrap_or(codedoc_context::DEFAULT_BUDGET));

    Ok(json!({
        "command": "context",
        "as_of": as_of,
        "pack": serde_json::to_value(&pack).unwrap_or(Value::Null),
        "claims": pack.claim_count(),
        "empty": pack.is_empty(),
        "scopes": found.scopes().iter().map(|scope| scope.as_str()).collect::<Vec<_>>(),
    }))
}

pub fn brief(
    root: &Path,
    files: &[String],
    since: Option<&str>,
    depth: u8,
    budget: Option<usize>,
) -> Outcome {
    let found = workspace(root)?;
    let mut targets: Vec<String> = files
        .iter()
        .map(|name| {
            let name = crate::repo_relative(&found, name, root);
            RepoPath::parse(&name)
                .map(|path| path.as_str().to_owned())
                .unwrap_or_else(|_| name.clone())
        })
        .collect();
    if let Some(revision) = since {
        let changed = codedoc_verify::history::changed_since(found.root(), revision)
            .ok_or_else(|| OpsError::RevisionUnreadable { revision: revision.to_string() })?;
        targets.extend(changed);
    }
    targets.sort();
    targets.dedup();

    if targets.is_empty() {
        return Err(OpsError::TargetUnnamed);
    }

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
    let graph = Graph::from_records(records);

    let mut pack: Option<codedoc_context::ContextPack> = None;
    for file in &targets {
        let assembled = codedoc_context::assemble(
            &graph,
            ContextTarget { file: file.clone(), line: None, symbol: None },
            depth,
        );
        match pack.as_mut() {
            Some(collected) => collected.absorb(assembled),
            None => pack = Some(assembled),
        }
    }
    let mut pack = pack.expect("targets is not empty");
    pack.deduplicate();
    pack.rank();
    pack.fit_within(budget.unwrap_or(codedoc_context::DEFAULT_BUDGET));

    Ok(json!({
        "command": "brief",
        "files": targets,
        "pack": serde_json::to_value(&pack).unwrap_or(Value::Null),
        "claims": pack.claim_count(),
        "empty": pack.is_empty(),
        "scopes": found.scopes().iter().map(|scope| scope.as_str()).collect::<Vec<_>>(),
    }))
}

fn at_moment(graph: Graph, as_of: Option<&str>) -> Result<Graph, OpsError> {
    let Some(text) = as_of else {
        return Ok(graph);
    };
    let moment = codedoc_ledger::Timestamp::parse(text)
        .ok_or_else(|| OpsError::MomentUnreadable { found: text.to_owned() })?;
    Ok(graph.as_of(moment))
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
        let changed = codedoc_verify::history::changed_since(found.root(), revision)
            .ok_or_else(|| OpsError::RevisionUnreadable { revision: revision.to_string() })?;
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

fn written_by(record: &codedoc_ledger::Record, wanted: &str) -> bool {
    let needle = wanted.to_ascii_lowercase();
    let haystack = match &record.content().author {
        codedoc_ledger::Author::Human { identity } => identity.clone(),
        codedoc_ledger::Author::Agent { model, session } => format!("{model} {session}"),
        codedoc_ledger::Author::Analyzer { name } => name.clone(),
        codedoc_ledger::Author::Runtime { name } => name.clone(),
        _ => String::new(),
    };
    haystack.to_ascii_lowercase().contains(&needle)
}

pub fn list(
    root: &Path,
    scope: Option<Scope>,
    file: Option<&str>,
    symbol: Option<&str>,
    as_of: Option<&str>,
    author: Option<&str>,
    limit: Option<usize>,
) -> Outcome {
    let found = workspace_in(root, scope)?;
    let graph = at_moment(Graph::across(&found)?, as_of)?;
    let records = match (file, symbol) {
        (_, Some(wanted)) => graph.for_symbol(wanted),
        (Some(wanted), None) => graph.in_file(wanted),
        (None, None) => graph.active(),
    };
    let records: Vec<_> = match author {
        Some(wanted) => records.into_iter().filter(|record| written_by(record, wanted)).collect(),
        None => records,
    };

    let total = records.len();
    let rows: Vec<Value> = records
        .iter()
        .take(limit.unwrap_or(usize::MAX))
        .map(|record| {
            json!({
                "record": record.id().to_string(),
                "kind": record.kind().as_str(),
                "claim": record.content().body.claim,
                "detail": record.content().body.detail,
                "author": serde_json::to_value(&record.content().author).unwrap_or(Value::Null),
                "file": record.subject().map(|anchor| anchor.file.as_str().to_owned()),
                "symbol": record
                    .subject()
                    .and_then(|anchor| anchor.symbol.as_ref().map(ToString::to_string)),
                "created": record.content().created.to_rfc3339(),
            })
        })
        .collect();
    Ok(json!({
        "command": "list",
        "count": rows.len(),
        "total": total,
        "as_of": as_of,
        "author": author,
        "scopes": found.scopes().iter().map(|entry| entry.as_str()).collect::<Vec<_>>(),
        "records": rows,
    }))
}

pub fn search(
    root: &Path,
    scope: Option<Scope>,
    query: &str,
    kind: Option<&str>,
    file: Option<&str>,
    limit: usize,
) -> Outcome {
    let found = workspace_in(root, scope)?;
    let now = codedoc_ledger::Timestamp::now().unix_seconds();
    let mut scored: Vec<(f64, codedoc_ledger::Record)> = Vec::new();

    for ledger in found.ledgers() {
        let index = Index::current(ledger)
            .map_err(|source| OpsError::Index { detail: source.to_string() })?;
        let batch = index
            .search(query, limit.saturating_mul(4).max(limit))
            .map_err(|source| OpsError::Index { detail: source.to_string() })?;
        for (record, relevance) in batch {
            scored.push((relevance * codedoc_context::trust_of(&record, now), record));
        }
    }

    scored.retain(|(_, record)| {
        kind.is_none_or(|wanted| record.kind().as_str() == wanted)
            && file.is_none_or(|wanted| {
                record
                    .content()
                    .anchors
                    .iter()
                    .any(|entry| entry.anchor.file.as_str().starts_with(wanted))
            })
    });
    scored.sort_by(|left, right| {
        right
            .0
            .partial_cmp(&left.0)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| left.1.id().to_string().cmp(&right.1.id().to_string()))
    });
    scored.dedup_by(|left, right| left.1.id() == right.1.id());
    scored.truncate(limit);

    let rows: Vec<Value> = scored
        .iter()
        .map(|(score, record)| {
            json!({
                "record": record.id().to_string(),
                "kind": record.kind().as_str(),
                "claim": record.content().body.claim,
                "detail": record.content().body.detail,
                "file": record.subject().map(|anchor| anchor.file.as_str().to_owned()),
                "symbol": record
                    .subject()
                    .and_then(|anchor| anchor.symbol.as_ref().map(ToString::to_string)),
                "line": record.subject().map(|anchor| anchor.range.start_line),
                "assurance": record.content().assurance.as_str(),
                "score": (score * 1000.0).round() / 1000.0,
            })
        })
        .collect();

    Ok(json!({"command": "search", "query": query, "count": rows.len(), "records": rows}))
}

pub fn history(root: &Path, reference: &str) -> Outcome {
    let found = workspace(root)?;
    let graph = Graph::across(&found)?;

    if reference.contains("://") {
        return symbol_history(&graph, reference);
    }

    let original = find(root, reference)?;
    let chain = graph.revision_history(original.id());
    let standing: std::collections::BTreeSet<String> =
        graph.active().iter().map(|record| record.id().to_string()).collect();
    let rows: Vec<Value> = chain
        .iter()
        .enumerate()
        .map(|(position, entry)| {
            let restates_its_parent = position
                .checked_sub(1)
                .and_then(|previous| chain.get(previous))
                .is_some_and(|parent| {
                    parent.content().body == entry.content().body
                        && parent.content().evidence == entry.content().evidence
                });
            json!({
                "record": entry.id().to_string(),
                "kind": entry.kind().as_str(),
                "claim": entry.content().body.claim,
                "detail": entry.content().body.detail,
                "created": entry.content().created.to_rfc3339(),
                "code_revision": entry.content().code_revision.as_ref().map(ToString::to_string),
                "affirmation": restates_its_parent,
                "standing": if standing.contains(&entry.id().to_string()) {
                    "believed"
                } else if entry.kind() == codedoc_ledger::Kind::Tombstone {
                    "retraction"
                } else {
                    "superseded"
                },
            })
        })
        .collect();
    Ok(json!({"command": "history", "revisions": rows.len(), "chain": rows}))
}

fn symbol_history(graph: &Graph, symbol: &str) -> Outcome {
    let standing: std::collections::BTreeSet<String> =
        graph.active().iter().map(|record| record.id().to_string()).collect();

    let mut everything: Vec<&codedoc_ledger::Record> = graph
        .all()
        .iter()
        .filter(|record| {
            record.content().anchors.iter().any(|entry| {
                entry.anchor.symbol.as_ref().is_some_and(|named| named.to_string() == symbol)
            })
        })
        .collect();
    if everything.is_empty() {
        return Err(OpsError::RecordMissing { reference: symbol.to_owned() });
    }
    everything.sort_by_key(|record| (record.content().created, record.id().to_string()));

    let rows: Vec<Value> = everything
        .iter()
        .map(|record| {
            let superseded_by = graph
                .all()
                .iter()
                .find(|later| later.content().parent == Some(record.id()))
                .map(|later| later.id().to_string());
            json!({
                "record": record.id().to_string(),
                "kind": record.kind().as_str(),
                "claim": record.content().body.claim,
                "detail": record.content().body.detail,
                "created": record.content().created.to_rfc3339(),
                "code_revision": record.content().code_revision.as_ref().map(ToString::to_string),
                "standing": if standing.contains(&record.id().to_string()) {
                    "believed"
                } else if record.kind() == codedoc_ledger::Kind::Tombstone {
                    "retraction"
                } else {
                    "withdrawn"
                },
                "superseded_by": superseded_by,
            })
        })
        .collect();

    Ok(json!({
        "command": "history",
        "symbol": symbol,
        "revisions": rows.len(),
        "chain": rows,
    }))
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
            return Err(OpsError::UnknownFormat {
                found: other.to_owned(),
                vocabulary: "markdown and mermaid".to_owned(),
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
    let changed = codedoc_verify::history::changed_since(found.root(), base)
        .ok_or_else(|| OpsError::RevisionUnreadable { revision: base.to_string() })?;

    let (payload, code) = verify_scoped(root, &changed, None)?;
    let findings: Vec<codedoc_verify::Finding> =
        serde_json::from_value(payload["findings"].clone()).unwrap_or_default();

    let review_graph = Graph::across(&found)?;
    let recorded_here = review_graph
        .active()
        .iter()
        .filter(|record| {
            record.content().code_revision.as_ref().is_some_and(|revision| {
                !codedoc_verify::history::is_ancestor(found.root(), revision.as_str(), base)
            })
        })
        .count();
    let mut stale = Vec::new();
    let mut edited = Vec::new();
    let mut detached = Vec::new();
    let mut unchanged = 0usize;
    let mut moved = 0usize;
    for finding in &findings {
        if matches!(
            finding.status,
            codedoc_verify::Status::Fresh | codedoc_verify::Status::Migrated
        ) && finding.drift.is_some_and(|amount| amount > 0)
        {
            moved += 1;
        }
        let symbol = finding.symbol.clone().unwrap_or_else(|| finding.file.clone());
        let unresolved = || codedoc_render::StaleClaim {
            file: finding.file.clone(),
            symbol: symbol.clone(),
            claim: finding.claim.clone(),
            drift: finding.drift,
            relocated_to: finding.relocated_to.clone(),
        };
        match finding.status {
            codedoc_verify::Status::Stale => stale.push(unresolved()),
            codedoc_verify::Status::Detached => {
                let hint = suggestions_for(found.root(), finding, &review_graph).first().and_then(
                    |value| value.get("symbol").and_then(|name| name.as_str()).map(str::to_owned),
                );
                detached.push((finding.file.clone(), symbol, finding.claim.clone(), hint))
            }
            codedoc_verify::Status::Fresh | codedoc_verify::Status::Migrated => {
                if finding.content_changed {
                    edited.push(unresolved());
                } else {
                    unchanged += 1;
                }
            }
            _ => stale.push(unresolved()),
        }
    }

    let (touched, undocumented) =
        crate::coverage::touched_declarations(found.root(), &changed).unwrap_or((0, 0));

    let rendered = codedoc_render::review_markdown(&codedoc_render::ReviewInput {
        base,
        files: &changed,
        stale,
        edited,
        detached,
        unchanged,
        moved,
        recorded_here,
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

pub fn stats(root: &Path, scope: Option<Scope>) -> Outcome {
    let found = workspace_in(root, scope)?;
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
