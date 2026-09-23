use codedoc_anchor::{Anchor, Resolution, Resolver, Rung};
use codedoc_core::RepoPath;
use codedoc_lang::{Adapter, Registry};
use proptest::prelude::*;
use tree_sitter::Node;

#[derive(Debug, Clone, PartialEq, Eq)]
struct Function {
    name: String,
    binding: String,
    value: u32,
}

impl Function {
    fn render(&self, expanded: bool) -> String {
        if expanded {
            format!(
                "fn {}() -> u32 {{\n    let {} = {};\n    {}\n}}\n",
                self.name, self.binding, self.value, self.binding
            )
        } else {
            format!(
                "fn {}()->u32{{let {}={};{}}}\n",
                self.name, self.binding, self.value, self.binding
            )
        }
    }
}

fn render_all(functions: &[Function], expanded: bool) -> String {
    functions.iter().map(|function| function.render(expanded)).collect::<Vec<_>>().join("\n")
}

fn rust() -> &'static Adapter {
    Registry::by_name("rust").unwrap()
}

fn file() -> RepoPath {
    RepoPath::parse("src/lib.rs").unwrap()
}

fn find_function<'tree>(root: Node<'tree>, source: &str, name: &str) -> Option<Node<'tree>> {
    let mut cursor = root.walk();
    root.named_children(&mut cursor).find(|node| {
        node.kind() == "function_item" && rust().declaration_name(*node, source) == Some(name)
    })
}

fn capture(source: &str, name: &str) -> Anchor {
    let tree = rust().parse(source).unwrap();
    let node = find_function(tree.root_node(), source, name).unwrap();
    Anchor::capture(file(), rust(), source, node)
}

fn resolve(anchor: &Anchor, source: &str) -> Resolution {
    let tree = rust().parse(source).unwrap();
    Resolver::new(rust()).resolve(anchor, source, &tree)
}

fn content_at(source: &str, range: codedoc_anchor::SourceRange) -> Option<String> {
    let lines: Vec<&str> = source.lines().collect();
    let start = range.start_line as usize;
    let end = range.end_line as usize;
    if start == 0 || end > lines.len() {
        return None;
    }
    Some(lines[start - 1..end].join("\n"))
}

#[test]
fn reformatting_resolves_by_content_identity() {
    let functions =
        vec![Function { name: "authorize".to_owned(), binding: "verdict".to_owned(), value: 7 }];
    let anchor = capture(&render_all(&functions, false), "authorize");
    let outcome = resolve(&anchor, &render_all(&functions, true));
    let located = outcome.located().expect("reformatting must not detach");
    assert_eq!(located.rung(), Rung::ContentIdentity);
}

#[test]
fn renaming_a_local_resolves_by_structural_identity() {
    let original = "fn authorize() -> u32 {\n    let verdict = 7;\n    verdict\n}\n";
    let renamed = "fn authorize() -> u32 {\n    let decision = 7;\n    decision\n}\n";
    let anchor = capture(original, "authorize");
    let located =
        resolve(&anchor, renamed).located().cloned().expect("a local rename must not detach");
    assert_eq!(located.rung(), Rung::StructuralIdentity);
}

#[test]
fn inserting_code_above_does_not_shift_the_anchor_onto_the_wrong_node() {
    let original = "fn authorize() -> u32 {\n    let verdict = 7;\n    verdict\n}\n";
    let shifted = format!("fn preamble() -> u32 {{\n    let seed = 1;\n    seed\n}}\n\n{original}");
    let anchor = capture(original, "authorize");
    let located = resolve(&anchor, &shifted).located().cloned().unwrap();
    let text = content_at(&shifted, located.range()).unwrap();
    assert!(text.contains("authorize"), "resolved onto {text:?}");
    assert!(located.range().start_line > 1);
}

fn statement_anchor(source: &str, index: usize) -> Anchor {
    let tree = rust().parse(source).unwrap();
    let host = find_function(tree.root_node(), source, "host").unwrap();
    let body = host.child_by_field_name("body").unwrap();
    let statement = body.named_child(index).unwrap();
    Anchor::capture(file(), rust(), source, statement)
}

#[test]
fn duplication_disambiguated_by_context_resolves_to_the_original() {
    let original = "fn host() -> u32 {\n    let v = compute(1);\n    v\n}\n";
    let anchor = statement_anchor(original, 0);

    let duplicated =
        "fn host() -> u32 {\n    let v = compute(1);\n    let v = compute(1);\n    v\n}\n";
    let outcome = resolve(&anchor, duplicated);

    match outcome.located() {
        None => (),
        Some(located) => assert_eq!(
            located.range().start_line,
            2,
            "the anchored statement is the first one; resolving onto the copy is a false reattachment"
        ),
    }
}

#[test]
fn genuinely_indistinguishable_candidates_detach_rather_than_guess() {
    let triplets = concat!(
        "fn alpha() {
    prepare();
    commit();
}

",
        "fn beta() {
    prepare();
    commit();
}

",
        "fn gamma() {
    prepare();
    commit();
}
"
    );
    let tree = rust().parse(triplets).unwrap();
    let alpha = find_function(tree.root_node(), triplets, "alpha").unwrap();
    let body = alpha.child_by_field_name("body").unwrap();
    let anchor = Anchor::capture(file(), rust(), triplets, body.named_child(1).unwrap());

    let alpha_removed = concat!(
        "fn beta() {
    prepare();
    commit();
}

",
        "fn gamma() {
    prepare();
    commit();
}
"
    );
    let outcome = resolve(&anchor, alpha_removed);
    assert!(
        outcome.is_detached(),
        "the documented function was deleted and two identical twins remain; nothing          distinguishes them, so resolving is a guess. got {outcome:?}"
    );
}

fn function_strategy() -> impl Strategy<Value = Function> {
    ("[a-j]{3,6}", "[k-t]{3,6}", 0u32..1000).prop_map(|(name, binding, value)| Function {
        name,
        binding,
        value,
    })
}

fn distinct(functions: Vec<Function>) -> Vec<Function> {
    let mut seen = Vec::new();
    for function in functions {
        let clashes = seen.iter().any(|existing: &Function| {
            existing.name == function.name || existing.binding == function.binding
        });
        if !clashes {
            seen.push(function);
        }
    }
    seen
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn resolution_is_correct_or_detached_never_wrong(
        functions in prop::collection::vec(function_strategy(), 2..7),
        target_index in 0usize..7,
        removals in prop::collection::vec(0usize..7, 0..3),
        additions in prop::collection::vec(function_strategy(), 0..3),
        expand in any::<bool>(),
    ) {
        let functions = distinct(functions);
        prop_assume!(functions.len() >= 2);
        let target = functions[target_index % functions.len()].clone();

        let original = render_all(&functions, false);
        let anchor = capture(&original, &target.name);

        let mut edited: Vec<Function> = functions
            .iter()
            .enumerate()
            .filter(|(index, function)| {
                function.name != target.name || !removals.contains(index)
            })
            .map(|(_, function)| function.clone())
            .collect();
        for addition in distinct(additions) {
            let clashes = edited.iter().any(|existing| {
                existing.name == addition.name
                    || existing.binding == addition.binding
                    || addition.name == target.name
            });
            if !clashes {
                edited.insert(0, addition);
            }
        }
        prop_assume!(edited.iter().any(|function| function.name == target.name));

        let mutated = render_all(&edited, expand);
        let outcome = resolve(&anchor, &mutated);

        if let Some(located) = outcome.located() {
            let text = content_at(&mutated, located.range()).unwrap_or_default();
            prop_assert!(
                text.contains(&format!("fn {}(", target.name)),
                "rung {:?} resolved anchor for {} onto {:?}",
                located.rung(),
                target.name,
                text
            );
        }
    }
}
