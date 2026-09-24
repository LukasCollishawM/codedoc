use std::fs;

use codedoc_ledger::{Evidence, Ledger};

fn project() -> tempfile::TempDir {
    let root = tempfile::tempdir().expect("a temporary directory");
    fs::create_dir_all(root.path().join("src")).unwrap();
    fs::write(
        root.path().join("src/lib.rs"),
        "pub fn compute(d: &[u8]) -> u32 {\n    d.len() as u32\n}\n",
    )
    .unwrap();
    fs::write(root.path().join("NOTES.md"), "# Notes\n").unwrap();
    Ledger::initialise(root.path()).unwrap();
    root
}

fn attach(root: &std::path::Path, claim: &str, evidence: Vec<Evidence>) {
    let request = codedoc_ops::AttachRequest {
        target: codedoc_ops::Target::symbol("src/lib.rs", "rust://compute"),
        kind: "invariant".to_owned(),
        claim: claim.to_owned(),
        detail: None,
    };
    codedoc_ops::attach(
        root,
        None,
        &request,
        &codedoc_ops::Attribution::human("tester"),
        codedoc_ops::Provenance { evidence, ..Default::default() },
    )
    .unwrap();
}

#[test]
fn a_corpus_with_nothing_wrong_reports_healthy_and_exits_zero() {
    let root = project();
    attach(root.path(), "Callers pass one frame at a time.", Vec::new());

    let (report, code) = codedoc_ops::doctor(root.path()).unwrap();
    assert_eq!(report["verdict"], "healthy", "{report}");
    assert_eq!(code, 0);
    assert!(report["next"].as_array().unwrap().is_empty());
}

#[test]
fn a_detached_anchor_blocks_but_a_drifted_one_only_asks_to_be_read() {
    let root = project();
    attach(root.path(), "Callers pass one frame at a time.", Vec::new());

    fs::write(
        root.path().join("src/lib.rs"),
        "pub fn compute(d: &[u8]) -> u32 {\n    let mut total = 0u32;\n    for byte in d {\n        total = total.wrapping_add(u32::from(*byte));\n    }\n    total\n}\n",
    )
    .unwrap();
    let (drifted, code) = codedoc_ops::doctor(root.path()).unwrap();
    assert_eq!(
        drifted["verdict"], "needs reading",
        "a claim whose code changed needs a person to decide, and failing a build on \
         it teaches people to ignore it: {drifted}"
    );
    assert_eq!(code, 0, "advisory findings must not fail a build");

    fs::write(root.path().join("src/lib.rs"), "pub fn unrelated() -> u32 {\n    7\n}\n").unwrap();
    let (gone, blocked) = codedoc_ops::doctor(root.path()).unwrap();
    assert_eq!(
        gone["verdict"], "unhealthy",
        "an anchor pointing at code that is not there is settled without anyone's \
         judgement, so it blocks: {gone}"
    );
    assert_eq!(blocked, 2);
}

#[test]
fn a_citation_that_stopped_resolving_blocks_and_says_what_to_run() {
    let root = project();
    attach(
        root.path(),
        "Callers pass one frame at a time.",
        vec![Evidence::Document("NOTES.md".to_owned())],
    );
    fs::remove_file(root.path().join("NOTES.md")).unwrap();

    let (report, code) = codedoc_ops::doctor(root.path()).unwrap();
    assert_eq!(report["verdict"], "unhealthy", "{report}");
    assert_eq!(code, 2);
    let next = report["next"].as_array().unwrap();
    assert!(
        next.iter().any(|line| line.as_str().unwrap_or_default().contains("codedoc evidence")),
        "a verdict that does not say what to run next is a verdict someone has to \
         go and decode: {report}"
    );
}

#[test]
fn coverage_says_when_a_thin_file_already_carries_a_claim_about_itself() {
    let root = project();
    let request = codedoc_ops::AttachRequest {
        target: codedoc_ops::Target::file("src/lib.rs"),
        kind: "explanation".to_owned(),
        claim: "Everything here assumes the input is a whole frame.".to_owned(),
        detail: None,
    };
    codedoc_ops::attach(
        root.path(),
        None,
        &request,
        &codedoc_ops::Attribution::human("tester"),
        codedoc_ops::Provenance::default(),
    )
    .unwrap();

    let report = codedoc_ops::coverage(root.path(), &[], 10).unwrap();
    let thinnest = report["thinnest"].as_array().expect("a listing");
    let entry = thinnest
        .iter()
        .find(|row| row["file"] == "src/lib.rs")
        .unwrap_or_else(|| panic!("src/lib.rs is missing: {report}"));

    assert_eq!(entry["documented"], 0, "a claim about the file documents no declaration");
    assert_eq!(
        entry["about_the_file"], 1,
        "this listing is where someone looks to decide what to document next, so \
         sending them to a file that already carries a claim about itself wastes \
         the trip: {report}"
    );
}
