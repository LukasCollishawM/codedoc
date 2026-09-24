use std::fs;
use std::process::Command;

use codedoc_ledger::{Evidence, Ledger};

fn project() -> tempfile::TempDir {
    let root = tempfile::tempdir().expect("a temporary directory");
    fs::create_dir_all(root.path().join("src")).unwrap();
    fs::create_dir_all(root.path().join("docs")).unwrap();
    fs::write(
        root.path().join("src/lib.rs"),
        "pub fn compute(d: &[u8]) -> u32 {\n    d.len() as u32\n}\n",
    )
    .unwrap();
    fs::write(root.path().join("docs/design.md"), "# Design\n").unwrap();
    Ledger::initialise(root.path()).unwrap();
    root
}

fn attach(root: &std::path::Path, claim: &str, evidence: Vec<Evidence>) -> String {
    let request = codedoc_ops::AttachRequest {
        target: codedoc_ops::Target::symbol("src/lib.rs", "rust://compute"),
        kind: "invariant".to_owned(),
        claim: claim.to_owned(),
        detail: None,
    };
    let written = codedoc_ops::attach(
        root,
        None,
        &request,
        &codedoc_ops::Attribution::human("tester"),
        codedoc_ops::Provenance { evidence, ..Default::default() },
    )
    .unwrap();
    written["record"].as_str().unwrap().to_owned()
}

#[test]
fn a_citation_that_still_resolves_is_not_reported() {
    let root = project();
    attach(
        root.path(),
        "The length is the whole answer.",
        vec![Evidence::Document("docs/design.md".to_owned())],
    );

    let (report, code) = codedoc_ops::evidence(root.path()).unwrap();
    assert_eq!(report["citations"], 1, "{report}");
    assert_eq!(report["broken"], 0, "{report}");
    assert_eq!(code, 0);
}

#[test]
fn a_claim_citing_a_document_that_was_deleted_is_reported() {
    let root = project();
    attach(
        root.path(),
        "The length is the whole answer.",
        vec![Evidence::Document("docs/design.md".to_owned())],
    );
    fs::remove_file(root.path().join("docs/design.md")).unwrap();

    let (report, code) = codedoc_ops::evidence(root.path()).unwrap();
    assert_eq!(
        report["broken"], 1,
        "a claim that cites a document nobody can read still looks well evidenced, \
         which is worse than citing nothing: {report}"
    );
    assert_eq!(report["records"][0]["standing"], "missing", "{report}");
    assert_eq!(code, 2, "a broken citation is a finding, so the exit code says so");
}

#[test]
fn a_claim_citing_a_retracted_record_is_reported_as_withdrawn() {
    let root = project();
    let supporting = attach(root.path(), "The input is always a whole frame.", Vec::new());
    attach(
        root.path(),
        "The length is the whole answer.",
        vec![Evidence::Record(supporting.parse().unwrap())],
    );

    let (intact, _) = codedoc_ops::evidence(root.path()).unwrap();
    assert_eq!(intact["broken"], 0, "{intact}");

    codedoc_ops::retract(root.path(), None, &supporting, Some("not true after all")).unwrap();

    let (report, _) = codedoc_ops::evidence(root.path()).unwrap();
    assert_eq!(
        report["broken"], 1,
        "a claim leaning on a record that was retracted has lost the support it cited: \
         {report}"
    );
    assert_eq!(report["records"][0]["standing"], "withdrawn", "{report}");
}

#[test]
fn a_url_is_recorded_but_never_fetched() {
    let root = project();
    attach(
        root.path(),
        "The length is the whole answer.",
        vec![Evidence::Url("https://example.invalid/never-requested".to_owned())],
    );

    let (report, code) = codedoc_ops::evidence(root.path()).unwrap();
    assert_eq!(
        report["broken"], 0,
        "checking a URL would mean codedoc making a network request from someone's \
         repository, which it must never do: {report}"
    );
    assert_eq!(code, 0);
}

#[test]
fn a_revision_that_is_no_longer_in_the_repository_is_reported() {
    let root = project();
    let git = |args: &[&str]| {
        Command::new("git").args(args).current_dir(root.path()).output().expect("git")
    };
    git(&["init", "-q"]);
    git(&["config", "user.email", "t@example.com"]);
    git(&["config", "user.name", "t"]);
    git(&["add", "-A"]);
    git(&["commit", "-qm", "first"]);

    attach(
        root.path(),
        "The length is the whole answer.",
        vec![Evidence::GitRevision("0123456789abcdef0123456789abcdef01234567".parse().unwrap())],
    );

    let (report, _) = codedoc_ops::evidence(root.path()).unwrap();
    assert_eq!(report["broken"], 1, "{report}");
    assert_eq!(report["records"][0]["standing"], "missing", "{report}");
}

#[test]
fn a_test_that_exists_but_is_not_committed_yet_still_counts_as_evidence() {
    let root = project();
    let git = |args: &[&str]| {
        Command::new("git").args(args).current_dir(root.path()).output().expect("git")
    };
    git(&["init", "-q"]);
    git(&["config", "user.email", "t@example.com"]);
    git(&["config", "user.name", "t"]);
    git(&["add", "-A"]);
    git(&["commit", "-qm", "first"]);

    fs::write(
        root.path().join("src/checks.rs"),
        "#[test]\nfn the_frame_length_is_checked() {\n    assert!(true);\n}\n",
    )
    .unwrap();

    attach(
        root.path(),
        "The length is the whole answer.",
        vec![Evidence::Test("the_frame_length_is_checked".to_owned())],
    );

    let (report, _) = codedoc_ops::evidence(root.path()).unwrap();
    assert_eq!(
        report["broken"], 0,
        "you write the test and the claim in the same change, so a search that only \
         looks at committed files calls your evidence missing the moment you cite it: \
         {report}"
    );
}
