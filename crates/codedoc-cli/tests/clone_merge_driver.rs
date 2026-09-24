use std::path::Path;
use std::process::{Command, Output};

use serde_json::Value;

fn codedoc(root: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_codedoc"))
        .arg("--root")
        .arg(root)
        .arg("--json")
        .args(args)
        .output()
        .expect("the binary runs")
}

fn git(root: &Path, args: &[&str]) -> Output {
    Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["-c", "user.email=test@example.invalid", "-c", "user.name=Test"])
        .args(args)
        .output()
        .expect("git runs")
}

fn payload(output: &Output) -> Value {
    serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|_| panic!("not JSON: {}", String::from_utf8_lossy(&output.stdout)))
}

fn origin(at: &Path) {
    std::fs::create_dir_all(at.join("src")).unwrap();
    std::fs::write(
        at.join("src/auth.rs"),
        "pub fn validate(token: &str) -> bool {\n    !token.is_empty()\n}\n",
    )
    .unwrap();
    assert!(git(at, &["init", "--quiet"]).status.success(), "git init");
    assert!(codedoc(at, &["init"]).status.success(), "codedoc init");
    assert!(
        codedoc(
            at,
            &[
                "attach",
                "src/auth.rs",
                "--symbol",
                "rust://validate",
                "--kind",
                "invariant",
                "--claim",
                "an empty token is never valid",
            ],
        )
        .status
        .success(),
        "codedoc attach"
    );
    assert!(
        codedoc(at, &["git", "install-merge-driver"]).status.success(),
        "codedoc git install-merge-driver"
    );
    assert!(
        git(at, &["add", "src", ".gitattributes", ".codedoc/ledger", ".codedoc/config.toml"])
            .status
            .success(),
        "git add"
    );
    assert!(git(at, &["commit", "--quiet", "-m", "seed"]).status.success(), "git commit");
}

fn flagged(root: &Path) -> bool {
    let report = payload(&codedoc(root, &["doctor"]));
    report["advisory"]["merge_driver_unregistered"].as_bool().unwrap_or_else(|| {
        panic!("doctor did not report whether the driver is registered: {report}")
    })
}

#[test]
fn a_clone_is_told_that_the_merge_driver_it_is_asked_for_is_not_registered() {
    let workspace = tempfile::tempdir().unwrap();
    let source = workspace.path().join("origin");
    std::fs::create_dir_all(&source).unwrap();
    origin(&source);

    assert!(!flagged(&source), "the repository that installed the driver has it registered");

    let clone = workspace.path().join("joiner");
    assert!(
        Command::new("git")
            .args(["clone", "--quiet"])
            .arg(&source)
            .arg(&clone)
            .output()
            .expect("git runs")
            .status
            .success(),
        "git clone"
    );

    assert!(
        clone.join(".gitattributes").exists(),
        ".gitattributes is committed, so the clone asks git for the driver"
    );
    assert!(
        codedoc(&clone, &["list"]).status.success(),
        "the clone can read the ledger it was handed, with no setup"
    );

    assert!(
        flagged(&clone),
        "registration lives in .git/config and a clone does not copy it, so every \
         teammate joining a project has a committed .gitattributes asking for a driver \
         git has never been given. The next merge of the ledger then writes conflict \
         markers into an append-only log and nothing has told them why."
    );

    let report = payload(&codedoc(&clone, &["doctor"]));
    let next = report["next"].to_string();
    assert!(
        next.contains("codedoc git install-merge-driver"),
        "doctor has to carry the remedy rather than only the finding: {next}"
    );

    assert!(codedoc(&clone, &["git", "install-merge-driver"]).status.success(), "install");
    assert!(!flagged(&clone), "installing it in the clone clears the finding");
}

#[test]
fn a_repository_that_never_asked_for_the_driver_is_not_nagged_about_it() {
    let workspace = tempfile::tempdir().unwrap();
    let at = workspace.path();
    std::fs::create_dir_all(at.join("src")).unwrap();
    std::fs::write(at.join("src/auth.rs"), "pub fn validate() -> bool {\n    true\n}\n").unwrap();
    assert!(git(at, &["init", "--quiet"]).status.success(), "git init");
    assert!(codedoc(at, &["init"]).status.success(), "codedoc init");

    assert!(
        !flagged(at),
        "partial adoption is the normal state and most repositories will never install \
         the driver. The finding is a mismatch between what .gitattributes asks for and \
         what git has, not an opinion about whether to use it."
    );
}
