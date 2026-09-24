use std::fs;
use std::path::Path;

use codedoc_ledger::Ledger;

fn project(files: &[(&str, &str)]) -> tempfile::TempDir {
    let root = tempfile::tempdir().expect("a temporary directory");
    fs::create_dir_all(root.path().join("src")).unwrap();
    for (name, body) in files {
        let path = root.path().join(name);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, body).unwrap();
    }
    Ledger::initialise(root.path()).unwrap();
    root
}

fn imported(root: &Path) -> Vec<(String, Option<String>)> {
    codedoc_ops::import(root, None, &["src".to_owned()], true, None).unwrap();
    let listing = codedoc_ops::list(root, None, None, None, None, None).unwrap();
    listing["records"]
        .as_array()
        .expect("a record listing")
        .iter()
        .map(|record| {
            (
                record["claim"].as_str().unwrap_or_default().to_owned(),
                record["detail"].as_str().map(str::to_owned),
            )
        })
        .collect()
}

#[test]
fn a_sphinx_attribute_marker_does_not_survive_into_the_claim() {
    let root = project(&[(
        "src/lib.py",
        "class Context:\n    def __init__(self):\n        #: the parent context or None if none exists.\n        self.parent = None\n",
    )]);

    let claims = imported(root.path());
    assert!(
        claims.iter().any(|(claim, _)| claim.starts_with("the parent context")),
        "#: is Sphinx's attribute marker; stripping only the # leaves a claim opening \
         with punctuation: {claims:?}"
    );
    assert!(
        !claims.iter().any(|(claim, _)| claim.starts_with(':')),
        "no claim may begin with a comment marker: {claims:?}"
    );
}

#[test]
fn a_wrapped_comment_becomes_one_line_of_prose() {
    let root = project(&[(
        "src/lib.py",
        "def compute():\n    # Map of parameter names to their parsed values. Parameters\n    # with expose_value=False are not stored.\n    return 0\n",
    )]);

    let claims = imported(root.path());
    let (claim, _) = claims.first().expect("one record");
    assert!(
        !claim.contains('\n'),
        "a claim is one statement, and a newline in it breaks every renderer that \
         indents: {claim:?}"
    );
    assert!(claim.contains("values. Parameters with expose_value"), "{claim:?}");
}

#[test]
fn a_paragraph_break_divides_claim_from_detail() {
    let root = project(&[(
        "src/lib.go",
        "// Host adds a matcher for the URL host.\n//\n// It accepts a template with zero or more variables.\nfunc Host() {}\n",
    )]);

    let claims = imported(root.path());
    let (claim, detail) = claims.first().expect("one record");
    assert_eq!(claim, "Host adds a matcher for the URL host.");
    assert_eq!(
        detail.as_deref(),
        Some("It accepts a template with zero or more variables."),
        "the first paragraph is the claim and the rest is what supports it",
    );
}

#[test]
fn a_tool_directive_is_not_knowledge() {
    let root = project(&[(
        "src/lib.py",
        "def compute():\n    # type: ignore[assignment]\n    value = 1\n    # noqa: E501\n    return value\n",
    )]);
    assert!(
        imported(root.path()).is_empty(),
        "a suppression aimed at another tool says nothing about the code",
    );
}

#[test]
fn a_directive_carrying_a_reason_keeps_the_reason() {
    let root = project(&[(
        "src/lib.ts",
        "// eslint-disable-next-line no-await-in-loop -- the requests must be serialised because the server rejects concurrent writes\nexport function send() {}\n",
    )]);

    let claims = imported(root.path());
    let (claim, _) = claims.first().expect("the reason is worth keeping");
    assert!(
        claim.starts_with("the requests must be serialised"),
        "the directive is noise and the reason after it is the knowledge: {claim:?}",
    );
}

#[test]
fn a_shebang_is_not_a_claim() {
    let root = project(&[(
        "src/tool.py",
        "#!/usr/bin/env python3\n# Resolve the configuration before anything reads it.\ndef main():\n    return 0\n",
    )]);

    let claims = imported(root.path());
    assert!(
        !claims.iter().any(|(claim, _)| claim.contains("/usr/bin")),
        "an interpreter line is not documentation: {claims:?}"
    );
    assert!(
        claims.iter().any(|(claim, _)| claim.starts_with("Resolve the configuration")),
        "and the real comment beneath it survives: {claims:?}"
    );
}
