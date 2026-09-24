use std::path::Path;

use codedoc_core::readable_path;
use codedoc_index::Index;
use codedoc_ledger::{Ledger, Scope};
use serde_json::json;

use crate::{OpsError, Outcome};

pub fn initialise(root: &Path, scope: Scope) -> Outcome {
    let ledger = Ledger::initialise_scope(root, scope)?;
    Index::rebuild(&ledger).map_err(|source| OpsError::Index { detail: source.to_string() })?;
    Ok(json!({
        "command": "init",
        "root": readable_path(root),
        "scope": scope.as_str(),
        "location": readable_path(ledger.base()),
        "describes": scope.describe(),
        "leaves_repository_evidence": scope.leaves_repository_evidence(),
    }))
}
