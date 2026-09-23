use codedoc_anchor::{Confidence, Rung};
use codedoc_ledger::{Kind, RelationVerb};

fn every_kind() -> Vec<Kind> {
    let mut kinds: Vec<Kind> =
        Kind::vocabulary().iter().filter_map(|name| Kind::parse(name)).collect();
    kinds.push(Kind::Tombstone);
    for verb in RelationVerb::vocabulary() {
        if let Some(parsed) = RelationVerb::parse(verb) {
            kinds.push(Kind::Relation(parsed));
        }
    }
    kinds
}

#[test]
fn the_vocabulary_is_fully_parseable() {
    let kinds = every_kind();
    assert!(kinds.len() >= 24, "expected the full vocabulary, got {}", kinds.len());
}

#[test]
fn no_record_kind_accepts_a_similarity_match_without_adjudication() {
    let reached = Rung::Similarity.confidence();
    for kind in every_kind() {
        assert!(
            kind.required_confidence() > reached,
            "{} accepts rung 6 automatically. Similarity is a guess ranked above other \
             guesses, not an identification; every kind must require adjudication for it.",
            kind.as_str()
        );
    }
}

#[test]
fn safety_weighted_kinds_reject_everything_below_high() {
    for kind in [Kind::Invariant, Kind::Security, Kind::Precondition, Kind::Postcondition] {
        assert_eq!(kind.required_confidence(), Confidence::High);
        assert!(kind.required_confidence() > Rung::ContextBracket.confidence());
        assert!(kind.required_confidence() > Rung::GitMigration.confidence());
    }
}

#[test]
fn ordinary_kinds_accept_exact_and_high_but_never_low() {
    for kind in [Kind::Explanation, Kind::Rationale, Kind::Performance, Kind::Ownership] {
        assert!(kind.required_confidence() <= Rung::SymbolAndNodePath.confidence());
        assert!(kind.required_confidence() > Rung::Similarity.confidence());
    }
}

#[test]
fn every_rung_confidence_is_ordered_by_strength() {
    let ladder = [
        Rung::ContentIdentity,
        Rung::StructuralIdentity,
        Rung::SymbolAndNodePath,
        Rung::ContextBracket,
        Rung::GitMigration,
        Rung::Similarity,
    ];
    for pair in ladder.windows(2) {
        let [earlier, later] = pair else { continue };
        assert!(
            earlier.confidence() >= later.confidence(),
            "rung {} must not be weaker than rung {}",
            earlier.position(),
            later.position()
        );
    }
}
