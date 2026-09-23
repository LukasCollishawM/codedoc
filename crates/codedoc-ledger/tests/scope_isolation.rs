use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::process::Command;

use codedoc_anchor::Anchor;
use codedoc_core::RepoPath;
use codedoc_lang::Registry;
use codedoc_ledger::{
    AnchorRole, Assurance, Author, Body, Kind, Ledger, Lifecycle, RecordContent, Role,
    SCHEMA_VERSION, Scope, Timestamp, Workspace,
};

fn git(root: &Path, arguments: &[&str]) -> String {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(arguments)
        .output()
        .expect("git must be available for these tests");
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn repository() -> tempfile::TempDir {
    let workspace = tempfile::tempdir().expect("a temporary directory");
    let root = workspace.path();
    git(root, &["init", "-q"]);
    git(root, &["config", "user.email", "t@t"]);
    git(root, &["config", "user.name", "t"]);
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(root.join("src/lib.rs"), "fn parse(raw: &str) -> bool {\n    !raw.is_empty()\n}\n")
        .unwrap();
    git(root, &["add", "-A"]);
    git(root, &["commit", "-q", "-m", "someone else's project"]);
    workspace
}

fn claim(root: &Path) -> RecordContent {
    let adapter = Registry::by_name("rust").unwrap();
    let source = fs::read_to_string(root.join("src/lib.rs")).unwrap();
    let tree = adapter.parse(&source).unwrap();
    let anchor = Anchor::capture(
        RepoPath::parse("src/lib.rs").unwrap(),
        adapter,
        &source,
        tree.root_node().child(0).unwrap(),
    );
    RecordContent {
        schema: SCHEMA_VERSION,
        kind: Kind::Invariant,
        anchors: vec![AnchorRole { role: Role::Subject, anchor }],
        body: Body { claim: "Empty input is rejected.".to_owned(), detail: None },
        evidence: Vec::new(),
        assurance: Assurance::Inferred,
        author: Author::Agent { model: "m".to_owned(), session: "s".to_owned() },
        code_revision: None,
        created: Timestamp::from_unix_seconds(1_700_000_000),
        lifecycle: Lifecycle::Active,
        parent: None,
        chain: None,
        unrecognised: BTreeMap::new(),
    }
}

#[test]
fn a_local_ledger_leaves_no_trace_in_the_working_tree() {
    let workspace = repository();
    let root = workspace.path();
    let before = git(root, &["status", "--porcelain", "--untracked-files=all"]);
    assert!(before.trim().is_empty(), "the fixture must start clean");

    let ledger = Ledger::initialise_scope(root, Scope::Local).unwrap();
    ledger.append(claim(root)).unwrap();

    let after = git(root, &["status", "--porcelain", "--untracked-files=all"]);
    assert!(
        after.trim().is_empty(),
        "a local ledger must be invisible to git. This is the whole reason the scope \
         exists: it is for repositories you do not own. git reported:\n{after}"
    );
    assert!(
        !root.join(".codedoc").exists(),
        "nothing may be written to the working tree for a local scope, including an \
         ignore file, because an ignore file is itself evidence"
    );
    assert!(root.join(".git/codedoc/ledger").is_dir(), "the ledger must exist somewhere");
}

#[test]
fn a_shared_ledger_is_deliberately_visible() {
    let workspace = repository();
    let root = workspace.path();
    let ledger = Ledger::initialise_scope(root, Scope::Shared).unwrap();
    ledger.append(claim(root)).unwrap();

    let status = git(root, &["status", "--porcelain"]);
    assert!(
        status.contains(".codedoc"),
        "a shared ledger is committed and must show up for review; got {status:?}"
    );
    assert!(root.join(".codedoc/.gitignore").exists(), "the derived index must be ignored");
}

#[test]
fn scopes_initialise_independently_of_one_another() {
    let workspace = repository();
    let root = workspace.path();
    Ledger::initialise_scope(root, Scope::Shared).unwrap();
    Ledger::initialise_scope(root, Scope::Local)
        .expect("a local ledger must not be blocked by a shared one already existing");
}

#[test]
fn reads_merge_across_scopes_while_writes_target_one() {
    let workspace = repository();
    let root = workspace.path();
    let shared = Ledger::initialise_scope(root, Scope::Shared).unwrap();
    let local = Ledger::initialise_scope(root, Scope::Local).unwrap();

    let mut team = claim(root);
    team.body.claim = "The team knows this.".to_owned();
    shared.append(team).unwrap();

    let mut mine = claim(root);
    mine.body.claim = "Only I know this.".to_owned();
    local.append(mine).unwrap();

    let found = Workspace::at(root);
    let claims: Vec<String> =
        found.records().unwrap().iter().map(|record| record.content().body.claim.clone()).collect();
    assert_eq!(claims.len(), 2, "reads must merge every scope present");
    assert!(claims.iter().any(|text| text == "The team knows this."));
    assert!(claims.iter().any(|text| text == "Only I know this."));

    assert_eq!(shared.records().unwrap().len(), 1, "writes must not leak between scopes");
    assert_eq!(local.records().unwrap().len(), 1);
}

#[test]
fn a_local_ledger_survives_a_commit_of_everything() {
    let workspace = repository();
    let root = workspace.path();
    let ledger = Ledger::initialise_scope(root, Scope::Local).unwrap();
    ledger.append(claim(root)).unwrap();

    git(root, &["add", "-A"]);
    git(root, &["commit", "-q", "-m", "commit everything"]);

    let tracked = git(root, &["ls-files"]);
    assert!(
        !tracked.contains("codedoc"),
        "even `git add -A` must not be able to capture a local ledger; tracked:\n{tracked}"
    );
    assert_eq!(Ledger::open_scope(root, Scope::Local).unwrap().records().unwrap().len(), 1);
}
