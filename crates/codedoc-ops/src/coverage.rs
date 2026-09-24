use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use codedoc_anchor::symbol_path_of;
use codedoc_core::RepoPath;
use codedoc_graph::Graph;
use codedoc_lang::Registry;
use rayon::prelude::*;
use serde_json::json;
use walkdir::WalkDir;

use crate::{Outcome, workspace};

pub(crate) struct FileCoverage {
    file: String,
    pub(crate) declared: Vec<String>,
}

fn is_excluded(path: &Path) -> bool {
    let vendored = path.components().any(|component| {
        let name = component.as_os_str().to_string_lossy();
        matches!(
            name.as_ref(),
            ".git"
                | "target"
                | "node_modules"
                | "vendor"
                | "dist"
                | "build"
                | ".codedoc"
                | "tests"
                | "test"
                | "fuzz"
                | "benches"
                | "examples"
        )
    });
    let named_as_test = path
        .file_stem()
        .map(|stem| {
            let name = stem.to_string_lossy();
            name.ends_with("_test") || name.ends_with("_tests") || name.starts_with("test_")
        })
        .unwrap_or(false);
    vendored || named_as_test
}

pub(crate) fn declarations_in(root: &Path, relative: &RepoPath) -> Option<FileCoverage> {
    let adapter = Registry::for_path(relative).ok()?;
    let source = fs::read_to_string(root.join(relative.as_str())).ok()?;
    let tree = adapter.parse(&source).ok()?;

    let mut declared = BTreeSet::new();
    let mut cursor = tree.root_node().walk();
    let mut descending = true;
    loop {
        if descending {
            let node = cursor.node();
            if adapter.declares_symbol(node.kind())
                && let Some(path) = symbol_path_of(node, adapter, &source)
            {
                declared.insert(path.to_string());
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

    Some(FileCoverage {
        file: relative.as_str().to_owned(),
        declared: declared.into_iter().collect(),
    })
}

pub(crate) fn touched_declarations(root: &Path, files: &[String]) -> Option<(usize, usize)> {
    let found = workspace(root).ok()?;
    let graph = Graph::across(&found).ok()?;
    let documented: BTreeSet<String> = graph
        .active()
        .iter()
        .flat_map(|record| record.content().anchors.iter())
        .filter_map(|entry| entry.anchor.symbol.as_ref().map(ToString::to_string))
        .collect();

    let mut total = 0usize;
    let mut missing = 0usize;
    for file in files {
        let Ok(relative) = RepoPath::parse(file) else {
            continue;
        };
        let Some(entry) = declarations_in(found.root(), &relative) else {
            continue;
        };
        for symbol in &entry.declared {
            total += 1;
            if !documented.contains(symbol) {
                missing += 1;
            }
        }
    }
    Some((total, missing))
}

pub fn coverage(root: &Path, paths: &[String], limit: usize) -> Outcome {
    let found = workspace(root)?;
    let graph = Graph::across(&found)?;

    let documented: BTreeSet<String> = graph
        .active()
        .iter()
        .flat_map(|record| record.content().anchors.iter())
        .filter_map(|entry| entry.anchor.symbol.as_ref().map(ToString::to_string))
        .collect();

    let mut about_the_file: BTreeMap<String, usize> = BTreeMap::new();
    for entry in graph.active().iter().flat_map(|record| record.content().anchors.iter()) {
        if matches!(entry.anchor.subject, codedoc_anchor::Subject::File) {
            *about_the_file.entry(entry.anchor.file.as_str().to_owned()).or_insert(0) += 1;
        }
    }

    let targets: Vec<String> = if paths.is_empty() { vec![".".to_owned()] } else { paths.to_vec() };

    let mut candidates = Vec::new();
    for target in &targets {
        for entry in WalkDir::new(found.root().join(target)).into_iter().filter_map(Result::ok) {
            let path = entry.path();
            if !path.is_file() || is_excluded(path) {
                continue;
            }
            let Ok(relative) = path.strip_prefix(found.root()) else {
                continue;
            };
            let Ok(repo_path) = RepoPath::parse(&relative.to_string_lossy()) else {
                continue;
            };
            if Registry::for_path(&repo_path).is_ok() {
                candidates.push(repo_path);
            }
        }
    }

    let scanned: Vec<FileCoverage> = candidates
        .par_iter()
        .filter_map(|relative| declarations_in(found.root(), relative))
        .collect();

    let mut total = 0usize;
    let mut covered = 0usize;
    let mut by_file: BTreeMap<String, (usize, usize)> = BTreeMap::new();
    let mut uncovered: Vec<(String, String)> = Vec::new();

    for entry in &scanned {
        let mut file_total = 0usize;
        let mut file_covered = 0usize;
        for symbol in &entry.declared {
            file_total += 1;
            if documented.contains(symbol) {
                file_covered += 1;
            } else {
                uncovered.push((entry.file.clone(), symbol.clone()));
            }
        }
        total += file_total;
        covered += file_covered;
        if file_total > 0 {
            by_file.insert(entry.file.clone(), (file_covered, file_total));
        }
    }

    let mut thinnest: Vec<(&String, &(usize, usize))> =
        by_file.iter().filter(|(_, (_, count))| *count > 0).collect();
    thinnest.sort_by(|left, right| {
        let ratio = |value: &(usize, usize)| value.0 as f64 / value.1 as f64;
        ratio(left.1)
            .partial_cmp(&ratio(right.1))
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| right.1.1.cmp(&left.1.1))
    });

    let percent = (covered * 100).checked_div(total).unwrap_or(0);

    Ok(json!({
        "command": "coverage",
        "files": scanned.len(),
        "declarations": total,
        "documented": covered,
        "percent": percent,
        "thinnest": thinnest
            .iter()
            .take(limit)
            .map(|(file, (have, count))| json!({
                "file": file,
                "documented": have,
                "declarations": count,
                "about_the_file": about_the_file.get(*file).copied().unwrap_or(0),
            }))
            .collect::<Vec<_>>(),
        "undocumented_sample": uncovered
            .iter()
            .take(limit)
            .map(|(file, symbol)| json!({"file": file, "symbol": symbol}))
            .collect::<Vec<_>>(),
    }))
}
