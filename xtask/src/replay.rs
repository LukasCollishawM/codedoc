use std::collections::BTreeMap;
use std::process::{Command, ExitCode};

use codedoc_anchor::{Anchor, Resolution, Rung};
use codedoc_core::RepoPath;
use codedoc_lang::Registry;

const DETACHMENT_LISTING: usize = 200;

struct Journey {
    from: String,
    to: String,
}

#[derive(Default)]
struct Tally {
    deleted_files: usize,
    renamed_files: usize,
    anchors: usize,
    by_rung: BTreeMap<&'static str, usize>,
    detached: usize,
    reasons: BTreeMap<&'static str, usize>,
    detached_detail: Vec<String>,
    suspicious: Vec<String>,
}

impl Tally {
    fn located(&self) -> usize {
        self.by_rung.values().sum()
    }

    fn survival(&self) -> f64 {
        if self.anchors == 0 {
            return 1.0;
        }
        self.located() as f64 / self.anchors as f64
    }
}

pub fn run(arguments: &[String]) -> ExitCode {
    let repository = value_of(arguments, "--repo").unwrap_or_else(|| ".".to_owned());
    let span: usize =
        value_of(arguments, "--commits").and_then(|value| value.parse().ok()).unwrap_or(200);

    let Some(commits) = revision_span(&repository, span) else {
        println!("replay: no git history available at {repository}, nothing to replay");
        return ExitCode::SUCCESS;
    };
    let (Some(oldest), Some(newest)) = (commits.first(), commits.last()) else {
        println!("replay: fewer than two commits available, nothing to replay");
        return ExitCode::SUCCESS;
    };
    if oldest == newest {
        println!("replay: a single commit is not a history, nothing to replay");
        return ExitCode::SUCCESS;
    }

    let (files, deleted) = changed_files(&repository, oldest, newest);
    if files.is_empty() {
        println!("replay: no supported source files changed across {} commits", commits.len());
        return ExitCode::SUCCESS;
    }
    let renamed = files.iter().filter(|journey| journey.from != journey.to).count();

    let mut tally = Tally { deleted_files: deleted, renamed_files: renamed, ..Default::default() };
    for journey in &files {
        replay_file(&repository, oldest, newest, journey, &mut tally);
    }

    println!("replay: {} commits, {} files", commits.len(), files.len());
    if tally.renamed_files > 0 || tally.deleted_files > 0 {
        println!(
            "  {:<18} {} renamed (replayed across the rename), {} deleted (not replayed)",
            "files moved", tally.renamed_files, tally.deleted_files
        );
    }
    println!("  {:<18} {}", "anchors captured", tally.anchors);
    println!("  {:<18} {} ({:.1}%)", "survived", tally.located(), tally.survival() * 100.0);
    for (rung, count) in &tally.by_rung {
        println!("    {rung:<20} {count}");
    }
    println!("  {:<18} {}", "detached", tally.detached);
    for (reason, count) in &tally.reasons {
        println!("    {reason:<20} {count}");
    }

    if !tally.detached_detail.is_empty() {
        println!();
        println!("  what detached, and why:");
        for entry in tally.detached_detail.iter().take(DETACHMENT_LISTING) {
            println!("    {entry}");
        }
        if let Some(hidden) = tally.detached_detail.len().checked_sub(DETACHMENT_LISTING)
            && hidden > 0
        {
            println!("    ... and {hidden} more, not shown");
        }
    }

    if tally.suspicious.is_empty() {
        println!();
        println!("  {:<18} 0", "suspicious");
        return ExitCode::SUCCESS;
    }

    println!("  {:<18} {}", "suspicious", tally.suspicious.len());
    for entry in tally.suspicious.iter().take(25) {
        println!("    {entry}");
    }
    println!();
    println!("A suspicious result is a confident rung landing on a different symbol.");
    println!("Replay cannot label these automatically; inspect them before trusting a release.");
    ExitCode::from(1)
}

fn replay_file(repository: &str, oldest: &str, newest: &str, journey: &Journey, tally: &mut Tally) {
    let (Ok(path), Ok(arrival)) = (RepoPath::parse(&journey.from), RepoPath::parse(&journey.to))
    else {
        return;
    };
    let Ok(adapter) = Registry::for_path(&arrival) else {
        return;
    };
    let migrated = journey.from != journey.to;
    let (Some(before), Some(after)) =
        (show(repository, oldest, &journey.from), show(repository, newest, &journey.to))
    else {
        return;
    };
    let (Ok(old_tree), Ok(new_tree)) = (adapter.parse(&before), adapter.parse(&after)) else {
        return;
    };

    let before_digests =
        codedoc_anchor::fingerprint::compute_all(old_tree.root_node(), adapter, &before);
    let before_symbols = codedoc_anchor::SymbolTable::build(old_tree.root_node(), adapter, &before);
    let index = codedoc_anchor::FileIndex::build(adapter, &after, &new_tree);

    let mut cursor = old_tree.root_node().walk();
    let mut declarations = Vec::new();
    for child in old_tree.root_node().named_children(&mut cursor) {
        if adapter.declares_symbol(child.kind()) {
            declarations.push(child);
            continue;
        }
        let mut inner = child.walk();
        declarations.extend(
            child.named_children(&mut inner).filter(|node| adapter.declares_symbol(node.kind())),
        );
    }

    for declaration in declarations {
        let anchor = Anchor::capture_with(
            path.clone(),
            adapter,
            &before,
            declaration,
            &before_digests,
            &before_symbols,
        );
        let expected = anchor.symbol.as_ref().map(ToString::to_string);
        tally.anchors += 1;

        let outcome =
            if migrated { index.resolve_after_migration(&anchor) } else { index.resolve(&anchor) };
        match outcome {
            Resolution::Detached(reason) => {
                tally.detached += 1;
                let named = match reason {
                    codedoc_anchor::DetachReason::NoCandidate => "no_candidate",
                    codedoc_anchor::DetachReason::Ambiguous { .. } => "ambiguous",
                    codedoc_anchor::DetachReason::BelowThreshold { .. } => "below_threshold",
                    codedoc_anchor::DetachReason::FileMissing => "file_missing",
                    codedoc_anchor::DetachReason::LanguageUnsupported => "unsupported",
                    _ => "unknown",
                };
                *tally.reasons.entry(named).or_insert(0) += 1;
                tally.detached_detail.push(format!(
                    "{named:<16} {}: {}",
                    journey.to,
                    expected.clone().unwrap_or_else(|| "<anonymous>".to_owned())
                ));
            }
            Resolution::Located(located) => {
                let rung = rung_name(located.rung());
                *tally.by_rung.entry(rung).or_insert(0) += 1;

                let confident = matches!(
                    located.rung(),
                    Rung::ContentIdentity | Rung::StructuralIdentity | Rung::SymbolAndNodePath
                );
                if !confident {
                    continue;
                }
                let landed = located
                    .node_path()
                    .descend(new_tree.root_node())
                    .and_then(|node| codedoc_anchor::symbol_path_of(node, adapter, &after))
                    .map(|path| path.to_string());
                if landed.is_some() && landed != expected {
                    tally.suspicious.push(format!(
                        "{}: {} resolved onto {} at {rung}",
                        journey.to,
                        expected.unwrap_or_else(|| "<anonymous>".to_owned()),
                        landed.unwrap_or_default()
                    ));
                }
            }
            _ => tally.detached += 1,
        }
    }
}

fn rung_name(rung: Rung) -> &'static str {
    match rung {
        Rung::ContentIdentity => "content_identity",
        Rung::StructuralIdentity => "structural_identity",
        Rung::SymbolAndNodePath => "symbol_and_node_path",
        Rung::ContextBracket => "context_bracket",
        Rung::GitMigration => "git_migration",
        Rung::Similarity => "similarity",
        _ => "unknown",
    }
}

fn value_of(arguments: &[String], flag: &str) -> Option<String> {
    let position = arguments.iter().position(|argument| argument == flag)?;
    arguments.get(position + 1).cloned()
}

fn git(repository: &str, arguments: &[&str]) -> Option<String> {
    let output = Command::new("git").arg("-C").arg(repository).args(arguments).output().ok()?;
    output.status.success().then(|| String::from_utf8_lossy(&output.stdout).into_owned())
}

fn show(repository: &str, revision: &str, file: &str) -> Option<String> {
    git(repository, &["show", &format!("{revision}:{file}")])
}

fn revision_span(repository: &str, span: usize) -> Option<Vec<String>> {
    let raw = git(repository, &["rev-list", "--reverse", "-n", &span.to_string(), "HEAD"])?;
    let commits: Vec<String> = raw.lines().map(str::to_owned).collect();
    (!commits.is_empty()).then_some(commits)
}

fn supported(name: &str) -> bool {
    RepoPath::parse(name).is_ok_and(|path| Registry::for_path(&path).is_ok())
}

fn changed_files(repository: &str, oldest: &str, newest: &str) -> (Vec<Journey>, usize) {
    let Some(raw) = git(repository, &["diff", "--name-status", "--find-renames", oldest, newest])
    else {
        return (Vec::new(), 0);
    };
    let mut journeys = Vec::new();
    let mut deleted = 0usize;
    for line in raw.lines() {
        let mut parts = line.split('\t');
        let Some(status) = parts.next() else { continue };
        let Some(first) = parts.next() else { continue };
        if status.starts_with('R') {
            let Some(second) = parts.next() else { continue };
            if supported(first) && supported(second) {
                journeys.push(Journey { from: first.to_owned(), to: second.to_owned() });
            }
            continue;
        }
        if status.starts_with('D') {
            if supported(first) {
                deleted += 1;
            }
            continue;
        }
        if status.starts_with('A') {
            continue;
        }
        if supported(first) {
            journeys.push(Journey { from: first.to_owned(), to: first.to_owned() });
        }
    }
    (journeys, deleted)
}
