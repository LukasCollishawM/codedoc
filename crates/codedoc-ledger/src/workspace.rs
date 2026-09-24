use std::path::{Path, PathBuf};

use crate::ledger::{Ledger, LedgerError};
use crate::record::Record;
use crate::scope::Scope;

pub struct Workspace {
    root: PathBuf,
    ledgers: Vec<Ledger>,
}

impl Workspace {
    pub fn discover(start: &Path) -> Result<Self, LedgerError> {
        let mut current = std::fs::canonicalize(start).unwrap_or_else(|_| start.to_path_buf());
        loop {
            let workspace = Workspace::at(&current);
            if !workspace.ledgers.is_empty() {
                return Ok(workspace);
            }
            if !current.pop() {
                return Err(LedgerError::Absent { root: start.display().to_string() });
            }
        }
    }

    pub fn at(root: &Path) -> Self {
        let ledgers = Scope::ALL
            .into_iter()
            .filter_map(|scope| Ledger::open_scope(root, scope).ok())
            .collect();
        Workspace { root: root.to_path_buf(), ledgers }
    }

    pub fn confined_to(mut self, scope: Option<Scope>) -> Self {
        if let Some(wanted) = scope {
            self.ledgers.retain(|ledger| ledger.scope() == wanted);
        }
        self
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn ledgers(&self) -> &[Ledger] {
        &self.ledgers
    }

    pub fn scopes(&self) -> Vec<Scope> {
        self.ledgers.iter().map(Ledger::scope).collect()
    }

    pub fn is_empty(&self) -> bool {
        self.ledgers.is_empty()
    }

    pub fn records(&self) -> Result<Vec<Record>, LedgerError> {
        let mut collected = Vec::new();
        for ledger in &self.ledgers {
            collected.extend(ledger.records()?);
        }
        collected.sort_by_key(|record| (record.content().created, record.id().to_string()));
        collected.dedup_by_key(|record| record.id().to_string());
        Ok(collected)
    }

    pub fn for_scope(&self, scope: Scope) -> Option<&Ledger> {
        self.ledgers.iter().find(|ledger| ledger.scope() == scope)
    }

    pub fn verify(&self) -> Result<crate::ledger::Verification, LedgerError> {
        let mut records = 0usize;
        let mut tips = Vec::new();
        let mut orphans = Vec::new();
        let mut duplicates = Vec::new();
        for ledger in &self.ledgers {
            let outcome = ledger.verify()?;
            records += outcome.records;
            tips.extend(outcome.tips);
            orphans.extend(outcome.orphans);
            duplicates.extend(outcome.duplicates);
        }
        Ok(crate::ledger::Verification { records, tips, orphans, duplicates })
    }

    pub fn write_target(&self, preferred: Option<Scope>) -> Result<&Ledger, LedgerError> {
        if let Some(scope) = preferred {
            return self.for_scope(scope).ok_or(LedgerError::ScopeUnavailable {
                scope: scope.as_str(),
                root: self.root.display().to_string(),
            });
        }
        self.ledgers
            .first()
            .ok_or_else(|| LedgerError::Absent { root: self.root.display().to_string() })
    }
}
