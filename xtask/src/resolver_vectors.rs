use std::fs;

use codedoc_anchor::{Anchor, FileIndex, Resolution};
use codedoc_core::RepoPath;
use codedoc_lang::Registry;

pub struct Case {
    pub name: &'static str,
    pub language: &'static str,
    pub path: &'static str,
    pub before: &'static str,
    pub after: &'static str,
    pub symbol: &'static str,
}

pub const CASES: &[Case] = &[
    Case {
        name: "reformatting_holds_by_content_identity",
        language: "rust",
        path: "src/lib.rs",
        before: "fn authorize()->u32{let verdict=7;verdict}\n",
        after: "fn authorize() -> u32 {\n    let verdict = 7;\n    verdict\n}\n",
        symbol: "rust://authorize",
    },
    Case {
        name: "local_rename_holds_by_structural_identity",
        language: "rust",
        path: "src/lib.rs",
        before: "fn authorize() -> u32 {\n    let verdict = 7;\n    verdict\n}\n",
        after: "fn authorize() -> u32 {\n    let decision = 7;\n    decision\n}\n",
        symbol: "rust://authorize",
    },
    Case {
        name: "insertion_above_does_not_shift_the_anchor",
        language: "rust",
        path: "src/lib.rs",
        before: "fn authorize() -> u32 {\n    let verdict = 7;\n    verdict\n}\n",
        after: "fn preamble() -> u32 {\n    let seed = 1;\n    seed\n}\n\nfn authorize() -> u32 {\n    let verdict = 7;\n    verdict\n}\n",
        symbol: "rust://authorize",
    },
    Case {
        name: "deleting_the_target_detaches",
        language: "rust",
        path: "src/lib.rs",
        before: "fn distinctive_target() -> u32 {\n    let unique_binding = 4242;\n    unique_binding\n}\n\nfn neighbour() -> u32 {\n    let other = 1;\n    other\n}\n",
        after: "fn neighbour() -> u32 {\n    let other = 1;\n    other\n}\n",
        symbol: "rust://distinctive_target",
    },
    Case {
        name: "literal_change_holds_by_structural_identity",
        language: "rust",
        path: "src/lib.rs",
        before: "fn compute() -> u32 {\n    let a = 1;\n    let b = 2;\n    a + b\n}\n",
        after: "fn compute() -> u32 {\n    let a = 10;\n    let b = 20;\n    a * b\n}\n",
        symbol: "rust://compute",
    },
    Case {
        name: "body_reshape_holds_by_symbol_and_node_path",
        language: "rust",
        path: "src/lib.rs",
        before: "fn compute(flag: bool) -> u32 {\n    let a = 1;\n    a\n}\n",
        after: "fn compute(flag: bool) -> u32 {\n    if flag {\n        1\n    } else {\n        2\n    }\n}\n",
        symbol: "rust://compute",
    },
    Case {
        name: "python_reindentation_holds",
        language: "python",
        path: "src/ledger.py",
        before: "class Ledger:\n    def append(self, record):\n        return record\n",
        after: "class Ledger:\n\n    def append(self, record):\n\n        return record\n",
        symbol: "python://Ledger/append",
    },
    Case {
        name: "python_method_survives_enclosing_class_rename",
        language: "python",
        path: "src/ledger.py",
        before: "class Ledger:\n    def flush(self):\n        return 1\n",
        after: "class Journal:\n    def flush(self):\n        return 1\n",
        symbol: "python://Ledger/flush",
    },
    Case {
        name: "identical_twins_after_deletion_detach",
        language: "rust",
        path: "src/lib.rs",
        before: "fn alpha() {\n    prepare();\n    commit();\n}\n\nfn beta() {\n    prepare();\n    commit();\n}\n\nfn gamma() {\n    prepare();\n    commit();\n}\n",
        after: "fn beta() {\n    prepare();\n    commit();\n}\n\nfn gamma() {\n    prepare();\n    commit();\n}\n",
        symbol: "rust://alpha",
    },
    Case {
        name: "cpp_reformatting_holds_by_content_identity",
        language: "cpp",
        path: "src/session.cpp",
        before: "int Session::open(){int handle=7;return handle;}\n",
        after: "int Session::open() {\n    int handle = 7;\n    return handle;\n}\n",
        symbol: "cpp://Session/open",
    },
    Case {
        name: "cpp_local_rename_holds_by_structural_identity",
        language: "cpp",
        path: "src/session.cpp",
        before: "int Session::open() {\n    int handle = 7;\n    return handle;\n}\n",
        after: "int Session::open() {\n    int descriptor = 7;\n    return descriptor;\n}\n",
        symbol: "cpp://Session/open",
    },
    Case {
        name: "cpp_insertion_above_does_not_shift_the_anchor",
        language: "cpp",
        path: "src/session.cpp",
        before: "int Session::open() {\n    int handle = 7;\n    return handle;\n}\n",
        after: "void Session::warm() {\n    int primer = 1;\n    (void)primer;\n}\n\nint Session::open() {\n    int handle = 7;\n    return handle;\n}\n",
        symbol: "cpp://Session/open",
    },
];

pub fn outcome_for(case: &Case) -> Option<(String, String)> {
    let adapter = Registry::by_name(case.language)?;
    let path = RepoPath::parse(case.path).ok()?;
    let before_tree = adapter.parse(case.before).ok()?;
    let node = codedoc_anchor::locate::by_symbol(&before_tree, case.before, adapter, case.symbol)?;
    let anchor = Anchor::capture(path, adapter, case.before, node);

    let after_tree = adapter.parse(case.after).ok()?;
    let index = FileIndex::build(adapter, case.after, &after_tree);
    match index.resolve(&anchor) {
        Resolution::Located(located) => {
            let rung = serde_json::to_string(&located.rung()).ok()?;
            Some(("located".to_owned(), rung.trim_matches('"').to_owned()))
        }
        Resolution::Detached(reason) => {
            let encoded = serde_json::to_value(reason).ok()?;
            let named = encoded.get("reason")?.as_str()?.to_owned();
            Some(("detached".to_owned(), named))
        }
        _ => Some(("detached".to_owned(), "unknown".to_owned())),
    }
}

pub fn generate() -> Result<usize, String> {
    let mut entries = Vec::new();
    for case in CASES {
        let (outcome, detail) = outcome_for(case)
            .ok_or_else(|| format!("case {} could not be evaluated", case.name))?;
        entries.push(format!(
            "    {{\n      \"name\": {},\n      \"language\": {},\n      \"path\": {},\n      \"symbol\": {},\n      \"before\": {},\n      \"after\": {},\n      \"expect\": {{ \"outcome\": {}, \"detail\": {} }}\n    }}",
            quote(case.name),
            quote(case.language),
            quote(case.path),
            quote(case.symbol),
            quote(case.before),
            quote(case.after),
            quote(&outcome),
            quote(&detail)
        ));
    }
    let document = format!(
        "{{\n  \"version\": 1,\n  \"description\": \"Resolver vectors. An implementation must capture the anchor for `symbol` in `before`, resolve it against `after`, and reach the stated outcome. Ambiguity must never be resolved by selection.\",\n  \"vectors\": [\n{}\n  ]\n}}\n",
        entries.join(",\n")
    );
    fs::write("conformance/resolver/vectors.json", document).map_err(|e| e.to_string())?;
    Ok(CASES.len())
}

fn quote(raw: &str) -> String {
    String::from_utf8(codedoc_core::Canonical::Text(raw.to_owned()).encode()).unwrap_or_default()
}
