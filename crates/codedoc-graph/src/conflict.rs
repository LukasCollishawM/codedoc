use codedoc_core::RecordId;
use codedoc_ledger::{Kind, Record};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::Graph;

pub const NEAR_DUPLICATE_FLOOR: f64 = 0.6;
pub const NEAR_DUPLICATE_CEILING: f64 = 0.99;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum Conflict {
    Declared,
    NearDuplicate,
    OppositeAssurance,
}

impl Conflict {
    pub fn as_str(self) -> &'static str {
        match self {
            Conflict::Declared => "declared",
            Conflict::NearDuplicate => "near_duplicate",
            Conflict::OppositeAssurance => "opposite_assurance",
        }
    }

    pub fn describes(self) -> &'static str {
        match self {
            Conflict::Declared => "a contradicts relation states outright that these disagree",
            Conflict::NearDuplicate => {
                "two active records on the same code say almost the same thing, which usually \
                 means one was meant to replace the other and was attached instead of superseding"
            }
            Conflict::OppositeAssurance => {
                "the same code carries both an asserted and a speculative claim of the same kind"
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Finding {
    pub kind: Conflict,
    pub left: RecordId,
    pub right: RecordId,
    pub anchor: Option<String>,
    pub left_claim: String,
    pub right_claim: String,
    pub similarity: Option<u32>,
}

fn tokens(text: &str) -> Vec<String> {
    text.to_ascii_lowercase()
        .split(|character: char| !character.is_alphanumeric())
        .filter(|word| word.len() > 2)
        .map(str::to_owned)
        .collect()
}

pub fn claim_similarity(left: &str, right: &str) -> f64 {
    let first = tokens(left);
    let second = tokens(right);
    if first.is_empty() || second.is_empty() {
        return 0.0;
    }
    let shared = first.iter().filter(|word| second.contains(word)).count();
    let union = first.len() + second.len() - shared;
    if union == 0 { 0.0 } else { shared as f64 / union as f64 }
}

fn subject_symbol(record: &Record) -> Option<String> {
    record.subject().and_then(|anchor| anchor.symbol.as_ref().map(ToString::to_string))
}

impl Graph {
    pub fn conflicts(&self) -> Vec<Finding> {
        let mut findings = Vec::new();
        let active = self.active();

        for relation in self.relations() {
            if relation.kind() != Kind::Relation(codedoc_ledger::RelationVerb::Contradicts) {
                continue;
            }
            let (Some(subject), Some(object)) = (relation.subject(), relation.object()) else {
                continue;
            };
            findings.push(Finding {
                kind: Conflict::Declared,
                left: relation.id(),
                right: relation.id(),
                anchor: subject.symbol.as_ref().map(ToString::to_string),
                left_claim: relation.content().body.claim.clone(),
                right_claim: object
                    .symbol
                    .as_ref()
                    .map(ToString::to_string)
                    .unwrap_or_else(|| object.file.as_str().to_owned()),
                similarity: None,
            });
        }

        let mut sharing: BTreeMap<(String, String), Vec<&Record>> = BTreeMap::new();
        for record in &active {
            if record.kind().is_relation() || record.kind() == Kind::Tombstone {
                continue;
            }
            let Some(symbol) = subject_symbol(record) else {
                continue;
            };
            sharing.entry((symbol, record.kind().as_str())).or_default().push(record);
        }

        for ((symbol, _), candidates) in &sharing {
            for (position, left) in candidates.iter().enumerate() {
                for right in candidates.iter().skip(position + 1) {
                    let anchor = Some(symbol.clone());

                    let score =
                        claim_similarity(&left.content().body.claim, &right.content().body.claim);
                    if (NEAR_DUPLICATE_FLOOR..NEAR_DUPLICATE_CEILING).contains(&score) {
                        findings.push(Finding {
                            kind: Conflict::NearDuplicate,
                            left: left.id(),
                            right: right.id(),
                            anchor: anchor.clone(),
                            left_claim: left.content().body.claim.clone(),
                            right_claim: right.content().body.claim.clone(),
                            similarity: Some((score * 100.0) as u32),
                        });
                        continue;
                    }

                    let opposed = matches!(
                        (left.content().assurance, right.content().assurance),
                        (
                            codedoc_ledger::Assurance::Asserted,
                            codedoc_ledger::Assurance::Speculative
                        ) | (
                            codedoc_ledger::Assurance::Speculative,
                            codedoc_ledger::Assurance::Asserted
                        )
                    );
                    if opposed {
                        findings.push(Finding {
                            kind: Conflict::OppositeAssurance,
                            left: left.id(),
                            right: right.id(),
                            anchor,
                            left_claim: left.content().body.claim.clone(),
                            right_claim: right.content().body.claim.clone(),
                            similarity: None,
                        });
                    }
                }
            }
        }
        findings
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests_support::{record_on, relation_between};
    use codedoc_ledger::Assurance;

    #[test]
    fn near_duplicates_on_one_anchor_are_reported() {
        let first = record_on(
            "rust://authorize",
            Kind::Invariant,
            "Authorization must precede reservation of funds.",
            Assurance::Asserted,
            10,
        );
        let second = record_on(
            "rust://authorize",
            Kind::Invariant,
            "Authorization must precede the reservation of funds always.",
            Assurance::Asserted,
            20,
        );
        let graph = Graph::from_records(vec![first, second]);
        let found = graph.conflicts();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].kind, Conflict::NearDuplicate);
    }

    #[test]
    fn a_pair_is_found_however_many_unrelated_records_sit_between_them() {
        let mut records = vec![record_on(
            "rust://authorize",
            Kind::Invariant,
            "Authorization must precede reservation of funds.",
            Assurance::Asserted,
            10,
        )];
        for index in 0..200 {
            records.push(record_on(
                &format!("rust://unrelated{index}"),
                Kind::Invariant,
                "Something entirely different is true here.",
                Assurance::Asserted,
                20 + index,
            ));
        }
        records.push(record_on(
            "rust://authorize",
            Kind::Invariant,
            "Authorization must precede the reservation of funds always.",
            Assurance::Asserted,
            900,
        ));

        let found = Graph::from_records(records).conflicts();
        assert_eq!(
            found.len(),
            1,
            "comparing only records that share a symbol is what makes this affordable,              and it must not depend on the two being near each other in the ledger"
        );
        assert_eq!(found[0].kind, Conflict::NearDuplicate);
    }

    #[test]
    fn unrelated_claims_on_one_anchor_are_not_reported() {
        let first = record_on(
            "rust://authorize",
            Kind::Invariant,
            "Authorization must precede reservation.",
            Assurance::Asserted,
            10,
        );
        let second = record_on(
            "rust://authorize",
            Kind::Invariant,
            "Idempotency keys survive retries across process restarts.",
            Assurance::Asserted,
            20,
        );
        let graph = Graph::from_records(vec![first, second]);
        assert!(graph.conflicts().is_empty());
    }

    #[test]
    fn identical_claims_are_duplicates_not_conflicts() {
        let text = "Authorization must precede reservation.";
        let first = record_on("rust://a", Kind::Invariant, text, Assurance::Asserted, 10);
        let second = record_on("rust://a", Kind::Invariant, text, Assurance::Asserted, 20);
        let graph = Graph::from_records(vec![first, second]);
        assert!(
            graph.conflicts().is_empty(),
            "exact duplicates are collapsed on retrieval, not flagged for adjudication"
        );
    }

    #[test]
    fn an_asserted_and_a_speculative_claim_are_flagged() {
        let first = record_on(
            "rust://a",
            Kind::Security,
            "The token is validated upstream.",
            Assurance::Asserted,
            10,
        );
        let second = record_on(
            "rust://a",
            Kind::Security,
            "Callers are trusted to sanitise input beforehand.",
            Assurance::Speculative,
            20,
        );
        let graph = Graph::from_records(vec![first, second]);
        let found = graph.conflicts();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].kind, Conflict::OppositeAssurance);
    }

    #[test]
    fn a_declared_contradiction_is_reported() {
        let relation =
            relation_between("rust://a", "rust://b", codedoc_ledger::RelationVerb::Contradicts);
        let graph = Graph::from_records(vec![relation]);
        let found = graph.conflicts();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].kind, Conflict::Declared);
    }

    #[test]
    fn claims_on_different_anchors_are_never_conflicts() {
        let first = record_on(
            "rust://a",
            Kind::Invariant,
            "Authorization must precede reservation of funds.",
            Assurance::Asserted,
            10,
        );
        let second = record_on(
            "rust://b",
            Kind::Invariant,
            "Authorization must precede reservation of funds.",
            Assurance::Asserted,
            20,
        );
        let graph = Graph::from_records(vec![first, second]);
        assert!(graph.conflicts().is_empty());
    }
}
