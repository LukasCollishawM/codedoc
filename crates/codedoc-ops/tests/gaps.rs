use std::fs;
use std::path::Path;
use std::process::Command;

use codedoc_ledger::Ledger;

fn project() -> tempfile::TempDir {
    let root = tempfile::tempdir().expect("a temporary directory");
    fs::create_dir_all(root.path().join("src")).unwrap();
    Ledger::initialise(root.path()).unwrap();

    write(root.path(), "steady", "0");
    git(root.path(), &["init", "-q"]);
    git(root.path(), &["config", "user.email", "t@example.com"]);
    git(root.path(), &["config", "user.name", "tester"]);
    git(root.path(), &["add", "src/lib.rs"]);
    git(root.path(), &["commit", "-qm", "Add the module"]);

    write(root.path(), "corrected", "1");
    git(root.path(), &["commit", "-qam", "Fix the rounding in corrected"]);
    write(root.path(), "corrected", "2");
    git(root.path(), &["commit", "-qam", "Fix corrected again, properly this time"]);
    write(root.path(), "churned", "1");
    git(root.path(), &["commit", "-qam", "Teach churned about prefixes"]);
    write(root.path(), "churned", "2");
    git(root.path(), &["commit", "-qam", "Extend churned to fixtures"]);

    root
}

fn git(root: &Path, args: &[&str]) {
    let done = Command::new("git").args(args).current_dir(root).output().expect("git");
    assert!(done.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&done.stderr));
}

fn write(root: &Path, changing: &str, body: &str) {
    let mut source = String::new();
    for name in ["steady", "corrected", "churned"] {
        let value = if name == changing { body } else { "0" };
        source.push_str(&format!("pub fn {name}() -> u32 {{\n {value}\n}}\n\n"));
    }
    fs::write(root.join("src/lib.rs"), source).unwrap();
}

fn symbols(report: &serde_json::Value) -> Vec<String> {
    report["gaps"]
        .as_array()
        .expect("a list of gaps")
        .iter()
        .map(|gap| gap["symbol"].as_str().unwrap_or_default().to_owned())
        .collect()
}

#[test]
fn the_declaration_that_was_corrected_outranks_the_one_that_merely_changed() {
    let root = project();
    let report = codedoc_ops::gaps(root.path(), &[], 10, 100).unwrap();

    assert_eq!(report["git"], true, "{report}");
    let found = symbols(&report);
    assert_eq!(
        found.first().map(String::as_str),
        Some("rust://corrected"),
        "corrections rank above bare churn: {report}"
    );
    assert!(
        found.contains(&"rust://churned".to_owned()),
        "a declaration touched more than once is still worth reporting: {report}"
    );
}

#[test]
fn a_declaration_nobody_went_back_to_is_not_a_gap() {
    let root = project();
    let report = codedoc_ops::gaps(root.path(), &[], 10, 100).unwrap();

    assert!(
        !symbols(&report).contains(&"rust://steady".to_owned()),
        "one commit and no corrections is not evidence that knowledge is missing: {report}"
    );
}

#[test]
fn fixture_is_not_fix() {
    let root = project();
    let report = codedoc_ops::gaps(root.path(), &[], 10, 100).unwrap();

    let churned = report["gaps"]
        .as_array()
        .unwrap()
        .iter()
        .find(|gap| gap["symbol"] == "rust://churned")
        .expect("churned is reported")
        .clone();
    assert_eq!(
        churned["corrections"], 0,
        "corrective words are matched whole, so 'prefixes' and 'fixtures' are not \
         corrections: {report}"
    );
}

#[test]
fn a_declaration_that_already_carries_a_record_is_not_a_gap() {
    let root = project();

    let request = codedoc_ops::AttachRequest {
        target: codedoc_ops::Target::symbol("src/lib.rs", "rust://corrected"),
        kind: "known_failure_mode".to_owned(),
        claim: "The rounding here is toward zero, which is what the two fixes settled on."
            .to_owned(),
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

    let report = codedoc_ops::gaps(root.path(), &[], 10, 100).unwrap();
    assert!(
        !symbols(&report).contains(&"rust://corrected".to_owned()),
        "once the knowledge is written down the declaration stops being a gap, which \
         is the only way this list ever gets shorter: {report}"
    );
}

#[test]
fn without_a_repository_it_reports_nothing_rather_than_guessing() {
    let root = tempfile::tempdir().expect("a temporary directory");
    fs::create_dir_all(root.path().join("src")).unwrap();
    fs::write(root.path().join("src/lib.rs"), "pub fn compute() -> u32 {\n 0\n}\n").unwrap();
    Ledger::initialise(root.path()).unwrap();

    let report = codedoc_ops::gaps(root.path(), &[], 10, 100).unwrap();
    assert_eq!(report["git"], false, "{report}");
    assert_eq!(report["gaps"].as_array().map(Vec::len), Some(0), "{report}");
}

#[test]
fn the_commit_that_created_the_files_is_not_a_correction() {
    let root = project();
    let report = codedoc_ops::gaps(root.path(), &[], 20, 400).expect("gaps runs");

    assert_eq!(
        report["commits_scanned"].as_u64(),
        Some(4),
        "five commits were made and the first one has no parent, so it added the module rather than correcting it. Counting it makes every file in the repository look corrected once by whatever that commit happened to say: {report}"
    );
    assert_eq!(
        report["commits_requested"].as_u64(),
        Some(400),
        "the window asked for and the history actually walked are different numbers, and reporting the first as though it were the second tells a reader the ranking rests on four hundred commits when it rests on four: {report}"
    );
    assert_eq!(report["history_shallow"].as_bool(), Some(false), "{report}");
    assert_eq!(report["history_capped"].as_bool(), Some(false), "{report}");

    let steady = report["gaps"]
        .as_array()
        .expect("gaps is a list")
        .iter()
        .find(|gap| gap["symbol"].as_str() == Some("rust://steady"));
    assert!(
        steady.is_none(),
        "steady was written once and never touched again. It only looks revisited if the commit that created it counts as having corrected it: {report}"
    );
}

#[test]
fn a_shallow_clone_says_so_rather_than_ranking_on_a_history_it_does_not_have() {
    let origin = project();
    let elsewhere = tempfile::tempdir().expect("a temporary directory");
    let clone = elsewhere.path().join("shallow");
    let done = Command::new("git")
        .args(["clone", "-q", "--no-local", "--depth", "1"])
        .arg(origin.path())
        .arg(&clone)
        .output()
        .expect("git");
    assert!(done.status.success(), "git clone: {}", String::from_utf8_lossy(&done.stderr));
    Ledger::initialise(&clone).unwrap();

    let report = codedoc_ops::gaps(&clone, &[], 20, 400).expect("gaps runs");

    assert_eq!(
        report["history_shallow"].as_bool(),
        Some(true),
        "a depth-1 checkout is what actions/checkout does by default: {report}"
    );
    assert_eq!(
        report["gaps"].as_array().map(Vec::len),
        Some(0),
        "the only commit a depth-1 clone has is its grafted boundary, which git reports as touching every file in the tree. Ranking on it names every declaration in the repository and attributes that one commit message to all of them, which reads as evidence and is noise: {report}"
    );
}
