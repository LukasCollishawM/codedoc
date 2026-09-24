use codedoc_anchor::symbol_path_of;
use codedoc_lang::Registry;

fn first_class_symbol(source: &str) -> Option<String> {
    let adapter = Registry::by_name("csharp").expect("the C# adapter");
    let tree = adapter.parse(source).expect("C# parses");
    let mut found = None;
    let mut stack = vec![tree.root_node()];
    while let Some(node) = stack.pop() {
        if node.kind() == "class_declaration" {
            found = symbol_path_of(node, adapter, source).map(|path| path.to_string());
            break;
        }
        let mut cursor = node.walk();
        for child in node.named_children(&mut cursor) {
            stack.push(child);
        }
    }
    found
}

const BLOCK: &str = "namespace Acme.Widgets\n{\n    public class Builder\n    {\n        public int Assemble(int count) { return count; }\n    }\n}\n";

const FILE_SCOPED: &str = "namespace Acme.Widgets;\n\npublic class Builder\n{\n    public int Assemble(int count) { return count; }\n}\n";

#[test]
fn a_block_namespace_qualifies_the_types_inside_it() {
    assert_eq!(first_class_symbol(BLOCK).as_deref(), Some("csharp://Acme.Widgets/Builder"));
}

#[test]
#[ignore = "codedoc-lang does not treat file_scoped_namespace_declaration as a naming \
            declaration, so every symbol in a C# 10 file loses its namespace"]
fn a_file_scoped_namespace_qualifies_them_the_same_way() {
    assert_eq!(
        first_class_symbol(FILE_SCOPED).as_deref(),
        Some("csharp://Acme.Widgets/Builder"),
        "`namespace Acme.Widgets;` and the braced form declare the same \
         namespace. Dropping the segment for one of them means two types called \
         Builder in different namespaces share a symbol path, and a file migrating \
         between the two forms detaches every record in it. C# 10 made the \
         file-scoped form the default project template, so this is most C# written \
         since 2021."
    );
}

#[test]
fn the_two_forms_currently_disagree_which_is_the_defect_above() {
    let block = first_class_symbol(BLOCK);
    let filed = first_class_symbol(FILE_SCOPED);
    assert_ne!(
        block, filed,
        "if these now agree, codedoc-lang has learned about file-scoped namespaces: \
         delete this test and un-ignore the one above it"
    );
}
