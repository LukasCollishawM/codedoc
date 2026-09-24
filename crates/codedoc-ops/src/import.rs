use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use codedoc_anchor::Anchor;
use codedoc_core::RepoPath;
use codedoc_lang::{Adapter, Registry};
use codedoc_ledger::{
    AnchorRole, Assurance, Author, Body, Kind, Lifecycle, RecordContent, Role, SCHEMA_VERSION,
    Scope, Timestamp,
};

use crate::{OpsError, Outcome, writable};
use serde_json::{Value, json};
use tree_sitter::Node;
use walkdir::WalkDir;

const MINIMUM_CLAIM_LENGTH: usize = 12;

const DIRECTIVES: &[&str] = &[
    "type: ignore",
    "noqa",
    "pylint:",
    "pyright:",
    "mypy:",
    "ruff:",
    "pragma:",
    "fmt: off",
    "fmt: on",
    "fmt: skip",
    "isort:",
    "-*- coding:",
    "eslint-disable",
    "eslint-enable",
    "@ts-ignore",
    "@ts-expect-error",
    "@ts-nocheck",
    "prettier-ignore",
    "istanbul ignore",
    "c8 ignore",
    "deno-lint-ignore",
    "biome-ignore",
    "sourcemappingurl",
    "nolint",
    "go:generate",
    "go:build",
    "go:embed",
    "go:noinline",
    "go:linkname",
    "+build",
    "clang-format off",
    "clang-format on",
    "spdx-license-identifier",
    "@generated",
    "checkstyle:",
    "codeql",
    "vim:",
    "local variables:",
];

const REASON_SEPARATORS: &[&str] = &[" -- ", " — ", " # ", " // "];

pub(crate) fn without_directive(claim: &str) -> Option<String> {
    let lowered = claim.to_ascii_lowercase();
    let matched = DIRECTIVES.iter().find(|directive| lowered.starts_with(*directive))?;
    let rest = &claim[matched.len()..];
    let reason = REASON_SEPARATORS
        .iter()
        .filter_map(|separator| rest.split_once(separator))
        .map(|(_, reason)| reason.trim())
        .find(|reason| !reason.is_empty())
        .unwrap_or("");
    Some(reason.to_owned())
}
const ADJACENCY_LINES: usize = 2;

struct Harvested {
    file: String,
    line: u32,
    kind: Kind,
    claim: String,
    symbol: Option<String>,
    content: RecordContent,
}

pub fn import(
    root: &Path,
    scope: Option<Scope>,
    paths: &[String],
    write: bool,
    limit: Option<usize>,
) -> Outcome {
    let ledger = writable(root, scope)?;
    let root = ledger.root().to_path_buf();
    let root = root.as_path();
    let targets: Vec<String> = if paths.is_empty() {
        vec![".".to_owned()]
    } else {
        crate::require_paths(root, paths, root)?
    };

    let revision = crate::revision::head_revision(root);
    let project = crate::coverage::ProjectFiles::of(root);
    let mut harvested = Vec::new();
    let mut files_scanned = 0usize;
    let mut files_unreadable = 0usize;

    for target in &targets {
        for entry in WalkDir::new(root.join(target)).into_iter().filter_map(Result::ok) {
            let path = entry.path();
            if !path.is_file() || is_excluded(path) {
                continue;
            }
            let Ok(relative) = path.strip_prefix(root) else {
                continue;
            };
            if project.excludes(relative) {
                continue;
            }
            let Ok(repo_path) = RepoPath::parse(&relative.to_string_lossy()) else {
                continue;
            };
            let Ok(adapter) = Registry::for_path(&repo_path) else {
                continue;
            };
            let Ok(source) = fs::read_to_string(path) else {
                files_unreadable += 1;
                continue;
            };
            files_scanned += 1;
            harvested.extend(harvest_file(&repo_path, adapter, &source, revision.clone()));
            if limit.is_some_and(|cap| harvested.len() >= cap) {
                break;
            }
        }
    }

    if let Some(cap) = limit {
        harvested.truncate(cap);
    }

    let mut by_kind: BTreeMap<String, usize> = BTreeMap::new();
    for item in &harvested {
        *by_kind.entry(item.kind.as_str()).or_insert(0) += 1;
    }

    let about_the_file = harvested
        .iter()
        .filter(|item| {
            item.content
                .anchors
                .iter()
                .any(|entry| matches!(entry.anchor.subject, codedoc_anchor::Subject::File))
        })
        .count();
    let unnamed = harvested.len()
        - about_the_file
        - harvested.iter().filter(|item| item.symbol.is_some()).count();

    let sample: Vec<Value> = harvested
        .iter()
        .take(10)
        .map(|item| {
            json!({
                "file": item.file,
                "line": item.line,
                "kind": item.kind.as_str(),
                "symbol": item.symbol,
                "claim": item.claim.chars().take(110).collect::<String>(),
            })
        })
        .collect();

    let mut written = 0usize;
    if write && !harvested.is_empty() {
        let drafts: Vec<RecordContent> = harvested.into_iter().map(|item| item.content).collect();
        let records = ledger
            .append_batch(drafts)
            .map_err(|source| OpsError::Ledger { detail: source.to_string() })?;
        codedoc_index::Index::append_many(&ledger, &records)
            .map_err(|source| OpsError::Index { detail: source.to_string() })?;
        written = records.len();
    }

    Ok(json!({
            "command": "import",
            "files_scanned": files_scanned,
            "files_unreadable": files_unreadable,
            "candidates": by_kind.values().sum::<usize>(),
            "by_kind": by_kind,
            "written": written,
            "about_the_file": about_the_file,
            "unnamed": unnamed,
            "dry_run": !write,
            "sample": sample,
            "scope": ledger.scope().as_str(),
    }))
}

fn is_excluded(path: &Path) -> bool {
    path.components().any(|component| {
        let name = component.as_os_str().to_string_lossy();
        matches!(
            name.as_ref(),
            ".git" | "target" | "node_modules" | "vendor" | "dist" | "build" | ".codedoc"
        )
    })
}

fn harvest_file(
    repo_path: &RepoPath,
    adapter: &Adapter,
    source: &str,
    revision: Option<codedoc_core::GitRev>,
) -> Vec<Harvested> {
    let source = source.to_owned();
    let Ok(tree) = adapter.parse(&source) else {
        return Vec::new();
    };

    let digests = codedoc_anchor::fingerprint::compute_all(tree.root_node(), adapter, &source);
    let symbols = codedoc_anchor::SymbolTable::build(tree.root_node(), adapter, &source);
    let first_declaration = first_declaration_row(tree.root_node(), adapter);
    let mut blocks: Vec<Vec<Node<'_>>> = Vec::new();
    collect_comment_blocks(tree.root_node(), adapter, &mut blocks);

    let mut harvested = Vec::new();
    for block in blocks {
        let Some(first) = block.first() else { continue };
        let Some(last) = block.last() else { continue };

        let text = block
            .iter()
            .filter_map(|node| node.utf8_text(source.as_bytes()).ok())
            .filter(|raw| !is_shebang(raw))
            .map(clean_comment)
            .collect::<Vec<_>>()
            .join("\n");

        let text = crate::doc_tags::break_before_first_block_tag(&reflow(&text));
        let carried = match without_directive(text.trim()) {
            Some(reason) => reason,
            None => text.trim().to_owned(),
        };
        let (claim, detail) = split_claim(&carried);
        if claim.chars().count() < MINIMUM_CLAIM_LENGTH || is_decorative(&claim) {
            continue;
        }
        if is_licence_header(&claim) {
            continue;
        }
        let kind = infer_kind(&claim);
        if kind == Kind::Explanation && is_data_sample(&claim) {
            continue;
        }

        let target = documented_node(*last, adapter);
        let anchor = if documents_the_file(&block, target, first_declaration, adapter, &source) {
            Anchor::capture_file_with(
                repo_path.clone(),
                adapter,
                &source,
                tree.root_node(),
                &digests,
                &symbols,
            )
        } else {
            let Some(target) = target else { continue };
            Anchor::capture_with(repo_path.clone(), adapter, &source, target, &digests, &symbols)
        };
        harvested.push(Harvested {
            file: repo_path.as_str().to_owned(),
            line: first.start_position().row as u32 + 1,
            kind,
            claim: claim.clone(),
            symbol: anchor.symbol.as_ref().map(ToString::to_string),
            content: RecordContent {
                schema: SCHEMA_VERSION,
                kind,
                anchors: vec![AnchorRole { role: Role::Subject, anchor }],
                body: Body { claim, detail },
                evidence: Vec::new(),
                assurance: Assurance::Inferred,
                author: Author::Analyzer { name: "comment-import".to_owned() },
                code_revision: revision.clone(),
                created: Timestamp::now(),
                lifecycle: Lifecycle::Active,
                parent: None,
                chain: None,
                unrecognised: BTreeMap::new(),
            },
        });
    }
    harvested
}

fn collect_comment_blocks<'tree>(
    root: Node<'tree>,
    adapter: &Adapter,
    blocks: &mut Vec<Vec<Node<'tree>>>,
) {
    let mut stack = vec![root];
    while let Some(node) = stack.pop() {
        let mut pending: Vec<Node<'tree>> = Vec::new();
        let mut descend: Vec<Node<'tree>> = Vec::new();
        let mut cursor = node.walk();

        for child in node.named_children(&mut cursor) {
            if adapter.is_ignorable(child.kind()) {
                let contiguous = pending.last().is_none_or(|previous| {
                    child.start_position().row.saturating_sub(previous.end_position().row) <= 1
                });
                if contiguous {
                    pending.push(child);
                } else {
                    blocks.push(std::mem::take(&mut pending));
                    pending.push(child);
                }
                continue;
            }
            if !pending.is_empty() {
                blocks.push(std::mem::take(&mut pending));
            }
            descend.push(child);
        }
        if !pending.is_empty() {
            blocks.push(pending);
        }
        for child in descend.into_iter().rev() {
            stack.push(child);
        }
    }
}

fn first_declaration_row(root: Node<'_>, adapter: &Adapter) -> Option<usize> {
    let mut earliest: Option<usize> = None;
    let mut pending = vec![root];
    while let Some(node) = pending.pop() {
        let mut cursor = node.walk();
        for child in node.named_children(&mut cursor) {
            if adapter.declares_symbol(child.kind()) {
                let row = child.start_position().row;
                earliest = Some(earliest.map_or(row, |seen: usize| seen.min(row)));
                continue;
            }
            pending.push(child);
        }
    }
    earliest
}

fn documents_the_file(
    block: &[Node<'_>],
    target: Option<Node<'_>>,
    first_declaration: Option<usize>,
    adapter: &Adapter,
    source: &str,
) -> bool {
    let Some(first) = block.first() else { return false };
    let precedes_every_declaration =
        first_declaration.is_none_or(|row| first.start_position().row < row);
    if !precedes_every_declaration {
        return false;
    }
    let inner_doc =
        block.iter().filter_map(|node| node.utf8_text(source.as_bytes()).ok()).any(|text| {
            let trimmed = text.trim_start();
            trimmed.starts_with("//!") || trimmed.starts_with("/*!")
        });
    inner_doc || !target.is_some_and(|node| adapter.declares_symbol(node.kind()))
}

fn documented_node<'tree>(comment: Node<'tree>, adapter: &Adapter) -> Option<Node<'tree>> {
    let mut candidate = comment.next_named_sibling();
    let mut first_adjacent = None;
    let mut previous_end = comment.end_position().row;

    while let Some(sibling) = candidate {
        if adapter.is_ignorable(sibling.kind()) {
            candidate = sibling.next_named_sibling();
            continue;
        }
        let gap = sibling.start_position().row.saturating_sub(previous_end);
        if gap > ADJACENCY_LINES {
            break;
        }
        if adapter.declares_symbol(sibling.kind()) {
            return Some(sibling);
        }
        if let Some(inner) = declaring_descendant(sibling, adapter) {
            return Some(inner);
        }
        if first_adjacent.is_none() {
            first_adjacent = Some(sibling);
        }
        if !is_modifier(sibling.kind()) {
            break;
        }
        previous_end = sibling.end_position().row;
        candidate = sibling.next_named_sibling();
    }
    if let Some(found) = first_adjacent {
        return Some(found);
    }
    let mut ancestor = comment.parent();
    while let Some(node) = ancestor {
        if adapter.declares_symbol(node.kind()) {
            return Some(node);
        }
        ancestor = node.parent();
    }
    None
}

fn declaring_descendant<'tree>(node: Node<'tree>, adapter: &Adapter) -> Option<Node<'tree>> {
    let mut cursor = node.walk();
    node.named_children(&mut cursor).find(|child| adapter.declares_symbol(child.kind()))
}

fn is_modifier(kind: &str) -> bool {
    matches!(kind, "attribute_item" | "attribute" | "decorator" | "annotation" | "modifiers")
}

fn clean_comment(raw: &str) -> String {
    let trimmed = raw.trim();
    let stripped = trimmed
        .strip_prefix("/**")
        .or_else(|| trimmed.strip_prefix("/*!"))
        .or_else(|| trimmed.strip_prefix("/*"))
        .map(|body| body.trim_end_matches("*/"))
        .map(|body| body.strip_prefix('!').unwrap_or(body))
        .unwrap_or(trimmed);
    stripped.lines().map(strip_markers).collect::<Vec<_>>().join("\n").trim().to_owned()
}

pub(crate) fn reflow(text: &str) -> String {
    let mut segments: Vec<(String, bool)> = Vec::new();
    let mut current: Vec<&str> = Vec::new();
    let mut after_blank = false;

    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            if !current.is_empty() {
                segments.push((current.join(" "), after_blank));
                current.clear();
            }
            after_blank = true;
            continue;
        }
        if starts_a_list_item(line) {
            if !current.is_empty() {
                segments.push((current.join(" "), after_blank));
                current.clear();
                after_blank = false;
            }
            segments.push((line.to_owned(), after_blank));
            after_blank = false;
            continue;
        }
        current.push(line);
    }
    if !current.is_empty() {
        segments.push((current.join(" "), after_blank));
    }

    let mut out = String::new();
    for (index, (segment, started_a_paragraph)) in segments.iter().enumerate() {
        if index > 0 {
            out.push_str(if *started_a_paragraph { "\n\n" } else { "\n" });
        }
        out.push_str(segment);
    }
    out.trim().to_owned()
}

fn starts_a_list_item(line: &str) -> bool {
    if let Some(rest) = line.strip_prefix(['-', '*', '+', '\u{2022}']) {
        return rest.starts_with(' ');
    }
    let digits: String = line.chars().take_while(char::is_ascii_digit).collect();
    if digits.is_empty() || digits.len() > 2 {
        return false;
    }
    let rest = &line[digits.len()..];
    rest.starts_with(". ") || rest.starts_with(") ")
}

fn strip_markers(line: &str) -> String {
    let trimmed = line.trim();
    for opener in ["///", "//!", "//", "--"] {
        if let Some(body) = trimmed.strip_prefix(opener) {
            let body = body.strip_prefix('<').unwrap_or(body);
            return body.trim().to_owned();
        }
    }
    if let Some(body) = trimmed.strip_prefix('#') {
        let body = body.strip_prefix(':').unwrap_or(body);
        return body.trim().to_owned();
    }
    trimmed.strip_prefix('*').unwrap_or(trimmed).trim().to_owned()
}

pub(crate) fn is_shebang(raw: &str) -> bool {
    raw.trim_start().starts_with("#!")
}

pub(crate) fn split_claim(text: &str) -> (String, Option<String>) {
    let boundary = text.match_indices('\n').map(|(at, _)| at).find(|at| {
        let rest = &text[at + 1..];
        rest.starts_with('\n') || starts_a_list_item(rest.trim_start_matches('\n'))
    });

    let Some(at) = boundary else {
        return (text.trim().to_owned(), None);
    };
    let head = text[..at].trim();
    let rest = text[at..].trim();
    if head.is_empty() || rest.is_empty() {
        return (text.trim().to_owned(), None);
    }
    (head.to_owned(), Some(rest.to_owned()))
}

pub(crate) fn is_data_sample(claim: &str) -> bool {
    let trimmed = claim.trim();
    if trimmed.chars().count() < 2 {
        return false;
    }
    if outside_quotes(trimmed).chars().filter(|glyph| glyph.is_alphabetic()).count() < 3 {
        return true;
    }
    let shouted =
        trimmed.chars().any(char::is_alphabetic) && !trimmed.chars().any(char::is_lowercase);
    let unpunctuated = !trimmed.contains(['.', '!', '?']);
    shouted && unpunctuated
}

fn outside_quotes(text: &str) -> String {
    let mut out = String::new();
    let mut opener: Option<char> = None;
    for glyph in text.chars() {
        match opener {
            Some(quote) if glyph == quote => opener = None,
            Some(_) => {}
            None if glyph == '\'' || glyph == '"' => opener = Some(glyph),
            None => out.push(glyph),
        }
    }
    out
}

const LICENCE_FORMULAS: &[&str] = &[
    "spdx-license-identifier",
    "all rights reserved",
    "use of this source code is governed",
    "licensed to the apache software foundation",
    "licensed under the apache license",
    "permission is hereby granted, free of charge",
    "redistribution and use in source and binary forms",
    "this program is free software",
    "gnu general public license",
    "mozilla public license",
    "without warranty of any kind, express or implied",
];

fn opens_with_a_copyright_notice(lowered: &str) -> bool {
    let start = lowered.trim_start();
    if start.starts_with("copyright") || start.starts_with('©') {
        return true;
    }
    let Some(year) = start.strip_prefix("(c)") else {
        return false;
    };
    year.trim_start().starts_with(|glyph: char| glyph.is_ascii_digit())
}

fn is_licence_header(claim: &str) -> bool {
    let lowered = claim.to_ascii_lowercase();
    LICENCE_FORMULAS.iter().any(|formula| lowered.contains(formula))
        || opens_with_a_copyright_notice(&lowered)
}

fn is_decorative(claim: &str) -> bool {
    if claim
        .chars()
        .all(|character| !character.is_alphanumeric() || character == '-' || character == '=')
    {
        return true;
    }
    let body: Vec<char> = claim.chars().filter(|character| !character.is_whitespace()).collect();
    if body.is_empty() {
        return false;
    }
    let ornament = body.iter().filter(|character| !character.is_alphanumeric()).count();
    ornament * 2 > body.len()
}

fn infer_kind(claim: &str) -> Kind {
    let lowered = claim.to_ascii_lowercase();
    let marker = |needle: &str| lowered.contains(needle);

    if marker("safety:") || marker("security") || marker("vulnerab") || marker("attack") {
        return Kind::Security;
    }
    if marker("todo") || marker("fixme") || marker("xxx:") || marker("deprecat") {
        return Kind::Warning;
    }
    if marker("hack") || marker("workaround") || marker("kludge") || marker("for now") {
        return Kind::Workaround;
    }
    if marker("must ") || marker("invariant") || marker("never ") || marker("always ") {
        return Kind::Invariant;
    }
    if marker("because") || marker("rationale") || marker("reason") || marker("why ") {
        return Kind::Rationale;
    }
    if marker("panic") || marker("fails if") || marker("race") || marker("deadlock") {
        return Kind::KnownFailureMode;
    }
    if marker("assume") || marker("assuming") || marker("assumption") {
        return Kind::Assumption;
    }
    if marker("performance") || marker("slow") || marker("o(n") || marker("allocat") {
        return Kind::Performance;
    }
    Kind::Explanation
}

#[cfg(test)]
mod tests {
    use super::{
        clean_comment, is_data_sample, is_licence_header, reflow, split_claim, without_directive,
    };

    #[test]
    fn a_licence_header_is_not_knowledge_about_the_code_beneath_it() {
        assert!(is_licence_header(
            "Copyright 2014 Manu Martinez-Almeida. All rights reserved. Use of this source              code is governed by a MIT style license that can be found in the LICENSE file."
        ));
        assert!(is_licence_header("Copyright (C) 2008 Google Inc."));
        assert!(is_licence_header("SPDX-License-Identifier: Apache-2.0"));
        assert!(is_licence_header(
            "Licensed to the Apache Software Foundation (ASF) under one or more contributor              license agreements."
        ));
    }

    #[test]
    fn prose_that_happens_to_mention_a_licence_is_kept() {
        assert!(!is_licence_header(
            "We cannot vendor this dependency because its licence is incompatible with ours."
        ));
        assert!(!is_licence_header("The licence check runs before the release job."));
        assert!(!is_licence_header(
            "Returns the copyright holder recorded in the document metadata."
        ));
    }

    #[test]
    fn a_pair_of_quoted_samples_is_a_sample() {
        assert!(is_data_sample("'25-10-2006' '20:45:29.000200'"));
        assert!(!is_data_sample("The caller passes a formatted date here."));
    }

    #[test]
    fn a_blank_comment_line_cleans_to_nothing() {
        assert_eq!(clean_comment("//"), "");
        assert_eq!(clean_comment("#"), "");
    }

    #[test]
    fn reflow_joins_wrapped_lines_and_keeps_paragraph_breaks() {
        assert_eq!(reflow("one\ntwo"), "one two");
        assert_eq!(reflow("one\n\ntwo"), "one\n\ntwo");
    }

    #[test]
    fn split_claim_divides_at_the_first_paragraph_break() {
        assert_eq!(split_claim("one\n\ntwo"), ("one".to_owned(), Some("two".to_owned())));
        assert_eq!(split_claim("one two"), ("one two".to_owned(), None));
    }

    #[test]
    fn a_block_of_three_lines_becomes_claim_and_detail() {
        let joined = ["Host adds a matcher.", "", "It accepts a template."]
            .iter()
            .map(|line| clean_comment(line))
            .collect::<Vec<_>>()
            .join("\n");
        let (claim, detail) = split_claim(&reflow(&joined));
        assert_eq!(claim, "Host adds a matcher.");
        assert_eq!(detail.as_deref(), Some("It accepts a template."));
    }

    #[test]
    fn a_bulleted_list_keeps_one_item_per_line() {
        let given = "Skip decompression for these:
- HEAD responses
- 204 No Content";
        assert_eq!(reflow(given), given);
    }

    #[test]
    fn a_numbered_list_is_a_list_too() {
        let given = "Order matters:
1. validate
2. resolve";
        assert_eq!(reflow(given), given);
    }

    #[test]
    fn a_hyphen_mid_sentence_does_not_start_a_list() {
        assert_eq!(
            reflow(
                "a well-known case
wrapped here"
            ),
            "a well-known case wrapped here"
        );
    }

    #[test]
    fn a_list_directly_under_a_lead_line_becomes_the_detail() {
        let (claim, detail) = split_claim(&reflow(
            "Skip these:
- HEAD
- 204",
        ));
        assert_eq!(claim, "Skip these:");
        assert_eq!(
            detail.as_deref(),
            Some(
                "- HEAD
- 204"
            )
        );
    }

    #[test]
    fn a_list_after_a_paragraph_break_keeps_the_break() {
        let (claim, detail) = split_claim(&reflow(
            "Title line.

- first
- second",
        ));
        assert_eq!(claim, "Title line.");
        assert_eq!(
            detail.as_deref(),
            Some(
                "- first
- second"
            )
        );
    }

    #[test]
    fn an_eslint_directive_keeps_only_the_reason() {
        let reason = without_directive(
            "eslint-disable-next-line no-await-in-loop -- requests must be serialised",
        );
        assert_eq!(reason.as_deref(), Some("requests must be serialised"));
    }
}
