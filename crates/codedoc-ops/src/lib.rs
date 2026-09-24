#![forbid(unsafe_code)]

mod author;
mod coverage;
mod doc_tags;
mod evidence;
mod gaps;
mod health;
mod import;
mod lifecycle;
mod query;
mod record;
mod repair;
mod revision;
mod setup;

pub use author::{Attribution, parse_assurance};
pub use coverage::coverage;
pub use evidence::evidence;
pub use gaps::gaps;
pub use health::{doctor, health};
pub use import::import;
pub use lifecycle::{Relocation, affirm, resolve, retract, supersede};
pub use query::{
    brief, conflicts, context, detached, history, list, render, review, search, stats, verify,
    verify_scoped,
};
pub use record::{AttachRequest, Provenance, RelateRequest, Target, attach, relate};
pub use repair::repair;
pub use revision::head_revision;
pub use setup::initialise;

use std::path::Path;

use codedoc_core::readable_path;
use codedoc_ledger::{Ledger, LedgerError, Scope, Workspace};
use thiserror::Error;

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum OpsError {
    #[error(
        "no ledger found at {root} — create one with codedoc_init, or `codedoc init --scope \
         local` to keep it inside .git/ where the repository cannot track it, or `--scope \
         shared` to commit it"
    )]
    NoLedger { root: String },

    #[error(
        "{path} is a directory. A claim is about a file, or about a construct inside one; \
         name the file it belongs to"
    )]
    NotAFile { path: String },

    #[error("{found} is not a date; use YYYY-MM-DD or an RFC 3339 instant")]
    MomentUnreadable { found: String },

    #[error("name at least one file, or --since a revision")]
    TargetUnnamed,

    #[error(
        "could not read what changed since {revision}. Name a revision this clone has: a \
         shallow checkout carries only the commits it was given, and actions/checkout takes \
         depth 1 by default, so comparing against a merge base needs fetch-depth: 0"
    )]
    RevisionUnreadable { revision: String },

    #[error("{detail}")]
    Ledger { detail: String },

    #[error("{detail}")]
    Language { detail: String },

    #[error("{path} could not be read: {detail}")]
    Unreadable { path: String, detail: String },

    #[error("no symbol {symbol} in {path}. It declares: {nearest}")]
    SymbolMissing { symbol: String, path: String, nearest: String },

    #[error(
        "line {line} of {path} covers no code; the file has {lines} lines. A blank \
         line, or one holding only a comment, has nothing to anchor to — name the \
         symbol instead, which also survives the code moving"
    )]
    LineMissing { line: u32, path: String, lines: usize },

    #[error(
        "{path} has no language adapter ({detail}), so nothing inside it can be named. Drop \
         --symbol and --line to record the claim against the file itself, which resolves by \
         path and needs no parse"
    )]
    Opaque { path: String, detail: String },

    #[error(
        "{path} is not in this repository. Scanning it would report nothing found, which \
         reads as an answer about your code rather than about the path"
    )]
    PathMissing { path: String },

    #[error(
        "a record needs a claim. An empty one is indistinguishable from no record at all \
         everywhere it is read, and it cannot be superseded by something better because \
         nobody can tell what it said"
    )]
    EmptyClaim,

    #[error("unknown record kind {found}; the vocabulary is {vocabulary}")]
    UnknownKind { found: String, vocabulary: String },

    #[error(
        "unknown assurance {found}; expected one of {vocabulary}. Ignoring it would \
         record whatever the author's default is, which is a confidence nobody chose"
    )]
    UnknownAssurance { found: String, vocabulary: String },

    #[error("{found} is not a relation verb; expected one of {vocabulary}")]
    UnknownVerb { found: String, vocabulary: String },

    #[error("no record in this ledger begins with {reference}")]
    RecordMissing { reference: String },

    #[error("{reference} is ambiguous across {count} records; use more characters")]
    RecordAmbiguous { reference: String, count: usize },

    #[error("record {reference} has no subject anchor")]
    NoSubject { reference: String },

    #[error("{detail}")]
    Detached { detail: String },

    #[error("the index could not be updated: {detail}")]
    Index { detail: String },
}

impl From<LedgerError> for OpsError {
    fn from(source: LedgerError) -> Self {
        OpsError::Ledger { detail: source.to_string() }
    }
}

pub type Outcome = Result<serde_json::Value, OpsError>;

pub(crate) fn workspace(root: &Path) -> Result<Workspace, OpsError> {
    Workspace::discover(root).map_err(|_| OpsError::NoLedger { root: readable_path(root) })
}

impl OpsError {
    pub fn exit_code(&self) -> u8 {
        match self {
            OpsError::Ledger { .. } | OpsError::Index { .. } => 3,
            _ => 4,
        }
    }
}

pub(crate) fn require_paths(
    base: &Path,
    paths: &[String],
    invoked_from: &Path,
) -> Result<Vec<String>, OpsError> {
    paths
        .iter()
        .map(|given| {
            let relative = repo_relative_from(base, given, invoked_from);
            if base.join(&relative).exists() {
                Ok(relative)
            } else {
                Err(OpsError::PathMissing { path: given.clone() })
            }
        })
        .collect()
}

pub(crate) fn repo_relative_from(base: &Path, given: &str, invoked_from: &Path) -> String {
    let canonical = |path: &Path| std::fs::canonicalize(path).ok();
    let Some(base) = canonical(base) else {
        return given.to_owned();
    };
    if canonical(&base.join(given)).is_some() {
        return given.to_owned();
    }
    let Some(resolved) = canonical(&invoked_from.join(given)) else {
        return given.to_owned();
    };
    match resolved.strip_prefix(&base) {
        Ok(relative) => relative.to_string_lossy().replace('\\', "/"),
        Err(_) => given.to_owned(),
    }
}

pub(crate) fn repo_relative(found: &Workspace, given: &str, invoked_from: &Path) -> String {
    let canonical = |path: &Path| std::fs::canonicalize(path).ok();
    let Some(base) = canonical(found.root()) else {
        return given.to_owned();
    };
    if canonical(&base.join(given)).is_some() {
        return given.to_owned();
    }
    let Some(resolved) = canonical(&invoked_from.join(given)) else {
        return given.to_owned();
    };
    match resolved.strip_prefix(&base) {
        Ok(relative) => relative.to_string_lossy().replace('\\', "/"),
        Err(_) => given.to_owned(),
    }
}

pub(crate) fn workspace_in(root: &Path, scope: Option<Scope>) -> Result<Workspace, OpsError> {
    Ok(workspace(root)?.confined_to(scope))
}

pub(crate) fn writable(root: &Path, scope: Option<Scope>) -> Result<Ledger, OpsError> {
    let found = workspace(root)?;
    let target = found.write_target(scope)?;
    let chosen = target.scope();
    Ledger::open_scope(found.root(), chosen).map_err(OpsError::from)
}
