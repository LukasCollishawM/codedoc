use std::fs;
use std::path::Path;

use codedoc_ledger::Ledger;

fn project() -> tempfile::TempDir {
    let root = tempfile::tempdir().expect("a temporary directory");
    fs::create_dir_all(root.path().join("src")).unwrap();
    fs::write(root.path().join("src/lib.rs"), "pub fn compute() -> u32 {\n    0\n}\n").unwrap();
    fs::write(root.path().join("Dockerfile"), "FROM debian:bookworm-slim\n\nRUN apt-get update\n")
        .unwrap();
    fs::write(root.path().join("notes.md"), "# Notes\n\nNothing here yet.\n").unwrap();
    Ledger::initialise(root.path()).unwrap();
    root
}

fn attach(
    root: &Path,
    file: &str,
    symbol: Option<&str>,
    line: Option<u32>,
) -> codedoc_ops::Outcome {
    let request = codedoc_ops::AttachRequest {
        target: codedoc_ops::Target {
            file: file.to_owned(),
            symbol: symbol.map(str::to_owned),
            line,
        },
        kind: "decision".to_owned(),
        claim: "The base image is pinned to bookworm because trixie moved the CA bundle."
            .to_owned(),
        detail: None,
    };
    codedoc_ops::attach(
        root,
        None,
        &request,
        &codedoc_ops::Attribution::human("tester"),
        codedoc_ops::Provenance::default(),
    )
}

#[test]
fn a_file_no_parser_understands_can_still_carry_a_claim() {
    let root = project();
    let written = attach(root.path(), "Dockerfile", None, None).expect("the claim is recorded");
    assert_eq!(written["file"], "Dockerfile", "{written}");
    assert_eq!(
        written["subject"], "file",
        "a file with no adapter has no construct to be about: {written}"
    );
    assert_eq!(written["opaque"], true, "{written}");
    assert_eq!(written["symbol"], serde_json::Value::Null, "{written}");
}

#[test]
fn naming_a_construct_inside_one_is_refused_with_the_reason() {
    let root = project();
    let refused = attach(root.path(), "Dockerfile", Some("rust://FROM"), None).unwrap_err();
    let message = refused.to_string();
    assert!(message.contains("no language adapter"), "{message}");
    assert!(
        message.contains("--symbol"),
        "the error must say what to do instead, not only what went wrong: {message}"
    );

    let by_line = attach(root.path(), "Dockerfile", None, Some(1)).unwrap_err();
    assert!(by_line.to_string().contains("no language adapter"), "{by_line}");
}

#[test]
fn the_claim_resolves_against_the_file_that_carries_it() {
    let root = project();
    attach(root.path(), "Dockerfile", None, None).unwrap();

    let (report, code) = codedoc_ops::verify(root.path()).unwrap();
    assert_eq!(report["counts"]["detached"], 0, "{report}");
    assert_eq!(code, 0, "{report}");
    let finding = &report["findings"][0];
    assert_eq!(finding["resolution"]["rung"], "file_identity", "{report}");
    assert_eq!(finding["resolution"]["confidence"], "exact", "{report}");
}

#[test]
fn editing_the_file_does_not_detach_the_claim_because_identity_is_the_path() {
    let root = project();
    attach(root.path(), "Dockerfile", None, None).unwrap();

    fs::write(
        root.path().join("Dockerfile"),
        "FROM debian:bookworm-slim\n\nRUN apt-get update && apt-get install -y ca-certificates\n\nCOPY . /app\n",
    )
    .unwrap();

    let (report, _) = codedoc_ops::verify(root.path()).unwrap();
    assert_eq!(
        report["counts"]["detached"], 0,
        "a file anchor resolves by path and must survive the file changing: {report}"
    );
}

#[test]
fn deleting_the_file_detaches_the_claim() {
    let root = project();
    attach(root.path(), "notes.md", None, None).unwrap();
    fs::remove_file(root.path().join("notes.md")).unwrap();

    let (report, _) = codedoc_ops::verify(root.path()).unwrap();
    assert_eq!(
        report["counts"]["detached"], 1,
        "the file is the identity, so its absence is the detachment: {report}"
    );
}

#[test]
fn a_parseable_file_still_gets_a_parsed_file_anchor() {
    let root = project();
    let written = attach(root.path(), "src/lib.rs", None, None).unwrap();
    assert_eq!(written["subject"], "file", "{written}");
    assert_eq!(
        written["opaque"], false,
        "an adapter exists for this one, so it must not be treated as opaque: {written}"
    );
}

#[test]
fn an_opaque_anchor_reports_no_drift_rather_than_a_number_it_cannot_measure() {
    let root = project();
    attach(root.path(), "Dockerfile", None, None).unwrap();
    fs::write(root.path().join("Dockerfile"), "FROM scratch\n").unwrap();

    let (report, _) = codedoc_ops::verify(root.path()).unwrap();
    assert_eq!(report["counts"]["stale"], 0, "{report}");
    let findings = report["findings"].as_array().expect("a finding listing");
    let entry = findings
        .iter()
        .find(|entry| entry["file"] == "Dockerfile")
        .expect("the Dockerfile claim is reported");
    assert!(
        entry.get("drift").is_none() || entry["drift"].is_null(),
        "there is no shape to compare, so the honest answer is no answer: {entry}"
    );
}

#[test]
fn a_claim_on_an_opaque_file_can_be_superseded_affirmed_and_retracted() {
    let root = project();
    let written = attach(root.path(), "Dockerfile", None, None).unwrap();
    let record = written["record"].as_str().expect("a record id").to_owned();

    let revised = codedoc_ops::supersede(
        root.path(),
        None,
        &record,
        Some("The base image is pinned because the newer one moves the CA bundle."),
        None,
        None,
    )
    .expect("an opaque anchor can be re-captured, so it can be superseded");
    let revised_id = revised["record"].as_str().expect("a new record id").to_owned();

    codedoc_ops::affirm(
        root.path(),
        None,
        &revised_id,
        &codedoc_ops::Attribution::human("t"),
        None,
    )
    .expect("and affirmed");

    codedoc_ops::retract(root.path(), None, &revised_id, Some("no longer pinned"))
        .expect("and retracted");
}
