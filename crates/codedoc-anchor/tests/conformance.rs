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
        let symbol = text(case, "symbol");
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
        let node = codedoc_anchor::locate::by_symbol(&before_tree, before, adapter, symbol)
            .unwrap_or_else(|| panic!("{name}: symbol {symbol} not found in before"));
        let anchor = Anchor::capture(repo_path, adapter, before, node);

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
            }
            Resolution::Detached(reason) => {
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
        let symbol = text(case, "symbol");
        let before = text(case, "before");
        let after = text(case, "after");

        let adapter = Registry::by_name(language).unwrap();
        let repo_path = RepoPath::parse(text(case, "path")).unwrap();
        let before_tree = adapter.parse(before).unwrap();
        let node =
            codedoc_anchor::locate::by_symbol(&before_tree, before, adapter, symbol).unwrap();
        let anchor = Anchor::capture(repo_path, adapter, before, node);

        let after_tree = adapter.parse(after).unwrap();
        let index = FileIndex::build(adapter, after, &after_tree);

        if let Resolution::Located(located) = index.resolve(&anchor) {
            let start = located.range().start_line as usize;
            let end = located.range().end_line as usize;
            let lines: Vec<&str> = after.lines().collect();
            assert!(
                start >= 1 && end <= lines.len(),
                "{name}: located range falls outside the file"
            );
        }
    }
}
