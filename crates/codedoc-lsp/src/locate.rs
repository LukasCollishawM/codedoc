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
