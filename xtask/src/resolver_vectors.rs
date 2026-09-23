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
    Case {
        name: "cpp_deleting_an_overload_with_an_identical_twin_detaches",
        language: "cpp",
        path: "src/gate.cpp",
        before: "int Gate::admit(int value) {\n    int scaled = value;\n    return scaled;\n}\n\nint Gate::admit(long value) {\n    int scaled = value;\n    return scaled;\n}\n",
        after: "int Gate::admit(long value) {\n    int scaled = value;\n    return scaled;\n}\n",
        symbol: "cpp://Gate/admit",
    },
    Case {
        name: "cpp_in_class_and_out_of_line_share_one_symbol",
        language: "cpp",
        path: "src/session.cpp",
        before: "class Session {\n    int open();\n};\n\nint Session::open() {\n    int handle = 7;\n    return handle;\n}\n",
        after: "class Session {\n    int open();\n};\n\nint Session::open() {\n    int descriptor = 7;\n    return descriptor;\n}\n",
        symbol: "cpp://Session/open",
    },
    Case {
        name: "java_reformatting_holds",
        language: "java",
        path: "src/Gate.java",
        before: "class Gate {\n    int admit(int id) { return id; }\n}\n",
        after: "class Gate {\n\n    int admit(int id) {\n        return id;\n    }\n}\n",
        symbol: "java://Gate/admit",
    },
    Case {
        name: "java_local_rename_holds_by_structural_identity",
        language: "java",
        path: "src/Gate.java",
        before: "class Gate {\n    int admit(int id) {\n        int checked = id;\n        return checked;\n    }\n}\n",
        after: "class Gate {\n    int admit(int id) {\n        int verified = id;\n        return verified;\n    }\n}\n",
        symbol: "java://Gate/admit",
    },
    Case {
        name: "go_reformatting_holds",
        language: "go",
        path: "cmd/gate.go",
        before: "package main\n\nfunc Admit(id int) int {\n\tchecked := id\n\treturn checked\n}\n",
        after: "package main\n\nfunc Admit(id int) int {\n\n\tchecked := id\n\n\treturn checked\n}\n",
        symbol: "go://Admit",
    },
    Case {
        name: "go_local_rename_holds_by_structural_identity",
        language: "go",
        path: "cmd/gate.go",
        before: "package main\n\nfunc Admit(id int) int {\n\tchecked := id\n\treturn checked\n}\n",
        after: "package main\n\nfunc Admit(id int) int {\n\tverified := id\n\treturn verified\n}\n",
        symbol: "go://Admit",
    },
    Case {
        name: "csharp_reformatting_holds",
        language: "csharp",
        path: "Gate.cs",
        before: "class Gate {\n    int Admit(int id) { return id; }\n}\n",
        after: "class Gate {\n\n    int Admit(int id) {\n        return id;\n    }\n}\n",
        symbol: "csharp://Gate/Admit",
    },
    Case {
        name: "csharp_local_rename_holds_by_structural_identity",
        language: "csharp",
        path: "Gate.cs",
        before: "class Gate {\n    int Admit(int id) {\n        int checked_id = id;\n        return checked_id;\n    }\n}\n",
        after: "class Gate {\n    int Admit(int id) {\n        int verified = id;\n        return verified;\n    }\n}\n",
        symbol: "csharp://Gate/Admit",
    },
    Case {
        name: "typescript_reformatting_holds",
        language: "typescript",
        path: "src/gate.ts",
        before: "export function admit(id: number): number { return id; }\n",
        after: "export function admit(id: number): number {\n    return id;\n}\n",
        symbol: "typescript://admit",
    },
    Case {
        name: "typescript_local_rename_holds_by_structural_identity",
        language: "typescript",
        path: "src/gate.ts",
        before: "export function admit(id: number): number {\n    const checked = id;\n    return checked;\n}\n",
        after: "export function admit(id: number): number {\n    const verified = id;\n    return verified;\n}\n",
        symbol: "typescript://admit",
    },
];

pub fn cardinality_for(case: &Case) -> Option<u32> {
    let adapter = Registry::by_name(case.language)?;
    let path = RepoPath::parse(case.path).ok()?;
    let tree = adapter.parse(case.before).ok()?;
    let node = codedoc_anchor::locate::by_symbol(&tree, case.before, adapter, case.symbol)?;
    Some(Anchor::capture(path, adapter, case.before, node).symbol_cardinality)
}

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
            "    {{\n      \"name\": {},\n      \"language\": {},\n      \"path\": {},\n      \"symbol\": {},\n      \"symbol_cardinality\": {},\n      \"before\": {},\n      \"after\": {},\n      \"expect\": {{ \"outcome\": {}, \"detail\": {} }}\n    }}",
            quote(case.name),
            quote(case.language),
            quote(case.path),
            quote(case.symbol),
            cardinality_for(case).unwrap_or(1),
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
