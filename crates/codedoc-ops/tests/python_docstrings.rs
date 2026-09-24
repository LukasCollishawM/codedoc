use std::fs;

use codedoc_ledger::Ledger;

const SOURCE: &str = "def validate(token):\n    \"\"\"Validate a token before tenant resolution.\n\n    Resolving a tenant from an unvalidated token allows tenant confusion.\n    \"\"\"\n    return bool(token)\n";

fn imported() -> (tempfile::TempDir, serde_json::Value) {
    let root = tempfile::tempdir().expect("a temporary directory");
    fs::create_dir_all(root.path().join("src")).unwrap();
    fs::write(root.path().join("src/auth.py"), SOURCE).unwrap();
    Ledger::initialise(root.path()).unwrap();

    codedoc_ops::import(root.path(), None, &["src".to_owned()], true, None).unwrap();
    let listing = codedoc_ops::list(root.path(), None, None, None, None, None, None).unwrap();
    (root, listing)
}

#[test]
fn the_gap_is_still_here() {
    let (_root, listing) = imported();
    assert_eq!(
        listing["total"], 0,
        "A docstring is an expression statement rather than a comment, so \
         `Adapter::is_ignorable` never sees it and import walks straight past. Delete this \
         test the day the ignored one below passes: django carries 3,990 docstrings across \
         12,513 declarations and codedoc imports none of them, while taking 6,217 records \
         from its `#` comments, which are the incidental notes rather than the documentation. \
         Fixing it needs a hook in `codedoc-lang` for documentation that is not a comment \
         node, because a language-specific branch outside that crate is a leak. Listing: \
         {listing}"
    );
}

#[test]
#[ignore = "codedoc-lang has no hook for documentation that is not a comment node"]
fn a_python_docstring_is_documentation() {
    let (_root, listing) = imported();

    assert_eq!(listing["total"], 1, "the docstring documents validate: {listing}");
    let record = &listing["records"][0];
    assert_eq!(record["symbol"], "python://validate", "{listing}");
    assert_eq!(
        record["claim"], "Validate a token before tenant resolution.",
        "the summary line is the claim: {listing}"
    );
    assert_eq!(
        record["detail"].as_str(),
        Some("Resolving a tenant from an unvalidated token allows tenant confusion."),
        "and the body beneath it is the detail: {listing}"
    );
}
