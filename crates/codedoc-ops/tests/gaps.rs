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
        source.push_str(&format!("pub fn {name}() -> u32 {{\n    {value}\n}}\n\n"));
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
    fs::write(root.path().join("src/lib.rs"), "pub fn compute() -> u32 {\n    0\n}\n").unwrap();
    Ledger::initialise(root.path()).unwrap();

    let report = codedoc_ops::gaps(root.path(), &[], 10, 100).unwrap();
    assert_eq!(report["git"], false, "{report}");
    assert_eq!(report["gaps"].as_array().map(Vec::len), Some(0), "{report}");
}
