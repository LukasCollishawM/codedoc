use std::fs;
use std::path::Path;

use codedoc_ledger::{Ledger, Workspace};

fn project(files: &[(&str, &str)]) -> tempfile::TempDir {
    let workspace = tempfile::tempdir().expect("a temporary directory");
    for (name, body) in files {
        let path = workspace.path().join(name);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, body).unwrap();
    }
    Ledger::initialise(workspace.path()).unwrap();
    workspace
}

fn symbols_after_import(root: &Path) -> Vec<Option<String>> {
    codedoc_ops::import(root, None, &["src".to_owned()], true, None).unwrap();
    let found = Workspace::at(root);
    let mut symbols: Vec<Option<String>> = found
        .records()
        .unwrap()
        .iter()
        .map(|record| {
            record.subject().and_then(|anchor| anchor.symbol.as_ref().map(ToString::to_string))
        })
        .collect();
    symbols.sort();
    symbols
}

#[test]
fn a_comment_above_attributes_anchors_to_the_declaration() {
    let workspace = project(&[(
        "src/lib.rs",
        "/// Calculates a checksum.\n#[cfg(feature = \"std\")]\n#[inline]\npub fn compute(d: &[u8]) -> u32 {\n    d.len() as u32\n}\n",
    )]);
    assert_eq!(
        symbols_after_import(workspace.path()),
        vec![Some("rust://compute".to_owned())],
        "an attribute is a sibling of the item it decorates, so taking the first node \
         after the comment anchors to text that repeats across the file and carries no \
         symbol, which can never resolve"
    );
}

#[test]
fn a_comment_above_a_python_decorator_anchors_to_the_function() {
    let workspace = project(&[(
        "src/app.py",
        "# Handles the request.\n@route(\"/\")\n@cached\ndef handle(request):\n    return request\n",
    )]);
    assert_eq!(symbols_after_import(workspace.path()), vec![Some("python://handle".to_owned())]);
}

#[test]
fn a_comment_above_a_plain_declaration_still_anchors_to_it() {
    let workspace = project(&[(
        "src/lib.rs",
        "/// Does the thing.\npub fn plain(d: &[u8]) -> u32 {\n    d.len() as u32\n}\n",
    )]);
    assert_eq!(symbols_after_import(workspace.path()), vec![Some("rust://plain".to_owned())]);
}

#[test]
fn a_comment_separated_by_blank_lines_is_not_attached_to_what_follows() {
    let workspace = project(&[(
        "src/lib.rs",
        "/// Floating remark about the module.\n\n\n\npub fn distant(d: &[u8]) -> u32 {\n    d.len() as u32\n}\n",
    )]);
    let symbols = symbols_after_import(workspace.path());
    assert!(
        symbols.iter().all(|symbol| symbol.as_deref() != Some("rust://distant")),
        "a comment four lines above a function is not documenting it, and guessing that \
         it is puts words in someone's mouth"
    );
}

#[test]
fn a_module_doc_comment_anchors_to_the_file_rather_than_the_import_below_it() {
    let workspace = project(&[(
        "src/lib.rs",
        "//! Parses the wire format.\n//! Every field is little-endian.\n\n#![no_std]\n\nuse core::fmt;\n\npub fn parse(d: &[u8]) -> u32 {\n    d.len() as u32\n}\n",
    )]);
    codedoc_ops::import(workspace.path(), None, &["src".to_owned()], true, None).unwrap();
    let found = Workspace::at(workspace.path());
    let records = found.records().unwrap();
    let module = records
        .iter()
        .find(|record| record.content().body.claim.starts_with("Parses the wire format"))
        .expect("the module doc was imported");
    let anchor = module.subject().expect("a subject anchor");
    assert_eq!(
        anchor.subject,
        codedoc_anchor::Subject::File,
        "a //! comment documents the module, not whatever token happens to follow it"
    );
    assert!(anchor.symbol.is_none());
    assert!(
        anchor.node_path.is_empty(),
        "a claim about the file has no path within it to descend, and a path that pointed somewhere would make the anchor resolvable to a construct"
    );
    assert_eq!(anchor.range.start_line, 1, "its range spans the file");
}

#[test]
fn a_file_anchor_resolves_while_the_file_exists_and_detaches_when_it_does_not() {
    let workspace = project(&[("src/lib.rs", "//! Parses the wire format.\n\nuse core::fmt;\n")]);
    codedoc_ops::import(workspace.path(), None, &["src".to_owned()], true, None).unwrap();

    let (verified, _) = codedoc_ops::verify(workspace.path()).unwrap();
    assert_eq!(verified["counts"]["detached"], 0, "the file is still there: {verified}");

    fs::write(workspace.path().join("src/lib.rs"), "use core::fmt;\nuse core::mem;\n").unwrap();
    let (rewritten, _) = codedoc_ops::verify(workspace.path()).unwrap();
    assert_eq!(
        rewritten["counts"]["detached"], 0,
        "rewriting a file does not delete it: {rewritten}"
    );

    fs::remove_file(workspace.path().join("src/lib.rs")).unwrap();
    let (deleted, _) = codedoc_ops::verify(workspace.path()).unwrap();
    assert_eq!(deleted["counts"]["detached"], 1, "the file it described is gone: {deleted}");
}

#[test]
fn attaching_to_a_file_without_a_symbol_or_line_claims_the_file_itself() {
    let workspace =
        project(&[("src/lib.rs", "pub fn compute(d: &[u8]) -> u32 {\n    d.len() as u32\n}\n")]);
    let request = codedoc_ops::AttachRequest {
        target: codedoc_ops::Target::file("src/lib.rs"),
        kind: "explanation".to_owned(),
        claim: "Every entry point in this module assumes little-endian input.".to_owned(),
        detail: None,
    };
    codedoc_ops::attach(
        workspace.path(),
        None,
        &request,
        &codedoc_ops::Attribution::human("tester"),
        codedoc_ops::Provenance::default(),
    )
    .unwrap();

    let found = Workspace::at(workspace.path());
    let records = found.records().unwrap();
    let anchor = records[0].subject().expect("a subject anchor");
    assert_eq!(anchor.subject, codedoc_anchor::Subject::File);
    assert_eq!(anchor.file.as_str(), "src/lib.rs");

    let (verified, _) = codedoc_ops::verify(workspace.path()).unwrap();
    assert_eq!(verified["counts"]["detached"], 0, "{verified}");
}

#[test]
fn a_file_claim_reaches_an_agent_asking_about_a_symbol_inside_that_file() {
    let workspace = project(&[(
        "src/lib.rs",
        "//! Every entry point here assumes little-endian input.\n\nuse core::fmt;\n\npub fn compute(d: &[u8]) -> u32 {\n    d.len() as u32\n}\n",
    )]);
    codedoc_ops::import(workspace.path(), None, &["src".to_owned()], true, None).unwrap();

    let pack = codedoc_ops::context(
        workspace.path(),
        "src/lib.rs",
        None,
        Some("rust://compute"),
        0,
        None,
        None,
    )
    .unwrap();
    let rendered = pack.to_string();
    assert!(
        rendered.contains("little-endian"),
        "a claim about the whole file is a claim about every symbol in it: {rendered}"
    );
}

#[test]
fn a_file_claim_follows_the_file_when_git_records_a_rename() {
    let workspace = project(&[("src/lib.rs", "//! Parses the wire format.\n\nuse core::fmt;\n")]);
    let root = workspace.path();
    let run = |args: &[&str]| {
        std::process::Command::new("git")
            .args(args)
            .current_dir(root)
            .output()
            .expect("git is available")
    };
    run(&["init", "-q"]);
    run(&["config", "user.email", "t@example.com"]);
    run(&["config", "user.name", "t"]);
    run(&["add", "src/lib.rs"]);
    run(&["commit", "-qm", "first"]);

    codedoc_ops::import(root, None, &["src".to_owned()], true, None).unwrap();

    fs::rename(root.join("src/lib.rs"), root.join("src/wire.rs")).unwrap();
    run(&["add", "-A", "src"]);
    run(&["commit", "-qm", "rename"]);

    let (verified, _) = codedoc_ops::verify(root).unwrap();
    assert_eq!(
        verified["counts"]["detached"], 0,
        "git recorded the rename, so the file the claim describes still exists: {verified}"
    );
}

#[test]
fn a_comment_below_a_nested_declaration_is_not_called_a_claim_about_the_file() {
    let workspace = project(&[(
        "src/header.h",
        "#ifndef GUARD_H\n#define GUARD_H\n\nnamespace acme {\n\n/// Returns true if the path is absolute.\nbool is_absolute(const char* path);\n\n/// The maximum depth we will descend.\n#define MAX_DEPTH 32\n\n}\n\n#endif\n",
    )]);
    codedoc_ops::import(workspace.path(), None, &["src".to_owned()], true, None).unwrap();

    let found = Workspace::at(workspace.path());
    let records = found.records().unwrap();
    let about_the_file: Vec<&str> = records
        .iter()
        .filter(|record| {
            record.subject().is_some_and(|anchor| anchor.subject == codedoc_anchor::Subject::File)
        })
        .map(|record| record.content().body.claim.as_str())
        .collect();

    assert!(
        about_the_file.is_empty(),
        "every declaration in a C or C++ header sits inside a namespace or an include \
         guard, so looking only at the root's own children finds no declaration and \
         calls every comment in the file a claim about the file. Measured on gtest.h, \
         695 claims were labelled that way, including one describing a single \
         function: {about_the_file:?}"
    );
}
