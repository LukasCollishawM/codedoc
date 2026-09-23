use std::collections::BTreeMap;

use codedoc_anchor::Anchor;
use codedoc_core::RepoPath;
use codedoc_lang::Registry;
use codedoc_ledger::{
    AnchorRole, Assurance, Author, Body, Kind, Lifecycle, Record, RecordContent, RelationVerb,
    Role, SCHEMA_VERSION, Timestamp,
};

pub fn anchor_for(symbol: &str) -> Anchor {
    let leaf = symbol.rsplit('/').next().unwrap_or("target");
    let source = format!("fn {leaf}() {{ work(); }}\n");
    let adapter = Registry::by_name("rust").unwrap();
    let tree = adapter.parse(&source).unwrap();
    let node = tree.root_node().child(0).unwrap();
    Anchor::capture(RepoPath::parse("src/lib.rs").unwrap(), adapter, &source, node)
}

pub fn record_on(symbol: &str, kind: Kind, claim: &str, assurance: Assurance, at: i64) -> Record {
    Record::seal(RecordContent {
        schema: SCHEMA_VERSION,
        kind,
        anchors: vec![AnchorRole { role: Role::Subject, anchor: anchor_for(symbol) }],
        body: Body { claim: claim.to_owned(), detail: None },
        evidence: Vec::new(),
        assurance,
        author: Author::Human { identity: "maintainer".to_owned() },
        code_revision: None,
        created: Timestamp::from_unix_seconds(at),
        lifecycle: Lifecycle::Active,
        parent: None,
        chain: None,
        unrecognised: BTreeMap::new(),
    })
    .unwrap()
}

pub fn relation_between(subject: &str, object: &str, verb: RelationVerb) -> Record {
    Record::seal(RecordContent {
        schema: SCHEMA_VERSION,
        kind: Kind::Relation(verb),
        anchors: vec![
            AnchorRole { role: Role::Subject, anchor: anchor_for(subject) },
            AnchorRole { role: Role::Object, anchor: anchor_for(object) },
        ],
        body: Body { claim: format!("{subject} {} {object}", verb.as_str()), detail: None },
        evidence: Vec::new(),
        assurance: Assurance::Asserted,
        author: Author::Human { identity: "maintainer".to_owned() },
        code_revision: None,
        created: Timestamp::from_unix_seconds(1),
        lifecycle: Lifecycle::Active,
        parent: None,
        chain: None,
        unrecognised: BTreeMap::new(),
    })
    .unwrap()
}
