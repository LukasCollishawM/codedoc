#![forbid(unsafe_code)]

mod author;
mod coverage;
mod import;
mod lifecycle;
mod query;
mod record;
mod repair;
mod revision;

pub use author::Attribution;
pub use coverage::coverage;
pub use import::import;
pub use lifecycle::{Relocation, resolve, retract, supersede};
pub use query::{
    conflicts, context, detached, history, list, render, review, stats, verify, verify_scoped,
};
pub use record::{AttachRequest, Provenance, RelateRequest, Target, attach, relate};
pub use repair::repair;
pub use revision::head_revision;

use std::path::Path;

use codedoc_ledger::{Ledger, LedgerError, Scope, Workspace};
use thiserror::Error;

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum OpsError {
    #[error("no ledger found at {root}; run codedoc init")]
    NoLedger { root: String },

    #[error("{detail}")]
    Ledger { detail: String },

    #[error("{detail}")]
    Language { detail: String },

    #[error("{path} could not be read: {detail}")]
    Unreadable { path: String, detail: String },

    #[error("no symbol {symbol} found in {path}")]
    SymbolMissing { symbol: String, path: String },

    #[error("line {line} covers no node in {path}")]
    LineMissing { line: u32, path: String },

    #[error("unknown record kind {found}; the vocabulary is {vocabulary}")]
    UnknownKind { found: String, vocabulary: String },

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
    Workspace::discover(root).map_err(|_| OpsError::NoLedger { root: root.display().to_string() })
}

pub(crate) fn writable(root: &Path, scope: Option<Scope>) -> Result<Ledger, OpsError> {
    let found = workspace(root)?;
    let target = found.write_target(scope)?;
    let chosen = target.scope();
    Ledger::open_scope(found.root(), chosen).map_err(OpsError::from)
}
