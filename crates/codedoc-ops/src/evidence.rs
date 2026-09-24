use std::collections::BTreeSet;
use std::path::Path;
use std::process::Command;

use codedoc_ledger::Evidence;
use serde_json::{Value, json};

use crate::{OpsError, workspace};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Standing {
    Present,
    Missing,
    Withdrawn,
    Unchecked,
}

impl Standing {
    fn as_str(self) -> &'static str {
        match self {
            Standing::Present => "present",
            Standing::Missing => "missing",
            Standing::Withdrawn => "withdrawn",
            Standing::Unchecked => "unchecked",
        }
    }

    fn is_broken(self) -> bool {
        matches!(self, Standing::Missing | Standing::Withdrawn)
    }
}

pub fn evidence(root: &Path) -> Result<(Value, i32), OpsError> {
    let found = workspace(root)?;
    let repository = found.root().to_path_buf();
    let graph = codedoc_graph::Graph::across(&found)?;

    let known: BTreeSet<String> =
        graph.all().iter().map(|record| record.id().to_string()).collect();
    let standing: BTreeSet<String> =
        graph.active().iter().map(|record| record.id().to_string()).collect();

    let mut rows: Vec<Value> = Vec::new();
    let mut broken = 0usize;
    let mut cited = 0usize;

    for record in graph.active() {
        for item in &record.content().evidence {
            cited += 1;
            let (verdict, citation, note) = assess(&repository, item, &known, &standing);
            if !verdict.is_broken() {
                continue;
            }
            broken += 1;
            rows.push(json!({
                "record": record.id().to_string(),
                "kind": record.kind().as_str(),
                "claim": record.content().body.claim,
                "file": record.subject().map(|anchor| anchor.file.as_str().to_owned()),
                "citation": citation,
                "standing": verdict.as_str(),
                "note": note,
            }));
        }
    }

    rows.sort_by(|left, right| left["record"].as_str().cmp(&right["record"].as_str()));
    let code = i32::from(broken > 0) * 2;
    Ok((
        json!({
            "command": "evidence",
            "citations": cited,
            "broken": broken,
            "records": rows,
        }),
        code,
    ))
}

fn assess(
    root: &Path,
    item: &Evidence,
    known: &BTreeSet<String>,
    standing: &BTreeSet<String>,
) -> (Standing, String, &'static str) {
    match item {
        Evidence::Document(path) => {
            let citation = format!("doc:{path}");
            if root.join(path).exists() {
                (Standing::Present, citation, "")
            } else {
                (Standing::Missing, citation, "the document this cites is not in the repository")
            }
        }
        Evidence::Record(id) => {
            let rendered = id.to_string();
            let citation = format!("record:{rendered}");
            if !known.contains(&rendered) {
                (Standing::Missing, citation, "the record this cites is not in this workspace")
            } else if !standing.contains(&rendered) {
                (
                    Standing::Withdrawn,
                    citation,
                    "the record this cites was retracted, so the support it lent is gone",
                )
            } else {
                (Standing::Present, citation, "")
            }
        }
        Evidence::GitRevision(revision) => {
            let citation = format!("git:{revision}");
            match revision_exists(root, revision.as_str()) {
                Some(true) => (Standing::Present, citation, ""),
                Some(false) => (
                    Standing::Missing,
                    citation,
                    "this revision is not in the repository; history may have been rewritten",
                ),
                None => (Standing::Unchecked, citation, "git could not be consulted here"),
            }
        }
        Evidence::Test(name) => {
            let citation = format!("test:{name}");
            match mentions(root, name) {
                Some(true) => (Standing::Present, citation, ""),
                Some(false) => {
                    (Standing::Missing, citation, "no test by this name appears in the repository")
                }
                None => (Standing::Unchecked, citation, "git could not be consulted here"),
            }
        }
        Evidence::Url(address) => (
            Standing::Unchecked,
            format!("url:{address}"),
            "codedoc does not fetch URLs, so this citation is recorded but never checked",
        ),
        _ => (Standing::Unchecked, String::from("unrecognised"), "citation kind not understood"),
    }
}

fn revision_exists(root: &Path, revision: &str) -> Option<bool> {
    let status = ran(root, &["cat-file", "-e", &format!("{revision}^{{commit}}")])?;
    Some(status == 0)
}

fn mentions(root: &Path, name: &str) -> Option<bool> {
    match ran(root, &["grep", "--untracked", "--fixed-strings", "--quiet", "--", name])? {
        0 => Some(true),
        1 => Some(false),
        _ => None,
    }
}

fn ran(root: &Path, arguments: &[&str]) -> Option<i32> {
    if !root.join(".git").exists() {
        return None;
    }
    Command::new("git").arg("-C").arg(root).args(arguments).output().ok()?.status.code()
}
