#![forbid(unsafe_code)]

mod registry;

pub use registry::{Adapter, Declaration, LanguageError, Registry};

pub use tree_sitter::Tree;
