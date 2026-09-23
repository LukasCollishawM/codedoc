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
