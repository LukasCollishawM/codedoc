use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use codedoc_lang::Registry;
use tree_sitter::Node;
use walkdir::WalkDir;

pub struct Violation {
    pub file: PathBuf,
    pub line: usize,
    pub text: String,
    pub reason: &'static str,
}

pub fn run() -> ExitCode {
    let roots = ["crates", "xtask"];
    let mut violations = Vec::new();
    let mut scanned = 0usize;

    for root in roots {
        if !Path::new(root).is_dir() {
            continue;
        }
        for entry in WalkDir::new(root).into_iter().filter_map(Result::ok) {
            let path = entry.path();
            if !path.is_file() || path.extension().is_none_or(|extension| extension != "rs") {
                continue;
            }
            if path.components().any(|component| component.as_os_str() == "target") {
                continue;
            }
            scanned += 1;
            violations.extend(inspect(path));
        }
    }

    if violations.is_empty() {
        println!("lint-comments: {scanned} files, no comments");
        return ExitCode::SUCCESS;
    }

    eprintln!("lint-comments: {} violation(s) across {scanned} files", violations.len());
    eprintln!();
    for violation in &violations {
        eprintln!("{}:{}", violation.file.display(), violation.line);
        eprintln!("  {}", violation.text.trim());
        eprintln!("  {}", violation.reason);
        eprintln!();
    }
    eprintln!("This repository keeps knowledge in the ledger, not in the source.");
    eprintln!("Record it instead:");
    eprintln!();
    eprintln!("  codedoc attach <file> --symbol <symbol> \\");
    eprintln!("    --kind rationale --claim \"...\"");
    eprintln!();
    eprintln!("See CONTRIBUTING.md for why, and `codedoc kinds` for the vocabulary.");
    ExitCode::from(1)
}

fn inspect(path: &Path) -> Vec<Violation> {
    let Ok(source) = fs::read_to_string(path) else {
        return Vec::new();
    };
    let Some(adapter) = Registry::by_name("rust") else {
        return Vec::new();
    };
    let Ok(tree) = adapter.parse(&source) else {
        return Vec::new();
    };

    let mut violations = Vec::new();
    let mut cursor = tree.root_node().walk();
    let mut descending = true;
    loop {
        if descending {
            let node = cursor.node();
            if node.kind() == "line_comment" || node.kind() == "block_comment" {
                if let Some(reason) = judge(node, &source) {
                    violations.push(Violation {
                        file: path.to_path_buf(),
                        line: node.start_position().row + 1,
                        text: node.utf8_text(source.as_bytes()).unwrap_or("").to_owned(),
                        reason,
                    });
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
    violations
}

fn judge(node: Node<'_>, source: &str) -> Option<&'static str> {
    let text = node.utf8_text(source.as_bytes()).ok()?;

    if text.starts_with("//!") {
        return None;
    }

    if text.starts_with("///") || text.starts_with("/**") {
        return match documents_public_item(node, source) {
            true => None,
            false => Some("doc comments are permitted only on pub items"),
        };
    }

    Some("explanatory comments belong in the ledger, not the source")
}

fn documents_public_item(node: Node<'_>, source: &str) -> bool {
    let mut candidate = node.next_named_sibling();
    while let Some(sibling) = candidate {
        if sibling.kind() == "line_comment" || sibling.kind() == "block_comment" {
            candidate = sibling.next_named_sibling();
            continue;
        }
        let text = sibling.utf8_text(source.as_bytes()).unwrap_or("");
        return text.trim_start().starts_with("pub");
    }
    false
}
