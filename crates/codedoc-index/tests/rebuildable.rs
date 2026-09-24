use std::collections::BTreeMap;
use std::fs;

use codedoc_anchor::Anchor;
use codedoc_core::RepoPath;
use codedoc_index::{INDEX_SCHEMA_VERSION, Index};
use codedoc_lang::Registry;
use codedoc_ledger::{
    AnchorRole, Assurance, Author, Body, Kind, Ledger, Lifecycle, RecordContent, Role,
    SCHEMA_VERSION, Timestamp,
};

fn anchor_at(path: &str, source: &str) -> Anchor {
    let adapter = Registry::by_name("rust").unwrap();
    let tree = adapter.parse(source).unwrap();
    let node = tree.root_node().child(0).unwrap();
    Anchor::capture(RepoPath::parse(path).unwrap(), adapter, source, node)
}

fn content(claim: &str, path: &str, source: &str, at: i64) -> RecordContent {
    RecordContent {
        schema: SCHEMA_VERSION,
        kind: Kind::Invariant,
        anchors: vec![AnchorRole { role: Role::Subject, anchor: anchor_at(path, source) }],
        body: Body { claim: claim.to_owned(), detail: None },
        evidence: Vec::new(),
        assurance: Assurance::Asserted,
        author: Author::Human { identity: "maintainer".to_owned() },
        code_revision: None,
        created: Timestamp::from_unix_seconds(at),
        lifecycle: Lifecycle::Active,
        parent: None,
        chain: None,
        unrecognised: BTreeMap::new(),
    }
}

fn populated() -> (tempfile::TempDir, Ledger) {
    let workspace = tempfile::tempdir().unwrap();
    let ledger = Ledger::initialise(workspace.path()).unwrap();
    ledger
        .append(content(
            "authorization precedes reservation",
            "src/auth.rs",
            "fn authorize() -> bool { true }",
            1_700_000_001,
        ))
        .unwrap();
    ledger
        .append(content(
            "reservation is idempotent",
            "src/reserve.rs",
            "fn reserve(key: u32) -> bool { key > 0 }",
            1_700_000_002,
        ))
        .unwrap();
    ledger
        .append(content(
            "retries preserve the idempotency key",
            "src/auth.rs",
            "fn retry() -> u32 { 3 }",
            1_700_000_003,
        ))
        .unwrap();
    (workspace, ledger)
}

#[test]
fn the_index_reconstructs_from_the_ledger_alone() {
    let (_workspace, ledger) = populated();

    let first = Index::rebuild(&ledger).unwrap();
    let original_digest = first.content_digest().unwrap();
    let original_count = first.record_count().unwrap();
    let path = first.path().to_path_buf();
    drop(first);

    fs::remove_file(&path).unwrap();
    assert!(!path.exists());

    let rebuilt = Index::rebuild(&ledger).unwrap();
    assert_eq!(rebuilt.record_count().unwrap(), original_count);
    assert_eq!(
        rebuilt.content_digest().unwrap(),
        original_digest,
        "a rebuilt index must reproduce the ledger projection exactly"
    );
}

#[test]
fn repeated_rebuilds_are_byte_identical() {
    let (_workspace, ledger) = populated();

    let first = Index::rebuild(&ledger).unwrap();
    let path = first.path().to_path_buf();
    drop(first);
    let first_bytes = fs::read(&path).unwrap();

    let second = Index::rebuild(&ledger).unwrap();
    drop(second);
    let second_bytes = fs::read(&path).unwrap();

    assert_eq!(
        first_bytes, second_bytes,
        "rebuilding the same ledger must produce identical index bytes"
    );
}

#[test]
fn anchors_are_queryable_by_file_and_symbol() {
    let (_workspace, ledger) = populated();
    let index = Index::rebuild(&ledger).unwrap();

    let in_auth = index.anchors_in_file("src/auth.rs").unwrap();
    assert_eq!(in_auth.len(), 2);
    assert!(in_auth.iter().all(|entry| entry.file == "src/auth.rs"));

    let by_symbol = index.anchors_for_symbol("rust://authorize").unwrap();
    assert_eq!(by_symbol.len(), 1);
    assert_eq!(by_symbol[0].claim, "authorization precedes reservation");

    assert_eq!(index.files().unwrap(), vec!["src/auth.rs", "src/reserve.rs"]);
}

#[test]
fn an_empty_ledger_yields_an_empty_index() {
    let workspace = tempfile::tempdir().unwrap();
    let ledger = Ledger::initialise(workspace.path()).unwrap();
    let index = Index::rebuild(&ledger).unwrap();
    assert_eq!(index.record_count().unwrap(), 0);
    assert!(index.files().unwrap().is_empty());
}

#[test]
fn an_index_written_by_an_older_schema_is_rebuilt_rather_than_refused() {
    let (_workspace, ledger) = populated();
    Index::rebuild(&ledger).unwrap();

    let path = Index::path_for(ledger.base());
    let connection = rusqlite::Connection::open(&path).unwrap();
    connection.pragma_update(None, "user_version", INDEX_SCHEMA_VERSION - 1).unwrap();
    drop(connection);
    assert!(!Index::is_current(ledger.base()));

    let index = Index::current(&ledger).expect(
        "every schema change strands the index of everyone who upgrades, so an old \
         version has to rebuild silently rather than fail",
    );
    assert_eq!(index.record_count().unwrap(), 3);
    assert!(Index::is_current(ledger.base()));
}

#[test]
fn an_index_that_is_not_a_database_is_replaced_rather_than_fatal() {
    let (_workspace, ledger) = populated();
    Index::rebuild(&ledger).unwrap();

    let path = Index::path_for(ledger.base());
    std::fs::write(&path, b"this is not a database").unwrap();

    let index = Index::current(&ledger).expect(
        "the index is derived state that a crash or a full disk can truncate. Losing \
         it costs a rebuild; refusing to start because of it costs the ledger",
    );
    assert_eq!(index.record_count().unwrap(), 3);
}
