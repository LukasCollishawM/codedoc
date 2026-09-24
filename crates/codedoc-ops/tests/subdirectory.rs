use std::fs;
use std::path::Path;

use codedoc_ledger::Ledger;

fn project() -> tempfile::TempDir {
    let root = tempfile::tempdir().expect("a temporary directory");
    fs::create_dir_all(root.path().join("src/db/models")).unwrap();
    fs::write(
        root.path().join("src/db/models/query.rs"),
        "pub fn filter(rows: u32) -> u32 {\n    rows\n}\n",
    )
    .unwrap();
    Ledger::initialise(root.path()).unwrap();
    root
}

fn attach_from(invoked_from: &Path, file: &str) -> codedoc_ops::Outcome {
    let request = codedoc_ops::AttachRequest {
        target: codedoc_ops::Target::symbol(file, "rust://filter"),
        kind: "invariant".to_owned(),
        claim: "Filtering never returns more rows than it was given.".to_owned(),
        detail: None,
    };
    codedoc_ops::attach(
        invoked_from,
        None,
        &request,
        &codedoc_ops::Attribution::human("tester"),
        codedoc_ops::Provenance::default(),
    )
}

#[test]
fn a_ledger_above_the_working_directory_is_found() {
    let root = project();
    let deep = root.path().join("src/db/models");

    let (report, _) = codedoc_ops::verify(&deep).expect(
        "git, cargo and npm all walk up to the root, and a tool that refuses sends the \
         user to create a second ledger in a subdirectory",
    );
    assert_eq!(report["counts"]["detached"], 0, "{report}");
}

#[test]
fn a_path_typed_from_a_subdirectory_resolves() {
    let root = project();
    let deep = root.path().join("src/db/models");
    let previous = std::env::current_dir().ok();
    std::env::set_current_dir(&deep).expect("move into the subdirectory");

    let written = attach_from(&deep, "query.rs");

    if let Some(back) = previous {
        let _ = std::env::set_current_dir(back);
    }
    let written = written.expect("the file is there, spelled the way the shell spells it");
    assert_eq!(
        written["file"], "src/db/models/query.rs",
        "a claim filed under the caller's working directory is a claim nobody else \
         can resolve: {written}"
    );
}

#[test]
fn a_repository_relative_path_still_works_from_anywhere() {
    let root = project();
    let deep = root.path().join("src/db/models");

    let written = attach_from(&deep, "src/db/models/query.rs")
        .expect("the repository-relative spelling is still accepted");
    assert_eq!(written["file"], "src/db/models/query.rs", "{written}");
}

#[test]
fn a_path_argument_typed_from_a_subdirectory_is_scanned() {
    let root = project();
    let deep = root.path().join("src/db");
    let previous = std::env::current_dir().ok();
    std::env::set_current_dir(&deep).expect("move into the subdirectory");

    let scanned = codedoc_ops::coverage(&deep, &["models".to_owned()], 5);

    if let Some(back) = previous {
        let _ = std::env::set_current_dir(back);
    }
    let scanned = scanned.expect("coverage runs");
    assert!(
        scanned["declarations"].as_u64().unwrap_or(0) > 0,
        "answering 'no declarations found' about a directory that is full of them \
         reads as a finding rather than a miss: {scanned}"
    );
}

#[test]
fn a_path_that_is_not_there_is_named_rather_than_answered_with_zero() {
    let root = project();

    let refused = codedoc_ops::coverage(root.path(), &["src/does-not-exist".to_owned()], 5)
        .expect_err("a path that is not in the repository is not a finding of zero");
    assert!(refused.to_string().contains("src/does-not-exist"), "{refused}");

    let imported = codedoc_ops::import(root.path(), None, &["nope".to_owned()], false, None)
        .expect_err("import too");
    assert!(imported.to_string().contains("nope"), "{imported}");

    let searched =
        codedoc_ops::gaps(root.path(), &["nope".to_owned()], 5, 50).expect_err("and gaps");
    assert!(searched.to_string().contains("nope"), "{searched}");
}
