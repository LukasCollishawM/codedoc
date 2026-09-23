#![forbid(unsafe_code)]

use codedoc_core::RecordId;
use codedoc_graph::Graph;
use codedoc_ledger::{Kind, Record};
use serde::{Deserialize, Serialize};

pub const DEFAULT_DEPTH: u8 = 2;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claim {
    pub record: RecordId,
    pub kind: String,
    pub claim: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    pub assurance: String,
    pub author: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub evidence: Vec<String>,
}

impl Claim {
    fn from(record: &Record) -> Self {
        let content = record.content();
        Claim {
            record: record.id(),
            kind: content.kind.as_str(),
            claim: content.body.claim.clone(),
            detail: content.body.detail.clone(),
            assurance: content.assurance.as_str().to_owned(),
            author: describe_author(record),
            evidence: content
                .evidence
                .iter()
                .map(|item| {
                    serde_json::to_string(item).unwrap_or_else(|_| "unrenderable".to_owned())
                })
                .collect(),
        }
    }

    fn weight(&self) -> usize {
        self.claim.len() + self.detail.as_ref().map(String::len).unwrap_or(0)
    }
}

fn describe_author(record: &Record) -> String {
    serde_json::to_string(&record.content().author).unwrap_or_else(|_| "unknown".to_owned())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelationEntry {
    pub record: RecordId,
    pub verb: String,
    pub subject: Option<String>,
    pub object: Option<String>,
    pub claim: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Target {
    pub file: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub symbol: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextPack {
    pub target: Target,
    pub depth: u8,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub invariants: Vec<Claim>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub security: Vec<Claim>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub rationale: Vec<Claim>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub failure_modes: Vec<Claim>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub other: Vec<Claim>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub relations: Vec<RelationEntry>,
    pub truncated: bool,
}

impl ContextPack {
    pub fn is_empty(&self) -> bool {
        self.invariants.is_empty()
            && self.security.is_empty()
            && self.rationale.is_empty()
            && self.failure_modes.is_empty()
            && self.other.is_empty()
            && self.relations.is_empty()
    }

    pub fn claim_count(&self) -> usize {
        self.invariants.len()
            + self.security.len()
            + self.rationale.len()
            + self.failure_modes.len()
            + self.other.len()
    }

    pub fn fit_within(&mut self, budget: usize) {
        let mut remaining = budget;
        let mut dropped = false;
        for section in [
            &mut self.invariants,
            &mut self.security,
            &mut self.failure_modes,
            &mut self.rationale,
            &mut self.other,
        ] {
            let mut kept = Vec::new();
            for claim in section.drain(..) {
                let weight = claim.weight();
                if weight <= remaining {
                    remaining -= weight;
                    kept.push(claim);
                } else {
                    dropped = true;
                }
            }
            *section = kept;
        }
        self.truncated = dropped;
    }
}

pub fn assemble(graph: &Graph, target: Target, depth: u8) -> ContextPack {
    let records: Vec<&Record> = match (&target.symbol, target.line) {
        (Some(symbol), _) => graph.for_symbol(symbol),
        (None, Some(line)) => graph.covering_line(&target.file, line),
        (None, None) => graph.in_file(&target.file),
    };

    let mut pack = ContextPack {
        target,
        depth,
        invariants: Vec::new(),
        security: Vec::new(),
        rationale: Vec::new(),
        failure_modes: Vec::new(),
        other: Vec::new(),
        relations: Vec::new(),
        truncated: false,
    };

    for record in &records {
        if record.kind().is_relation() {
            continue;
        }
        let claim = Claim::from(record);
        match record.kind() {
            Kind::Invariant | Kind::Precondition | Kind::Postcondition => {
                pack.invariants.push(claim)
            }
            Kind::Security => pack.security.push(claim),
            Kind::Rationale | Kind::Decision | Kind::Specification => pack.rationale.push(claim),
            Kind::KnownFailureMode | Kind::Warning | Kind::Workaround => {
                pack.failure_modes.push(claim)
            }
            _ => pack.other.push(claim),
        }
    }

    if depth > 0 {
        let symbols: Vec<String> = records
            .iter()
            .flat_map(|record| record.content().anchors.iter())
            .filter_map(|entry| entry.anchor.symbol.as_ref().map(ToString::to_string))
            .collect();
        for symbol in symbols {
            for relation in graph.related_to(&symbol) {
                let Kind::Relation(verb) = relation.kind() else {
                    continue;
                };
                let entry = RelationEntry {
                    record: relation.id(),
                    verb: verb.as_str().to_owned(),
                    subject: relation
                        .subject()
                        .and_then(|anchor| anchor.symbol.as_ref().map(ToString::to_string)),
                    object: relation
                        .object()
                        .and_then(|anchor| anchor.symbol.as_ref().map(ToString::to_string)),
                    claim: relation.content().body.claim.clone(),
                };
                let already = pack
                    .relations
                    .iter()
                    .any(|existing| existing.record.to_string() == entry.record.to_string());
                if !already {
                    pack.relations.push(entry);
                }
            }
        }
    }

    pack
}
