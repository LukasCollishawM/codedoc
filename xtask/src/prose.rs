use std::fs;
use std::path::Path;
use std::process::ExitCode;

use walkdir::WalkDir;

const COLLAPSED_INDENT: usize = 6;

pub fn run() -> ExitCode {
    let roots = ["crates", "xtask"];
    let mut collapsed = Vec::new();
    let mut scanned = 0usize;

    for root in roots {
        if !Path::new(root).is_dir() {
            continue;
        }
        for entry in WalkDir::new(root).into_iter().filter_map(Result::ok) {
            let path = entry.path();
            if !path.is_file() || path.extension().is_none_or(|extension| extension != "rs") {
                continue;
            }
            if path.components().any(|component| component.as_os_str() == "target") {
                continue;
            }
            scanned += 1;
            let Ok(source) = fs::read_to_string(path) else {
                continue;
            };
            for (line, excerpt) in swallowed_continuations(&source) {
                collapsed.push(format!("{}:{line}: {excerpt}", path.display()));
            }
        }
    }

    if collapsed.is_empty() {
        println!("lint-prose: {scanned} files, no collapsed line continuations");
        return ExitCode::SUCCESS;
    }

    eprintln!("lint-prose: {} run(s) of spaces inside a string literal", collapsed.len());
    eprintln!();
    for entry in &collapsed {
        eprintln!("  {entry}");
    }
    eprintln!();
    eprintln!("A Rust line continuation strips the newline and the indent that follows it.");
    eprintln!("Lose the backslash and that indent stays in the text, so an error message or");
    eprintln!("a test failure reads with a gap in the middle of a sentence. Restore the");
    eprintln!("backslash, or put the string on one line.");
    ExitCode::from(1)
}

fn swallowed_continuations(source: &str) -> Vec<(usize, String)> {
    let mut found = Vec::new();
    let mut inside = false;
    let mut escaped = false;
    let mut after_newline = true;
    let mut run = 0usize;
    let mut line = 1usize;
    let mut current = String::new();

    for glyph in source.chars() {
        if glyph == '\n' {
            line += 1;
            run = 0;
            after_newline = true;
            escaped = false;
            current.clear();
            continue;
        }
        if escaped {
            escaped = false;
            after_newline = glyph == 'n' || glyph == 't' || glyph == 'r';
            run = 0;
            current.push(glyph);
            continue;
        }
        if inside && glyph == '\\' {
            escaped = true;
            current.push(glyph);
            continue;
        }
        if glyph == '"' {
            inside = !inside;
            run = 0;
            after_newline = true;
            current.clear();
            continue;
        }
        if !inside {
            after_newline = false;
            current.clear();
            continue;
        }
        if glyph == ' ' {
            if after_newline {
                continue;
            }
            run += 1;
            if run == COLLAPSED_INDENT {
                let excerpt: Vec<char> = current.chars().rev().take(36).collect();
                let excerpt: String = excerpt.into_iter().rev().collect();
                found.push((line, format!("{}   ...", excerpt.trim_start())));
            }
            current.push(glyph);
            continue;
        }
        run = 0;
        after_newline = false;
        current.push(glyph);
    }
    found
}

#[cfg(test)]
mod tests {
    use super::swallowed_continuations;

    fn spaces(count: usize) -> String {
        " ".repeat(count)
    }

    fn literal(body: &str) -> String {
        format!("const M: &str = \"{body}\";")
    }

    fn continuation(indent: usize) -> String {
        format!("{}\n{}", char::from(92), spaces(indent))
    }

    #[test]
    fn a_run_left_by_a_lost_continuation_is_found_even_on_one_line() {
        let source = literal(&format!("Ignoring it would{}record the default", spaces(10)));
        assert_eq!(swallowed_continuations(&source).len(), 1, "{source}");
    }

    #[test]
    fn an_intact_continuation_is_not_a_finding() {
        let source = literal(&format!("Ignoring it would {}record the default", continuation(9)));
        assert!(swallowed_continuations(&source).is_empty(), "{source}");
    }

    #[test]
    fn indentation_at_the_start_of_a_continued_line_is_not_a_run() {
        let source = literal(&format!("first {}second {}third", continuation(8), continuation(8)));
        assert!(swallowed_continuations(&source).is_empty(), "{source}");
    }

    #[test]
    fn code_outside_a_literal_is_never_a_finding() {
        let source = format!(
            "fn wide() {{
    let x = 1;
{}let y = 2;
}}
",
            spaces(12)
        );
        assert!(swallowed_continuations(&source).is_empty(), "{source}");
    }
}
