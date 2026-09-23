use std::borrow::Cow;

use codedoc_core::RepoPath;
use thiserror::Error;
use tree_sitter::{Node, Parser, Tree};
use tree_sitter_language::LanguageFn;

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum LanguageError {
    #[error("no language adapter is registered for {path}")]
    Unsupported { path: String },

    #[error("grammar for {language} could not be loaded: {detail}")]
    Grammar { language: &'static str, detail: String },

    #[error("source for {path} could not be parsed into a syntax tree")]
    Unparsable { path: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Declaration {
    pub node_kind: &'static str,
    pub name_fields: &'static [&'static str],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NameStyle {
    Terminal,
    Qualified,
}

#[derive(Clone, Copy)]
pub struct Adapter {
    name: &'static str,
    extensions: &'static [&'static str],
    grammar: LanguageFn,
    declarations: &'static [Declaration],
    ignorable_kinds: &'static [&'static str],
    name_style: NameStyle,
    descend_fields: &'static [&'static str],
}

impl std::fmt::Debug for Adapter {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Adapter")
            .field("name", &self.name)
            .field("extensions", &self.extensions)
            .finish_non_exhaustive()
    }
}

impl Adapter {
    pub fn name(&self) -> &'static str {
        self.name
    }

    pub fn extensions(&self) -> &'static [&'static str] {
        self.extensions
    }

    pub fn parser(&self) -> Result<Parser, LanguageError> {
        let mut parser = Parser::new();
        parser.set_language(&self.grammar.into()).map_err(|source| LanguageError::Grammar {
            language: self.name,
            detail: source.to_string(),
        })?;
        Ok(parser)
    }

    pub fn parse(&self, source: &str) -> Result<Tree, LanguageError> {
        let mut parser = self.parser()?;
        parser
            .parse(source, None)
            .ok_or_else(|| LanguageError::Unparsable { path: self.name.to_owned() })
    }

    pub fn is_ignorable(&self, node_kind: &str) -> bool {
        self.ignorable_kinds.contains(&node_kind)
    }

    pub fn declaration_name<'tree>(
        &self,
        node: Node<'tree>,
        source: &'tree str,
    ) -> Option<Cow<'tree, str>> {
        let raw = self.raw_declaration_name(node, source)?;
        match self.name_style {
            NameStyle::Terminal => Some(Cow::Borrowed(base_name(raw))),
            NameStyle::Qualified => {
                let qualified = qualified_name(raw);
                (!qualified.is_empty()).then_some(Cow::Owned(qualified))
            }
        }
    }

    fn raw_declaration_name<'tree>(
        &self,
        node: Node<'tree>,
        source: &'tree str,
    ) -> Option<&'tree str> {
        let declaration =
            self.declarations.iter().find(|candidate| candidate.node_kind == node.kind())?;
        for field in declaration.name_fields {
            if let Some(named) = node.child_by_field_name(field) {
                let resolved = self.descend_to_name(named);
                if let Ok(text) = resolved.utf8_text(source.as_bytes()) {
                    return Some(text);
                }
            }
        }
        None
    }

    fn descend_to_name<'tree>(&self, node: Node<'tree>) -> Node<'tree> {
        if self.descend_fields.is_empty() {
            return node;
        }
        let mut current = node;
        for _ in 0..MAX_DECLARATOR_DEPTH {
            let mut advanced = false;
            for field in self.descend_fields {
                if let Some(inner) = current.child_by_field_name(field) {
                    current = inner;
                    advanced = true;
                    break;
                }
            }
            if !advanced {
                break;
            }
        }
        current
    }

    pub fn declares_symbol(&self, node_kind: &str) -> bool {
        self.declarations.iter().any(|declaration| declaration.node_kind == node_kind)
    }
}

const RUST_DECLARATIONS: &[Declaration] = &[
    Declaration { node_kind: "function_item", name_fields: &["name"] },
    Declaration { node_kind: "struct_item", name_fields: &["name"] },
    Declaration { node_kind: "enum_item", name_fields: &["name"] },
    Declaration { node_kind: "trait_item", name_fields: &["name"] },
    Declaration { node_kind: "impl_item", name_fields: &["type"] },
    Declaration { node_kind: "mod_item", name_fields: &["name"] },
    Declaration { node_kind: "const_item", name_fields: &["name"] },
    Declaration { node_kind: "static_item", name_fields: &["name"] },
    Declaration { node_kind: "type_item", name_fields: &["name"] },
    Declaration { node_kind: "macro_definition", name_fields: &["name"] },
];

const PYTHON_DECLARATIONS: &[Declaration] = &[
    Declaration { node_kind: "function_definition", name_fields: &["name"] },
    Declaration { node_kind: "class_definition", name_fields: &["name"] },
];

const CSHARP_DECLARATIONS: &[Declaration] = &[
    Declaration { node_kind: "namespace_declaration", name_fields: &["name"] },
    Declaration { node_kind: "class_declaration", name_fields: &["name"] },
    Declaration { node_kind: "struct_declaration", name_fields: &["name"] },
    Declaration { node_kind: "interface_declaration", name_fields: &["name"] },
    Declaration { node_kind: "record_declaration", name_fields: &["name"] },
    Declaration { node_kind: "enum_declaration", name_fields: &["name"] },
    Declaration { node_kind: "method_declaration", name_fields: &["name"] },
    Declaration { node_kind: "property_declaration", name_fields: &["name"] },
    Declaration { node_kind: "constructor_declaration", name_fields: &["name"] },
];

const GO_DECLARATIONS: &[Declaration] = &[
    Declaration { node_kind: "function_declaration", name_fields: &["name"] },
    Declaration { node_kind: "method_declaration", name_fields: &["name"] },
    Declaration { node_kind: "type_spec", name_fields: &["name"] },
];

const JAVA_DECLARATIONS: &[Declaration] = &[
    Declaration { node_kind: "class_declaration", name_fields: &["name"] },
    Declaration { node_kind: "interface_declaration", name_fields: &["name"] },
    Declaration { node_kind: "enum_declaration", name_fields: &["name"] },
    Declaration { node_kind: "record_declaration", name_fields: &["name"] },
    Declaration { node_kind: "method_declaration", name_fields: &["name"] },
    Declaration { node_kind: "constructor_declaration", name_fields: &["name"] },
];

const TYPESCRIPT_DECLARATIONS: &[Declaration] = &[
    Declaration { node_kind: "function_declaration", name_fields: &["name"] },
    Declaration { node_kind: "class_declaration", name_fields: &["name"] },
    Declaration { node_kind: "method_definition", name_fields: &["name"] },
    Declaration { node_kind: "interface_declaration", name_fields: &["name"] },
    Declaration { node_kind: "type_alias_declaration", name_fields: &["name"] },
    Declaration { node_kind: "enum_declaration", name_fields: &["name"] },
    Declaration { node_kind: "abstract_class_declaration", name_fields: &["name"] },
];

const CPP_DECLARATIONS: &[Declaration] = &[
    Declaration { node_kind: "namespace_definition", name_fields: &["name"] },
    Declaration { node_kind: "class_specifier", name_fields: &["name"] },
    Declaration { node_kind: "struct_specifier", name_fields: &["name"] },
    Declaration { node_kind: "union_specifier", name_fields: &["name"] },
    Declaration { node_kind: "enum_specifier", name_fields: &["name"] },
    Declaration { node_kind: "function_definition", name_fields: &["declarator"] },
    Declaration { node_kind: "field_declaration", name_fields: &["declarator"] },
    Declaration { node_kind: "alias_declaration", name_fields: &["name"] },
    Declaration { node_kind: "type_definition", name_fields: &["declarator"] },
];

const CPP_DECLARATOR_FIELDS: &[&str] = &["declarator"];

const SLASH_COMMENTS: &[&str] = &["line_comment", "block_comment", "comment"];
const HASH_COMMENTS: &[&str] = &["comment"];

const ADAPTERS: &[Adapter] = &[
    Adapter {
        name: "rust",
        extensions: &["rs"],
        grammar: tree_sitter_rust::LANGUAGE,
        declarations: RUST_DECLARATIONS,
        ignorable_kinds: SLASH_COMMENTS,
        name_style: NameStyle::Terminal,
        descend_fields: &[],
    },
    Adapter {
        name: "python",
        extensions: &["py", "pyi"],
        grammar: tree_sitter_python::LANGUAGE,
        declarations: PYTHON_DECLARATIONS,
        ignorable_kinds: HASH_COMMENTS,
        name_style: NameStyle::Terminal,
        descend_fields: &[],
    },
    Adapter {
        name: "csharp",
        extensions: &["cs"],
        grammar: tree_sitter_c_sharp::LANGUAGE,
        declarations: CSHARP_DECLARATIONS,
        ignorable_kinds: SLASH_COMMENTS,
        name_style: NameStyle::Terminal,
        descend_fields: &[],
    },
    Adapter {
        name: "go",
        extensions: &["go"],
        grammar: tree_sitter_go::LANGUAGE,
        declarations: GO_DECLARATIONS,
        ignorable_kinds: SLASH_COMMENTS,
        name_style: NameStyle::Terminal,
        descend_fields: &[],
    },
    Adapter {
        name: "java",
        extensions: &["java"],
        grammar: tree_sitter_java::LANGUAGE,
        declarations: JAVA_DECLARATIONS,
        ignorable_kinds: SLASH_COMMENTS,
        name_style: NameStyle::Terminal,
        descend_fields: &[],
    },
    Adapter {
        name: "typescript",
        extensions: &["ts", "mts", "cts"],
        grammar: tree_sitter_typescript::LANGUAGE_TYPESCRIPT,
        declarations: TYPESCRIPT_DECLARATIONS,
        ignorable_kinds: SLASH_COMMENTS,
        name_style: NameStyle::Terminal,
        descend_fields: &[],
    },
    Adapter {
        name: "tsx",
        extensions: &["tsx", "jsx"],
        grammar: tree_sitter_typescript::LANGUAGE_TSX,
        declarations: TYPESCRIPT_DECLARATIONS,
        ignorable_kinds: SLASH_COMMENTS,
        name_style: NameStyle::Terminal,
        descend_fields: &[],
    },
    Adapter {
        name: "cpp",
        extensions: &["cpp", "cc", "cxx", "hpp", "hh", "hxx", "h", "c"],
        grammar: tree_sitter_cpp::LANGUAGE,
        declarations: CPP_DECLARATIONS,
        ignorable_kinds: SLASH_COMMENTS,
        name_style: NameStyle::Qualified,
        descend_fields: CPP_DECLARATOR_FIELDS,
    },
];

const MAX_DECLARATOR_DEPTH: usize = 8;

fn base_name(raw: &str) -> &str {
    let truncated = raw.split(['<', '(', ' ']).next().unwrap_or(raw);
    truncated.rsplit("::").next().unwrap_or(truncated).trim()
}

fn qualified_name(raw: &str) -> String {
    let mut without_templates = String::with_capacity(raw.len());
    let mut depth = 0usize;
    for character in raw.chars() {
        match character {
            '<' => depth += 1,
            '>' => depth = depth.saturating_sub(1),
            _ if depth == 0 => without_templates.push(character),
            _ => {}
        }
    }
    let callable = without_templates.split('(').next().unwrap_or_default().to_owned();
    callable
        .split("::")
        .map(|segment| segment.trim_matches(|c: char| c.is_whitespace() || c == '*' || c == '&'))
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>()
        .join("/")
}

pub struct Registry;

impl Registry {
    pub fn all() -> &'static [Adapter] {
        ADAPTERS
    }

    pub fn by_name(name: &str) -> Option<&'static Adapter> {
        ADAPTERS.iter().find(|adapter| adapter.name == name)
    }

    pub fn for_path(path: &RepoPath) -> Result<&'static Adapter, LanguageError> {
        let extension = path
            .extension()
            .ok_or_else(|| LanguageError::Unsupported { path: path.to_string() })?
            .to_ascii_lowercase();
        ADAPTERS
            .iter()
            .find(|adapter| adapter.extensions.contains(&extension.as_str()))
            .ok_or_else(|| LanguageError::Unsupported { path: path.to_string() })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn path(text: &str) -> RepoPath {
        RepoPath::parse(text).unwrap()
    }

    #[test]
    fn every_adapter_loads_its_grammar() {
        for adapter in Registry::all() {
            assert!(adapter.parser().is_ok(), "{} failed to load", adapter.name());
        }
    }

    #[test]
    fn extensions_resolve_to_adapters() {
        assert_eq!(Registry::for_path(&path("src/lib.rs")).unwrap().name(), "rust");
        assert_eq!(Registry::for_path(&path("app/main.py")).unwrap().name(), "python");
        assert_eq!(Registry::for_path(&path("Svc.cs")).unwrap().name(), "csharp");
        assert_eq!(Registry::for_path(&path("cmd/root.go")).unwrap().name(), "go");
        assert_eq!(Registry::for_path(&path("Main.java")).unwrap().name(), "java");
        assert_eq!(Registry::for_path(&path("api.ts")).unwrap().name(), "typescript");
    }

    #[test]
    fn unknown_extensions_are_rejected() {
        assert!(Registry::for_path(&path("notes.txt")).is_err());
        assert!(Registry::for_path(&path("Makefile")).is_err());
    }

    #[test]
    fn declaration_names_are_extracted_per_language() {
        let adapter = Registry::by_name("rust").unwrap();
        let source = "fn authorize(token: &str) -> bool { true }";
        let tree = adapter.parse(source).unwrap();
        let function = tree.root_node().child(0).unwrap();
        assert_eq!(adapter.declaration_name(function, source).as_deref(), Some("authorize"));
    }

    #[test]
    fn python_declarations_are_extracted() {
        let adapter = Registry::by_name("python").unwrap();
        let source = "class Ledger:\n    def append(self):\n        pass\n";
        let tree = adapter.parse(source).unwrap();
        let class = tree.root_node().child(0).unwrap();
        assert_eq!(adapter.declaration_name(class, source).as_deref(), Some("Ledger"));
    }

    #[test]
    fn generic_parameters_do_not_leak_into_symbol_names() {
        let adapter = Registry::by_name("rust").unwrap();
        let source = "impl<'a, T> Cache<'a, T> { fn get(&self) {} }";
        let tree = adapter.parse(source).unwrap();
        let block = tree.root_node().child(0).unwrap();
        assert_eq!(adapter.declaration_name(block, source).as_deref(), Some("Cache"));
    }

    #[test]
    fn qualified_impl_targets_reduce_to_their_base_name() {
        let adapter = Registry::by_name("rust").unwrap();
        let source = "impl inner::Thing { fn go(&self) {} }";
        let tree = adapter.parse(source).unwrap();
        let block = tree.root_node().child(0).unwrap();
        assert_eq!(adapter.declaration_name(block, source).as_deref(), Some("Thing"));
    }

    #[test]
    fn comments_are_ignorable_in_every_adapter() {
        for adapter in Registry::all() {
            assert!(adapter.is_ignorable("comment") || adapter.is_ignorable("line_comment"));
        }
    }
    fn cpp() -> &'static Adapter {
        Registry::by_name("cpp").unwrap()
    }

    fn first_name(source: &str, kind: &str) -> Option<String> {
        let adapter = cpp();
        let tree = adapter.parse(source).unwrap();
        fn find<'t>(node: Node<'t>, kind: &str) -> Option<Node<'t>> {
            if node.kind() == kind {
                return Some(node);
            }
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                if let Some(found) = find(child, kind) {
                    return Some(found);
                }
            }
            None
        }
        let node = find(tree.root_node(), kind)?;
        adapter.declaration_name(node, source).map(|name| name.into_owned())
    }

    #[test]
    fn cpp_extensions_resolve_to_the_cpp_adapter() {
        for file in ["a.cpp", "a.cc", "a.cxx", "b.hpp", "b.hh", "b.hxx", "b.h", "c.c"] {
            assert_eq!(Registry::for_path(&path(file)).unwrap().name(), "cpp", "{file}");
        }
    }

    #[test]
    fn cpp_reaches_the_name_through_nested_declarators() {
        assert_eq!(first_name("void run() {}", "function_definition").as_deref(), Some("run"));
        assert_eq!(first_name("int* make() { return nullptr; }", "function_definition").as_deref(), Some("make"));
        assert_eq!(first_name("const char& at(int i) { return *\"\"; }", "function_definition").as_deref(), Some("at"));
    }

    #[test]
    fn cpp_out_of_line_definitions_keep_their_qualification() {
        assert_eq!(first_name("void Session::open() {}", "function_definition").as_deref(), Some("Session/open"));
        assert_eq!(
            first_name("int Outer::Inner::depth() { return 0; }", "function_definition").as_deref(),
            Some("Outer/Inner/depth")
        );
    }

    #[test]
    fn cpp_out_of_line_matches_the_in_class_declaration() {
        let declared = first_name("class Session { void open(); };", "field_declaration");
        let defined = first_name("void Session::open() {}", "function_definition");
        assert_eq!(declared.as_deref(), Some("open"));
        assert_eq!(defined.as_deref(), Some("Session/open"));
    }

    #[test]
    fn cpp_template_arguments_do_not_leak_into_names() {
        assert_eq!(
            first_name("void Buffer<int>::clear() {}", "function_definition").as_deref(),
            Some("Buffer/clear")
        );
    }

    #[test]
    fn cpp_declares_classes_namespaces_and_enums() {
        assert_eq!(first_name("namespace helix { }", "namespace_definition").as_deref(), Some("helix"));
        assert_eq!(first_name("class Target { };", "class_specifier").as_deref(), Some("Target"));
        assert_eq!(first_name("struct Pair { };", "struct_specifier").as_deref(), Some("Pair"));
        assert_eq!(first_name("enum class Kind { A };", "enum_specifier").as_deref(), Some("Kind"));
    }

    #[test]
    fn rust_still_reduces_qualified_impl_targets() {
        let adapter = Registry::by_name("rust").unwrap();
        let source = "impl inner::Thing { }";
        let tree = adapter.parse(source).unwrap();
        let mut cursor = tree.root_node().walk();
        let block = tree.root_node().children(&mut cursor).next().unwrap();
        assert_eq!(adapter.declaration_name(block, source).as_deref(), Some("Thing"));
    }

}
