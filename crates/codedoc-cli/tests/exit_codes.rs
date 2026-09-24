use std::path::Path;
use std::process::{Command, Output};

use serde_json::Value;

fn run(root: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_codedoc"))
        .arg("--root")
        .arg(root)
        .arg("--json")
        .args(args)
        .output()
        .expect("the binary runs")
}

fn payload(output: &Output) -> Value {
    serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|_| panic!("not JSON: {}", String::from_utf8_lossy(&output.stdout)))
}

fn project() -> tempfile::TempDir {
    let root = tempfile::tempdir().expect("a temporary directory");
    std::fs::create_dir_all(root.path().join("src")).unwrap();
    std::fs::write(
        root.path().join("src/auth.rs"),
        "pub fn validate(token: &str) -> bool {\n    !token.is_empty()\n}\n",
    )
    .unwrap();
    root
}

#[test]
fn the_documented_exit_codes_are_what_the_binary_returns() {
    let root = project();

    let missing = run(root.path(), &["verify"]);
    assert_eq!(
        missing.status.code(),
        Some(4),
        "docs/cli.md says 4 means the command itself failed, and a CI job branches \
         on these numbers"
    );

    assert!(run(root.path(), &["init"]).status.success());
    let clean = run(root.path(), &["verify"]);
    assert_eq!(clean.status.code(), Some(0), "nothing recorded, nothing wrong");

    let attached = run(
        root.path(),
        &[
            "attach",
            "src/auth.rs",
            "--symbol",
            "rust://validate",
            "--kind",
            "invariant",
            "--claim",
            "Validation must precede tenant resolution.",
        ],
    );
    assert!(attached.status.success(), "{:?}", String::from_utf8_lossy(&attached.stderr));
    assert_eq!(payload(&attached)["command"], "attach");

    std::fs::write(root.path().join("src/auth.rs"), "pub fn unrelated() -> u32 {\n    7\n}\n")
        .unwrap();
    let detached = run(root.path(), &["verify"]);
    assert_eq!(
        detached.status.code(),
        Some(2),
        "docs/cli.md says 2 means something needs a decision, which is what a \
         detached anchor is"
    );
    assert_eq!(payload(&detached)["counts"]["detached"], 1);
}

#[test]
fn an_unknown_kind_is_refused_with_the_vocabulary_rather_than_a_panic() {
    let root = project();
    assert!(run(root.path(), &["init"]).status.success());

    let refused = run(
        root.path(),
        &[
            "attach",
            "src/auth.rs",
            "--symbol",
            "rust://validate",
            "--kind",
            "vibes",
            "--claim",
            "This feels about right.",
        ],
    );
    assert_eq!(refused.status.code(), Some(4));
    let rendered = payload(&refused)["error"].as_str().unwrap_or_default().to_owned();
    assert!(
        rendered.contains("invariant"),
        "telling someone a kind is unknown without telling them the vocabulary makes \
         them go and find it: {rendered}"
    );
}

#[test]
fn json_output_is_json_on_every_path_including_failure() {
    let root = project();
    for args in [vec!["stats"], vec!["list"], vec!["doctor"], vec!["kinds"]] {
        let output = run(root.path(), &args);
        let rendered = String::from_utf8_lossy(&output.stdout);
        assert!(
            serde_json::from_str::<Value>(&rendered).is_ok(),
            "--json has to mean JSON even when the command fails, because whatever \
             is reading it cannot switch parsers halfway: `{}` printed {rendered}",
            args.join(" ")
        );
    }
}
