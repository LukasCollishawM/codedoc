use std::fs;
use std::path::Path;

use codedoc_ledger::Ledger;

const BEFORE: &str = "pub fn host(template: &str) -> bool {\n    !template.is_empty()\n}\n";

fn project(claim: &str) -> tempfile::TempDir {
    let root = tempfile::tempdir().expect("a temporary directory");
    fs::create_dir_all(root.path().join("src")).unwrap();
    fs::write(root.path().join("src/route.rs"), BEFORE).unwrap();
    Ledger::initialise(root.path()).unwrap();

    let request = codedoc_ops::AttachRequest {
        target: codedoc_ops::Target::symbol("src/route.rs", "rust://host"),
        kind: "explanation".to_owned(),
        claim: claim.to_owned(),
        detail: None,
    };
    codedoc_ops::attach(
        root.path(),
        None,
        &request,
        &codedoc_ops::Attribution::human("tester"),
        codedoc_ops::Provenance::default(),
    )
    .expect("the claim is recorded");
    root
}

fn rename(root: &Path) {
    let source = fs::read_to_string(root.join("src/route.rs")).unwrap();
    fs::write(root.join("src/route.rs"), source.replace("pub fn host", "pub fn host_pattern"))
        .unwrap();
}

fn detached_record(root: &Path) -> String {
    let (listing, _) = codedoc_ops::detached(root).expect("a detached listing");
    listing["records"][0]["record"].as_str().expect("a detached record").to_owned()
}

fn resolve_to(root: &Path, record: &str, symbol: &str) -> serde_json::Value {
    codedoc_ops::resolve(root, None, record, &codedoc_ops::Target::symbol("src/route.rs", symbol))
        .expect("the anchor is placed")
}

#[test]
fn renaming_a_function_detaches_the_claim_rather_than_moving_it() {
    let root = project("Host matches the URL host against a template.");
    rename(root.path());

    let (report, _) = codedoc_ops::verify(root.path()).unwrap();
    assert_eq!(
        report["counts"]["detached"], 1,
        "a rename is not evidence of identity, so the claim waits for a decision: {report}"
    );
}

#[test]
fn resolving_onto_a_renamed_symbol_says_the_claim_still_names_the_old_one() {
    let root = project("host validates the template before matching.");
    rename(root.path());
    let record = detached_record(root.path());

    let placed = resolve_to(root.path(), &record, "rust://host_pattern");

    assert_eq!(placed["symbol"], "rust://host_pattern", "{placed}");
    assert_eq!(placed["was_symbol"], "rust://host", "{placed}");
    assert_eq!(
        placed["claim_names_the_old_symbol"], true,
        "the claim opens with a name that no longer exists, and the moment to say so \
         is the moment it is reattached: {placed}"
    );
}

#[test]
fn a_claim_that_never_named_the_symbol_prompts_nothing() {
    let root = project("The template is matched case-insensitively against the request.");
    rename(root.path());
    let record = detached_record(root.path());

    let placed = resolve_to(root.path(), &record, "rust://host_pattern");

    assert_eq!(
        placed["claim_names_the_old_symbol"], false,
        "nothing in this claim went stale with the rename, so prompting would be noise: \
         {placed}"
    );
}

#[test]
fn the_reattached_claim_answers_for_the_new_symbol() {
    let root = project("host validates the template before matching.");
    rename(root.path());
    let record = detached_record(root.path());
    resolve_to(root.path(), &record, "rust://host_pattern");

    let (report, code) = codedoc_ops::verify(root.path()).unwrap();
    assert_eq!(report["counts"]["detached"], 0, "{report}");
    assert_eq!(code, 0, "{report}");

    let context = codedoc_ops::context(
        root.path(),
        "src/route.rs",
        None,
        Some("rust://host_pattern"),
        1,
        None,
        None,
    )
    .expect("context for the new symbol");
    let claims = context["pack"]["other"].as_array().map(Vec::len).unwrap_or(0);
    assert_eq!(claims, 1, "the claim now answers for the code it was moved onto: {context}");
}
