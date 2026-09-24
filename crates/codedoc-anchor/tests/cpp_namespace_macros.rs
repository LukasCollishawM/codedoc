use codedoc_anchor::symbol_path_of;
use codedoc_lang::Registry;

fn enum_symbol(source: &str) -> Option<String> {
    let adapter = Registry::by_name("cpp").expect("the C++ adapter");
    let tree = adapter.parse(source).expect("C++ parses");
    let mut stack = vec![tree.root_node()];
    while let Some(node) = stack.pop() {
        if node.kind() == "enum_specifier" {
            return symbol_path_of(node, adapter, source).map(|path| path.to_string());
        }
        let mut cursor = node.walk();
        for child in node.named_children(&mut cursor) {
            stack.push(child);
        }
    }
    None
}

const PLAIN: &str = "namespace detail\n{\nenum class value_t { null, object };\n}\n";

const MACRO: &str = "#define ACME_BEGIN namespace acme { inline namespace v3 {\n#define ACME_END } }\n\nACME_BEGIN\nnamespace detail\n{\nenum class value_t { null, object };\n}\nACME_END\n";

#[test]
fn a_written_out_namespace_qualifies_what_it_contains() {
    assert_eq!(enum_symbol(PLAIN).as_deref(), Some("cpp://detail/value_t"));
}

#[test]
#[ignore = "codedoc-lang names a declaration whose declarator is the token `namespace`, \
            so a namespace opened by a macro puts a C++ keyword in the symbol path"]
fn a_namespace_opened_by_a_macro_does_not_put_a_keyword_in_the_symbol() {
    let symbol = enum_symbol(MACRO);
    assert!(
        !symbol.as_deref().unwrap_or_default().contains("namespace"),
        "tree-sitter-cpp cannot expand ACME_BEGIN, so it reads `ACME_BEGIN namespace detail \
         {{ ... }}` as a function_definition whose return type is ACME_BEGIN, whose name is the \
         token `namespace`, and whose parameter list is an ERROR holding `detail`. The adapter \
         then names that function, and every declaration beneath it is qualified by a C++ \
         keyword. `namespace` is a reserved word and can never be a declarator, so refusing it \
         costs nothing that is real. Two consequences, both measured on nlohmann/json at depth \
         400 and fmtlib/fmt: 33.0% of nlohmann's 1,987 imported anchors are rooted at \
         `cpp://namespace`, 33 of them collapsing onto that bare symbol, which by the overload \
         rule identifies none of them; and the symbol changes depending on whether the \
         preprocessor context happened to parse, which relabelled five anchors mid-replay. \
         Got: {symbol:?}"
    );
}

#[test]
fn the_macro_form_currently_yields_a_keyword_which_is_the_defect_above() {
    assert_eq!(
        enum_symbol(MACRO).as_deref(),
        Some("cpp://namespace/value_t"),
        "if this no longer holds, codedoc-lang has stopped naming the mis-parse: \
         delete this test and un-ignore the one above it"
    );
}
