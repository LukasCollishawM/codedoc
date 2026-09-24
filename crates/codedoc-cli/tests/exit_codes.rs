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

fn ledger_lines(root: &Path) -> Vec<String> {
    let mut lines = Vec::new();
    let directory = root.join(".codedoc").join("ledger");
    for entry in std::fs::read_dir(directory).expect("a ledger directory").flatten() {
        let body = std::fs::read_to_string(entry.path()).expect("a shard");
        lines.extend(body.lines().filter(|line| !line.is_empty()).map(str::to_owned));
    }
    lines
}

fn merge(root: &Path, base: &str, ours: &str, theirs: &str) -> (Output, String) {
    let scratch = root.join("merge");
    std::fs::create_dir_all(&scratch).unwrap();
    let paths: Vec<_> = [("base", base), ("ours", ours), ("theirs", theirs)]
        .iter()
        .map(|(name, body)| {
            let path = scratch.join(format!("{name}.jsonl"));
            std::fs::write(&path, body).unwrap();
            path
        })
        .collect();
    let output = Command::new(env!("CARGO_BIN_EXE_codedoc"))
        .arg("git")
        .arg("merge-driver")
        .args(&paths)
        .output()
        .expect("the driver runs");
    let merged = std::fs::read_to_string(&paths[1]).expect("ours is readable");
    (output, merged)
}

#[test]
fn the_merge_driver_unions_two_branches_that_both_recorded_something() {
    let root = project();
    assert!(run(root.path(), &["init"]).status.success());
    for (symbol, claim) in
        [("rust://validate", "Validation precedes tenancy."), ("rust://validate", "Tokens expire.")]
    {
        let written = run(
            root.path(),
            &["attach", "src/auth.rs", "--symbol", symbol, "--kind", "invariant", "--claim", claim],
        );
        assert!(written.status.success());
    }
    let lines = ledger_lines(root.path());
    assert_eq!(lines.len(), 2);

    let (output, merged) =
        merge(root.path(), "", &format!("{}\n", lines[0]), &format!("{}\n", lines[1]));
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    assert_eq!(
        merged.lines().filter(|line| !line.is_empty()).count(),
        2,
        "two branches that each recorded something must end up with both, or a team \
         loses knowledge every time it merges"
    );
}

#[test]
fn the_merge_driver_refuses_rather_than_writing_a_ledger_that_would_not_verify() {
    let root = project();
    assert!(run(root.path(), &["init"]).status.success());
    let written = run(
        root.path(),
        &[
            "attach",
            "src/auth.rs",
            "--symbol",
            "rust://validate",
            "--kind",
            "invariant",
            "--claim",
            "Validation precedes tenancy.",
        ],
    );
    assert!(written.status.success());
    let ours = format!("{}\n", ledger_lines(root.path())[0]);

    let (output, merged) = merge(root.path(), "", &ours, "{\"not\":\"a record\"}\n");
    assert_eq!(output.status.code(), Some(4), "a refusal is a failure, not a quiet success");
    assert_eq!(
        merged, ours,
        "refusing has to mean leaving what was there. Half-writing a ledger during a \
         merge is how a repository ends up with records nothing can read."
    );
}

#[test]
fn history_marks_the_revision_that_is_believed_rather_than_guessing_from_order() {
    let root = project();
    assert!(run(root.path(), &["init"]).status.success());
    let written = run(
        root.path(),
        &[
            "attach",
            "src/auth.rs",
            "--symbol",
            "rust://validate",
            "--kind",
            "invariant",
            "--claim",
            "The original wording of this claim.",
        ],
    );
    let id = payload(&written)["record"].as_str().expect("an id").to_owned();
    assert!(
        run(root.path(), &["supersede", &id, "--claim", "The revised wording of this claim."])
            .status
            .success()
    );

    let listed = payload(&run(root.path(), &["history", &id]));
    let chain = listed["chain"].as_array().expect("a chain");
    assert_eq!(chain.len(), 2, "{listed}");

    let believed: Vec<&str> = chain
        .iter()
        .filter(|entry| entry["standing"] == "believed")
        .filter_map(|entry| entry["claim"].as_str())
        .collect();
    assert_eq!(
        believed,
        vec!["The revised wording of this claim."],
        "exactly one revision is believed, and which one cannot be inferred from \
         position: two records written in the same second tie on their timestamp, \
         and the reader would be told the claim was revised into the wording it \
         started with: {listed}"
    );
}
