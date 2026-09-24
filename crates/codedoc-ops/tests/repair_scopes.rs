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
