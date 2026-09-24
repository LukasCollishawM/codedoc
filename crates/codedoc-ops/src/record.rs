use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use codedoc_anchor::{Anchor, locate};
use codedoc_core::RepoPath;
use codedoc_index::Index;
use codedoc_ledger::{
    AnchorRole, Assurance, Body, Evidence, Kind, Ledger, Lifecycle, RecordContent, RelationVerb,
    Role, SCHEMA_VERSION, Scope, Timestamp,
};
use serde_json::{Value, json};

use crate::author::Attribution;
use crate::{OpsError, Outcome, writable};

#[derive(Debug, Clone)]
pub struct Target {
    pub file: String,
    pub symbol: Option<String>,
    pub line: Option<u32>,
}

impl Target {
    pub fn symbol(file: &str, symbol: &str) -> Self {
        Target { file: file.to_owned(), symbol: Some(symbol.to_owned()), line: None }
    }

    pub fn line(file: &str, line: u32) -> Self {
        Target { file: file.to_owned(), symbol: None, line: Some(line) }
    }

    pub fn file(file: &str) -> Self {
        Target { file: file.to_owned(), symbol: None, line: None }
    }
}

pub(crate) fn capture(root: &Path, target: &Target) -> Result<Anchor, OpsError> {
    let path = RepoPath::parse(&target.file)
        .map_err(|source| OpsError::Language { detail: source.to_string() })?;
    let adapter = codedoc_lang::Registry::for_path(&path)
        .map_err(|source| OpsError::Language { detail: source.to_string() })?;
    let source = fs::read_to_string(root.join(path.as_str())).map_err(|source| {
        OpsError::Unreadable { path: target.file.clone(), detail: source.to_string() }
    })?;
    let tree = adapter
        .parse(&source)
        .map_err(|source| OpsError::Language { detail: source.to_string() })?;

    let node = match (&target.symbol, target.line) {
        (Some(symbol), _) => {
            locate::by_symbol(&tree, &source, adapter, symbol).ok_or_else(|| {
                OpsError::SymbolMissing {
                    symbol: symbol.clone(),
                    path: target.file.clone(),
                    nearest: nearest_symbols(root, &path, symbol),
                }
            })?
        }
        (None, Some(line)) => locate::by_line(&tree, adapter, line).ok_or_else(|| {
            OpsError::LineMissing { line, path: target.file.clone(), lines: source.lines().count() }
        })?,
        (None, None) => {
            return Ok(Anchor::capture_file(path, adapter, &source, tree.root_node()));
        }
    };
    Ok(Anchor::capture(path, adapter, &source, node))
}

fn nearest_symbols(root: &Path, relative: &RepoPath, wanted: &str) -> String {
    let Some(found) = crate::coverage::declarations_in(root, relative) else {
        return String::new();
    };
    let target = wanted.rsplit(['/', ':']).find(|part| !part.is_empty()).unwrap_or(wanted);
    let mut ranked: Vec<(usize, &String)> = found
        .declared
        .iter()
        .map(|declaration| {
            let candidate = &declaration.symbol;
            let terminal =
                candidate.rsplit(['/', ':']).find(|part| !part.is_empty()).unwrap_or(candidate);
            let shared = terminal
                .chars()
                .zip(target.chars())
                .take_while(|(left, right)| left.eq_ignore_ascii_case(right))
                .count();
            (usize::MAX - shared, candidate)
        })
        .collect();
    ranked.sort();
    let names: Vec<&str> = ranked.iter().take(4).map(|(_, name)| name.as_str()).collect();
    names.join(", ")
}

pub(crate) fn append(
    ledger: &Ledger,
    content: RecordContent,
) -> Result<codedoc_ledger::Record, OpsError> {
    let record = ledger.append(content)?;
    Index::append(ledger, &record)
        .map_err(|source| OpsError::Index { detail: source.to_string() })?;
    Ok(record)
}

pub(crate) fn draft(
    kind: Kind,
    anchors: Vec<AnchorRole>,
    claim: &str,
    detail: Option<&str>,
    attribution: &Attribution,
    evidence: Vec<Evidence>,
    revision: Option<codedoc_core::GitRev>,
) -> RecordContent {
    RecordContent {
        schema: SCHEMA_VERSION,
        kind,
        anchors,
        body: Body { claim: claim.to_owned(), detail: detail.map(str::to_owned) },
        evidence,
        assurance: attribution.assurance,
        author: attribution.author.clone(),
        code_revision: revision,
        created: Timestamp::now(),
        lifecycle: Lifecycle::Active,
        parent: None,
        chain: None,
        unrecognised: BTreeMap::new(),
    }
}

#[derive(Debug, Clone, Default)]
pub struct Provenance {
    pub assurance: Option<Assurance>,
    pub evidence: Vec<Evidence>,
    pub revision: Option<codedoc_core::GitRev>,
}

#[derive(Debug, Clone)]
pub struct AttachRequest {
    pub target: Target,
    pub kind: String,
    pub claim: String,
    pub detail: Option<String>,
}

#[derive(Debug, Clone)]
pub struct RelateRequest {
    pub subject: Target,
    pub verb: String,
    pub object: Target,
    pub claim: Option<String>,
    pub detail: Option<String>,
}

pub fn attach(
    root: &Path,
    scope: Option<Scope>,
    request: &AttachRequest,
    attribution: &Attribution,
    provenance: Provenance,
) -> Outcome {
    let Provenance { assurance, evidence, revision } = provenance;
    let target = &request.target;
    let claim = request.claim.as_str();
    let detail = request.detail.as_deref();
    let kind = Kind::parse(&request.kind).ok_or_else(|| OpsError::UnknownKind {
        found: request.kind.clone(),
        vocabulary: Kind::vocabulary().join(", "),
    })?;
    let ledger = writable(root, scope)?;
    let anchor = capture(ledger.root(), target)?;
    let attribution = attribution.clone().with_assurance(assurance);

    let content = draft(
        kind,
        vec![AnchorRole { role: Role::Subject, anchor: anchor.clone() }],
        claim,
        detail,
        &attribution,
        evidence,
        revision,
    );
    let similar = already_recorded(&ledger, &anchor, claim)?;
    let echoes_the_name =
        restates_the_symbol(claim, anchor.symbol.as_ref().map(ToString::to_string).as_deref());
    let record = append(&ledger, content)?;

    Ok(json!({
        "command": "attach",
        "similar": similar,
        "restates_the_symbol": echoes_the_name,
        "record": record.id().to_string(),
        "scope": ledger.scope().as_str(),
        "kind": record.kind().as_str(),
        "file": anchor.file.as_str(),
        "symbol": anchor.symbol.as_ref().map(ToString::to_string),
        "range": anchor.range.to_string(),
        "assurance": attribution.assurance.as_str(),
        "claim": claim,
    }))
}

const IDLE_WORDS: &[&str] = &[
    "the", "a", "an", "this", "that", "these", "those", "it", "its", "is", "are", "was", "be",
    "will", "would", "should", "can", "may", "must", "and", "or", "but", "for", "of", "to", "in",
    "on", "at", "by", "with", "from", "as", "into", "then", "than", "when", "which", "we", "you",
    "here", "there", "any", "all", "each", "every", "given", "used", "using", "does", "do",
];

fn content_words(text: &str) -> Vec<String> {
    text.split(|glyph: char| !glyph.is_alphanumeric())
        .map(str::to_lowercase)
        .filter(|word| word.chars().count() > 2 && !IDLE_WORDS.contains(&word.as_str()))
        .collect()
}

fn identifier_words(symbol: &str) -> Vec<String> {
    let terminal = symbol.rsplit(['/', ':']).find(|part| !part.is_empty()).unwrap_or(symbol);
    let mut words = Vec::new();
    let mut current = String::new();
    for glyph in terminal.chars() {
        if !glyph.is_alphanumeric() {
            if !current.is_empty() {
                words.push(std::mem::take(&mut current));
            }
            continue;
        }
        if glyph.is_uppercase() && !current.is_empty() {
            words.push(std::mem::take(&mut current));
        }
        current.push(glyph.to_ascii_lowercase());
    }
    if !current.is_empty() {
        words.push(current);
    }
    words.retain(|word| word.chars().count() > 2);
    words
}

fn shares_a_stem(left: &str, right: &str) -> bool {
    let shared = left.chars().zip(right.chars()).take_while(|(a, b)| a == b).count();
    shared >= 4 && shared >= left.chars().count().min(right.chars().count())
}

fn restates_the_symbol(claim: &str, symbol: Option<&str>) -> bool {
    let Some(symbol) = symbol else { return false };
    let named = identifier_words(symbol);
    if named.is_empty() {
        return false;
    }
    let said = content_words(claim);
    if said.is_empty() {
        return false;
    }
    said.iter().all(|word| named.iter().any(|part| shares_a_stem(word, part)))
}

fn already_recorded(ledger: &Ledger, anchor: &Anchor, claim: &str) -> Result<Vec<Value>, OpsError> {
    let index =
        Index::current(ledger).map_err(|source| OpsError::Index { detail: source.to_string() })?;
    let nearby = match anchor.symbol.as_ref() {
        Some(symbol) => index.active_for_symbol(&symbol.to_string()),
        None => index.active_in_file(anchor.file.as_str()),
    }
    .map_err(|source| OpsError::Index { detail: source.to_string() })?;

    let mut scored: Vec<(u32, &codedoc_ledger::Record)> = nearby
        .iter()
        .map(|found| {
            let score = codedoc_graph::claim_similarity(&found.content().body.claim, claim);
            ((score * 100.0) as u32, found)
        })
        .filter(|(score, _)| f64::from(*score) / 100.0 >= codedoc_graph::NEAR_DUPLICATE_FLOOR)
        .collect();
    scored.sort_by_key(|(score, _)| std::cmp::Reverse(*score));
    scored.truncate(3);

    Ok(scored
        .iter()
        .map(|(score, found)| {
            json!({
                "record": found.id().to_string(),
                "kind": found.kind().as_str(),
                "similarity": score,
                "claim": found.content().body.claim,
            })
        })
        .collect())
}

pub fn relate(
    root: &Path,
    scope: Option<Scope>,
    request: &RelateRequest,
    attribution: &Attribution,
    provenance: Provenance,
) -> Outcome {
    let Provenance { assurance, revision, .. } = provenance;
    let (subject, object) = (&request.subject, &request.object);
    let claim = request.claim.as_deref();
    let detail = request.detail.as_deref();
    let verb = RelationVerb::parse(&request.verb).ok_or_else(|| OpsError::UnknownVerb {
        found: request.verb.clone(),
        vocabulary: RelationVerb::vocabulary().join(", "),
    })?;
    let ledger = writable(root, scope)?;
    let subject_anchor = capture(ledger.root(), subject)?;
    let object_anchor = capture(ledger.root(), object)?;
    let attribution = attribution.clone().with_assurance(assurance);

    let rendered = claim.map(str::to_owned).unwrap_or_else(|| {
        format!(
            "{} {} {}",
            describe(&subject_anchor),
            verb.as_str().replace('_', " "),
            describe(&object_anchor)
        )
    });

    let content = draft(
        Kind::Relation(verb),
        vec![
            AnchorRole { role: Role::Subject, anchor: subject_anchor.clone() },
            AnchorRole { role: Role::Object, anchor: object_anchor.clone() },
        ],
        &rendered,
        detail,
        &attribution,
        Vec::new(),
        revision,
    );
    let record = append(&ledger, content)?;

    Ok(json!({
        "command": "relate",
        "record": record.id().to_string(),
        "scope": ledger.scope().as_str(),
        "verb": verb.as_str(),
        "subject": describe(&subject_anchor),
        "object": describe(&object_anchor),
        "claim": rendered,
    }))
}

fn describe(anchor: &Anchor) -> String {
    anchor
        .symbol
        .as_ref()
        .map(ToString::to_string)
        .unwrap_or_else(|| format!("{}:{}", anchor.file.as_str(), anchor.range.start_line))
}
