use std::collections::BTreeMap;
use std::path::Path;

use codedoc_core::RecordId;
use codedoc_ledger::{Record, RecordContent, Scope};
use serde_json::json;

use crate::{OpsError, Outcome, workspace, writable};

pub fn repair(root: &Path, scope: Option<Scope>, write: bool) -> Outcome {
    let found = workspace(root)?;
    let ledger = writable(found.root(), scope)?;
    let records = ledger.records()?;

    let known: Vec<String> = records.iter().map(|record| record.id().to_string()).collect();
    let orphans: Vec<&Record> = records
        .iter()
        .filter(|record| {
            record.content().chain.as_ref().is_some_and(|head| !known.contains(&head.to_string()))
        })
        .collect();

    if orphans.is_empty() {
        return Ok(json!({
            "command": "repair",
            "scope": ledger.scope().as_str(),
            "records": records.len(),
            "orphans": 0,
            "rewritten": 0,
            "dry_run": !write,
        }));
    }

    if !write {
        return Ok(json!({
            "command": "repair",
            "scope": ledger.scope().as_str(),
            "records": records.len(),
            "orphans": orphans.len(),
            "rewritten": 0,
            "dry_run": true,
            "sample": orphans
                .iter()
                .take(5)
                .map(|record| json!({
                    "record": record.id().to_string(),
                    "claim": record.content().body.claim,
                }))
                .collect::<Vec<_>>(),
        }));
    }

    let mut ordered = records.clone();
    ordered.sort_by_key(|record| (record.content().created, record.id().to_string()));

    let mut remapped: BTreeMap<String, RecordId> = BTreeMap::new();
    let mut rebuilt: Vec<Record> = Vec::new();
    let mut head: Option<codedoc_core::LedgerHead> = None;

    for record in &ordered {
        let mut content: RecordContent = record.content().clone();
        content.chain = head;
        if let Some(parent) = content.parent {
            let original = parent.to_string();
            content.parent = remapped.get(&original).copied().or(Some(parent));
        }
        let sealed = Record::seal(content)
            .map_err(|source| OpsError::Ledger { detail: source.to_string() })?;
        remapped.insert(record.id().to_string(), sealed.id());
        head = sealed.id().to_string().parse().ok();
        rebuilt.push(sealed);
    }

    ledger.replace_all(&rebuilt)?;
    codedoc_index::Index::rebuild(&ledger)
        .map_err(|source| OpsError::Index { detail: source.to_string() })?;

    Ok(json!({
        "command": "repair",
        "scope": ledger.scope().as_str(),
        "records": records.len(),
        "orphans": orphans.len(),
        "rewritten": rebuilt.len(),
        "dry_run": false,
    }))
}
