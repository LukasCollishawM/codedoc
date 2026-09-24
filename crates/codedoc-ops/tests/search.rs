use std::fs;

use codedoc_ledger::Ledger;

fn workspace() -> tempfile::TempDir {
    let root = tempfile::tempdir().expect("a temporary directory");
    fs::create_dir_all(root.path().join("src")).unwrap();
    fs::write(
        root.path().join("src/auth.rs"),
        "pub fn validate(token: &str) -> bool {\n    !token.is_empty()\n}\n",
    )
    .unwrap();
    fs::write(
        root.path().join("src/cache.rs"),
        "pub fn evict(key: &str) -> bool {\n    !key.is_empty()\n}\n",
    )
    .unwrap();
    Ledger::initialise(root.path()).unwrap();
    root
}

fn record(root: &std::path::Path, file: &str, symbol: &str, kind: &str, claim: &str) {
    let request = codedoc_ops::AttachRequest {
        target: codedoc_ops::Target::symbol(file, symbol),
        kind: kind.to_owned(),
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

fn claims(found: &serde_json::Value) -> Vec<String> {
    found["records"]
        .as_array()
        .expect("records is an array")
        .iter()
        .map(|row| row["claim"].as_str().unwrap_or_default().to_owned())
        .collect()
}

#[test]
fn a_claim_is_found_by_words_without_naming_the_file_it_lives_in() {
    let root = workspace();
    record(
        root.path(),
        "src/auth.rs",
        "rust://validate",
        "invariant",
        "Tenant isolation depends on validating the token before resolving the tenant.",
    );
    record(root.path(), "src/cache.rs", "rust://evict", "performance", "Eviction is O(1).");

    let found = codedoc_ops::search(root.path(), None, "tenant isolation", None, None, 10).unwrap();
    assert_eq!(found["count"], 1, "{found}");
    assert!(claims(&found)[0].contains("Tenant isolation"));
}

#[test]
fn a_query_whose_terms_are_spread_across_records_returns_all_of_them_best_first() {
    let root = workspace();
    record(
        root.path(),
        "src/auth.rs",
        "rust://validate",
        "invariant",
        "Token validation must precede tenant resolution.",
    );
    record(
        root.path(),
        "src/cache.rs",
        "rust://evict",
        "performance",
        "Eviction never touches a token.",
    );

    let found = codedoc_ops::search(root.path(), None, "token validation", None, None, 10).unwrap();
    assert_eq!(
        found["count"], 2,
        "requiring every term turns a broad question into no answer at all: {found}"
    );
    assert!(
        claims(&found)[0].contains("validation"),
        "the record matching both terms must rank above the one matching one: {found}"
    );
}

#[test]
fn search_filters_by_kind_and_by_path_prefix() {
    let root = workspace();
    record(
        root.path(),
        "src/auth.rs",
        "rust://validate",
        "invariant",
        "The token is checked here.",
    );
    record(root.path(), "src/cache.rs", "rust://evict", "performance", "The token is cached here.");

    let by_kind =
        codedoc_ops::search(root.path(), None, "token", Some("performance"), None, 10).unwrap();
    assert_eq!(by_kind["count"], 1, "{by_kind}");
    assert!(claims(&by_kind)[0].contains("cached"));

    let by_file =
        codedoc_ops::search(root.path(), None, "token", None, Some("src/auth"), 10).unwrap();
    assert_eq!(by_file["count"], 1, "{by_file}");
    assert!(claims(&by_file)[0].contains("checked"));
}

#[test]
fn a_retracted_claim_does_not_come_back_from_search() {
    let root = workspace();
    record(
        root.path(),
        "src/auth.rs",
        "rust://validate",
        "invariant",
        "Tokens are validated exactly once.",
    );
    let listed = codedoc_ops::list(root.path(), None, None, None, None, None).unwrap();
    let id = listed["records"][0]["record"].as_str().unwrap().to_owned();

    codedoc_ops::retract(root.path(), None, &id, Some("no longer true")).unwrap();

    let found = codedoc_ops::search(root.path(), None, "validated", None, None, 10).unwrap();
    assert_eq!(found["count"], 0, "a retracted claim is not a current answer: {found}");
}

#[test]
fn a_query_with_no_searchable_terms_returns_nothing_rather_than_failing() {
    let root = workspace();
    record(root.path(), "src/auth.rs", "rust://validate", "invariant", "Tokens are validated.");

    for query in ["", "   ", "*", "\"", "-", "AND OR NOT"] {
        let found = codedoc_ops::search(root.path(), None, query, None, None, 10).unwrap();
        assert!(
            found["count"].as_u64().is_some(),
            "a query the user typed must never reach the full-text parser raw: {query:?}"
        );
    }
}

#[test]
fn a_record_is_found_by_the_name_of_the_code_it_is_about() {
    let root = workspace();
    record(
        root.path(),
        "src/auth.rs",
        "rust://validate",
        "invariant",
        "Nothing in this sentence mentions the function it describes.",
    );

    let by_symbol = codedoc_ops::search(root.path(), None, "validate", None, None, 10).unwrap();
    assert_eq!(
        by_symbol["count"], 1,
        "an agent usually knows the name of the thing before it knows which file \
         holds it, so a record has to be findable by what it is about and not only \
         by what it says: {by_symbol}"
    );

    let by_file = codedoc_ops::search(root.path(), None, "auth.rs", None, None, 10).unwrap();
    assert_eq!(by_file["count"], 1, "{by_file}");
}

#[test]
fn what_a_claim_says_still_outranks_where_it_lives() {
    let root = workspace();
    record(root.path(), "src/auth.rs", "rust://validate", "invariant", "Tokens expire hourly.");
    record(
        root.path(),
        "src/cache.rs",
        "rust://evict",
        "performance",
        "Eviction is unrelated to validate and merely mentions expiry.",
    );

    let found = codedoc_ops::search(root.path(), None, "expire", None, None, 10).unwrap();
    let first = found["records"][0]["claim"].as_str().unwrap_or_default();
    assert!(
        first.contains("Tokens expire hourly"),
        "the words of a claim carry more than the name it is attached to: {found}"
    );
}
