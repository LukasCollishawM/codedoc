use codedoc_anchor::symbol_path_of;
use codedoc_lang::Registry;

fn symbols(source: &str) -> Vec<String> {
    let adapter = Registry::by_name("go").expect("the Go adapter");
    let tree = adapter.parse(source).expect("Go parses");
    let mut found = Vec::new();
    let mut stack = vec![tree.root_node()];
    while let Some(node) = stack.pop() {
        if adapter.declares_symbol(node.kind())
            && let Some(path) = symbol_path_of(node, adapter, source)
        {
            found.push(path.to_string());
        }
        let mut cursor = node.walk();
        for child in node.named_children(&mut cursor) {
            stack.push(child);
        }
    }
    found.sort();
    found.dedup();
    found
}

const FUNCTION: &str = "package gin\n\nfunc SetMode(value string) {\n\t_ = value\n}\n";
const TYPE: &str = "package gin\n\ntype Engine struct {\n\ttrees int\n}\n";
const CONSTANT: &str = "package gin\n\nconst TestMode = \"test\"\n";
const VARIABLE: &str = "package gin\n\nvar EnvGinMode = \"GIN_MODE\"\n";

#[test]
fn a_function_and_a_type_are_named() {
    assert_eq!(symbols(FUNCTION), vec!["go://SetMode".to_owned()]);
    assert_eq!(symbols(TYPE), vec!["go://Engine".to_owned()]);
}

#[test]
#[ignore = "codedoc-lang does not treat a Go const_declaration or var_declaration as a \
            naming declaration, so a documented package-level constant carries no symbol"]
fn a_package_level_constant_is_named_too() {
    assert_eq!(
        symbols(CONSTANT),
        vec!["go://TestMode".to_owned()],
        "`const TestMode = \"test\"` declares a name as plainly as `func` does, and Go \
         documents it the same way, with a comment above it opening with the \
         identifier. An anchor with no symbol cannot reach rungs 2, 3, 4 or 6, so the \
         only evidence left to it is its content fingerprint: it is lost the moment its \
         own line changes, and two constants with the same value are indistinguishable. \
         Measured over imported anchors on a construct, counting only what the adapter \
         could not name: gin 6.4% of 1,079, cobra 1.7% of 776, viper 0.5%, gorilla/mux \
         0.3%. gin is highest because it documents its modes, environment names and \
         error types as package-level constants, which is ordinary Go."
    );
    assert_eq!(symbols(VARIABLE), vec!["go://EnvGinMode".to_owned()]);
}

#[test]
fn a_constant_currently_carries_no_symbol_which_is_the_defect_above() {
    assert!(
        symbols(CONSTANT).is_empty() && symbols(VARIABLE).is_empty(),
        "if these now carry symbols, codedoc-lang has learned about Go const and var \
         declarations: delete this test and un-ignore the one above it"
    );
}
