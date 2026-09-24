use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::process::{Command, ExitCode};

const DOCUMENTS: [&str; 7] = [
    "README.md",
    "AGENTS.md",
    "CLAUDE.md",
    "CONTRIBUTING.md",
    "SECURITY.md",
    "docs/cli.md",
    "docs/spec/format.md",
];

struct Invocation {
    document: String,
    text: String,
    subcommand: String,
    flags: BTreeSet<String>,
}

pub fn run() -> ExitCode {
    let Some(subcommands) = codedoc_subcommands() else {
        eprintln!("lint-invocations: could not enumerate codedoc subcommands");
        return ExitCode::from(1);
    };
    let xtask = xtask_subcommands();

    let mut found = Vec::new();
    for document in DOCUMENTS {
        let Ok(body) = fs::read_to_string(document) else {
            continue;
        };
        found.extend(invocations_in(document, &body));
    }

    let mut help: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut broken = Vec::new();
    for invocation in &found {
        if invocation.document == "docs/cli.md" && invocation.subcommand == "xtask" {
            continue;
        }
        if invocation.subcommand.starts_with("xtask:") {
            let name = invocation.subcommand.trim_start_matches("xtask:");
            if !xtask.contains(name) {
                broken.push(format!(
                    "{}: `cargo xtask {name}` is not a subcommand xtask has",
                    invocation.document
                ));
            }
            continue;
        }
        if !subcommands.contains(&invocation.subcommand) {
            broken.push(format!(
                "{}: `{}` names no codedoc subcommand",
                invocation.document, invocation.text
            ));
            continue;
        }
        let accepted = help
            .entry(invocation.subcommand.clone())
            .or_insert_with(|| flags_accepted_by(&invocation.subcommand));
        for flag in &invocation.flags {
            if !accepted.contains(flag) {
                broken.push(format!(
                    "{}: `{}` passes --{flag}, which `codedoc {} --help` does not list",
                    invocation.document, invocation.text, invocation.subcommand
                ));
            }
        }
    }

    if broken.is_empty() {
        println!(
            "lint-invocations: {} command invocations across {} documents, all real",
            found.len(),
            DOCUMENTS.len()
        );
        return ExitCode::SUCCESS;
    }

    eprintln!("lint-invocations: {} invocation(s) the binaries do not support", broken.len());
    eprintln!();
    for entry in &broken {
        eprintln!("  {entry}");
    }
    eprintln!();
    eprintln!("A command written in the documentation is an instruction somebody will run.");
    eprintln!("One that does not exist wastes their time and tells them the page is stale.");
    eprintln!("Correct the prose, or add the flag it promises.");
    ExitCode::from(1)
}

fn invocations_in(document: &str, body: &str) -> Vec<Invocation> {
    let mut found = Vec::new();
    for chunk in body.split('`').skip(1).step_by(2) {
        let text = chunk.trim();
        let rest = if let Some(rest) = text.strip_prefix("cargo xtask ") {
            let name = rest.split_whitespace().next().unwrap_or_default();
            if !name.is_empty() {
                found.push(Invocation {
                    document: document.to_owned(),
                    text: text.to_owned(),
                    subcommand: format!("xtask:{name}"),
                    flags: BTreeSet::new(),
                });
            }
            continue;
        } else if let Some(rest) = text.strip_prefix("codedoc ") {
            rest
        } else {
            continue;
        };

        let mut words = rest.split_whitespace();
        let Some(subcommand) = words.next() else {
            continue;
        };
        if !subcommand.chars().all(|glyph| glyph.is_ascii_lowercase() || glyph == '-') {
            continue;
        }
        let flags = words
            .filter_map(|word| word.strip_prefix("--"))
            .map(|flag| {
                flag.split(['=', '<', '>']).next().unwrap_or(flag).trim_matches('`').to_owned()
            })
            .filter(|flag| {
                !flag.is_empty() && flag.chars().all(|g| g.is_ascii_lowercase() || g == '-')
            })
            .collect();
        found.push(Invocation {
            document: document.to_owned(),
            text: text.to_owned(),
            subcommand: subcommand.to_owned(),
            flags,
        });
    }
    found
}

fn codedoc_subcommands() -> Option<BTreeSet<String>> {
    let help = run_cli(&["--help"])?;
    Some(parse_subcommands(&help))
}

fn xtask_subcommands() -> BTreeSet<String> {
    let Ok(source) = fs::read_to_string("xtask/src/main.rs") else {
        return BTreeSet::new();
    };
    source
        .split('"')
        .skip(1)
        .step_by(2)
        .filter(|token| {
            !token.is_empty()
                && token.chars().all(|glyph| glyph.is_ascii_lowercase() || glyph == '-')
        })
        .map(str::to_owned)
        .collect()
}

fn flags_accepted_by(subcommand: &str) -> BTreeSet<String> {
    let Some(help) = run_cli(&[subcommand, "--help"]) else {
        return BTreeSet::new();
    };
    help.split_whitespace()
        .filter_map(|word| word.strip_prefix("--"))
        .map(|flag| flag.trim_end_matches(',').to_owned())
        .filter(|flag| !flag.is_empty())
        .collect()
}

fn run_cli(arguments: &[&str]) -> Option<String> {
    let mut command = Command::new("cargo");
    command.args(["run", "--quiet", "-p", "codedoc-cli", "--"]);
    command.args(arguments);
    let output = command.output().ok()?;
    Some(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn parse_subcommands(help: &str) -> BTreeSet<String> {
    let mut found = BTreeSet::new();
    let mut inside = false;
    for line in help.lines() {
        if line.starts_with("Commands:") {
            inside = true;
            continue;
        }
        if inside {
            if line.trim().is_empty() || line.starts_with("Options:") {
                break;
            }
            if let Some(name) = line.split_whitespace().next()
                && name.chars().all(|glyph| glyph.is_ascii_lowercase() || glyph == '-')
            {
                found.insert(name.to_owned());
            }
        }
    }
    found
}
