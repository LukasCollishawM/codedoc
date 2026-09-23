#![forbid(unsafe_code)]

use codedoc_context::ContextPack;
use codedoc_ledger::{Author, Kind, Record};

pub fn hover_markdown(records: &[&Record]) -> String {
    if records.is_empty() {
        return String::new();
    }
    let mut out = String::new();
    for (position, record) in records.iter().enumerate() {
        if position > 0 {
            out.push_str("\n\n---\n\n");
        }
        out.push_str(&record_markdown(record));
    }
    out
}

pub fn record_markdown(record: &Record) -> String {
    let content = record.content();
    let mut out = String::new();

    out.push_str(&format!("**{}**\n\n", heading_for(record.kind())));
    out.push_str(&content.body.claim);
    out.push('\n');

    if let Some(detail) = &content.body.detail {
        out.push('\n');
        out.push_str(detail);
        out.push('\n');
    }

    if let Some(anchor) = record.subject()
        && let Some(symbol) = &anchor.symbol
    {
        out.push_str(&format!("\n`{symbol}`\n"));
    }

    out.push_str(&format!(
        "\n_{} · {} · {}_\n",
        content.kind.as_str(),
        content.assurance.as_str(),
        describe_author(&content.author)
    ));

    if let Some(revision) = &content.code_revision {
        out.push_str(&format!(
            "_recorded against {}_\n",
            &revision.as_str()[..7.min(revision.as_str().len())]
        ));
    }

    out
}

pub fn pack_markdown(pack: &ContextPack) -> String {
    let mut out = String::new();
    out.push_str("# Context\n\n");
    out.push_str(&format!("`{}`", pack.target.file));
    if let Some(line) = pack.target.line {
        out.push_str(&format!(":{line}"));
    }
    out.push_str("\n\n");

    section(&mut out, "Invariants", &pack.invariants);
    section(&mut out, "Security", &pack.security);
    section(&mut out, "Known failure modes", &pack.failure_modes);
    section(&mut out, "Rationale", &pack.rationale);
    section(&mut out, "Other", &pack.other);

    if !pack.relations.is_empty() {
        out.push_str("## Relations\n\n");
        for relation in &pack.relations {
            out.push_str(&format!(
                "- `{}` **{}** `{}`\n",
                relation.subject.as_deref().unwrap_or("?"),
                relation.verb.replace('_', " "),
                relation.object.as_deref().unwrap_or("?")
            ));
        }
        out.push('\n');
    }

    if pack.truncated {
        out.push_str("_Truncated to the requested budget._\n");
    }

    out
}

fn section(out: &mut String, heading: &str, claims: &[codedoc_context::Claim]) {
    if claims.is_empty() {
        return;
    }
    out.push_str(&format!("## {heading}\n\n"));
    for claim in claims {
        out.push_str(&format!("- {}\n", claim.claim));
        if let Some(detail) = &claim.detail {
            for line in detail.lines() {
                out.push_str(&format!("  {line}\n"));
            }
        }
    }
    out.push('\n');
}

pub fn heading_for(kind: Kind) -> &'static str {
    match kind {
        Kind::Invariant => "INVARIANT",
        Kind::Precondition => "PRECONDITION",
        Kind::Postcondition => "POSTCONDITION",
        Kind::Security => "SECURITY",
        Kind::Performance => "PERFORMANCE",
        Kind::Rationale => "RATIONALE",
        Kind::Decision => "DECISION",
        Kind::Specification => "SPECIFICATION",
        Kind::KnownFailureMode => "KNOWN FAILURE MODE",
        Kind::Warning => "WARNING",
        Kind::Workaround => "WORKAROUND",
        Kind::Assumption => "ASSUMPTION",
        Kind::Ownership => "OWNERSHIP",
        Kind::Explanation => "EXPLANATION",
        Kind::Relation(_) => "RELATION",
        Kind::Tombstone => "RETRACTED",
        _ => "RECORD",
    }
}

pub fn lens_title(record: &Record) -> String {
    let claim = &record.content().body.claim;
    let trimmed: String = claim.chars().take(72).collect();
    let ellipsis = if claim.chars().count() > 72 { "…" } else { "" };
    format!("{} · {trimmed}{ellipsis}", record.content().kind.as_str())
}

fn describe_author(author: &Author) -> String {
    match author {
        Author::Human { identity } => identity.clone(),
        Author::Agent { model, .. } => format!("agent {model}"),
        Author::Analyzer { name } => format!("analyzer {name}"),
        Author::Runtime { name } => format!("runtime {name}"),
        _ => "unattributed".to_owned(),
    }
}

pub fn mermaid_relations(records: &[&Record]) -> String {
    let mut nodes: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    let mut edges: std::collections::BTreeSet<(String, String, String)> =
        std::collections::BTreeSet::new();

    for record in records {
        let Kind::Relation(verb) = record.kind() else {
            continue;
        };
        let (Some(subject), Some(object)) = (record.subject(), record.object()) else {
            continue;
        };
        let from = node_label(subject);
        let to = node_label(object);
        nodes.insert(from.clone());
        nodes.insert(to.clone());
        edges.insert((from, verb.as_str().replace('_', " "), to));
    }

    if edges.is_empty() {
        return String::from(
            "graph LR
  none[\"no relations recorded\"]
",
        );
    }

    let mut out = String::from(
        "graph LR
",
    );
    for label in &nodes {
        out.push_str(&format!(
            "  {}[\"{}\"]
",
            identifier(label),
            label
        ));
    }
    for (from, verb, to) in &edges {
        out.push_str(&format!(
            "  {} -->|{verb}| {}
",
            identifier(from),
            identifier(to)
        ));
    }
    out
}

fn node_label(anchor: &codedoc_anchor::Anchor) -> String {
    anchor
        .symbol
        .as_ref()
        .map(ToString::to_string)
        .unwrap_or_else(|| format!("{}:{}", anchor.file.as_str(), anchor.range.start_line))
}

fn identifier(label: &str) -> String {
    let cleaned: String = label
        .chars()
        .map(|character| if character.is_alphanumeric() { character } else { '_' })
        .collect();
    format!("n{}", &cleaned[..cleaned.len().min(48)])
}

pub fn overview_markdown(records: &[&Record], title: &str) -> String {
    let mut out = format!("# {title}\n\n");
    if records.is_empty() {
        out.push_str("No records yet.\n");
        return out;
    }

    let mut by_file: std::collections::BTreeMap<String, Vec<&Record>> =
        std::collections::BTreeMap::new();
    for record in records {
        let file = record
            .subject()
            .map(|anchor| anchor.file.as_str().to_owned())
            .unwrap_or_else(|| "unanchored".to_owned());
        by_file.entry(file).or_default().push(record);
    }

    out.push_str(&format!("{} records across {} files.\n\n", records.len(), by_file.len()));

    for (file, entries) in &by_file {
        out.push_str(&format!("## `{file}`\n\n"));
        let mut ordered = entries.clone();
        ordered.sort_by_key(|record| {
            record.subject().map(|anchor| anchor.range.start_line).unwrap_or(0)
        });
        for record in ordered {
            let symbol = record
                .subject()
                .and_then(|anchor| anchor.symbol.as_ref().map(ToString::to_string))
                .unwrap_or_default();
            out.push_str(&format!(
                "- **{}** {}\n  {}\n",
                heading_for(record.kind()),
                if symbol.is_empty() { String::new() } else { format!("`{symbol}`") },
                record.content().body.claim
            ));
            if let Some(detail) = &record.content().body.detail {
                for line in detail.lines() {
                    out.push_str(&format!("  > {line}\n"));
                }
            }
        }
        out.push('\n');
    }
    out
}
