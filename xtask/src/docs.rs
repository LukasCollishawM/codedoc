use std::collections::BTreeSet;
use std::fs;
use std::process::{Command, ExitCode};

pub fn run() -> ExitCode {
    let Some(declared) = subcommands() else {
        eprintln!("lint-docs: could not enumerate CLI subcommands");
        return ExitCode::from(1);
    };
    let Ok(reference) = fs::read_to_string("docs/cli.md") else {
        eprintln!("lint-docs: docs/cli.md is missing");
        return ExitCode::from(1);
    };

    let documented: BTreeSet<String> = declared
        .iter()
        .filter(|name| reference.contains(&format!("codedoc {name}")))
        .cloned()
        .collect();
    let undocumented: Vec<&String> =
        declared.iter().filter(|name| !documented.contains(*name)).collect();

    if undocumented.is_empty() {
        println!("lint-docs: {} subcommands, all documented in docs/cli.md", declared.len());
        return ExitCode::SUCCESS;
    }

    eprintln!("lint-docs: {} subcommand(s) missing from docs/cli.md", undocumented.len());
    eprintln!();
    for name in &undocumented {
        eprintln!("  codedoc {name}");
    }
    eprintln!();
    eprintln!("A command that exists but is undocumented may as well not exist.");
    eprintln!("Add it to docs/cli.md, in this commit.");
    ExitCode::from(1)
}

fn subcommands() -> Option<BTreeSet<String>> {
    let output = Command::new("cargo")
        .args(["run", "--quiet", "-p", "codedoc-cli", "--", "--help"])
        .output()
        .ok()?;
    let help = String::from_utf8_lossy(&output.stdout);
    let body = help.split("Commands:").nth(1)?;
    let mut names = BTreeSet::new();
    for line in body.lines() {
        if line.starts_with("Options:") || line.trim().is_empty() {
            if line.starts_with("Options:") {
                break;
            }
            continue;
        }
        let trimmed = line.trim_start();
        if !line.starts_with("  ") || trimmed.starts_with('-') {
            continue;
        }
        let Some(name) = trimmed.split_whitespace().next() else {
            continue;
        };
        if name == "help" {
            continue;
        }
        names.insert(name.to_owned());
    }
    (!names.is_empty()).then_some(names)
}
