#![forbid(unsafe_code)]

pub mod anchor;
pub mod fingerprint;
pub mod locate;
pub mod resolver;

pub use anchor::{Anchor, NodePath, NodeStep, SourceRange, SymbolTable, symbol_path_of};
pub use resolver::{Confidence, DetachReason, FileIndex, Located, Resolution, Resolver, Rung};
