use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use codedoc_anchor::{Anchor, locate};
use codedoc_context::{Target, assemble};
use codedoc_core::RepoPath;
use codedoc_graph::Graph;
use codedoc_index::Index;
use codedoc_lang::Registry;
use codedoc_ledger::{
    AnchorRole, Assurance, Author, Body, Kind, Ledger, Lifecycle, RecordContent, Role,
    SCHEMA_VERSION, Timestamp,
};
use codedoc_verify::Verifier;
use serde_json::{Value, json};

fn open(root: &Path) -> Result<Ledger, String> {
    Ledger::open(root).map_err(|failure| failure.to_string())
}

pub fn context(root: &Path, arguments: &Value) -> Result<Value, String> {
    let ledger = open(root)?;
    let graph = Graph::load(&ledger).map_err(|failure| failure.to_string())?;
    let file = arguments
        .get("file")
        .and_then(Value::as_str)
        .ok_or_else(|| "file is required".to_owned())?;
    let normalised = RepoPath::parse(file)
        .map(|path| path.as_str().to_owned())
        .map_err(|failure| failure.to_string())?;

    let pack = assemble(
        &graph,
        Target {
            file: normalised,
            line: arguments.get("line").and_then(Value::as_u64).map(|line| line as u32),
            symbol: arguments.get("symbol").and_then(Value::as_str).map(str::to_owned),
        },
        arguments
            .get("depth")
            .and_then(Value::as_u64)
            .map(|depth| depth as u8)
            .unwrap_or(codedoc_context::DEFAULT_DEPTH),
    );
    serde_json::to_value(&pack).map_err(|failure| failure.to_string())
}

pub fn attach(root: &Path, arguments: &Value) -> Result<Value, String> {
    let ledger = open(root)?;
    let file = arguments
        .get("file")
        .and_then(Value::as_str)
        .ok_or_else(|| "file is required".to_owned())?;
    let kind = arguments
        .get("kind")
        .and_then(Value::as_str)
        .and_then(Kind::parse)
        .ok_or_else(|| format!("kind must be one of {:?}", Kind::vocabulary()))?;
    let claim = arguments
        .get("claim")
        .and_then(Value::as_str)
        .ok_or_else(|| "claim is required".to_owned())?;

    let path = RepoPath::parse(file).map_err(|failure| failure.to_string())?;
    let adapter = Registry::for_path(&path).map_err(|failure| failure.to_string())?;
    let source = fs::read_to_string(root.join(path.as_str()))
        .map_err(|failure| format!("reading {file}: {failure}"))?;
    let tree = adapter.parse(&source).map_err(|failure| failure.to_string())?;

    let node = match (
        arguments.get("symbol").and_then(Value::as_str),
        arguments.get("line").and_then(Value::as_u64),
    ) {
        (Some(symbol), _) => locate::by_symbol(&tree, &source, adapter, symbol)
            .ok_or_else(|| format!("no symbol {symbol} in {file}"))?,
        (None, Some(line)) => locate::by_line(&tree, adapter, line as u32)
            .ok_or_else(|| format!("line {line} covers no node in {file}"))?,
        (None, None) => return Err("attach requires symbol or line".to_owned()),
    };

    let anchor = Anchor::capture(path, adapter, &source, node);
    let assurance = arguments
        .get("assurance")
        .and_then(Value::as_str)
        .and_then(Assurance::parse)
        .unwrap_or(Assurance::Inferred);

    let content = RecordContent {
        schema: SCHEMA_VERSION,
        kind,
        anchors: vec![AnchorRole { role: Role::Subject, anchor: anchor.clone() }],
        body: Body {
            claim: claim.to_owned(),
            detail: arguments.get("detail").and_then(Value::as_str).map(str::to_owned),
        },
        evidence: Vec::new(),
        assurance,
        author: Author::Agent {
            model: arguments
                .get("model")
                .and_then(Value::as_str)
                .unwrap_or("unidentified")
                .to_owned(),
            session: arguments
                .get("session")
                .and_then(Value::as_str)
                .unwrap_or("unrecorded")
                .to_owned(),
        },
        code_revision: None,
        created: Timestamp::now(),
        lifecycle: Lifecycle::Active,
        parent: None,
        chain: None,
        unrecognised: BTreeMap::new(),
    };

    let record = ledger.append(content).map_err(|failure| failure.to_string())?;
    Index::append(&ledger, &record).map_err(|failure| failure.to_string())?;

    Ok(json!({
        "record": record.id().to_string(),
        "kind": record.kind().as_str(),
        "symbol": anchor.symbol.as_ref().map(ToString::to_string),
        "range": anchor.range.to_string(),
    }))
}

pub fn verify(root: &Path) -> Result<Value, String> {
    let ledger = open(root)?;
    let report = Verifier::new(root).run(&ledger).map_err(|failure| failure.to_string())?;
    Ok(json!({
        "counts": report.counts(),
        "records": report.records,
        "integrity_intact": report.integrity_intact,
        "exit_code": report.exit_code(),
        "findings": serde_json::to_value(&report.findings).unwrap_or(Value::Null),
    }))
}

pub fn list(root: &Path, arguments: &Value) -> Result<Value, String> {
    let ledger = open(root)?;
    let graph = Graph::load(&ledger).map_err(|failure| failure.to_string())?;
    let records = match (
        arguments.get("file").and_then(Value::as_str),
        arguments.get("symbol").and_then(Value::as_str),
    ) {
        (_, Some(symbol)) => graph.for_symbol(symbol),
        (Some(file), None) => graph.in_file(file),
        (None, None) => graph.active(),
    };
    let rows: Vec<Value> = records
        .iter()
        .map(|record| {
            json!({
                "record": record.id().to_string(),
                "kind": record.kind().as_str(),
                "claim": record.content().body.claim,
                "symbol": record
                    .subject()
                    .and_then(|anchor| anchor.symbol.as_ref().map(ToString::to_string)),
            })
        })
        .collect();
    Ok(json!({"count": rows.len(), "records": rows}))
}
