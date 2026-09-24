use codedoc_lang::Adapter;
use tree_sitter::{Node, Tree};

use crate::anchor::symbol_path_of;

pub fn by_symbol<'tree>(
    tree: &'tree Tree,
    source: &str,
    adapter: &Adapter,
    symbol: &str,
) -> Option<Node<'tree>> {
    let mut found = None;
    let mut cursor = tree.root_node().walk();
    let mut descending = true;
    loop {
        if descending {
            let node = cursor.node();
            if adapter.declares_symbol(node.kind())
                && symbol_path_of(node, adapter, source)
                    .is_some_and(|path| path.to_string() == symbol)
            {
                found = Some(node);
                break;
            }
            if cursor.goto_first_child() {
                continue;
            }
            descending = false;
        }
        if cursor.goto_next_sibling() {
            descending = true;
            continue;
        }
        if !cursor.goto_parent() {
            break;
        }
    }
    found
}

pub fn by_line<'tree>(tree: &'tree Tree, adapter: &Adapter, line: u32) -> Option<Node<'tree>> {
    let target = line.checked_sub(1)? as usize;
    let mut best: Option<Node<'tree>> = None;
    let mut cursor = tree.root_node().walk();
    let mut descending = true;
    loop {
        if descending {
            let node = cursor.node();
            let covers = node.start_position().row <= target && node.end_position().row >= target;
            if covers && node.is_named() && !adapter.is_ignorable(node.kind()) {
                let span = node.end_position().row - node.start_position().row;
                let better = best.is_none_or(|current| {
                    span < current.end_position().row - current.start_position().row
                });
                if better {
                    best = Some(node);
                }
            }
            if cursor.goto_first_child() {
                continue;
            }
            descending = false;
        }
        if cursor.goto_next_sibling() {
            descending = true;
            continue;
        }
        if !cursor.goto_parent() {
            break;
        }
    }
    best
}

pub fn enclosing_declaration<'tree>(node: Node<'tree>, adapter: &Adapter) -> Option<Node<'tree>> {
    let mut current = Some(node);
    while let Some(candidate) = current {
        if adapter.declares_symbol(candidate.kind()) {
            return Some(candidate);
        }
        current = candidate.parent();
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use codedoc_lang::Registry;

    #[test]
    fn symbols_are_located_by_path() {
        let adapter = Registry::by_name("rust").unwrap();
        let source = "fn alpha() {}\nfn beta() -> u32 { 1 }\n";
        let tree = adapter.parse(source).unwrap();
        let node = by_symbol(&tree, source, adapter, "rust://beta").unwrap();
        assert_eq!(node.kind(), "function_item");
        assert_eq!(node.start_position().row, 1);
    }

    #[test]
    fn missing_symbols_return_nothing() {
        let adapter = Registry::by_name("rust").unwrap();
        let source = "fn alpha() {}\n";
        let tree = adapter.parse(source).unwrap();
        assert!(by_symbol(&tree, source, adapter, "rust://absent").is_none());
    }

    #[test]
    fn lines_resolve_to_the_smallest_covering_node() {
        let adapter = Registry::by_name("rust").unwrap();
        let source = "fn alpha() {\n    let value = 1;\n}\n";
        let tree = adapter.parse(source).unwrap();
        let node = by_line(&tree, adapter, 2).unwrap();
        assert!(node.end_position().row - node.start_position().row == 0);
    }

    #[test]
    fn a_line_anchors_to_the_statement_not_to_a_token_inside_it() {
        let adapter = Registry::by_name("rust").unwrap();
        let source = "fn compute() -> u32 {
    let total = 42;
    total
}
";
        let tree = adapter.parse(source).unwrap();
        let node = by_line(&tree, adapter, 2).unwrap();
        assert_eq!(
            node.kind(),
            "let_declaration",
            "a statement and the identifier and literal inside it all span one line, so              ties must resolve outward. Anchoring to a bare identifier would attach a              claim to a token that says nothing about what the line does."
        );
    }

    #[test]
    fn python_indentation_blocks_locate_correctly() {
        let adapter = Registry::by_name("python").unwrap();
        let source = "class Ledger:\n    def append(self):\n        return 1\n";
        let tree = adapter.parse(source).unwrap();
        let node = by_symbol(&tree, source, adapter, "python://Ledger/append").unwrap();
        assert_eq!(node.kind(), "function_definition");
    }
}
