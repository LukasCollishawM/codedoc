use std::fs;

use codedoc_ledger::{Ledger, Record, Scope, Workspace};

fn project() -> tempfile::TempDir {
    let root = tempfile::tempdir().expect("a temporary directory");
    fs::create_dir_all(root.path().join("src")).unwrap();
    fs::create_dir_all(root.path().join(".git")).unwrap();
    fs::write(root.path().join("src/lib.rs"), "pub fn only() -> u32 {\n    1\n}\n").unwrap();
    Ledger::initialise_scope(root.path(), Scope::Shared).unwrap();
    Ledger::initialise_scope(root.path(), Scope::Local).unwrap();
    root
}

fn attach(root: &std::path::Path, scope: Scope, claim: &str) {
    let request = codedoc_ops::AttachRequest {
        target: codedoc_ops::Target::symbol("src/lib.rs", "rust://only"),
        kind: "invariant".to_owned(),
        claim: claim.to_owned(),
        detail: None,
    };
    codedoc_ops::attach(
        root,
        Some(scope),
        &request,
        &codedoc_ops::Attribution::human("tester"),
        codedoc_ops::Provenance::default(),
    )
    .unwrap();
}

fn orphan_the_first_record(ledger: &Ledger) {
    let records = ledger.records().unwrap();
    let mut rewritten: Vec<Record> = Vec::new();
    for (position, record) in records.iter().enumerate() {
        let mut content = record.content().clone();
        if position == 0 {
            content.chain = Some(
                "0000000000000000000000000000000000000000000000000000000000000001".parse().unwrap(),
            );
        }
        rewritten.push(Record::seal(content).unwrap());
    }
    ledger.replace_all(&rewritten).unwrap();
}

#[test]
fn repair_without_a_scope_mends_every_ledger_verify_looks_at() {
    let root = project();
    attach(root.path(), Scope::Shared, "The shared claim about this function.");
    attach(root.path(), Scope::Local, "The local claim about this function.");

    orphan_the_first_record(&Ledger::open_scope(root.path(), Scope::Shared).unwrap());
    orphan_the_first_record(&Ledger::open_scope(root.path(), Scope::Local).unwrap());

    let broken = Workspace::at(root.path());
    let orphaned: usize =
        broken.ledgers().iter().map(|ledger| ledger.verify().unwrap().orphans.len()).sum();
    assert_eq!(orphaned, 2, "both ledgers start broken");

    let repaired = codedoc_ops::repair(root.path(), None, true).unwrap();
    assert_eq!(repaired["orphans"], 2, "{repaired}");
    assert_eq!(
        repaired["scopes"].as_array().expect("per-scope detail").len(),
        2,
        "verify reports across every scope, so repair must mend every scope or its \
         diagnosis and its cure disagree: {repaired}"
    );

    let mended = Workspace::at(root.path());
    for ledger in mended.ledgers() {
        assert!(
            ledger.verify().unwrap().is_intact(),
            "{} is still broken after repair",
            ledger.scope().as_str()
        );
    }
}

#[test]
fn repair_with_a_scope_touches_only_that_scope() {
    let root = project();
    attach(root.path(), Scope::Shared, "The shared claim about this function.");
    attach(root.path(), Scope::Local, "The local claim about this function.");
    orphan_the_first_record(&Ledger::open_scope(root.path(), Scope::Shared).unwrap());
    orphan_the_first_record(&Ledger::open_scope(root.path(), Scope::Local).unwrap());

    let repaired = codedoc_ops::repair(root.path(), Some(Scope::Local), true).unwrap();
    assert_eq!(repaired["scopes"].as_array().unwrap().len(), 1, "{repaired}");

    assert!(Ledger::open_scope(root.path(), Scope::Local).unwrap().verify().unwrap().is_intact());
    assert!(
        !Ledger::open_scope(root.path(), Scope::Shared).unwrap().verify().unwrap().is_intact(),
        "naming a scope must not quietly rewrite the others"
    );
}

#[test]
fn scope_narrows_the_commands_that_enumerate_records() {
    let root = project();
    attach(root.path(), Scope::Shared, "The shared claim about this function.");
    attach(root.path(), Scope::Local, "The local claim about this function.");

    let everything = codedoc_ops::list(root.path(), None, None, None, None, None).unwrap();
    assert_eq!(everything["count"], 2, "{everything}");

    let only_local =
        codedoc_ops::list(root.path(), Some(Scope::Local), None, None, None, None).unwrap();
    assert_eq!(
        only_local["count"], 1,
        "working in someone else's repository, 'what have I recorded locally' is the \
         question you most need answered: {only_local}"
    );
    assert!(only_local["records"][0]["claim"].as_str().unwrap().contains("local claim"));

    let found =
        codedoc_ops::search(root.path(), Some(Scope::Shared), "claim", None, None, 10).unwrap();
    assert_eq!(found["count"], 1, "{found}");
    assert!(found["records"][0]["claim"].as_str().unwrap().contains("shared claim"));

    let counted = codedoc_ops::stats(root.path(), Some(Scope::Local)).unwrap();
    assert_eq!(counted["total_records"], 1, "{counted}");
}

#[test]
fn verification_still_reads_every_scope_when_one_is_named_for_writing() {
    let root = project();
    attach(root.path(), Scope::Shared, "The shared claim about this function.");
    attach(root.path(), Scope::Local, "The local claim about this function.");

    let (checked, _) = codedoc_ops::verify(root.path()).unwrap();
    assert_eq!(
        checked["records"], 2,
        "a claim recorded locally is still true while you verify, so narrowing what \
         verification reads would make it answer the wrong question: {checked}"
    );
}

#[test]
fn superseding_a_local_record_stays_local() {
    let root = project();
    attach(root.path(), Scope::Local, "The local claim about this function.");
    let listed =
        codedoc_ops::list(root.path(), Some(Scope::Local), None, None, None, None).unwrap();
    let id = listed["records"][0]["record"].as_str().unwrap().to_owned();

    codedoc_ops::affirm(
        root.path(),
        None,
        &id,
        &codedoc_ops::Attribution::human("a reviewer"),
        None,
    )
    .unwrap();

    let shared = Ledger::open_scope(root.path(), Scope::Shared).unwrap();
    assert!(
        shared.records().unwrap().is_empty(),
        "a record kept in the untracked ledger must not be copied into the repository \
         by affirming it — that would publish, into a repository the owners did not \
         agree to put it in, exactly the note the local scope exists to keep out"
    );
    assert_eq!(
        Ledger::open_scope(root.path(), Scope::Local).unwrap().records().unwrap().len(),
        2,
        "the affirmation belongs beside the record it affirms"
    );
}

#[test]
fn retracting_a_local_record_stays_local() {
    let root = project();
    attach(root.path(), Scope::Local, "The local claim about this function.");
    let listed =
        codedoc_ops::list(root.path(), Some(Scope::Local), None, None, None, None).unwrap();
    let id = listed["records"][0]["record"].as_str().unwrap().to_owned();

    codedoc_ops::retract(root.path(), None, &id, Some("no longer true")).unwrap();

    assert!(
        Ledger::open_scope(root.path(), Scope::Shared).unwrap().records().unwrap().is_empty(),
        "a tombstone quotes the claim it retires, so writing it to the shared ledger \
         would publish the very text the local scope was keeping out of the repository"
    );
}
