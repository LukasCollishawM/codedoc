use std::fs;

use codedoc_ledger::{Assurance, Ledger};

fn project(body: &str) -> tempfile::TempDir {
    let root = tempfile::tempdir().expect("a temporary directory");
    fs::create_dir_all(root.path().join("src")).unwrap();
    fs::write(root.path().join("src/lib.rs"), body).unwrap();
    Ledger::initialise(root.path()).unwrap();
    root
}

fn attach(root: &std::path::Path, claim: &str) -> String {
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
        &codedoc_ops::Attribution::agent("a-model", "a-session"),
        codedoc_ops::Provenance::default(),
    )
    .unwrap();
    written["record"].as_str().unwrap().to_owned()
}

fn stale_count(root: &std::path::Path) -> u64 {
    let (report, _) = codedoc_ops::verify(root).unwrap();
    report["counts"]["stale"].as_u64().unwrap()
}

#[test]
fn affirming_a_drifted_claim_clears_its_staleness_without_restating_it() {
    let root = project("pub fn compute(d: &[u8]) -> u32 {\n    d.len() as u32\n}\n");
    let id = attach(root.path(), "Callers must not pass a slice longer than one frame.");

    fs::write(
        root.path().join("src/lib.rs"),
        "pub fn compute(d: &[u8]) -> u32 {\n    let mut total = 0u32;\n    for byte in d {\n        total = total.wrapping_add(u32::from(*byte));\n    }\n    total\n}\n",
    )
    .unwrap();
    assert_eq!(stale_count(root.path()), 1, "rewriting the body drifts the claim");

    let affirmed = codedoc_ops::affirm(
        root.path(),
        None,
        &id,
        &codedoc_ops::Attribution::human("a reviewer"),
        None,
    )
    .unwrap();
    assert_eq!(affirmed["affirms"], id, "{affirmed}");
    assert!(
        affirmed["drift_cleared"].as_u64().unwrap() > 0,
        "the output must say how much drift the re-reading cleared: {affirmed}"
    );
    assert_eq!(
        stale_count(root.path()),
        0,
        "a claim re-read against the code as it now is must stop reporting stale, or \
         staleness becomes noise everyone learns to ignore"
    );
}

#[test]
fn an_affirmation_keeps_the_claim_and_records_who_checked_it() {
    let root = project("pub fn compute(d: &[u8]) -> u32 {\n    d.len() as u32\n}\n");
    let id = attach(root.path(), "Callers must not pass a slice longer than one frame.");

    codedoc_ops::affirm(
        root.path(),
        None,
        &id,
        &codedoc_ops::Attribution::human("a reviewer"),
        Some(Assurance::Asserted),
    )
    .unwrap();

    let history = codedoc_ops::history(root.path(), &id).unwrap();
    let chain = history["chain"].as_array().expect("a chain");
    assert_eq!(chain.len(), 2, "{history}");
    assert_eq!(chain[0]["affirmation"], false, "the original is not an affirmation");
    assert_eq!(
        chain[1]["affirmation"], true,
        "a revision that restates its parent word for word is a re-reading, not an edit: \
         {history}"
    );
    assert_eq!(chain[1]["claim"], chain[0]["claim"], "an affirmation does not reword the claim");

    let listed = codedoc_ops::list(root.path(), None, None, None).unwrap();
    assert_eq!(listed["count"], 1, "the superseded original leaves the active set: {listed}");
}

#[test]
fn a_detached_claim_cannot_be_affirmed() {
    let root = project("pub fn compute(d: &[u8]) -> u32 {\n    d.len() as u32\n}\n");
    let id = attach(root.path(), "Callers must not pass a slice longer than one frame.");

    fs::write(root.path().join("src/lib.rs"), "pub fn unrelated() -> u32 {\n    7\n}\n").unwrap();

    let refused = codedoc_ops::affirm(
        root.path(),
        None,
        &id,
        &codedoc_ops::Attribution::human("a reviewer"),
        None,
    );
    assert!(
        refused.is_err(),
        "affirming means 'I re-read this code and the claim still holds'. If the code \
         cannot be found, there was nothing to re-read."
    );
}

#[test]
fn attaching_a_claim_that_restates_an_existing_one_says_so() {
    let root = project("pub fn compute(d: &[u8]) -> u32 {\n    d.len() as u32\n}\n");
    attach(root.path(), "Callers must not pass a slice longer than one single frame.");

    let request = codedoc_ops::AttachRequest {
        target: codedoc_ops::Target::symbol("src/lib.rs", "rust://compute"),
        kind: "invariant".to_owned(),
        claim: "Callers must not pass a slice longer than one frame.".to_owned(),
        detail: None,
    };
    let written = codedoc_ops::attach(
        root.path(),
        None,
        &request,
        &codedoc_ops::Attribution::agent("a-model", "a-session"),
        codedoc_ops::Provenance::default(),
    )
    .unwrap();

    let similar = written["similar"].as_array().expect("a similar list");
    assert_eq!(
        similar.len(),
        1,
        "agents write continuously, so the moment to notice a restatement is when one \
         is written, not in a cleanup pass later: {written}"
    );
    assert!(similar[0]["similarity"].as_u64().unwrap() >= 60, "{written}");
    assert!(
        written["record"].as_str().is_some(),
        "the record is still written; a near-duplicate is a prompt to consider \
         superseding, not a refusal"
    );
}

#[test]
fn an_unrelated_claim_on_the_same_code_is_not_called_a_duplicate() {
    let root = project("pub fn compute(d: &[u8]) -> u32 {\n    d.len() as u32\n}\n");
    attach(root.path(), "Callers must not pass a slice longer than one frame.");

    let request = codedoc_ops::AttachRequest {
        target: codedoc_ops::Target::symbol("src/lib.rs", "rust://compute"),
        kind: "performance".to_owned(),
        claim: "This runs in constant time regardless of input size.".to_owned(),
        detail: None,
    };
    let written = codedoc_ops::attach(
        root.path(),
        None,
        &request,
        &codedoc_ops::Attribution::agent("a-model", "a-session"),
        codedoc_ops::Provenance::default(),
    )
    .unwrap();
    assert!(written["similar"].as_array().unwrap().is_empty(), "{written}");
}

#[test]
fn history_of_a_symbol_shows_what_was_believed_and_what_was_withdrawn() {
    let root = project("pub fn compute(d: &[u8]) -> u32 {\n    d.len() as u32\n}\n");
    let first = attach(root.path(), "Callers pass at most one frame.");
    let second = attach(root.path(), "The result is never zero for a non-empty slice.");

    codedoc_ops::supersede(
        root.path(),
        None,
        &first,
        Some("Callers pass exactly one frame, never a partial one."),
        None,
        None,
    )
    .unwrap();
    codedoc_ops::retract(root.path(), None, &second, Some("not true for an empty slice")).unwrap();

    let story = codedoc_ops::history(root.path(), "rust://compute").unwrap();
    assert_eq!(story["symbol"], "rust://compute", "{story}");
    let chain = story["chain"].as_array().expect("a chain");

    let standings: Vec<&str> =
        chain.iter().map(|entry| entry["standing"].as_str().unwrap_or("")).collect();
    assert!(
        standings.contains(&"withdrawn"),
        "a claim that was revised away is part of how the code came to be understood, \
         so it belongs in the story rather than vanishing from it: {story}"
    );
    assert!(standings.contains(&"believed"), "{story}");
    assert!(standings.contains(&"retraction"), "{story}");

    let created: Vec<&str> =
        chain.iter().map(|entry| entry["created"].as_str().unwrap_or("")).collect();
    let mut ordered = created.clone();
    ordered.sort_unstable();
    assert_eq!(created, ordered, "the story is told in the order it happened: {story}");
}

#[test]
fn asking_about_a_symbol_nobody_recorded_anything_about_says_so() {
    let root = project("pub fn compute(d: &[u8]) -> u32 {\n    d.len() as u32\n}\n");
    attach(root.path(), "Callers pass at most one frame.");

    let missing = codedoc_ops::history(root.path(), "rust://never_mentioned");
    assert!(
        missing.is_err(),
        "an empty chain and an unknown symbol are different answers, and returning \
         the first for the second reads as 'nothing was ever known' rather than \
         'you asked about something that is not there'"
    );
}

fn attach_to(root: &std::path::Path, symbol: &str, claim: &str) -> serde_json::Value {
    let request = codedoc_ops::AttachRequest {
        target: codedoc_ops::Target::symbol("src/lib.rs", symbol),
        kind: "explanation".to_owned(),
        claim: claim.to_owned(),
        detail: None,
    };
    codedoc_ops::attach(
        root,
        None,
        &request,
        &codedoc_ops::Attribution::agent("a-model", "a-session"),
        codedoc_ops::Provenance::default(),
    )
    .unwrap()
}

#[test]
fn a_claim_that_only_restates_the_name_is_flagged_as_saying_nothing_new() {
    let root = project("pub fn validate_token(token: &str) -> bool {\n    !token.is_empty()\n}\n");
    let written = attach_to(root.path(), "rust://validate_token", "Validates the token.");
    assert_eq!(
        written["restates_the_symbol"], true,
        "this project exists because comments restate the code. A record that says \
         only what the name says has the same problem and costs a reader the same \
         time: {written}"
    );
}

#[test]
fn a_claim_that_adds_something_the_name_does_not_say_is_not_flagged() {
    let root = project("pub fn validate_token(token: &str) -> bool {\n    !token.is_empty()\n}\n");
    for claim in [
        "Validation must precede tenant resolution, or a forged token selects a tenant.",
        "The token is checked against the cached key set, not the issuer, so rotation lags.",
        "Returns false for an empty token rather than erroring, which callers rely on.",
    ] {
        let written = attach_to(root.path(), "rust://validate_token", claim);
        assert_eq!(
            written["restates_the_symbol"], false,
            "flagging a claim that carries real information would train agents to \
             ignore the signal: {claim:?}"
        );
    }
}

#[test]
fn a_claim_on_a_file_is_never_called_a_restatement_of_a_name_it_has_no_symbol_for() {
    let root = project("pub fn compute() -> u32 {\n    1\n}\n");
    let request = codedoc_ops::AttachRequest {
        target: codedoc_ops::Target::file("src/lib.rs"),
        kind: "explanation".to_owned(),
        claim: "Compute is the only entry point here.".to_owned(),
        detail: None,
    };
    let written = codedoc_ops::attach(
        root.path(),
        None,
        &request,
        &codedoc_ops::Attribution::human("tester"),
        codedoc_ops::Provenance::default(),
    )
    .unwrap();
    assert_eq!(written["restates_the_symbol"], false, "{written}");
}
