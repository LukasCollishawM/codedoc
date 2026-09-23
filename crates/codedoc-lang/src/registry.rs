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

#[derive(Clone, Copy)]
pub struct Adapter {
    name: &'static str,
    extensions: &'static [&'static str],
    grammar: LanguageFn,
    declarations: &'static [Declaration],
    ignorable_kinds: &'static [&'static str],
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
    ) -> Option<&'tree str> {
        let declaration =
            self.declarations.iter().find(|candidate| candidate.node_kind == node.kind())?;
        for field in declaration.name_fields {
            if let Some(named) = node.child_by_field_name(field)
                && let Ok(text) = named.utf8_text(source.as_bytes())
            {
                return Some(text);
            }
        }
        None
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

const SLASH_COMMENTS: &[&str] = &["line_comment", "block_comment", "comment"];
const HASH_COMMENTS: &[&str] = &["comment"];

const ADAPTERS: &[Adapter] = &[
    Adapter {
        name: "rust",
        extensions: &["rs"],
        grammar: tree_sitter_rust::LANGUAGE,
        declarations: RUST_DECLARATIONS,
        ignorable_kinds: SLASH_COMMENTS,
    },
    Adapter {
        name: "python",
        extensions: &["py", "pyi"],
        grammar: tree_sitter_python::LANGUAGE,
        declarations: PYTHON_DECLARATIONS,
        ignorable_kinds: HASH_COMMENTS,
    },
    Adapter {
        name: "csharp",
        extensions: &["cs"],
        grammar: tree_sitter_c_sharp::LANGUAGE,
        declarations: CSHARP_DECLARATIONS,
        ignorable_kinds: SLASH_COMMENTS,
    },
    Adapter {
        name: "go",
        extensions: &["go"],
        grammar: tree_sitter_go::LANGUAGE,
        declarations: GO_DECLARATIONS,
        ignorable_kinds: SLASH_COMMENTS,
    },
    Adapter {
        name: "java",
        extensions: &["java"],
        grammar: tree_sitter_java::LANGUAGE,
        declarations: JAVA_DECLARATIONS,
        ignorable_kinds: SLASH_COMMENTS,
    },
    Adapter {
        name: "typescript",
        extensions: &["ts", "mts", "cts"],
        grammar: tree_sitter_typescript::LANGUAGE_TYPESCRIPT,
        declarations: TYPESCRIPT_DECLARATIONS,
        ignorable_kinds: SLASH_COMMENTS,
    },
    Adapter {
        name: "tsx",
        extensions: &["tsx", "jsx"],
        grammar: tree_sitter_typescript::LANGUAGE_TSX,
        declarations: TYPESCRIPT_DECLARATIONS,
        ignorable_kinds: SLASH_COMMENTS,
    },
];

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
        assert_eq!(adapter.declaration_name(function, source), Some("authorize"));
    }

    #[test]
    fn python_declarations_are_extracted() {
        let adapter = Registry::by_name("python").unwrap();
        let source = "class Ledger:\n    def append(self):\n        pass\n";
        let tree = adapter.parse(source).unwrap();
        let class = tree.root_node().child(0).unwrap();
        assert_eq!(adapter.declaration_name(class, source), Some("Ledger"));
    }

    #[test]
    fn comments_are_ignorable_in_every_adapter() {
        for adapter in Registry::all() {
            assert!(adapter.is_ignorable("comment") || adapter.is_ignorable("line_comment"));
        }
    }
}
