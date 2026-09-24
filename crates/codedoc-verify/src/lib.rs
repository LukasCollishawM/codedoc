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
use rayon::prelude::*;
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub relocated_to: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub drift: Option<u32>,
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
    pub integrity_checked: bool,
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

struct Outcome {
    resolution: Resolution,
    drift: Option<u32>,
    relocated_to: Option<String>,
}

impl Outcome {
    fn detached(reason: DetachReason) -> Self {
        Outcome { resolution: Resolution::Detached(reason), drift: None, relocated_to: None }
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
        self.report(graph, integrity, &[], true)
    }

    pub fn run_across(&self, workspace: &Workspace) -> Result<Report, VerifyError> {
        let graph = Graph::across(workspace)?;
        let integrity: Verification = workspace.verify()?;
        self.report(graph, integrity, &[], true)
    }

    pub fn run_over(&self, workspace: &Workspace, files: &[String]) -> Result<Report, VerifyError> {
        let graph = Graph::across(workspace)?;
        let integrity: Verification = workspace.verify()?;
        self.report(graph, integrity, files, true)
    }

    pub fn run_records(
        &self,
        records: Vec<codedoc_ledger::Record>,
        files: &[String],
    ) -> Result<Report, VerifyError> {
        let graph = Graph::from_records(records);
        let empty = Verification {
            records: 0,
            tips: Vec::new(),
            orphans: Vec::new(),
            duplicates: Vec::new(),
        };
        self.report(graph, empty, files, false)
    }

    fn report(
        &self,
        graph: Graph,
        integrity: Verification,
        only: &[String],
        checked: bool,
    ) -> Result<Report, VerifyError> {
        let mut pending: BTreeMap<String, Vec<Pending>> = BTreeMap::new();
        for record in graph.active() {
            let content = record.content();
            for entry in &content.anchors {
                let file = entry.anchor.file.as_str();
                if !only.is_empty() && !only.iter().any(|wanted| wanted == file) {
                    continue;
                }
                pending.entry(file.to_owned()).or_default().push(Pending {
                    record: record.id(),
                    kind: content.kind.as_str(),
                    claim: content.body.claim.clone(),
                    anchor: entry.anchor.clone(),
                    revision: content.code_revision.clone(),
                });
            }
        }

        let work: Vec<(String, Vec<Pending>)> = pending.into_iter().collect();
        let mut findings: Vec<Finding> = work
            .into_par_iter()
            .flat_map(|(file, entries)| {
                let resolutions = self.resolve_file(&file, &entries);
                entries
                    .into_iter()
                    .zip(resolutions)
                    .map(|(item, outcome)| {
                        let Outcome { resolution, drift, relocated_to } = outcome;
                        Finding {
                            record: item.record,
                            relocated_to,
                            drift,
                            kind: item.kind,
                            claim: item.claim,
                            file: file.clone(),
                            symbol: item.anchor.symbol.as_ref().map(ToString::to_string),
                            status: classify(&resolution, drift),
                            recorded_range: item.anchor.range,
                            resolution,
                        }
                    })
                    .collect::<Vec<_>>()
            })
            .collect();

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
            integrity_checked: checked,
            integrity_intact: integrity.is_intact(),
            orphaned_records: integrity.orphans,
        })
    }
}

impl Verifier {
    fn resolve_file(&self, file: &str, entries: &[Pending]) -> Vec<Outcome> {
        let Some(first) = entries.first() else {
            return Vec::new();
        };
        match fs::read_to_string(self.root.join(file)) {
            Ok(source) => {
                let path = first.anchor.file.clone();
                let mut outcomes = self.resolve_in(&path, &source, entries, false);
                if outcomes.iter().any(|outcome| outcome.resolution.is_detached()) {
                    self.rescue_detached(file, entries, &mut outcomes);
                }
                outcomes
            }
            Err(_) => self.resolve_after_rename(file, entries),
        }
    }

    fn rescue_detached(&self, origin: &str, entries: &[Pending], outcomes: &mut [Outcome]) {
        let Some(elsewhere) = self.relocated_elsewhere(origin, entries) else {
            return;
        };
        for (slot, rescued) in outcomes.iter_mut().zip(elsewhere) {
            if slot.resolution.is_detached() && rescued.resolution.located().is_some() {
                *slot = rescued;
            }
        }
    }

    fn relocated_elsewhere(&self, origin: &str, entries: &[Pending]) -> Option<Vec<Outcome>> {
        let revision = entries.iter().find_map(|item| item.revision.clone())?;
        let siblings = history::changed_since(&self.root, revision.as_str())?;

        let mut elsewhere: Vec<(RepoPath, String)> = Vec::new();
        for candidate in siblings {
            if candidate == origin {
                continue;
            }
            let Ok(path) = RepoPath::parse(&candidate) else {
                continue;
            };
            if Registry::for_path(&path).is_err() {
                continue;
            }
            let Ok(source) = fs::read_to_string(self.root.join(path.as_str())) else {
                continue;
            };
            elsewhere.push((path, source));
        }
        if elsewhere.is_empty() {
            return None;
        }

        let mut outcomes: Vec<Outcome> = Vec::new();
        for item in entries {
            let mut hits: Vec<Outcome> = Vec::new();
            for (path, source) in &elsewhere {
                let Ok(adapter) = Registry::for_path(path) else {
                    continue;
                };
                let Ok(tree) = adapter.parse(source) else {
                    continue;
                };
                let index = FileIndex::build(adapter, source, &tree);
                let outcome = index.resolve_after_migration(&item.anchor);
                if outcome.located().is_some() {
                    let drift = self.drift_of(&item.anchor, &outcome, adapter, &tree);
                    hits.push(Outcome {
                        resolution: outcome,
                        drift,
                        relocated_to: Some(path.as_str().to_owned()),
                    });
                }
            }
            match hits.len() {
                1 => outcomes.push(hits.remove(0)),
                0 => outcomes.push(Outcome::detached(DetachReason::NoCandidate)),
                count => outcomes.push(Outcome::detached(DetachReason::Ambiguous {
                    rung: Rung::GitMigration,
                    candidates: count as u32,
                })),
            }
        }
        Some(outcomes)
    }

    fn resolve_after_rename(&self, file: &str, entries: &[Pending]) -> Vec<Outcome> {
        let detached: Vec<Outcome> =
            (0..entries.len()).map(|_| Outcome::detached(DetachReason::FileMissing)).collect();
        let Some(revision) = entries.iter().find_map(|item| item.revision.clone()) else {
            return detached;
        };
        let Some(moved) = history::renamed_to(&self.root, &revision, file) else {
            return self.relocated_elsewhere(file, entries).unwrap_or(detached);
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
    ) -> Vec<Outcome> {
        let Ok(adapter) = Registry::for_path(path) else {
            return (0..entries.len())
                .map(|_| Outcome::detached(DetachReason::LanguageUnsupported))
                .collect();
        };
        let Ok(tree) = adapter.parse(source) else {
            return (0..entries.len())
                .map(|_| Outcome::detached(DetachReason::LanguageUnsupported))
                .collect();
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
                }
                .require(required);
                let drift = self.drift_of(&item.anchor, &resolution, adapter, &tree);
                Outcome { resolution, drift, relocated_to: None }
            })
            .collect()
    }

    fn drift_of(
        &self,
        anchor: &Anchor,
        resolution: &Resolution,
        adapter: &codedoc_lang::Adapter,
        tree: &codedoc_lang::Tree,
    ) -> Option<u32> {
        let located = resolution.located()?;
        let node = located.node_path().descend(tree.root_node())?;
        let current = codedoc_anchor::fingerprint::shape_histogram(node, adapter);
        Some(drift_between(&anchor.shape, &current))
    }
}

pub const DRIFT_STALE_THRESHOLD: u32 = 25;

pub fn drift_between(
    recorded: &std::collections::BTreeMap<String, u32>,
    current: &std::collections::BTreeMap<String, u32>,
) -> u32 {
    if recorded.is_empty() && current.is_empty() {
        return 0;
    }
    let mut shared = 0u32;
    let mut union = 0u32;
    let kinds: std::collections::BTreeSet<&String> =
        recorded.keys().chain(current.keys()).collect();
    for kind in kinds {
        let left = recorded.get(kind).copied().unwrap_or(0);
        let right = current.get(kind).copied().unwrap_or(0);
        shared += left.min(right);
        union += left.max(right);
    }
    if union == 0 {
        return 0;
    }
    100 - ((f64::from(shared) / f64::from(union)) * 100.0).round() as u32
}

fn classify(resolution: &Resolution, drift: Option<u32>) -> Status {
    let Some(located) = resolution.located() else {
        return Status::Detached;
    };
    let positional = match located.rung() {
        Rung::FileIdentity | Rung::ContentIdentity => Status::Fresh,
        Rung::StructuralIdentity | Rung::SymbolAndNodePath => Status::Migrated,
        Rung::ContextBracket | Rung::GitMigration => Status::Stale,
        Rung::Similarity => Status::Detached,
        _ => Status::Detached,
    };
    match drift {
        Some(amount) if amount >= DRIFT_STALE_THRESHOLD => positional.max(Status::Stale),
        _ => positional,
    }
}
