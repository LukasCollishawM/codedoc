use std::fs;
use std::path::Path;
use std::process::Command;

use codedoc_anchor::Anchor;
use codedoc_core::RepoPath;
use codedoc_lang::Registry;
use codedoc_ledger::{
    AnchorRole, Assurance, Author, Body, Kind, Ledger, Lifecycle, RecordContent, Role,
    SCHEMA_VERSION, Timestamp, Workspace,
};
use codedoc_verify::{Status, Verifier};

fn git(root: &Path, arguments: &[&str]) -> String {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(arguments)
        .output()
        .expect("git must be available");
    String::from_utf8_lossy(&output.stdout).into_owned()
}

const MOVED: &str = "fn guess_encoding(raw: &[u8]) -> bool {\n    !raw.is_empty()\n}\n";

fn repository() -> tempfile::TempDir {
    let workspace = tempfile::tempdir().expect("a temporary directory");
    let root = workspace.path();
    git(root, &["init", "-q"]);
    git(root, &["config", "user.email", "t@t"]);
    git(root, &["config", "user.name", "t"]);
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(root.join("src/utils.rs"), MOVED).unwrap();
    fs::write(root.join("src/lib.rs"), "fn placeholder() {}\n").unwrap();
    git(root, &["add", "-A"]);
    git(root, &["commit", "-q", "-m", "initial"]);
    workspace
}

fn record_for(root: &Path, revision: &str) -> RecordContent {
    let adapter = Registry::by_name("rust").unwrap();
    let source = fs::read_to_string(root.join("src/utils.rs")).unwrap();
    let tree = adapter.parse(&source).unwrap();
    let anchor = Anchor::capture(
        RepoPath::parse("src/utils.rs").unwrap(),
        adapter,
        &source,
        tree.root_node().child(0).unwrap(),
    );
    RecordContent {
        schema: SCHEMA_VERSION,
        kind: Kind::Rationale,
        anchors: vec![AnchorRole { role: Role::Subject, anchor }],
        body: Body { claim: "Empty input is never a known encoding.".to_owned(), detail: None },
        evidence: Vec::new(),
        assurance: Assurance::Asserted,
        author: Author::Human { identity: "maintainer".to_owned() },
        code_revision: revision.parse().ok(),
        created: Timestamp::from_unix_seconds(1_700_000_000),
        lifecycle: Lifecycle::Active,
        parent: None,
        chain: None,
        unrecognised: std::collections::BTreeMap::new(),
    }
}

#[test]
fn a_function_moved_to_another_file_is_followed_rather_than_detached() {
    let workspace = repository();
    let root = workspace.path();
    let revision = git(root, &["rev-parse", "HEAD"]).trim().to_owned();

    let ledger = Ledger::initialise(root).unwrap();
    ledger.append(record_for(root, &revision)).unwrap();

    fs::write(root.join("src/lib.rs"), format!("fn placeholder() {{}}\n\n{MOVED}")).unwrap();
    fs::write(root.join("src/utils.rs"), "fn other() {}\n").unwrap();
    git(root, &["add", "-A"]);
    git(root, &["commit", "-q", "-m", "move the function"]);

    let found = Workspace::at(root);
    let report = Verifier::new(root).run_across(&found).unwrap();
    let finding = report.findings.first().expect("one finding");

    assert_ne!(
        finding.status,
        Status::Detached,
        "a construct that moved between files still exists and must be found, not \
         reported as gone"
    );
    assert_eq!(
        finding.relocated_to.as_deref(),
        Some("src/lib.rs"),
        "the report must name where the code went, or it points the reader at a file \
         that no longer contains it"
    );
}

#[test]
fn a_function_that_was_actually_deleted_still_detaches() {
    let workspace = repository();
    let root = workspace.path();
    let revision = git(root, &["rev-parse", "HEAD"]).trim().to_owned();

    let ledger = Ledger::initialise(root).unwrap();
    ledger.append(record_for(root, &revision)).unwrap();

    fs::write(root.join("src/utils.rs"), "fn other() {}\n").unwrap();
    git(root, &["add", "-A"]);
    git(root, &["commit", "-q", "-m", "delete the function"]);

    let found = Workspace::at(root);
    let report = Verifier::new(root).run_across(&found).unwrap();
    let finding = report.findings.first().expect("one finding");

    assert_eq!(
        finding.status,
        Status::Detached,
        "following a move must not become a licence to find something that is gone"
    );
    assert!(finding.relocated_to.is_none());
}
