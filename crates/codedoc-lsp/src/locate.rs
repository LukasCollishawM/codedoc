use std::fs;
use std::path::Path;

use codedoc_anchor::{Resolution, Resolver};
use codedoc_core::RepoPath;
use codedoc_graph::Graph;
use codedoc_lang::Registry;
use codedoc_ledger::{Ledger, Record};

pub struct Located {
    pub markdown: String,
    pub start_line: u32,
    pub end_line: u32,
}

pub struct Concern {
    pub start_line: u32,
    pub end_line: u32,
    pub severity: Severity,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Stale,
    Detached,
}

pub struct Lens {
    pub line: u32,
    pub title: String,
    pub record: String,
}

struct Placement {
    record: Record,
    start_line: u32,
    end_line: u32,
}

pub fn records_at(root: &Path, file: &Path, line: u32) -> Option<Located> {
    let placements = resolve_file(root, file)?;
    let covering: Vec<&Record> = placements
        .iter()
        .filter(|placement| (placement.start_line..=placement.end_line).contains(&line))
        .map(|placement| &placement.record)
        .collect();
    if covering.is_empty() {
        return None;
    }
    let span = placements
        .iter()
        .filter(|placement| (placement.start_line..=placement.end_line).contains(&line))
        .fold((u32::MAX, 0u32), |(start, end), placement| {
            (start.min(placement.start_line), end.max(placement.end_line))
        });
    Some(Located {
        markdown: codedoc_render::hover_markdown(&covering),
        start_line: span.0,
        end_line: span.1,
    })
}

pub fn lenses_for(root: &Path, file: &Path) -> Vec<Lens> {
    let Some(placements) = resolve_file(root, file) else {
        return Vec::new();
    };
    placements
        .into_iter()
        .map(|placement| Lens {
            line: placement.start_line,
            title: codedoc_render::lens_title(&placement.record),
            record: placement.record.id().to_string(),
        })
        .collect()
}

pub fn concerns_for(root: &Path, file: &Path) -> Vec<Concern> {
    let relative = file.strip_prefix(root).unwrap_or(file);
    let Ok(repo_path) = RepoPath::parse(&relative.to_string_lossy()) else {
        return Vec::new();
    };
    let Ok(workspace) = codedoc_ledger::Workspace::discover(root) else {
        return Vec::new();
    };
    let Ok(report) = codedoc_verify::Verifier::new(workspace.root())
        .run_over(&workspace, &[repo_path.as_str().to_owned()])
    else {
        return Vec::new();
    };

    report
        .findings
        .iter()
        .filter_map(|finding| {
            let severity = match finding.status {
                codedoc_verify::Status::Stale => Severity::Stale,
                codedoc_verify::Status::Detached => Severity::Detached,
                _ => return None,
            };
            let (start, end) = match finding.resolution.located() {
                Some(located) => (located.range().start_line, located.range().end_line),
                None => (finding.recorded_range.start_line, finding.recorded_range.end_line),
            };
            let headline = match severity {
                Severity::Stale => match finding.drift {
                    Some(amount) => format!(
                        "codedoc: this {} may no longer describe the code ({amount}% \
                         changed). Read it, then `codedoc affirm` if it still holds, or \
                         `codedoc supersede` if it does not.",
                        finding.kind
                    ),
                    None => format!(
                        "codedoc: this {} resolved only through a weak signal. Read it, \
                         then `codedoc affirm` if it still holds, or `codedoc supersede` \
                         if it does not.",
                        finding.kind
                    ),
                },
                Severity::Detached => format!(
                    "codedoc: the code this {} described could not be found, and codedoc will \
                     not guess. Place it with `codedoc resolve`.",
                    finding.kind
                ),
            };
            let message = format!("{headline}\n\n{}", finding.claim);
            Some(Concern { start_line: start, end_line: end, severity, message })
        })
        .collect()
}

fn resolve_file(root: &Path, file: &Path) -> Option<Vec<Placement>> {
    let relative = file.strip_prefix(root).unwrap_or(file);
    let repo_path = RepoPath::parse(&relative.to_string_lossy()).ok()?;
    let ledger = Ledger::open(root).ok()?;
    let graph = Graph::load(&ledger).ok()?;
    let adapter = Registry::for_path(&repo_path).ok()?;
    let source = fs::read_to_string(file).ok()?;
    let tree = adapter.parse(&source).ok()?;
    let resolver = Resolver::new(adapter);

    let mut placements = Vec::new();
    for record in graph.in_file(repo_path.as_str()) {
        for entry in &record.content().anchors {
            if entry.anchor.file.as_str() != repo_path.as_str() {
                continue;
            }
            let resolution = resolver
                .resolve(&entry.anchor, &source, &tree)
                .require(record.content().kind.required_confidence());
            if let Resolution::Located(located) = resolution {
                placements.push(Placement {
                    record: record.clone(),
                    start_line: located.range().start_line,
                    end_line: located.range().end_line,
                });
            }
        }
    }
    Some(placements)
}
