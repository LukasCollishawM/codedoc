#![forbid(unsafe_code)]

pub mod history;

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use codedoc_anchor::{Anchor, DetachReason, FileIndex, Resolution, Rung, SourceRange};
use codedoc_core::{GitRev, RecordId, RepoPath};
use codedoc_graph::Graph;
use codedoc_lang::Registry;
use codedoc_ledger::{Kind, Ledger, LedgerError, Verification, Workspace};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum VerifyError {
    #[error("the ledger could not be read: {source}")]
    Ledger {
        #[from]
        source: LedgerError,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum Status {
    Fresh,
    Migrated,
    Stale,
    Detached,
}

impl Status {
    pub fn as_str(self) -> &'static str {
        match self {
            Status::Fresh => "fresh",
            Status::Migrated => "migrated",
            Status::Stale => "stale",
            Status::Detached => "detached",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Finding {
    pub record: RecordId,
    pub kind: String,
    pub claim: String,
    pub file: String,
    pub symbol: Option<String>,
    pub status: Status,
    pub recorded_range: SourceRange,
    pub resolution: Resolution,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Report {
    pub findings: Vec<Finding>,
    pub records: usize,
    pub integrity_intact: bool,
    pub orphaned_records: Vec<RecordId>,
}

impl Report {
    pub fn counts(&self) -> BTreeMap<&'static str, usize> {
        let mut counts = BTreeMap::new();
        for status in [Status::Fresh, Status::Migrated, Status::Stale, Status::Detached] {
            counts.insert(status.as_str(), 0);
        }
        for finding in &self.findings {
            *counts.entry(finding.status.as_str()).or_insert(0) += 1;
        }
        counts
    }

    pub fn exit_code(&self) -> i32 {
        if !self.integrity_intact {
            return 3;
        }
        if self.findings.iter().any(|finding| finding.status == Status::Detached) {
            return 2;
        }
        if self.findings.iter().any(|finding| finding.status == Status::Stale) {
            return 1;
        }
        0
    }
}

struct Pending {
    record: RecordId,
    kind: String,
    claim: String,
    anchor: Anchor,
    revision: Option<GitRev>,
}

pub struct Verifier {
    root: PathBuf,
}

impl Verifier {
    pub fn new(root: &Path) -> Self {
        Verifier { root: root.to_path_buf() }
    }

    pub fn run(&self, ledger: &Ledger) -> Result<Report, VerifyError> {
        let graph = Graph::load(ledger)?;
        let integrity: Verification = ledger.verify()?;
        self.report(graph, integrity)
    }

    pub fn run_across(&self, workspace: &Workspace) -> Result<Report, VerifyError> {
        let graph = Graph::across(workspace)?;
        let integrity: Verification = workspace.verify()?;
        self.report(graph, integrity)
    }

    fn report(&self, graph: Graph, integrity: Verification) -> Result<Report, VerifyError> {
        let mut pending: BTreeMap<String, Vec<Pending>> = BTreeMap::new();
        for record in graph.active() {
            let content = record.content();
            for entry in &content.anchors {
                pending.entry(entry.anchor.file.as_str().to_owned()).or_default().push(Pending {
                    record: record.id(),
                    kind: content.kind.as_str(),
                    claim: content.body.claim.clone(),
                    anchor: entry.anchor.clone(),
                    revision: content.code_revision.clone(),
                });
            }
        }

        let mut findings = Vec::new();
        for (file, entries) in pending {
            let resolutions = self.resolve_file(&file, &entries);
            for (item, resolution) in entries.into_iter().zip(resolutions) {
                findings.push(Finding {
                    record: item.record,
                    kind: item.kind,
                    claim: item.claim,
                    file: file.clone(),
                    symbol: item.anchor.symbol.as_ref().map(ToString::to_string),
                    status: classify(&resolution),
                    recorded_range: item.anchor.range,
                    resolution,
                });
            }
        }

        findings.sort_by(|left, right| {
            right
                .status
                .cmp(&left.status)
                .then_with(|| left.file.cmp(&right.file))
                .then_with(|| left.recorded_range.start_line.cmp(&right.recorded_range.start_line))
        });

        Ok(Report {
            findings,
            records: graph.active().len(),
            integrity_intact: integrity.is_intact(),
            orphaned_records: integrity.orphans,
        })
    }
}

impl Verifier {
    fn resolve_file(&self, file: &str, entries: &[Pending]) -> Vec<Resolution> {
        let Some(first) = entries.first() else {
            return Vec::new();
        };
        match fs::read_to_string(self.root.join(file)) {
            Ok(source) => {
                let path = first.anchor.file.clone();
                self.resolve_in(&path, &source, entries, false)
            }
            Err(_) => self.resolve_after_rename(file, entries),
        }
    }

    fn resolve_after_rename(&self, file: &str, entries: &[Pending]) -> Vec<Resolution> {
        let detached = vec![Resolution::Detached(DetachReason::FileMissing); entries.len()];
        let Some(revision) = entries.iter().find_map(|item| item.revision.clone()) else {
            return detached;
        };
        let Some(moved) = history::renamed_to(&self.root, &revision, file) else {
            return detached;
        };
        let Ok(path) = RepoPath::parse(&moved) else {
            return detached;
        };
        let Ok(source) = fs::read_to_string(self.root.join(path.as_str())) else {
            return detached;
        };
        self.resolve_in(&path, &source, entries, true)
    }

    fn resolve_in(
        &self,
        path: &RepoPath,
        source: &str,
        entries: &[Pending],
        migrated: bool,
    ) -> Vec<Resolution> {
        let Ok(adapter) = Registry::for_path(path) else {
            return vec![Resolution::Detached(DetachReason::LanguageUnsupported); entries.len()];
        };
        let Ok(tree) = adapter.parse(source) else {
            return vec![Resolution::Detached(DetachReason::LanguageUnsupported); entries.len()];
        };

        let index = FileIndex::build(adapter, source, &tree);
        entries
            .iter()
            .map(|item| {
                let required = Kind::parse(&item.kind)
                    .map(Kind::required_confidence)
                    .unwrap_or(codedoc_anchor::Confidence::High);
                let resolution = if migrated {
                    index.resolve_after_migration(&item.anchor)
                } else {
                    index.resolve(&item.anchor)
                };
                resolution.require(required)
            })
            .collect()
    }
}

fn classify(resolution: &Resolution) -> Status {
    match resolution.located() {
        None => Status::Detached,
        Some(located) => match located.rung() {
            Rung::ContentIdentity => Status::Fresh,
            Rung::StructuralIdentity | Rung::SymbolAndNodePath => Status::Migrated,
            Rung::ContextBracket | Rung::GitMigration => Status::Stale,
            Rung::Similarity => Status::Detached,
            _ => Status::Detached,
        },
    }
}
