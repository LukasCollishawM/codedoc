use std::collections::{BTreeMap, HashMap};

use codedoc_core::{ContentFingerprint, ContextFingerprint, StructuralFingerprint};
use codedoc_lang::Adapter;
use tree_sitter::Node;

pub const CONTEXT_SIBLING_SPAN: usize = 2;

pub fn structural_of(node: Node<'_>, digests: &Digests) -> StructuralFingerprint {
    digests
        .structural
        .get(&node.id())
        .copied()
        .unwrap_or_else(|| StructuralFingerprint::of(node.kind().as_bytes()))
}

pub fn content_of(node: Node<'_>, digests: &Digests) -> ContentFingerprint {
    digests
        .content
        .get(&node.id())
        .copied()
        .unwrap_or_else(|| ContentFingerprint::of(node.kind().as_bytes()))
}

#[derive(Default)]
pub struct Digests {
    pub content: HashMap<usize, ContentFingerprint>,
    pub structural: HashMap<usize, StructuralFingerprint>,
}

pub fn compute_all(root: Node<'_>, adapter: &Adapter, source: &str) -> Digests {
    let mut digests = Digests::default();
    memoise(root, adapter, source, &mut digests);
    digests
}

fn memoise(
    node: Node<'_>,
    adapter: &Adapter,
    source: &str,
    out: &mut Digests,
) -> (ContentFingerprint, StructuralFingerprint) {
    let mut structural_payload = Vec::new();
    structural_payload.extend_from_slice(node.kind().as_bytes());
    structural_payload.push(b'(');

    let mut content_payload = Vec::new();
    let childless = node.child_count() == 0;
    if childless {
        if let Ok(text) = node.utf8_text(source.as_bytes()) {
            content_payload.extend_from_slice(text.as_bytes());
        }
    }

    let mut cursor = node.walk();
    if cursor.goto_first_child() {
        loop {
            let child = cursor.node();
            if adapter.is_ignorable(child.kind()) {
                if !cursor.goto_next_sibling() {
                    break;
                }
                continue;
            }
            let (child_content, child_structural) = memoise(child, adapter, source, out);
            content_payload.extend_from_slice(child_content.digest().bytes());
            if child.is_named() {
                if let Some(field) = cursor.field_name() {
                    structural_payload.extend_from_slice(field.as_bytes());
                    structural_payload.push(b':');
                }
                structural_payload.extend_from_slice(child_structural.digest().bytes());
            }
            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }
    structural_payload.push(b')');

    let content = ContentFingerprint::of(&content_payload);
    let structural = StructuralFingerprint::of(&structural_payload);
    out.content.insert(node.id(), content);
    out.structural.insert(node.id(), structural);
    (content, structural)
}

pub fn context_siblings(node: Node<'_>, adapter: &Adapter) -> u32 {
    let mut count = 0u32;
    let mut cursor = node;
    for _ in 0..CONTEXT_SIBLING_SPAN {
        match previous_named_sibling(cursor, adapter) {
            Some(sibling) => {
                count += 1;
                cursor = sibling;
            }
            None => break,
        }
    }
    let mut cursor = node;
    for _ in 0..CONTEXT_SIBLING_SPAN {
        match next_named_sibling(cursor, adapter) {
            Some(sibling) => {
                count += 1;
                cursor = sibling;
            }
            None => break,
        }
    }
    count
}

pub fn preceding_context_with(
    node: Node<'_>,
    adapter: &Adapter,
    digests: &Digests,
) -> ContextFingerprint {
    let mut siblings = Vec::new();
    let mut cursor = node;
    for _ in 0..CONTEXT_SIBLING_SPAN {
        match previous_named_sibling(cursor, adapter) {
            Some(sibling) => {
                siblings.push(sibling);
                cursor = sibling;
            }
            None => break,
        }
    }
    siblings.reverse();
    context_from_digests(&siblings, node, digests)
}

pub fn following_context_with(
    node: Node<'_>,
    adapter: &Adapter,
    digests: &Digests,
) -> ContextFingerprint {
    let mut siblings = Vec::new();
    let mut cursor = node;
    for _ in 0..CONTEXT_SIBLING_SPAN {
        match next_named_sibling(cursor, adapter) {
            Some(sibling) => {
                siblings.push(sibling);
                cursor = sibling;
            }
            None => break,
        }
    }
    context_from_digests(&siblings, node, digests)
}

fn context_from_digests(
    siblings: &[Node<'_>],
    anchor: Node<'_>,
    digests: &Digests,
) -> ContextFingerprint {
    let mut payload = Vec::new();
    match anchor.parent() {
        Some(parent) => payload.extend_from_slice(parent.kind().as_bytes()),
        None => payload.extend_from_slice(b"<root>"),
    }
    payload.push(0);
    for sibling in siblings {
        if let Some(digest) = digests.structural.get(&sibling.id()) {
            payload.extend_from_slice(digest.digest().bytes());
        }
        payload.push(0);
    }
    ContextFingerprint::of(&payload)
}

pub fn shape_histogram(node: Node<'_>, adapter: &Adapter) -> BTreeMap<String, u32> {
    let mut histogram = BTreeMap::new();
    accumulate_shape(node, adapter, &mut histogram);
    histogram
}

fn accumulate_shape(node: Node<'_>, adapter: &Adapter, histogram: &mut BTreeMap<String, u32>) {
    if adapter.is_ignorable(node.kind()) {
        return;
    }
    *histogram.entry(node.kind().to_owned()).or_insert(0) += 1;
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        accumulate_shape(child, adapter, histogram);
    }
}

fn previous_named_sibling<'tree>(node: Node<'tree>, adapter: &Adapter) -> Option<Node<'tree>> {
    let mut candidate = node.prev_named_sibling();
    while let Some(sibling) = candidate {
        if !adapter.is_ignorable(sibling.kind()) {
            return Some(sibling);
        }
        candidate = sibling.prev_named_sibling();
    }
    None
}

fn next_named_sibling<'tree>(node: Node<'tree>, adapter: &Adapter) -> Option<Node<'tree>> {
    let mut candidate = node.next_named_sibling();
    while let Some(sibling) = candidate {
        if !adapter.is_ignorable(sibling.kind()) {
            return Some(sibling);
        }
        candidate = sibling.next_named_sibling();
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use codedoc_lang::Registry;

    struct Fixture {
        tree: tree_sitter::Tree,
        digests: Digests,
        adapter: &'static Adapter,
    }

    fn rust_fixture(source: &str) -> Fixture {
        let adapter = Registry::by_name("rust").unwrap();
        let tree = adapter.parse(source).unwrap();
        let digests = compute_all(tree.root_node(), adapter, source);
        Fixture { tree, digests, adapter }
    }

    impl Fixture {
        fn item(&self, index: usize) -> Node<'_> {
            self.tree.root_node().child(index).unwrap()
        }

        fn structural(&self, index: usize) -> StructuralFingerprint {
            structural_of(self.item(index), &self.digests)
        }

        fn content(&self, index: usize) -> ContentFingerprint {
            content_of(self.item(index), &self.digests)
        }
    }

    #[test]
    fn whitespace_does_not_change_either_fingerprint() {
        let dense = rust_fixture("fn a(){let x=1;}");
        let loose = rust_fixture(
            "fn a() {
    let x = 1;
}
",
        );
        assert_eq!(dense.structural(0), loose.structural(0));
        assert_eq!(dense.content(0), loose.content(0));
    }

    #[test]
    fn comments_do_not_change_either_fingerprint() {
        let bare = rust_fixture("fn a() { let x = 1; }");
        let annotated = rust_fixture(
            "fn a() {
    let x = 1;
}",
        );
        assert_eq!(bare.structural(0), annotated.structural(0));
        assert_eq!(bare.content(0), annotated.content(0));
    }

    #[test]
    fn renaming_an_identifier_preserves_structure_but_changes_content() {
        let original = rust_fixture("fn a() { let value = 1; }");
        let renamed = rust_fixture("fn a() { let amount = 1; }");
        assert_eq!(original.structural(0), renamed.structural(0));
        assert_ne!(original.content(0), renamed.content(0));
    }

    #[test]
    fn reshaping_the_body_changes_the_structural_fingerprint() {
        let original = rust_fixture("fn a() { if x { y(); } }");
        let reshaped = rust_fixture("fn a() { while x { y(); } }");
        assert_ne!(original.structural(0), reshaped.structural(0));
    }

    #[test]
    fn context_distinguishes_position_among_identical_siblings() {
        let fixture = rust_fixture(
            "fn a() {}
fn t() {}
fn b() {}
fn t2() {}
",
        );
        let first = fixture.item(1);
        let second = fixture.item(3);
        assert_ne!(
            preceding_context_with(first, fixture.adapter, &fixture.digests),
            preceding_context_with(second, fixture.adapter, &fixture.digests)
        );
    }

    #[test]
    fn identical_subtrees_share_a_digest_regardless_of_position() {
        let fixture = rust_fixture(
            "fn a() { work(); }
fn b() { work(); }
",
        );
        let left = fixture.item(0).child_by_field_name("body").unwrap();
        let right = fixture.item(1).child_by_field_name("body").unwrap();
        assert_eq!(content_of(left, &fixture.digests), content_of(right, &fixture.digests));
    }
}
