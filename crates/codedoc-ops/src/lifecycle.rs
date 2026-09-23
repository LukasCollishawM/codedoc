use std::fs;
use std::path::Path;

use codedoc_anchor::{Anchor, FileIndex, Resolution};
use codedoc_graph::Graph;
use codedoc_lang::Registry;
use codedoc_ledger::{Body, Kind, Record, Scope};
use serde_json::json;

use crate::record::{Target, append, capture, draft};
use crate::{Attribution, OpsError, Outcome, workspace, writable};

pub type Relocation = Target;

pub(crate) fn find(root: &Path, reference: &str) -> Result<Record, OpsError> {
    let found = workspace(root)?;
    let graph = Graph::across(&found)?;

    let matches: Vec<&Record> = graph
        .all()
        .iter()
        .filter(|record| record.id().to_string().starts_with(reference))
        .collect();
    match matches.as_slice() {
        [single] => Ok((*single).clone()),
        [] => Err(OpsError::RecordMissing { reference: reference.to_owned() }),
        many => {
            Err(OpsError::RecordAmbiguous { reference: reference.to_owned(), count: many.len() })
        }
    }
}

fn current_position(root: &Path, anchor: &Anchor) -> Result<Anchor, OpsError> {
    let adapter = Registry::for_path(&anchor.file)
        .map_err(|source| OpsError::Language { detail: source.to_string() })?;
    let source = fs::read_to_string(root.join(anchor.file.as_str())).map_err(|source| {
        OpsError::Unreadable { path: anchor.file.to_string(), detail: source.to_string() }
    })?;
    let tree = adapter
        .parse(&source)
        .map_err(|source| OpsError::Language { detail: source.to_string() })?;
    let index = FileIndex::build(adapter, &source, &tree);

    match index.resolve(anchor) {
        Resolution::Located(located) => {
            let node = located.node_path().descend(tree.root_node()).ok_or_else(|| {
                OpsError::Detached {
                    detail: "the resolved position could not be re-read".to_owned(),
                }
            })?;
            Ok(Anchor::capture(anchor.file.clone(), adapter, &source, node))
        }
        Resolution::Detached(reason) => Err(OpsError::Detached {
            detail: format!(
                "this record's anchor is detached ({reason:?}); place it explicitly with resolve \
                 rather than superseding blindly"
            ),
        }),
        _ => Err(OpsError::Detached { detail: "unrecognised resolution outcome".to_owned() }),
    }
}

fn emit(
    root: &Path,
    scope: Option<Scope>,
    original: &Record,
    anchor: Anchor,
    kind: Kind,
    body: Body,
) -> Result<Record, OpsError> {
    let ledger = writable(root, scope)?;
    let attribution = Attribution {
        author: original.content().author.clone(),
        assurance: original.content().assurance,
    };
    let mut content = draft(
        kind,
        vec![codedoc_ledger::AnchorRole { role: codedoc_ledger::Role::Subject, anchor }],
        &body.claim,
        body.detail.as_deref(),
        &attribution,
        original.content().evidence.clone(),
        original.content().code_revision.clone(),
    );
    content.parent = Some(original.id());
    append(&ledger, content)
}

pub fn supersede(
    root: &Path,
    scope: Option<Scope>,
    reference: &str,
    claim: Option<&str>,
    detail: Option<&str>,
    kind: Option<&str>,
) -> Outcome {
    let original = find(root, reference)?;
    let anchor = original
        .subject()
        .ok_or_else(|| OpsError::NoSubject { reference: reference.to_owned() })?;
    let relocated = current_position(root, anchor)?;

    let kind = match kind {
        Some(name) => Kind::parse(name).ok_or_else(|| OpsError::UnknownKind {
            found: name.to_owned(),
            vocabulary: Kind::vocabulary().join(", "),
        })?,
        None => original.kind(),
    };
    let body = Body {
        claim: claim.unwrap_or(&original.content().body.claim).to_owned(),
        detail: detail.map(str::to_owned).or_else(|| original.content().body.detail.clone()),
    };

    let record = emit(root, scope, &original, relocated.clone(), kind, body)?;
    Ok(json!({
        "command": "supersede",
        "record": record.id().to_string(),
        "supersedes": original.id().to_string(),
        "kind": record.kind().as_str(),
        "range": relocated.range.to_string(),
        "claim": record.content().body.claim,
    }))
}

pub fn resolve(
    root: &Path,
    scope: Option<Scope>,
    reference: &str,
    relocation: &Relocation,
) -> Outcome {
    let original = find(root, reference)?;
    let anchor = original
        .subject()
        .ok_or_else(|| OpsError::NoSubject { reference: reference.to_owned() })?;

    let mut target = relocation.clone();
    if target.file.is_empty() {
        target.file = anchor.file.as_str().to_owned();
    }
    let found = workspace(root)?;
    let placed = capture(found.root(), &target)?;
    let body = original.content().body.clone();
    let kind = original.kind();
    let record = emit(root, scope, &original, placed.clone(), kind, body)?;

    Ok(json!({
        "command": "resolve",
        "record": record.id().to_string(),
        "readopts": original.id().to_string(),
        "file": placed.file.as_str(),
        "symbol": placed.symbol.as_ref().map(ToString::to_string),
        "range": placed.range.to_string(),
    }))
}

pub fn retract(
    root: &Path,
    scope: Option<Scope>,
    reference: &str,
    reason: Option<&str>,
) -> Outcome {
    let original = find(root, reference)?;
    let anchor = original
        .subject()
        .ok_or_else(|| OpsError::NoSubject { reference: reference.to_owned() })?
        .clone();

    let body = Body {
        claim: reason.unwrap_or("Retracted.").to_owned(),
        detail: Some(format!("Retracts: {}", original.content().body.claim)),
    };
    let record = emit(root, scope, &original, anchor, Kind::Tombstone, body)?;

    Ok(json!({
        "command": "retract",
        "record": record.id().to_string(),
        "retracts": original.id().to_string(),
        "was": original.content().body.claim,
    }))
}
