use std::collections::BTreeMap;
use std::fs;

use codedoc_anchor::Anchor;
use codedoc_core::RepoPath;
use codedoc_lang::Registry;
use codedoc_ledger::{
    AnchorRole, Assurance, Author, Body, Kind, Ledger, Lifecycle, Record, RecordContent, Role,
    SCHEMA_VERSION, Timestamp,
};

fn anchor_for(source: &str) -> Anchor {
    let adapter = Registry::by_name("rust").unwrap();
    let tree = adapter.parse(source).unwrap();
    let node = tree.root_node().child(0).unwrap();
    Anchor::capture(RepoPath::parse("src/auth.rs").unwrap(), adapter, source, node)
}

fn content(claim: &str, at: i64) -> RecordContent {
    RecordContent {
        schema: SCHEMA_VERSION,
        kind: Kind::Rationale,
        anchors: vec![AnchorRole {
            role: Role::Subject,
            anchor: anchor_for("fn authorize() -> bool { true }"),
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

#[test]
fn appended_records_chain_to_the_previous_head() {
    let workspace = tempfile::tempdir().unwrap();
    let ledger = Ledger::initialise(workspace.path()).unwrap();

    let first = ledger.append(content("first", 1_700_000_001)).unwrap();
    let second = ledger.append(content("second", 1_700_000_002)).unwrap();
    let third = ledger.append(content("third", 1_700_000_003)).unwrap();

    assert!(first.content().chain.is_none());
    assert_eq!(second.content().chain.unwrap().to_string(), first.id().to_string());
    assert_eq!(third.content().chain.unwrap().to_string(), second.id().to_string());

    let verification = ledger.verify().unwrap();
    assert_eq!(verification.records, 3);
    assert_eq!(verification.tips, vec![third.id()]);
    assert!(verification.is_intact());
}

#[test]
fn records_survive_reopening_the_ledger() {
    let workspace = tempfile::tempdir().unwrap();
    {
        let ledger = Ledger::initialise(workspace.path()).unwrap();
        ledger.append(content("persisted", 1_700_000_001)).unwrap();
    }
    let reopened = Ledger::open(workspace.path()).unwrap();
    let records = reopened.records().unwrap();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].content().body.claim, "persisted");
}

#[test]
fn a_tampered_record_orphans_its_successor() {
    let workspace = tempfile::tempdir().unwrap();
    let ledger = Ledger::initialise(workspace.path()).unwrap();
    let first = ledger.append(content("original", 1_700_000_001)).unwrap();
    ledger.append(content("successor", 1_700_000_002)).unwrap();
    assert!(ledger.verify().unwrap().is_intact());

    let shard = workspace
        .path()
        .join(".codedoc/ledger")
        .join(format!("{}.jsonl", &first.id().to_string()[..2]));
    let raw = fs::read_to_string(&shard).unwrap();
    let tampered = raw.replace("original", "falsified");
    fs::write(&shard, tampered).unwrap();

    let verification = ledger.verify().unwrap();
    assert!(
        !verification.is_intact(),
        "editing a record must break the chain, got {verification:?}"
    );
    assert_eq!(verification.orphans.len(), 1);
}

#[test]
fn identical_content_produces_identical_identity() {
    let left = Record::seal(content("same", 1_700_000_000)).unwrap();
    let right = Record::seal(content("same", 1_700_000_000)).unwrap();
    assert_eq!(left.id(), right.id());
}

#[test]
fn shards_partition_records_by_identity_prefix() {
    let workspace = tempfile::tempdir().unwrap();
    let ledger = Ledger::initialise(workspace.path()).unwrap();
    for index in 0..12 {
        ledger.append(content(&format!("claim {index}"), 1_700_000_000 + index)).unwrap();
    }
    let directory = workspace.path().join(".codedoc/ledger");
    let shards: Vec<_> = fs::read_dir(&directory).unwrap().filter_map(Result::ok).collect();
    assert!(shards.len() > 1, "expected sharding across multiple files");
    assert_eq!(ledger.records().unwrap().len(), 12);

    for shard in shards {
        let name = shard.file_name().to_string_lossy().to_string();
        let prefix = name.trim_end_matches(".jsonl").to_owned();
        let raw = fs::read_to_string(shard.path()).unwrap();
        for line in raw.lines().filter(|line| !line.is_empty()) {
            let record = Record::decode_line(line.as_bytes()).unwrap();
            assert_eq!(&record.id().to_string()[..2], prefix);
        }
    }
}

#[test]
fn initialising_twice_is_refused() {
    let workspace = tempfile::tempdir().unwrap();
    Ledger::initialise(workspace.path()).unwrap();
    assert!(Ledger::initialise(workspace.path()).is_err());
}

#[test]
fn the_index_is_excluded_from_version_control() {
    let workspace = tempfile::tempdir().unwrap();
    Ledger::initialise(workspace.path()).unwrap();
    let ignore = fs::read_to_string(workspace.path().join(".codedoc/.gitignore")).unwrap();
    assert!(ignore.contains("index.sqlite"));
}
