use std::fs;

use codedoc_ledger::Ledger;

fn project() -> tempfile::TempDir {
    let root = tempfile::tempdir().expect("a temporary directory");
    fs::create_dir_all(root.path().join("src")).unwrap();
    for (name, body) in [
        ("src/auth.rs", "pub fn validate(token: &str) -> bool {\n    !token.is_empty()\n}\n"),
        ("src/cache.rs", "pub fn evict(key: &str) -> bool {\n    !key.is_empty()\n}\n"),
        ("src/untouched.rs", "pub fn other() -> bool {\n    true\n}\n"),
    ] {
        fs::write(root.path().join(name), body).unwrap();
    }
    Ledger::initialise(root.path()).unwrap();
    root
}

fn attach(root: &std::path::Path, file: &str, symbol: &str, kind: &str, claim: &str) {
    let request = codedoc_ops::AttachRequest {
        target: codedoc_ops::Target::symbol(file, symbol),
        kind: kind.to_owned(),
        claim: claim.to_owned(),
        detail: None,
    };
    codedoc_ops::attach(
        root,
        None,
        &request,
        &codedoc_ops::Attribution::human("tester"),
        codedoc_ops::Provenance::default(),
    )
    .unwrap();
}

fn populate(root: &std::path::Path) {
    attach(root, "src/auth.rs", "rust://validate", "invariant", "Validation precedes tenancy.");
    attach(root, "src/cache.rs", "rust://evict", "performance", "Eviction is constant time.");
    attach(root, "src/untouched.rs", "rust://other", "invariant", "Nothing here is relevant.");
}

#[test]
fn a_brief_covers_every_named_file_and_nothing_else() {
    let root = project();
    populate(root.path());

    let pack = codedoc_ops::brief(
        root.path(),
        &["src/auth.rs".to_owned(), "src/cache.rs".to_owned()],
        None,
        0,
        None,
    )
    .unwrap();

    assert_eq!(pack["claims"], 2, "{pack}");
    let rendered = pack.to_string();
    assert!(rendered.contains("Validation precedes tenancy"));
    assert!(rendered.contains("Eviction is constant time"));
    assert!(
        !rendered.contains("Nothing here is relevant"),
        "a brief is about the work you are starting, not the whole repository"
    );
}

#[test]
fn a_claim_in_a_brief_says_which_file_it_is_about() {
    let root = project();
    populate(root.path());

    let pack = codedoc_ops::brief(
        root.path(),
        &["src/auth.rs".to_owned(), "src/cache.rs".to_owned()],
        None,
        0,
        None,
    )
    .unwrap();

    let invariant = &pack["pack"]["invariants"][0];
    assert_eq!(
        invariant["file"], "src/auth.rs",
        "across several files, a claim with no file attached is a claim you cannot \
         act on: {pack}"
    );
    assert_eq!(invariant["symbol"], "rust://validate", "{pack}");
}

#[test]
fn the_budget_is_spent_across_the_change_rather_than_per_file() {
    let root = project();
    populate(root.path());

    let tight = codedoc_ops::brief(
        root.path(),
        &["src/auth.rs".to_owned(), "src/cache.rs".to_owned()],
        None,
        0,
        Some(30),
    )
    .unwrap();

    assert_eq!(
        tight["claims"], 1,
        "a budget applied per file returns something from each and overruns the \
         caller's context window; applied across the change it returns what matters \
         most about the change: {tight}"
    );
    assert_eq!(tight["pack"]["truncated"], true, "{tight}");
}

#[test]
fn naming_no_files_and_no_revision_is_refused_rather_than_answered_with_everything() {
    let root = project();
    populate(root.path());

    let refused = codedoc_ops::brief(root.path(), &[], None, 0, None);
    assert!(
        refused.is_err(),
        "answering an unnamed brief with the whole ledger would quietly hand back \
         something far larger than the caller asked for"
    );
}
