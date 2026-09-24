#![forbid(unsafe_code)]

pub const DEFAULT_BUDGET: usize = 12_000;

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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub symbol: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub evidence: Vec<String>,
    pub trust: u32,
}

fn now_seconds() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|span| span.as_secs() as i64)
        .unwrap_or(0)
}

pub const RECENCY_HALF_LIFE_DAYS: f64 = 180.0;

fn assurance_weight(record: &Record) -> f64 {
    match record.content().assurance {
        codedoc_ledger::Assurance::Asserted => 1.0,
        codedoc_ledger::Assurance::Inferred => 0.6,
        codedoc_ledger::Assurance::Speculative => 0.3,
        _ => 0.3,
    }
}

fn authority_weight(record: &Record) -> f64 {
    match record.content().author {
        codedoc_ledger::Author::Human { .. } => 1.0,
        codedoc_ledger::Author::Analyzer { .. } => 0.9,
        codedoc_ledger::Author::Runtime { .. } => 0.9,
        codedoc_ledger::Author::Agent { .. } => 0.7,
        _ => 0.5,
    }
}

fn recency_weight(record: &Record, now: i64) -> f64 {
    let age_days = ((now - record.content().created.unix_seconds()).max(0) as f64) / 86_400.0;
    0.5f64.powf(age_days / RECENCY_HALF_LIFE_DAYS).clamp(0.25, 1.0)
}

pub fn trust_of(record: &Record, now: i64) -> f64 {
    assurance_weight(record) * authority_weight(record) * recency_weight(record, now)
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
            file: record.subject().map(|anchor| anchor.file.as_str().to_owned()),
            symbol: record
                .subject()
                .and_then(|anchor| anchor.symbol.as_ref().map(ToString::to_string)),
            evidence: content
                .evidence
                .iter()
                .map(|item| {
                    serde_json::to_string(item).unwrap_or_else(|_| "unrenderable".to_owned())
                })
                .collect(),
            trust: (trust_of(record, now_seconds()) * 100.0).round() as u32,
        }
    }

    fn weight(&self) -> usize {
        const ENVELOPE: usize = 220;
        ENVELOPE + self.claim.len() + self.detail.as_ref().map(String::len).unwrap_or(0)
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

    pub fn absorb(&mut self, other: ContextPack) {
        self.invariants.extend(other.invariants);
        self.security.extend(other.security);
        self.rationale.extend(other.rationale);
        self.failure_modes.extend(other.failure_modes);
        self.other.extend(other.other);
        self.relations.extend(other.relations);
        self.truncated |= other.truncated;
    }

    pub fn deduplicate(&mut self) {
        for section in [
            &mut self.invariants,
            &mut self.security,
            &mut self.failure_modes,
            &mut self.rationale,
            &mut self.other,
        ] {
            let mut seen: Vec<(String, Option<String>)> = Vec::new();
            section.retain(|claim| {
                let identity = (claim.claim.clone(), claim.detail.clone());
                if seen.contains(&identity) {
                    return false;
                }
                seen.push(identity);
                true
            });
        }
        let mut seen_relations: Vec<String> = Vec::new();
        self.relations.retain(|relation| {
            let identity = format!(
                "{}|{}|{}",
                relation.subject.as_deref().unwrap_or(""),
                relation.verb,
                relation.object.as_deref().unwrap_or("")
            );
            if seen_relations.contains(&identity) {
                return false;
            }
            seen_relations.push(identity);
            true
        });
    }

    pub fn rank(&mut self) {
        for section in [
            &mut self.invariants,
            &mut self.security,
            &mut self.failure_modes,
            &mut self.rationale,
            &mut self.other,
        ] {
            section.sort_by_key(|claim| std::cmp::Reverse(claim.trust));
        }
    }

    pub fn fit_within(&mut self, budget: usize) {
        let mut remaining = budget;
        let mut dropped = false;
        let mut anything_kept = false;
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
                if weight <= remaining || !anything_kept {
                    remaining = remaining.saturating_sub(weight);
                    anything_kept = true;
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
    let mut records: Vec<&Record> = match (&target.symbol, target.line) {
        (Some(symbol), _) => graph.for_symbol(symbol),
        (None, Some(line)) => graph.covering_line(&target.file, line),
        (None, None) => graph.in_file(&target.file),
    };
    if target.symbol.is_some() {
        records.extend(graph.file_scoped(&target.file));
    }
    records.dedup_by_key(|record| record.id());

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
        let mut symbols: Vec<String> = records
            .iter()
            .flat_map(|record| record.content().anchors.iter())
            .filter_map(|entry| entry.anchor.symbol.as_ref().map(ToString::to_string))
            .collect();
        if let Some(requested) = pack.target.symbol.clone() {
            symbols.push(requested);
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use codedoc_anchor::Anchor;
    use codedoc_core::RepoPath;
    use codedoc_lang::Registry;
    use codedoc_ledger::{
        AnchorRole, Assurance, Author, Body, Kind, Lifecycle, RecordContent, Role, SCHEMA_VERSION,
        Timestamp,
    };
    use std::collections::BTreeMap;

    const NOW: i64 = 1_800_000_000;

    fn record(author: Author, assurance: Assurance, age_days: i64) -> Record {
        let adapter = Registry::by_name("rust").unwrap();
        let source = "fn target() { work(); }\n";
        let tree = adapter.parse(source).unwrap();
        let anchor = Anchor::capture(
            RepoPath::parse("src/lib.rs").unwrap(),
            adapter,
            source,
            tree.root_node().child(0).unwrap(),
        );
        Record::seal(RecordContent {
            schema: SCHEMA_VERSION,
            kind: Kind::Invariant,
            anchors: vec![AnchorRole { role: Role::Subject, anchor }],
            body: Body { claim: "a claim".to_owned(), detail: None },
            evidence: Vec::new(),
            assurance,
            author,
            code_revision: None,
            created: Timestamp::from_unix_seconds(NOW - age_days * 86_400),
            lifecycle: Lifecycle::Active,
            parent: None,
            chain: None,
            unrecognised: BTreeMap::new(),
        })
        .unwrap()
    }

    fn human() -> Author {
        Author::Human { identity: "maintainer".to_owned() }
    }

    fn agent() -> Author {
        Author::Agent { model: "some-model".to_owned(), session: "s".to_owned() }
    }

    #[test]
    fn a_verified_human_claim_outranks_a_fresh_agent_guess() {
        let asserted = record(human(), Assurance::Asserted, 0);
        let speculated = record(agent(), Assurance::Speculative, 0);
        assert!(trust_of(&asserted, NOW) > trust_of(&speculated, NOW));
    }

    #[test]
    fn an_old_speculation_ranks_below_a_recent_one() {
        let stale = record(agent(), Assurance::Speculative, 720);
        let fresh = record(agent(), Assurance::Speculative, 0);
        assert!(trust_of(&fresh, NOW) > trust_of(&stale, NOW));
    }

    #[test]
    fn age_never_erases_a_human_assertion_below_a_fresh_speculation() {
        let ancient = record(human(), Assurance::Asserted, 3650);
        let fresh_guess = record(agent(), Assurance::Speculative, 0);
        assert!(
            trust_of(&ancient, NOW) > trust_of(&fresh_guess, NOW),
            "recency is a tiebreak between comparable claims, not a way for a guess to \
             displace something a human verified"
        );
    }

    #[test]
    fn trust_stays_within_bounds() {
        for author in [human(), agent()] {
            for assurance in [Assurance::Asserted, Assurance::Inferred, Assurance::Speculative] {
                for age in [0, 365, 10_000] {
                    let score = trust_of(&record(author.clone(), assurance, age), NOW);
                    assert!((0.0..=1.0).contains(&score), "score {score} out of range");
                }
            }
        }
    }
}
