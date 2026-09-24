use std::fs;

fn repository() -> tempfile::TempDir {
    let root = tempfile::tempdir().expect("a temporary directory");
    fs::create_dir_all(root.path().join("src")).unwrap();
    fs::create_dir_all(root.path().join(".git")).unwrap();
    fs::write(root.path().join("src/lib.rs"), "pub fn only() -> u32 {\n    1\n}\n").unwrap();
    root
}

fn claim(root: &std::path::Path) -> codedoc_ops::Outcome {
    let request = codedoc_ops::AttachRequest {
        target: codedoc_ops::Target::symbol("src/lib.rs", "rust://only"),
        kind: "invariant".to_owned(),
        claim: "This returns the same number every time.".to_owned(),
        detail: None,
    };
    codedoc_ops::attach(
        root,
        None,
        &request,
        &codedoc_ops::Attribution::agent("a-model", "a-session"),
        codedoc_ops::Provenance::default(),
    )
}

#[test]
fn an_agent_meeting_a_repository_with_no_ledger_is_told_how_to_make_one() {
    let root = repository();
    let refused = claim(root.path()).expect_err("there is no ledger yet");
    let explanation = refused.to_string();
    assert!(
        explanation.contains("codedoc_init"),
        "an agent reading this may have no shell, so the message has to name the tool \
         it can actually call: {explanation}"
    );
    assert!(explanation.contains("--scope local"), "{explanation}");
}

#[test]
fn an_agent_can_start_recording_without_leaving_a_trace_in_the_repository() {
    let root = repository();
    let created = codedoc_ops::initialise(root.path(), codedoc_ledger::Scope::Local).unwrap();
    assert_eq!(created["scope"], "local");
    assert_eq!(
        created["leaves_repository_evidence"], false,
        "an agent adopting codedoc on a repository it does not own must not add files \
         the repository will track: {created}"
    );

    let written = claim(root.path()).expect("the ledger exists now");
    assert_eq!(written["scope"], "local", "{written}");
    assert!(
        !root.path().join(".codedoc").exists(),
        "a local ledger lives inside .git/, which git cannot track"
    );
    assert!(root.path().join(".git/codedoc").is_dir());
}
