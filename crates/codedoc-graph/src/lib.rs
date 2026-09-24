#![forbid(unsafe_code)]

pub mod conflict;
#[cfg(test)]
pub(crate) mod tests_support;

pub use conflict::{Conflict, Finding};

use std::collections::{BTreeMap, BTreeSet};

use codedoc_core::RecordId;
use codedoc_ledger::{Kind, Ledger, LedgerError, Record, Timestamp, Workspace};

pub struct Graph {
    records: Vec<Record>,
    superseded: BTreeSet<RecordId>,
    tombstoned: BTreeSet<RecordId>,
}

impl Graph {
    pub fn load(ledger: &Ledger) -> Result<Self, LedgerError> {
        Ok(Graph::from_records(ledger.records()?))
    }

    pub fn across(workspace: &Workspace) -> Result<Self, LedgerError> {
        Ok(Graph::from_records(workspace.records()?))
    }

    pub fn from_records(records: Vec<Record>) -> Self {
        let mut superseded = BTreeSet::new();
        let mut tombstoned = BTreeSet::new();
        for record in &records {
            let Some(parent) = record.content().parent else {
                continue;
            };
            if record.kind() == Kind::Tombstone {
                tombstoned.insert(parent);
            } else {
                superseded.insert(parent);
            }
        }
        Graph { records, superseded, tombstoned }
    }

    pub fn all(&self) -> &[Record] {
        &self.records
    }

    pub fn active(&self) -> Vec<&Record> {
        self.records
            .iter()
            .filter(|record| record.kind() != Kind::Tombstone)
            .filter(|record| {
                let id = record.id();
                !self.superseded.contains(&id) && !self.tombstoned.contains(&id)
            })
            .collect()
    }

    pub fn as_of(&self, moment: Timestamp) -> Graph {
        let records: Vec<Record> = self
            .records
            .iter()
            .filter(|record| record.content().created <= moment)
            .cloned()
            .collect();
        Graph::from_records(records)
    }

    pub fn find(&self, id: RecordId) -> Option<&Record> {
        self.records.iter().find(|record| record.id() == id)
    }

    pub fn supersession_chain(&self, id: RecordId) -> Vec<&Record> {
        let mut chain = Vec::new();
        let mut current = self.find(id);
        while let Some(record) = current {
            chain.push(record);
            current = record.content().parent.and_then(|parent| self.find(parent));
        }
        chain
    }

    pub fn revision_history(&self, id: RecordId) -> Vec<&Record> {
        let mut lineage: BTreeSet<String> =
            self.supersession_chain(id).iter().map(|record| record.id().to_string()).collect();
        if lineage.is_empty() {
            return Vec::new();
        }
        loop {
            let grown: Vec<String> = self
                .all()
                .iter()
                .filter(|record| {
                    record
                        .content()
                        .parent
                        .is_some_and(|parent| lineage.contains(&parent.to_string()))
                })
                .map(|record| record.id().to_string())
                .filter(|found| !lineage.contains(found))
                .collect();
            if grown.is_empty() {
                break;
            }
            lineage.extend(grown);
        }
        let mut found: Vec<&Record> =
            self.all().iter().filter(|record| lineage.contains(&record.id().to_string())).collect();
        found.sort_by_key(|record| (record.content().created, record.id().to_string()));
        found
    }

    pub fn for_symbol(&self, symbol: &str) -> Vec<&Record> {
        self.active()
            .into_iter()
            .filter(|record| {
                record.content().anchors.iter().any(|entry| {
                    entry.anchor.symbol.as_ref().is_some_and(|path| path.to_string() == symbol)
                })
            })
            .collect()
    }

    pub fn in_file(&self, file: &str) -> Vec<&Record> {
        self.active()
            .into_iter()
            .filter(|record| {
                record.content().anchors.iter().any(|entry| entry.anchor.file.as_str() == file)
            })
            .collect()
    }

    pub fn file_scoped(&self, file: &str) -> Vec<&Record> {
        self.in_file(file)
            .into_iter()
            .filter(|record| {
                record.content().anchors.iter().any(|entry| {
                    entry.anchor.file.as_str() == file
                        && matches!(entry.anchor.subject, codedoc_anchor::Subject::File)
                })
            })
            .collect()
    }

    pub fn covering_line(&self, file: &str, line: u32) -> Vec<&Record> {
        let mut covering: Vec<&Record> = self
            .in_file(file)
            .into_iter()
            .filter(|record| {
                record.content().anchors.iter().any(|entry| {
                    entry.anchor.file.as_str() == file && entry.anchor.range.contains_line(line)
                })
            })
            .collect();
        covering.sort_by_key(|record| {
            record
                .content()
                .anchors
                .iter()
                .map(|entry| entry.anchor.range.end_line - entry.anchor.range.start_line)
                .min()
                .unwrap_or(u32::MAX)
        });
        covering
    }

    pub fn relations(&self) -> Vec<&Record> {
        self.active().into_iter().filter(|record| record.kind().is_relation()).collect()
    }

    pub fn related_to(&self, symbol: &str) -> Vec<&Record> {
        self.relations()
            .into_iter()
            .filter(|record| {
                record.content().anchors.iter().any(|entry| {
                    entry.anchor.symbol.as_ref().is_some_and(|path| path.to_string() == symbol)
                })
            })
            .collect()
    }

    pub fn counts_by_kind(&self) -> BTreeMap<String, usize> {
        let mut counts = BTreeMap::new();
        for record in self.active() {
            *counts.entry(record.kind().as_str()).or_insert(0) += 1;
        }
        counts
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use codedoc_anchor::Anchor;
    use codedoc_core::RepoPath;
    use codedoc_lang::Registry;
    use codedoc_ledger::{
        AnchorRole, Assurance, Author, Body, Lifecycle, RecordContent, Role, SCHEMA_VERSION,
    };

    fn anchor() -> Anchor {
        let adapter = Registry::by_name("rust").unwrap();
        let source = "fn authorize() -> bool { true }";
        let tree = adapter.parse(source).unwrap();
        Anchor::capture(
            RepoPath::parse("src/auth.rs").unwrap(),
            adapter,
            source,
            tree.root_node().child(0).unwrap(),
        )
    }

    fn record(claim: &str, kind: Kind, parent: Option<RecordId>, at: i64) -> Record {
        Record::seal(RecordContent {
            schema: SCHEMA_VERSION,
            kind,
            anchors: vec![AnchorRole { role: Role::Subject, anchor: anchor() }],
            body: Body { claim: claim.to_owned(), detail: None },
            evidence: Vec::new(),
            assurance: Assurance::Asserted,
            author: Author::Human { identity: "m".to_owned() },
            code_revision: None,
            created: Timestamp::from_unix_seconds(at),
            lifecycle: Lifecycle::Active,
            parent,
            chain: None,
            unrecognised: BTreeMap::new(),
        })
        .unwrap()
    }

    #[test]
    fn superseded_records_leave_the_active_set() {
        let original = record("first belief", Kind::Rationale, None, 10);
        let revision = record("revised belief", Kind::Rationale, Some(original.id()), 20);
        let graph = Graph::from_records(vec![original.clone(), revision.clone()]);

        let active = graph.active();
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].id(), revision.id());
    }

    #[test]
    fn tombstones_remove_their_target_and_themselves() {
        let original = record("retracted", Kind::Rationale, None, 10);
        let tombstone = record("no longer true", Kind::Tombstone, Some(original.id()), 20);
        let graph = Graph::from_records(vec![original, tombstone]);
        assert!(graph.active().is_empty());
    }

    #[test]
    fn history_is_queryable_at_an_earlier_moment() {
        let original = record("first belief", Kind::Rationale, None, 10);
        let revision = record("revised belief", Kind::Rationale, Some(original.id()), 20);
        let graph = Graph::from_records(vec![original.clone(), revision]);

        let earlier = graph.as_of(Timestamp::from_unix_seconds(15));
        assert_eq!(earlier.active().len(), 1);
        assert_eq!(earlier.active()[0].id(), original.id());
    }

    #[test]
    fn supersession_chain_walks_back_through_history() {
        let first = record("v1", Kind::Rationale, None, 10);
        let second = record("v2", Kind::Rationale, Some(first.id()), 20);
        let third = record("v3", Kind::Rationale, Some(second.id()), 30);
        let graph = Graph::from_records(vec![first.clone(), second, third.clone()]);

        let chain = graph.supersession_chain(third.id());
        assert_eq!(chain.len(), 3);
        assert_eq!(chain[2].id(), first.id());
    }

    #[test]
    fn records_are_found_by_symbol_and_covering_line() {
        let entry = record("claim", Kind::Invariant, None, 10);
        let graph = Graph::from_records(vec![entry]);
        assert_eq!(graph.for_symbol("rust://authorize").len(), 1);
        assert_eq!(graph.covering_line("src/auth.rs", 1).len(), 1);
        assert!(graph.covering_line("src/auth.rs", 99).is_empty());
    }
}
