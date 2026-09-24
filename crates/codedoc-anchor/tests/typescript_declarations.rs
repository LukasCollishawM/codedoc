use codedoc_anchor::symbol_path_of;
use codedoc_lang::Registry;

fn symbols(source: &str) -> Vec<String> {
    let adapter = Registry::by_name("typescript").expect("the TypeScript adapter");
    let tree = adapter.parse(source).expect("TypeScript parses");
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

const FUNCTION: &str =
    "export function parseToken(raw: string): string[] {\n  return raw.split(\":\");\n}\n";
const ARROW: &str =
    "export const validate = (raw: string): boolean => {\n  return raw.length > 0;\n};\n";
const CONSTANT: &str = "export const TIMEOUT = 5000;\n";

#[test]
fn a_function_declaration_is_named() {
    assert_eq!(symbols(FUNCTION), vec!["typescript://parseToken".to_owned()]);
}

#[test]
#[ignore = "codedoc-lang does not treat a lexical declaration as a naming declaration, \
            so an arrow function bound to a const has no symbol"]
fn an_arrow_function_bound_to_a_const_is_named_too() {
    assert_eq!(
        symbols(ARROW),
        vec!["typescript://validate".to_owned()],
        "binding an arrow function to a const is the ordinary way to declare a \
         function in modern TypeScript. A claim attached to one cannot be found by \
         symbol, cannot reach rungs 2, 3, 4 or 6, and survives only while the file is \
         byte-identical. Measured over zod's sources, 16.4% of imported anchors \
         carried no symbol, against 0.0% for Java and 1.2% for C++."
    );
}

#[test]
#[ignore = "same cause: an exported constant is a lexical declaration"]
fn an_exported_constant_is_named_too() {
    assert_eq!(symbols(CONSTANT), vec!["typescript://TIMEOUT".to_owned()]);
}

#[test]
fn the_gap_is_still_there_which_is_why_the_two_above_are_ignored() {
    assert!(
        symbols(ARROW).is_empty() && symbols(CONSTANT).is_empty(),
        "if these now have symbols, codedoc-lang has learned about lexical \
         declarations: delete this test and un-ignore the two above it"
    );
}
