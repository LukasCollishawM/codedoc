use std::fs;
use std::path::Path;

use codedoc_anchor::Anchor;
use codedoc_core::RepoPath;
use codedoc_lang::Registry;
use codedoc_ledger::{
    AnchorRole, Assurance, Author, Body, Kind, Ledger, Lifecycle, RecordContent, Role,
    SCHEMA_VERSION, Timestamp, Workspace,
};
use codedoc_verify::{Status, Verifier};

const BEFORE: &str = "fn realm(given: &str) -> String {\n    if given.is_empty() {\n        return \"Authorization Required\".to_owned();\n    }\n    given.to_owned()\n}\n";

fn project(source: &str) -> tempfile::TempDir {
    let workspace = tempfile::tempdir().expect("a temporary directory");
    let root = workspace.path();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(root.join("src/auth.rs"), source).unwrap();
    let ledger = Ledger::initialise(root).unwrap();
    ledger.append(record_for(root)).unwrap();
    workspace
}

fn record_for(root: &Path) -> RecordContent {
    let adapter = Registry::by_name("rust").unwrap();
    let source = fs::read_to_string(root.join("src/auth.rs")).unwrap();
    let tree = adapter.parse(&source).unwrap();
    let anchor = Anchor::capture(
        RepoPath::parse("src/auth.rs").unwrap(),
        adapter,
        &source,
        tree.root_node().child(0).unwrap(),
    );
    RecordContent {
        schema: SCHEMA_VERSION,
        kind: Kind::Explanation,
        anchors: vec![AnchorRole { role: Role::Subject, anchor }],
        body: Body {
            claim: "An empty realm falls back to Authorization Required.".to_owned(),
            detail: None,
        },
        evidence: Vec::new(),
        assurance: Assurance::Asserted,
        author: Author::Human { identity: "maintainer".to_owned() },
        code_revision: None,
        created: Timestamp::from_unix_seconds(1_700_000_000),
        lifecycle: Lifecycle::Active,
        parent: None,
        chain: None,
        unrecognised: std::collections::BTreeMap::new(),
    }
}

fn only_finding(root: &Path) -> codedoc_verify::Finding {
    let found = Workspace::at(root);
    let report = Verifier::new(root).run_across(&found).expect("verify runs");
    let mut findings = report.findings;
    assert_eq!(findings.len(), 1, "one record was recorded");
    findings.remove(0)
}

#[test]
fn nothing_edited_is_not_reported_as_edited() {
    let workspace = project(BEFORE);
    let finding = only_finding(workspace.path());
    assert!(!finding.content_changed, "the file was not touched");
    assert_eq!(finding.status, Status::Fresh);
}

#[test]
fn a_changed_literal_is_reported_although_the_shape_is_identical() {
    let workspace = project(BEFORE);
    let root = workspace.path();
    fs::write(root.join("src/auth.rs"), BEFORE.replace("Authorization Required", "Auth Required"))
        .unwrap();

    let finding = only_finding(root);
    assert_eq!(
        finding.drift,
        Some(0),
        "drift counts node kinds and a different string is still a string, so the \
         structural measure cannot see this edit"
    );
    assert!(
        finding.content_changed,
        "the claim quotes the value that changed, which is the edit most likely to \
         falsify it, and shape drift reports zero. Without this the reviewer is told the \
         claim still resolves and hears nothing else"
    );
}

#[test]
fn reformatting_is_not_an_edit() {
    let workspace = project(BEFORE);
    let root = workspace.path();
    fs::write(root.join("src/auth.rs"), BEFORE.replace("    ", "\t").replace("}\n", "}\n\n"))
        .unwrap();

    let finding = only_finding(root);
    assert!(
        !finding.content_changed,
        "the content fingerprint is a normalised token stream with whitespace stripped, \
         so indentation and blank lines are not edits and must not reach a reviewer as \
         though they were"
    );
}
