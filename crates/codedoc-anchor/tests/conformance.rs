use std::fs;
use std::path::PathBuf;

use codedoc_anchor::{Anchor, FileIndex, Resolution};
use codedoc_core::{Canonical, RepoPath};
use codedoc_lang::Registry;

fn vectors() -> Canonical {
    let path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../conformance/resolver/vectors.json");
    let raw = fs::read(path).expect("resolver conformance vectors are missing");
    Canonical::decode(&raw).expect("resolver vectors are not canonical JSON")
}

fn text<'a>(value: &'a Canonical, name: &str) -> &'a str {
    value
        .field(name)
        .and_then(Canonical::as_text)
        .unwrap_or_else(|| panic!("vector is missing the {name} member"))
}

#[test]
fn every_resolver_vector_reaches_its_stated_outcome() {
    let document = vectors();
    let Some(Canonical::Array(cases)) = document.field("vectors") else {
        panic!("vectors member must be an array");
    };
    assert!(!cases.is_empty(), "the resolver vector set must not be empty");

    for case in cases {
        let name = text(case, "name");
        let language = text(case, "language");
        let path = text(case, "path");
        let subject = case.field("subject").and_then(Canonical::as_text).unwrap_or("construct");
        let before = text(case, "before");
        let after = text(case, "after");
        let expect = case.field("expect").expect("vector is missing expect");
        let expected_outcome = text(expect, "outcome");
        let expected_detail = text(expect, "detail");

        let adapter =
            Registry::by_name(language).unwrap_or_else(|| panic!("{name}: unknown language"));
        let repo_path = RepoPath::parse(path).unwrap_or_else(|_| panic!("{name}: bad path"));
        let before_tree =
            adapter.parse(before).unwrap_or_else(|_| panic!("{name}: before does not parse"));
        let anchor = if subject == "file" {
            Anchor::capture_file(repo_path, adapter, before, before_tree.root_node())
        } else {
            let symbol = text(case, "symbol");
            let node = codedoc_anchor::locate::by_symbol(&before_tree, before, adapter, symbol)
                .unwrap_or_else(|| panic!("{name}: symbol {symbol} not found in before"));
            Anchor::capture(repo_path, adapter, before, node)
        };

        if let Some(Canonical::Integer(expected)) = case.field("symbol_cardinality") {
            assert_eq!(
                i64::from(anchor.symbol_cardinality),
                *expected,
                "{name}: a symbol path that names several declarations must record how                  many, or deleting one of them silently reattaches its record to a sibling"
            );
        }

        let after_tree =
            adapter.parse(after).unwrap_or_else(|_| panic!("{name}: after does not parse"));
        let index = FileIndex::build(adapter, after, &after_tree);

        match index.resolve(&anchor) {
            Resolution::Located(located) => {
                assert_eq!(expected_outcome, "located", "{name}: expected detachment");
                let rung = serde_json::to_string(&located.rung()).unwrap();
                assert_eq!(
                    rung.trim_matches('"'),
                    expected_detail,
                    "{name}: reached a different rung"
                );

                if let Some(Canonical::Integer(expected_drift)) = case.field("drift") {
                    let target = located
                        .node_path()
                        .descend(after_tree.root_node())
                        .unwrap_or_else(|| panic!("{name}: located node could not be re-read"));
                    let current = codedoc_anchor::fingerprint::shape_histogram(target, adapter);
                    let measured = codedoc_verify::drift_between(&anchor.shape, &current);
                    assert_eq!(
                        i64::from(measured),
                        *expected_drift,
                        "{name}: drift diverged. Reformatting and renaming must measure                          zero in every language, or staleness reporting becomes noise                          that people learn to ignore."
                    );
                }
            }
            Resolution::Detached(reason) => {
                assert!(
                    matches!(case.field("drift"), Some(Canonical::Null) | None),
                    "{name}: a detached anchor has nothing to measure drift against"
                );
                assert_eq!(expected_outcome, "detached", "{name}: expected a location");
                let encoded = serde_json::to_value(reason).unwrap();
                assert_eq!(
                    encoded.get("reason").and_then(|value| value.as_str()).unwrap_or(""),
                    expected_detail,
                    "{name}: detached for a different reason"
                );
            }
            other => panic!("{name}: unrecognised resolution {other:?}"),
        }
    }
}

#[test]
fn no_vector_resolves_onto_a_node_absent_from_the_original() {
    let document = vectors();
    let Some(Canonical::Array(cases)) = document.field("vectors") else {
        panic!("vectors member must be an array");
    };

    for case in cases {
        let name = text(case, "name");
        let language = text(case, "language");
        let subject = case.field("subject").and_then(Canonical::as_text).unwrap_or("construct");
        let before = text(case, "before");
        let after = text(case, "after");

        let adapter = Registry::by_name(language).unwrap();
        let repo_path = RepoPath::parse(text(case, "path")).unwrap();
        let before_tree = adapter.parse(before).unwrap();
        let anchor = if subject == "file" {
            Anchor::capture_file(repo_path, adapter, before, before_tree.root_node())
        } else {
            let symbol = text(case, "symbol");
            let node =
                codedoc_anchor::locate::by_symbol(&before_tree, before, adapter, symbol).unwrap();
            Anchor::capture(repo_path, adapter, before, node)
        };

        if let Some(Canonical::Integer(expected)) = case.field("symbol_cardinality") {
            assert_eq!(
                i64::from(anchor.symbol_cardinality),
                *expected,
                "{name}: a symbol path that names several declarations must record how                  many, or deleting one of them silently reattaches its record to a sibling"
            );
        }

        let after_tree = adapter.parse(after).unwrap();
        let index = FileIndex::build(adapter, after, &after_tree);

        if let Resolution::Located(located) = index.resolve(&anchor) {
            let start = located.range().start_line as usize;
            let end = located.range().end_line as usize;
            let lines: Vec<&str> = after.lines().collect();
            if subject == "file" {
                assert!(
                    start == 1 && end >= lines.len(),
                    "{name}: a claim about the file must span the file"
                );
            } else {
                assert!(
                    start >= 1 && end <= lines.len(),
                    "{name}: located range falls outside the file"
                );
            }
        }
    }
}
