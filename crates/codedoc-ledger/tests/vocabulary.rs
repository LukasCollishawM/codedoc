use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

use codedoc_ledger::{Assurance, Kind, Lifecycle, RelationVerb, Role};
use serde_json::Value;

fn frozen() -> Value {
    let path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../conformance/vocabulary/wire.json");
    let raw = fs::read_to_string(path).expect("the frozen vocabulary is missing");
    serde_json::from_str(&raw).expect("the frozen vocabulary is not valid JSON")
}

fn expected(document: &Value, member: &str) -> BTreeSet<String> {
    document[member]
        .as_array()
        .unwrap_or_else(|| panic!("the fixture has no {member} member"))
        .iter()
        .map(|entry| entry.as_str().expect("a string").to_owned())
        .collect()
}

const WHY: &str = "a discriminant is part of the encoding, not an implementation detail. \
                   Renaming one makes every record already written with it unreadable, and \
                   a ledger cannot be regenerated from the code. Add entries; never rename \
                   or remove them.";

#[test]
fn every_record_kind_still_serialises_to_the_string_it_was_written_with() {
    let document = frozen();
    let mut produced: BTreeSet<String> = Kind::vocabulary().into_iter().collect();
    produced.insert(Kind::Tombstone.as_str());
    produced = produced
        .into_iter()
        .map(|name| name.strip_prefix("relation.").map(str::to_owned).unwrap_or(name))
        .collect();

    let mut known = expected(&document, "kinds");
    known.extend(expected(&document, "relation_verbs"));

    let lost: Vec<&String> = known.difference(&produced).collect();
    assert!(lost.is_empty(), "these kinds no longer serialise as recorded: {lost:?}. {WHY}");
}

#[test]
fn every_frozen_kind_still_parses() {
    let document = frozen();
    for name in expected(&document, "kinds") {
        assert!(Kind::parse(&name).is_some(), "{name} no longer parses. {WHY}");
    }
    for verb in expected(&document, "relation_verbs") {
        assert!(RelationVerb::parse(&verb).is_some(), "relation {verb} no longer parses. {WHY}");
        assert!(
            Kind::parse(&format!("relation.{verb}")).is_some(),
            "relation.{verb} no longer parses. {WHY}"
        );
    }
}

#[test]
fn the_smaller_vocabularies_are_unchanged() {
    let document = frozen();

    let assurance: BTreeSet<String> =
        [Assurance::Asserted, Assurance::Inferred, Assurance::Speculative]
            .into_iter()
            .map(|value| value.as_str().to_owned())
            .collect();
    assert_eq!(assurance, expected(&document, "assurance"), "{WHY}");

    let lifecycle: BTreeSet<String> =
        [Lifecycle::Active, Lifecycle::Superseded, Lifecycle::Tombstoned]
            .into_iter()
            .map(|value| value.as_str().to_owned())
            .collect();
    assert_eq!(lifecycle, expected(&document, "lifecycle"), "{WHY}");

    let roles: BTreeSet<String> =
        [Role::Subject, Role::Object].into_iter().map(|value| value.as_str().to_owned()).collect();
    assert_eq!(roles, expected(&document, "roles"), "{WHY}");
}
