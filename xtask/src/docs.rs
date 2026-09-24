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
        let options = options_are_documented(&declared, &reference);
        if options != ExitCode::SUCCESS {
            return options;
        }
        return instructions_match_agents_md();
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

fn options_are_documented(declared: &BTreeSet<String>, reference: &str) -> ExitCode {
    let mut orphaned = Vec::new();
    let mut counted = 0usize;

    for name in declared {
        let Some(help) = help_for(name) else {
            continue;
        };
        let Some(section) = section_for(name, reference) else {
            continue;
        };
        for flag in flags_in(&help) {
            if matches!(flag.as_str(), "help" | "version" | "root" | "json" | "scope") {
                continue;
            }
            counted += 1;
            if !section.contains(&format!("--{flag}")) {
                orphaned.push(format!("codedoc {name} --{flag}"));
            }
        }
    }

    if orphaned.is_empty() {
        println!("lint-docs: {counted} options, all named in their own section");
        return ExitCode::SUCCESS;
    }

    eprintln!("lint-docs: {} option(s) that docs/cli.md never mentions", orphaned.len());
    eprintln!();
    for entry in &orphaned {
        eprintln!("  {entry}");
    }
    eprintln!();
    eprintln!("An option nobody documents is one nobody finds. Name it in that command's");
    eprintln!("own section, in this commit.");
    ExitCode::from(1)
}

fn help_for(name: &str) -> Option<String> {
    let output = Command::new("cargo")
        .args(["run", "--quiet", "-p", "codedoc-cli", "--", name, "--help"])
        .output()
        .ok()?;
    Some(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn section_for(name: &str, reference: &str) -> Option<String> {
    let heading = format!("### `codedoc {name}");
    let start = reference.find(&heading)?;
    let rest = &reference[start + heading.len()..];
    let end = rest
        .find(
            "
### ",
        )
        .unwrap_or(rest.len());
    Some(rest[..end].to_owned())
}

fn flags_in(help: &str) -> BTreeSet<String> {
    let mut found = BTreeSet::new();
    let mut rest = help;
    while let Some(position) = rest.find("--") {
        rest = &rest[position + 2..];
        let name: String =
            rest.chars().take_while(|glyph| glyph.is_ascii_lowercase() || *glyph == '-').collect();
        if name.len() > 1 {
            found.insert(name);
        }
    }
    found
}

fn instructions_match_agents_md() -> ExitCode {
    let Ok(server) = fs::read_to_string("crates/codedoc-mcp/src/main.rs") else {
        eprintln!("lint-docs: the MCP server source is missing");
        return ExitCode::from(1);
    };
    let Ok(guide) = fs::read_to_string("AGENTS.md") else {
        eprintln!("lint-docs: AGENTS.md is missing");
        return ExitCode::from(1);
    };
    let Some(instructions) = literal(&normalise(&server)) else {
        eprintln!("lint-docs: could not read INSTRUCTIONS from the MCP server");
        return ExitCode::from(1);
    };

    let guide = normalise(&guide);
    let quoted: String = guide
        .lines()
        .filter(|line| line.starts_with('>'))
        .map(|line| line.trim_start_matches('>').trim())
        .collect::<Vec<_>>()
        .join(" ");
    let quoted = collapse(&quoted.replace('`', ""));

    let missing: Vec<&str> = instructions
        .split(
            "

",
        )
        .map(str::trim)
        .filter(|paragraph| !paragraph.is_empty())
        .filter(|paragraph| !quoted.contains(&collapse(paragraph)))
        .collect();

    if missing.is_empty() {
        println!("lint-docs: AGENTS.md quotes the server instructions verbatim");
        return tool_count_is_stated_correctly(&server);
    }

    eprintln!("lint-docs: AGENTS.md quotes the server instructions, and they have drifted");
    eprintln!();
    for paragraph in &missing {
        eprintln!("  missing: {}", collapse(paragraph).chars().take(90).collect::<String>());
    }
    eprintln!();
    eprintln!("AGENTS.md tells a reader this is what every agent is sent. If it is not,");
    eprintln!("the page is describing a product nobody is running. Update the blockquote.");
    ExitCode::from(1)
}

const NUMERALS: [&str; 30] = [
    "zero",
    "one",
    "two",
    "three",
    "four",
    "five",
    "six",
    "seven",
    "eight",
    "nine",
    "ten",
    "eleven",
    "twelve",
    "thirteen",
    "fourteen",
    "fifteen",
    "sixteen",
    "seventeen",
    "eighteen",
    "nineteen",
    "twenty",
    "twenty-one",
    "twenty-two",
    "twenty-three",
    "twenty-four",
    "twenty-five",
    "twenty-six",
    "twenty-seven",
    "twenty-eight",
    "twenty-nine",
];

const EDITOR_LANGUAGES: [(&str, &str); 8] = [
    ("rust", "rust"),
    ("python", "python"),
    ("csharp", "csharp"),
    ("go", "go"),
    ("java", "java"),
    ("typescript", "typescript"),
    ("tsx", "typescriptreact"),
    ("cpp", "cpp"),
];

fn the_editor_activates_for_every_language() -> ExitCode {
    let Ok(registry) = fs::read_to_string("crates/codedoc-lang/src/registry.rs") else {
        println!("lint-docs: no language registry to compare the editor against");
        return ExitCode::SUCCESS;
    };
    let Ok(manifest) = fs::read_to_string("editors/vscode/package.json") else {
        println!("lint-docs: no editor extension to check");
        return ExitCode::SUCCESS;
    };

    let mut silent = Vec::new();
    for (adapter, editor) in EDITOR_LANGUAGES {
        if !registry.contains(&format!("name: \"{adapter}\"")) {
            continue;
        }
        if !manifest.contains(&format!("onLanguage:{editor}")) {
            silent.push(format!("{adapter} (onLanguage:{editor})"));
        }
    }

    if silent.is_empty() {
        println!("lint-docs: the editor extension activates for every supported language");
        return ExitCode::SUCCESS;
    }

    eprintln!("lint-docs: the extension never activates for {} language(s)", silent.len());
    eprintln!();
    for entry in &silent {
        eprintln!("  {entry}");
    }
    eprintln!();
    eprintln!("Someone opening one of these files gets no hovers, no lenses and no");
    eprintln!("diagnostics, and nothing tells them why. Add it to activationEvents.");
    ExitCode::from(1)
}

fn tool_count_is_stated_correctly(server: &str) -> ExitCode {
    let declared = server.matches("#[tool(").count();
    let Ok(readme) = fs::read_to_string("README.md") else {
        eprintln!("lint-docs: README.md is missing");
        return ExitCode::from(1);
    };
    let Some(sentence) = readme.lines().find(|line| line.contains("tools, discovers them")) else {
        println!("lint-docs: README states no tool count");
        return ExitCode::SUCCESS;
    };
    let spelled = NUMERALS.get(declared).copied().unwrap_or("");
    if !spelled.is_empty() && sentence.contains(spelled) {
        println!("lint-docs: README states {declared} agent tools, which is how many there are");
        return the_editor_activates_for_every_language();
    }

    eprintln!("lint-docs: the MCP server exposes {declared} tools and README.md says otherwise");
    eprintln!();
    eprintln!("  {}", sentence.trim());
    eprintln!();
    eprintln!("A number in the README is a claim about the product, and a wrong one is");
    eprintln!("read by everyone who arrives. Say \"{spelled}\", or drop the count.");
    ExitCode::from(1)
}

fn literal(source: &str) -> Option<String> {
    let marker = "const INSTRUCTIONS: &str = \"";
    let start = source.find(marker)?;
    let body = &source[start + marker.len()..];
    let end = body.find("\";")?;
    let mut out = String::new();
    let mut rest = &body[..end];
    while let Some(position) = rest.find('\\') {
        out.push_str(&rest[..position]);
        let after = &rest[position + 1..];
        let after = after.strip_prefix('\n').unwrap_or(after);
        rest = after.trim_start_matches(' ');
    }
    out.push_str(rest);
    Some(out)
}

fn normalise(text: &str) -> String {
    text.replace("\r\n", "\n")
}

fn collapse(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
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
