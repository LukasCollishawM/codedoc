use std::fs;
use std::path::Path;

use codedoc_ledger::Ledger;

#[path = "../src/locate.rs"]
#[allow(dead_code)]
mod locate;

fn project(body: &str) -> tempfile::TempDir {
    let root = tempfile::tempdir().expect("a temporary directory");
    fs::create_dir_all(root.path().join("src")).unwrap();
    fs::write(root.path().join("src/lib.rs"), body).unwrap();
    Ledger::initialise(root.path()).unwrap();
    root
}

fn attach(root: &Path, claim: &str) {
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
        codedoc_ops::Provenance::default(),
    )
    .unwrap();
}

#[test]
fn a_stale_claim_in_an_editor_names_what_to_do_about_it() {
    let root = project("pub fn compute(d: &[u8]) -> u32 {\n    d.len() as u32\n}\n");
    attach(root.path(), "Callers pass exactly one frame.");

    fs::write(
        root.path().join("src/lib.rs"),
        "pub fn compute(d: &[u8]) -> u32 {\n    let mut total = 0u32;\n    for byte in d {\n        total = total.wrapping_add(u32::from(*byte));\n    }\n    total\n}\n",
    )
    .unwrap();

    let concerns = locate::concerns_for(root.path(), Path::new("src/lib.rs"));
    let stale: Vec<&locate::Concern> =
        concerns.iter().filter(|concern| concern.severity == locate::Severity::Stale).collect();
    assert_eq!(stale.len(), 1, "the rewritten body should drift the claim");
    assert!(
        stale[0].message.contains("codedoc affirm"),
        "an editor warning that a claim may no longer describe the code, without \
         saying what to do once you have read it, leaves the reading unrecorded and \
         the warning permanent: {}",
        stale[0].message
    );
    assert!(stale[0].message.contains("codedoc supersede"), "{}", stale[0].message);
}

#[test]
fn a_detached_claim_still_points_at_resolve() {
    let root = project("pub fn compute(d: &[u8]) -> u32 {\n    d.len() as u32\n}\n");
    attach(root.path(), "Callers pass exactly one frame.");
    fs::write(root.path().join("src/lib.rs"), "pub fn unrelated() -> u32 {\n    7\n}\n").unwrap();

    let concerns = locate::concerns_for(root.path(), Path::new("src/lib.rs"));
    let detached: Vec<&locate::Concern> =
        concerns.iter().filter(|concern| concern.severity == locate::Severity::Detached).collect();
    assert_eq!(detached.len(), 1);
    assert!(detached[0].message.contains("codedoc resolve"), "{}", detached[0].message);
}
