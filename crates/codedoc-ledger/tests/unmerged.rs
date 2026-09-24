use std::collections::BTreeMap;
use std::fs;

use codedoc_anchor::Anchor;
use codedoc_core::RepoPath;
use codedoc_lang::Registry;
use codedoc_ledger::{
    AnchorRole, Assurance, Author, Body, Kind, Ledger, Lifecycle, RecordContent, Role,
    SCHEMA_VERSION, Timestamp,
};

fn content(claim: &str, at: i64) -> RecordContent {
    let adapter = Registry::by_name("rust").unwrap();
    let source = "fn authorize() -> bool { true }";
    let tree = adapter.parse(source).unwrap();
    let node = tree.root_node().child(0).unwrap();
    RecordContent {
        schema: SCHEMA_VERSION,
        kind: Kind::Invariant,
        anchors: vec![AnchorRole {
            role: Role::Subject,
            anchor: Anchor::capture(RepoPath::parse("src/auth.rs").unwrap(), adapter, source, node),
        }],
        body: Body { claim: claim.to_owned(), detail: None },
        evidence: Vec::new(),
        assurance: Assurance::Asserted,
        author: Author::Agent {
            model: "claude-opus-5".to_owned(),
            session: "session-1".to_owned(),
        },
        code_revision: None,
        created: Timestamp::from_unix_seconds(at),
        lifecycle: Lifecycle::Active,
        parent: None,
        chain: None,
        unrecognised: BTreeMap::new(),
    }
}

fn shard_holding_records(root: &std::path::Path) -> std::path::PathBuf {
    let shards = fs::read_dir(root.join(".codedoc/ledger")).expect("the ledger directory");
    shards
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .find(|path| path.extension().is_some_and(|extension| extension == "jsonl"))
        .expect("a shard was written")
}

fn conflicted(ours: &str, theirs: &str) -> String {
    format!("<<<<<<< HEAD\n{ours}=======\n{theirs}>>>>>>> branch\n")
}

#[test]
fn a_shard_left_conflicted_by_git_names_the_missing_merge_driver() {
    let workspace = tempfile::tempdir().unwrap();
    let ledger = Ledger::initialise(workspace.path()).unwrap();
    ledger.append(content("tokens are validated before tenant resolution", 1_700_000_001)).unwrap();

    let shard = shard_holding_records(workspace.path());
    let ours = fs::read_to_string(&shard).unwrap();
    let theirs = ours.replace("tenant resolution", "tenant lookup");
    fs::write(&shard, conflicted(&ours, &theirs)).unwrap();

    let reopened = Ledger::open(workspace.path()).unwrap();
    let failure = reopened.records().expect_err("a conflicted shard cannot be read");
    let message = failure.to_string();

    assert!(
        message.contains("conflict marker"),
        "git wrote conflict markers into an append-only log, which is a merge that was \
         never set up rather than a corrupt record. Reporting it as malformed canonical \
         encoding sends the reader looking for a bad record. Got: {message}"
    );
    assert!(
        message.contains("codedoc git install-merge-driver"),
        "the error has to carry the remedy: registration lives in .git/config, which a \
         clone does not copy, so the second person on a project hits this with a \
         committed .gitattributes and no driver. Got: {message}"
    );
}

#[test]
fn a_genuinely_malformed_record_is_still_reported_as_malformed() {
    let workspace = tempfile::tempdir().unwrap();
    let ledger = Ledger::initialise(workspace.path()).unwrap();
    ledger.append(content("tokens are validated", 1_700_000_001)).unwrap();

    let shard = shard_holding_records(workspace.path());
    fs::write(&shard, "{\"not\":\"a record\"}\n").unwrap();

    let reopened = Ledger::open(workspace.path()).unwrap();
    let message = reopened.records().expect_err("a bad record cannot be read").to_string();

    assert!(
        message.contains("malformed"),
        "only conflict markers get the merge advice; everything else stays a decode \
         failure. Got: {message}"
    );
    assert!(!message.contains("install-merge-driver"), "{message}");
}
