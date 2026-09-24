use std::collections::BTreeSet;
use std::fs;
use std::path::Path;
use std::process::ExitCode;

use codedoc_anchor::{locate, symbol_path_of};
use codedoc_core::RepoPath;
use codedoc_graph::Graph;
use codedoc_lang::Registry;
use codedoc_ledger::Workspace;
use walkdir::WalkDir;

struct Suppression {
    file: String,
    line: usize,
    text: String,
    symbol: Option<String>,
}

pub fn run() -> ExitCode {
    let root = Path::new(".");
    let justified = match justified_symbols(root) {
        Ok(found) => found,
        Err(reason) => {
            eprintln!("lint-allows: {reason}");
            return ExitCode::from(1);
        }
    };

    let mut found = Vec::new();
    let mut scanned = 0usize;
    for entry in WalkDir::new("crates").into_iter().filter_map(Result::ok) {
        let path = entry.path();
        if !path.is_file() || path.extension().is_none_or(|extension| extension != "rs") {
            continue;
        }
        if !path.components().any(|component| component.as_os_str() == "src") {
            continue;
        }
        scanned += 1;
        found.extend(suppressions_in(path));
    }

    let unjustified: Vec<&Suppression> = found
        .iter()
        .filter(|item| match &item.symbol {
            Some(symbol) => !justified.contains(symbol),
            None => true,
        })
        .collect();

    if unjustified.is_empty() {
        println!(
            "lint-allows: {scanned} files, {} suppression(s), each with a workaround record",
            found.len()
        );
        return ExitCode::SUCCESS;
    }

    eprintln!(
        "lint-allows: {} suppression(s) with nothing recorded against them",
        unjustified.len()
    );
    eprintln!();
    for item in &unjustified {
        eprintln!("  {}:{}", item.file, item.line);
        eprintln!("    {}", item.text.trim());
        match &item.symbol {
            Some(symbol) => {
                eprintln!();
                eprintln!("    codedoc attach {} --symbol {symbol} \\", item.file);
                eprintln!("      --kind workaround --claim \"...\"");
            }
            None => {
                eprintln!("    no enclosing declaration could be named, so nothing can be");
                eprintln!("    anchored here. Move the suppression onto a named item.");
            }
        }
        eprintln!();
    }
    eprintln!("A suppression says a rule does not apply here. Which rule, and why, is the");
    eprintln!("part the next reader needs and the attribute cannot carry. Record it against");
    eprintln!("the item, where it is found by whoever asks what this code does.");
    ExitCode::from(1)
}

fn justified_symbols(root: &Path) -> Result<BTreeSet<String>, String> {
    let workspace = Workspace::discover(root).map_err(|error| error.to_string())?;
    let graph = Graph::across(&workspace).map_err(|error| error.to_string())?;
    Ok(graph
        .active()
        .iter()
        .filter(|record| record.kind().as_str() == "workaround")
        .flat_map(|record| record.content().anchors.iter())
        .filter_map(|entry| entry.anchor.symbol.as_ref().map(ToString::to_string))
        .collect())
}

fn suppressions_in(path: &Path) -> Vec<Suppression> {
    let Ok(source) = fs::read_to_string(path) else {
        return Vec::new();
    };
    let display = path.to_string_lossy().replace('\\', "/");
    let lines: Vec<(usize, &str)> = source
        .lines()
        .enumerate()
        .filter(|(_, text)| text.contains("#[allow("))
        .map(|(index, text)| (index + 1, text))
        .collect();
    if lines.is_empty() {
        return Vec::new();
    }

    let parsed = RepoPath::parse(&display)
        .ok()
        .and_then(|relative| Registry::for_path(&relative).ok())
        .and_then(|adapter| adapter.parse(&source).ok().map(|tree| (adapter, tree)));

    lines
        .into_iter()
        .map(|(line, text)| {
            let symbol = parsed.as_ref().and_then(|(adapter, tree)| {
                let node = locate::by_line(tree, adapter, line as u32)?;
                let declaration = locate::enclosing_declaration(node, adapter)?;
                symbol_path_of(declaration, adapter, &source).map(|path| path.to_string())
            });
            Suppression { file: display.clone(), line, text: text.to_owned(), symbol }
        })
        .collect()
}
