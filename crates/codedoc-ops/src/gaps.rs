//! Where has this repository corrected itself without writing down what it learnt?
//!
//! `coverage` ranks files by how little of them is documented, which rewards
//! whichever file happens to be largest. This ranks *declarations* by the
//! evidence in the history that somebody once knew something about them: a
//! correction is proof that the code as written did not say enough, because
//! code that said enough would not have needed correcting.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::process::Command;

use rayon::prelude::*;
use serde_json::json;

use crate::coverage::{declarations_in, documented_symbols, source_files};
use crate::{Outcome, workspace};

const CORRECTIVE: &[&str] = &[
    "fix",
    "fixes",
    "fixed",
    "fixing",
    "bug",
    "bugs",
    "regression",
    "regressions",
    "regressed",
    "revert",
    "reverts",
    "reverted",
    "broke",
    "broken",
    "breaks",
    "crash",
    "crashes",
    "crashed",
    "hang",
    "hangs",
    "deadlock",
    "corruption",
    "corrupted",
    "leak",
    "leaks",
    "leaked",
    "race",
    "races",
];

const FILES_EXAMINED: usize = 24;

fn is_corrective(subject: &str) -> bool {
    subject
        .split(|character: char| !character.is_ascii_alphanumeric())
        .filter(|word| !word.is_empty())
        .any(|word| CORRECTIVE.contains(&word.to_ascii_lowercase().as_str()))
}

#[derive(Default)]
struct History {
    touches: usize,
    corrections: usize,
    authors: BTreeSet<String>,
    latest_correction: Option<String>,
}

impl History {
    fn observe(&mut self, author: &str, subject: &str) {
        self.touches += 1;
        self.authors.insert(author.to_owned());
        if is_corrective(subject) {
            self.corrections += 1;
            if self.latest_correction.is_none() {
                self.latest_correction = Some(subject.to_owned());
            }
        }
    }

    fn rank(&self) -> (usize, usize, usize, usize) {
        let density = if self.touches < 2 { 0 } else { self.corrections * 100 / self.touches };
        (self.corrections, density, self.authors.len(), self.touches)
    }

    fn worth_reporting(&self) -> bool {
        self.corrections > 0 || self.touches > 1
    }
}

fn git(root: &Path, arguments: &[String]) -> Option<String> {
    let output = Command::new("git").arg("-C").arg(root).args(arguments).output().ok()?;
    output.status.success().then(|| String::from_utf8_lossy(&output.stdout).into_owned())
}

fn history_by_file(root: &Path, commits: usize) -> BTreeMap<String, History> {
    let arguments: Vec<String> =
        ["log", "-n", &commits.to_string(), "--format=%x00%an%x00%s", "--name-only", "--no-merges"]
            .iter()
            .map(|argument| (*argument).to_owned())
            .collect();

    let mut by_file: BTreeMap<String, History> = BTreeMap::new();
    let Some(listing) = git(root, &arguments) else {
        return by_file;
    };

    let mut author = String::new();
    let mut subject = String::new();
    for line in listing.lines() {
        if let Some(header) = line.strip_prefix('\u{0}') {
            let mut fields = header.split('\u{0}');
            author = fields.next().unwrap_or_default().to_owned();
            subject = fields.next().unwrap_or_default().to_owned();
            continue;
        }
        if line.is_empty() {
            continue;
        }
        by_file.entry(line.to_owned()).or_default().observe(&author, &subject);
    }
    by_file
}

fn history_of_range(root: &Path, file: &str, first: usize, last: usize, commits: usize) -> History {
    let arguments: Vec<String> = vec![
        "log".to_owned(),
        "-n".to_owned(),
        commits.to_string(),
        "-s".to_owned(),
        "--format=%an%x00%s".to_owned(),
        format!("-L{first},{last}:{file}"),
    ];

    let mut history = History::default();
    let Some(listing) = git(root, &arguments) else {
        return history;
    };
    for line in listing.lines() {
        if line.is_empty() {
            continue;
        }
        let mut fields = line.split('\u{0}');
        let author = fields.next().unwrap_or_default();
        let subject = fields.next().unwrap_or_default();
        history.observe(author, subject);
    }
    history
}

pub fn gaps(root: &Path, paths: &[String], limit: usize, commits: usize) -> Outcome {
    let found = workspace(root)?;
    let graph = codedoc_graph::Graph::across(&found)?;
    let documented = documented_symbols(&graph);

    if !codedoc_verify::history::is_available(found.root()) {
        return Ok(json!({
            "command": "gaps",
            "git": false,
            "commits_scanned": 0,
            "files_examined": 0,
            "undocumented": 0,
            "examined": 0,
            "gaps": [],
        }));
    }

    let by_file = history_by_file(found.root(), commits);

    let mut files: Vec<(String, (usize, usize, usize, usize))> = source_files(found.root(), paths)
        .into_iter()
        .map(|relative| {
            let name = relative.as_str().to_owned();
            let rank = by_file.get(&name).map(History::rank).unwrap_or_default();
            (name, rank)
        })
        .filter(|(_, rank)| rank.3 > 0)
        .collect();
    files.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));
    files.truncate(FILES_EXAMINED);

    let mut candidates: Vec<(String, String, usize, usize)> = Vec::new();
    let mut undocumented = 0usize;
    for (file, _) in &files {
        let Ok(relative) = codedoc_core::RepoPath::parse(file) else {
            continue;
        };
        let Some(entry) = declarations_in(found.root(), &relative) else {
            continue;
        };
        for declaration in entry.declared {
            if documented.contains(&declaration.symbol) {
                continue;
            }
            undocumented += 1;
            candidates.push((
                entry.file.clone(),
                declaration.symbol,
                declaration.first_line,
                declaration.last_line,
            ));
        }
    }

    let mut ranked: Vec<(History, (String, String, usize, usize))> = candidates
        .into_par_iter()
        .map(|candidate| {
            let history =
                history_of_range(found.root(), &candidate.0, candidate.2, candidate.3, commits);
            (history, candidate)
        })
        .filter(|(history, _)| history.worth_reporting())
        .collect();

    ranked.sort_by(|left, right| {
        right.0.rank().cmp(&left.0.rank()).then_with(|| left.1.0.cmp(&right.1.0))
    });

    Ok(json!({
        "command": "gaps",
        "git": true,
        "commits_scanned": commits,
        "files_examined": files.len(),
        "undocumented": undocumented,
        "examined": ranked.len(),
        "gaps": ranked
            .iter()
            .take(limit)
            .map(|(history, (file, symbol, first, last))| json!({
                "file": file,
                "symbol": symbol,
                "first_line": first,
                "last_line": last,
                "commits": history.touches,
                "corrections": history.corrections,
                "authors": history.authors.len(),
                "latest_correction": history.latest_correction,
            }))
            .collect::<Vec<_>>(),
    }))
}
