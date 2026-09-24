use std::fmt::Write as _;

use serde_json::Value;

pub fn human(payload: &Value) -> String {
    let mut out = String::new();
    match payload.get("command").and_then(Value::as_str) {
        Some("init") => render_init(payload, &mut out),
        Some("attach") => render_attach(payload, &mut out),
        Some("relate") => render_relate(payload, &mut out),
        Some("verify") => render_verify(payload, &mut out),
        Some("context") => render_context(payload, &mut out),
        Some("reindex") => render_reindex(payload, &mut out),
        Some("list") => render_list(payload, &mut out),
        Some("history") => render_history(payload, &mut out),
        Some("stats") => render_stats(payload, &mut out),
        Some("kinds") => render_kinds(payload, &mut out),
        Some("import") => render_import(payload, &mut out),
        Some("supersede") => render_supersede(payload, &mut out),
        Some("affirm") => render_affirm(payload, &mut out),
        Some("resolve") => render_resolve(payload, &mut out),
        Some("retract") => render_retract(payload, &mut out),
        Some("detached") => render_detached(payload, &mut out),
        Some("conflicts") => render_conflicts(payload, &mut out),
        Some("coverage") => render_coverage(payload, &mut out),
        Some("gaps") => render_gaps(payload, &mut out),
        Some("doctor") => render_doctor(payload, &mut out),
        Some("search") => render_search(payload, &mut out),
        Some("brief") => render_brief(payload, &mut out),
        Some("evidence") => render_evidence(payload, &mut out),
        Some("repair") => render_repair(payload, &mut out),
        Some("review") => {
            let _ = writeln!(out, "{}", text(payload, "output"));
        }
        Some("render") => {
            let _ = writeln!(out, "{}", text(payload, "output"));
        }
        Some("migrate") => render_migrate(payload, &mut out),
        Some("git-install-merge-driver") => render_merge_driver(payload, &mut out),
        Some("git-merge-driver") => {
            let _ = writeln!(out, "merged {} records", count(payload, "records"));
        }
        _ => {
            let _ = writeln!(out, "{payload}");
        }
    }
    out
}

fn text(payload: &Value, key: &str) -> String {
    payload.get(key).and_then(Value::as_str).unwrap_or("").to_owned()
}

fn count(payload: &Value, key: &str) -> u64 {
    payload.get(key).and_then(Value::as_u64).unwrap_or(0)
}

fn render_init(payload: &Value, out: &mut String) {
    let _ = writeln!(out, "initialised a {} ledger", text(payload, "scope"));
    let _ = writeln!(out, "  {}", text(payload, "location"));
    let _ = writeln!(out, "  {}", text(payload, "describes"));
    let _ = writeln!(out);
    if payload["leaves_repository_evidence"].as_bool().unwrap_or(true) {
        let _ = writeln!(out, "This ledger is committed. Everyone cloning the repository gets it.");
        let _ = writeln!(out);
        let _ = writeln!(
            out,
            "On a repository you do not own, `codedoc init --scope local` writes inside"
        );
        let _ = writeln!(
            out,
            ".git/ instead, where nothing appears in the working tree or in git status."
        );
    } else {
        let _ = writeln!(out, "Nothing was written to the working tree; git will not see this.");
    }
    let _ = writeln!(out);
    let _ = writeln!(out, "run `codedoc attach` to record the first claim");
}

fn render_attach(payload: &Value, out: &mut String) {
    let _ = writeln!(out, "recorded {}", &text(payload, "record")[..16]);
    let _ = writeln!(out, "  kind    {}", text(payload, "kind"));
    let _ = writeln!(out, "  anchor  {} {}", text(payload, "file"), text(payload, "range"));
    if let Some(symbol) = payload.get("symbol").and_then(Value::as_str) {
        let _ = writeln!(out, "  symbol  {symbol}");
    }
}

fn render_relate(payload: &Value, out: &mut String) {
    let _ = writeln!(out, "recorded {}", &text(payload, "record")[..16]);
    let _ = writeln!(
        out,
        "  {} {} {}",
        text(payload, "subject"),
        text(payload, "verb").replace('_', " "),
        text(payload, "object")
    );
    let _ = writeln!(out, "  scope {}", text(payload, "scope"));
    let _ = writeln!(out);
    let _ = writeln!(out, "This fact belongs to neither endpoint, so it has nowhere to live");
    let _ = writeln!(out, "in a comment. It surfaces in the context of both.");
}

fn render_verify(payload: &Value, out: &mut String) {
    let counts = &payload["counts"];
    let fresh = counts.get("fresh").and_then(Value::as_u64).unwrap_or(0);
    let migrated = counts.get("migrated").and_then(Value::as_u64).unwrap_or(0);
    let stale = counts.get("stale").and_then(Value::as_u64).unwrap_or(0);
    let detached = counts.get("detached").and_then(Value::as_u64).unwrap_or(0);

    let empty_scope = Vec::new();
    let scoped = payload["scoped_to"].as_array().unwrap_or(&empty_scope);
    if !scoped.is_empty() {
        let _ = writeln!(out, "scoped to {} file(s); ledger integrity not checked", scoped.len());
        let _ = writeln!(out);
    }

    if payload["integrity_checked"].as_bool().unwrap_or(true)
        && !payload["integrity_intact"].as_bool().unwrap_or(true)
    {
        let _ = writeln!(out, "ledger integrity FAILED");
        for orphan in payload["orphaned_records"].as_array().unwrap_or(&Vec::new()) {
            let _ = writeln!(out, "  orphaned {}", orphan.as_str().unwrap_or_default());
        }
        let _ = writeln!(out);
    }

    let _ = writeln!(
        out,
        "{fresh} unchanged   {migrated} migrated   {stale} stale   {detached} detached"
    );

    let empty = Vec::new();
    let findings = payload["findings"].as_array().unwrap_or(&empty);
    for finding in findings {
        let status = finding["status"].as_str().unwrap_or("");
        if status == "fresh" || status == "migrated" {
            continue;
        }
        let _ = writeln!(out);
        let _ =
            writeln!(out, "{} {}", status.to_uppercase(), finding["file"].as_str().unwrap_or(""));
        if let Some(symbol) = finding.get("symbol").and_then(Value::as_str) {
            let _ = writeln!(out, "  {symbol}");
        }
        if let Some(moved) = finding.get("relocated_to").and_then(Value::as_str) {
            let _ = writeln!(out, "  moved to {moved}");
        }
        let _ = writeln!(
            out,
            "  {} :: {}",
            finding["kind"].as_str().unwrap_or(""),
            finding["claim"].as_str().unwrap_or("")
        );
        let _ = writeln!(out, "  {}", &finding["record"].as_str().unwrap_or("")[..16]);
    }

    if stale > 0 || detached > 0 {
        let _ = writeln!(out);
        let _ = writeln!(out, "run `codedoc verify --json` for the full resolution detail");
    }
}

fn render_context(payload: &Value, out: &mut String) {
    let pack = &payload["pack"];
    let target = &pack["target"];
    let _ = writeln!(out, "TARGET");
    let _ = writeln!(
        out,
        "  {}{}",
        target["file"].as_str().unwrap_or(""),
        target
            .get("line")
            .and_then(Value::as_u64)
            .map(|line| format!(":{line}"))
            .unwrap_or_default()
    );
    if let Some(symbol) = target.get("symbol").and_then(Value::as_str) {
        let _ = writeln!(out, "  {symbol}");
    }

    if payload["empty"].as_bool().unwrap_or(false) {
        let _ = writeln!(out);
        let _ = writeln!(out, "no recorded knowledge covers this location");
        return;
    }

    render_section(pack, "invariants", "INVARIANTS", out);
    render_section(pack, "security", "SECURITY", out);
    render_section(pack, "failure_modes", "KNOWN FAILURE MODES", out);
    render_section(pack, "rationale", "RATIONALE", out);
    render_section(pack, "other", "OTHER", out);

    let empty = Vec::new();
    let relations = pack.get("relations").and_then(Value::as_array).unwrap_or(&empty);
    if !relations.is_empty() {
        let _ = writeln!(out);
        let _ = writeln!(out, "RELATIONS");
        for relation in relations {
            let _ = writeln!(
                out,
                "  {} {} {}",
                relation["subject"].as_str().unwrap_or("?"),
                relation["verb"].as_str().unwrap_or(""),
                relation["object"].as_str().unwrap_or("?")
            );
        }
    }

    if pack["truncated"].as_bool().unwrap_or(false) {
        let _ = writeln!(out);
        let _ = writeln!(out, "(truncated to fit the requested budget)");
    }
}

fn render_section(pack: &Value, key: &str, heading: &str, out: &mut String) {
    let empty = Vec::new();
    let claims = pack.get(key).and_then(Value::as_array).unwrap_or(&empty);
    if claims.is_empty() {
        return;
    }
    let _ = writeln!(out);
    let _ = writeln!(out, "{heading}");
    for claim in claims {
        let _ = writeln!(out, "  - {}", claim["claim"].as_str().unwrap_or(""));
        if let Some(detail) = claim.get("detail").and_then(Value::as_str) {
            for line in detail.lines() {
                let _ = writeln!(out, "    {line}");
            }
        }
        let kind = claim["kind"].as_str().unwrap_or("");
        let assurance = claim["assurance"].as_str().unwrap_or("");
        if heading == "OTHER" && !kind.is_empty() {
            let _ = writeln!(out, "    [{} · {}]", kind.replace('_', " "), assurance);
        } else {
            let _ = writeln!(out, "    [{assurance}]");
        }
    }
}

fn render_reindex(payload: &Value, out: &mut String) {
    let _ = writeln!(out, "indexed {} records", count(payload, "records"));
    let _ = writeln!(out, "digest {}", &text(payload, "digest")[..16]);
}

fn render_list(payload: &Value, out: &mut String) {
    let empty = Vec::new();
    let records = payload["records"].as_array().unwrap_or(&empty);
    if records.is_empty() {
        let _ = writeln!(out, "no records");
        return;
    }
    for record in records {
        let _ = writeln!(
            out,
            "{}  {:<20} {}",
            &record["record"].as_str().unwrap_or("")[..12],
            record["kind"].as_str().unwrap_or(""),
            record["claim"].as_str().unwrap_or("")
        );
        if let Some(symbol) = record.get("symbol").and_then(Value::as_str) {
            let _ = writeln!(out, "              {symbol}");
        }
    }
    let _ = writeln!(out);
    let _ = writeln!(out, "{} records", records.len());
}

fn render_history(payload: &Value, out: &mut String) {
    let empty = Vec::new();
    let chain = payload["chain"].as_array().unwrap_or(&empty);
    if let Some(symbol) = payload.get("symbol").and_then(Value::as_str) {
        let _ = writeln!(out, "everything ever recorded about {symbol}");
        let _ = writeln!(out);
    }
    for (position, entry) in chain.iter().enumerate() {
        let marker = match entry.get("standing").and_then(Value::as_str) {
            Some(standing) => standing,
            None if position + 1 == chain.len() => "current",
            None => "superseded",
        };
        let _ = writeln!(out, "{} {}", &entry["record"].as_str().unwrap_or("")[..12], marker);
        if entry["affirmation"].as_bool().unwrap_or(false) {
            let _ = writeln!(out, "  re-read and affirmed, wording unchanged");
        }
        let _ = writeln!(out, "  {}", entry["claim"].as_str().unwrap_or(""));
        let _ = writeln!(out, "  {}", entry["created"].as_str().unwrap_or(""));
        if let Some(revision) = entry.get("code_revision").and_then(Value::as_str) {
            let _ = writeln!(out, "  code {revision}");
        }
        let _ = writeln!(out);
    }
}

fn render_stats(payload: &Value, out: &mut String) {
    let _ = writeln!(
        out,
        "{} active of {} total records",
        count(payload, "active_records"),
        count(payload, "total_records")
    );
    if let Some(kinds) = payload["by_kind"].as_object() {
        for (kind, number) in kinds {
            let _ = writeln!(out, "  {:<22} {}", kind, number.as_u64().unwrap_or(0));
        }
    }
    let _ = writeln!(out);
    let _ = writeln!(
        out,
        "integrity {}",
        if payload["integrity_intact"].as_bool().unwrap_or(false) { "intact" } else { "BROKEN" }
    );
}

fn render_supersede(payload: &Value, out: &mut String) {
    let _ = writeln!(out, "recorded {}", &text(payload, "record")[..16]);
    let _ = writeln!(out, "  supersedes {}", &text(payload, "supersedes")[..16]);
    let _ = writeln!(out, "  anchored at {}", text(payload, "range"));
    let _ = writeln!(out, "  the superseded record remains readable via `codedoc history`");
}

fn render_affirm(payload: &Value, out: &mut String) {
    let _ = writeln!(out, "affirmed {}", &text(payload, "affirms")[..16]);
    let _ = writeln!(out, "  recorded as {}", &text(payload, "record")[..16]);
    let drift = count(payload, "drift_cleared");
    if drift > 0 {
        let _ = writeln!(out, "  cleared {drift}% drift; the claim is unchanged");
    } else {
        let _ = writeln!(out, "  the claim is unchanged");
    }
    let _ = writeln!(out, "  anchored at {}", text(payload, "range"));
}

fn render_resolve(payload: &Value, out: &mut String) {
    let _ = writeln!(out, "reattached {}", &text(payload, "readopts")[..16]);
    let _ = writeln!(out, "  as {}", &text(payload, "record")[..16]);
    let _ = writeln!(out, "  {} {}", text(payload, "file"), text(payload, "range"));
    if let Some(symbol) = payload.get("symbol").and_then(Value::as_str) {
        let _ = writeln!(out, "  {symbol}");
    }
    if payload["claim_names_the_old_symbol"].as_bool().unwrap_or(false) {
        let was = text(payload, "was_symbol");
        let _ = writeln!(out);
        let _ = writeln!(out, "  the claim still names {was}, which this no longer points at");
        let _ = writeln!(out, "  `codedoc supersede` to reword it");
    }
}

fn render_retract(payload: &Value, out: &mut String) {
    let _ = writeln!(out, "retracted {}", &text(payload, "retracts")[..16]);
    let _ = writeln!(out, "  was: {}", text(payload, "was"));
    let _ = writeln!(out, "  the claim stays in history; it leaves the active set");
}

fn render_detached(payload: &Value, out: &mut String) {
    let empty = Vec::new();
    let records = payload["records"].as_array().unwrap_or(&empty);
    if records.is_empty() {
        let _ = writeln!(out, "no detached anchors");
        return;
    }
    for record in records {
        let _ = writeln!(
            out,
            "{}  {}",
            &record["record"].as_str().unwrap_or("")[..12],
            record["file"].as_str().unwrap_or("")
        );
        if let Some(symbol) = record.get("symbol").and_then(Value::as_str) {
            let _ = writeln!(out, "  was {symbol}");
        }
        let _ = writeln!(
            out,
            "  {} :: {}",
            record["kind"].as_str().unwrap_or(""),
            record["claim"].as_str().unwrap_or("")
        );
        let empty_suggestions = Vec::new();
        let suggestions = record["suggestions"].as_array().unwrap_or(&empty_suggestions);
        if !suggestions.is_empty() {
            let _ = writeln!(out, "  possibly now:");
            for candidate in suggestions {
                let _ = writeln!(
                    out,
                    "    {:>3}%  {}  {}",
                    candidate["likeness"].as_u64().unwrap_or(0),
                    candidate["symbol"].as_str().unwrap_or(""),
                    candidate["range"].as_str().unwrap_or("")
                );
            }
        }
        let _ = writeln!(out);
    }
    let _ = writeln!(out, "{} detached", records.len());
    let _ = writeln!(out);
    let _ = writeln!(out, "place one explicitly:");
    let _ = writeln!(out, "  codedoc resolve <record> --to-symbol <symbol>");
    let _ = writeln!(out, "  codedoc resolve <record> --to-line <line> --in-file <path>");
    let _ = writeln!(out, "or drop it:");
    let _ = writeln!(out, "  codedoc retract <record> --reason \"...\"");
}

fn render_search(payload: &Value, out: &mut String) {
    let found = count(payload, "count");
    if found == 0 {
        let _ = writeln!(out, "nothing recorded matches {}", text(payload, "query"));
        return;
    }
    let _ = writeln!(out, "{found} record{}", if found == 1 { "" } else { "s" });
    let empty = Vec::new();
    for entry in payload["records"].as_array().unwrap_or(&empty) {
        let _ = writeln!(out);
        let _ = writeln!(
            out,
            "  {} · {}",
            entry["kind"].as_str().unwrap_or(""),
            entry["symbol"].as_str().unwrap_or_else(|| entry["file"].as_str().unwrap_or(""))
        );
        let _ = writeln!(out, "  {}", entry["claim"].as_str().unwrap_or(""));
        let _ = writeln!(out, "  {}", &entry["record"].as_str().unwrap_or("")[..12]);
    }
}

fn render_brief(payload: &Value, out: &mut String) {
    let empty = Vec::new();
    let files = payload["files"].as_array().unwrap_or(&empty);
    let claims = count(payload, "claims");
    if claims == 0 {
        let _ = writeln!(out, "nothing recorded covers those {} file(s)", files.len());
        return;
    }
    let _ = writeln!(
        out,
        "{claims} recorded claim{} across {} file{}",
        if claims == 1 { "" } else { "s" },
        files.len(),
        if files.len() == 1 { "" } else { "s" }
    );

    let pack = &payload["pack"];
    for (heading, key) in [
        ("invariants", "invariants"),
        ("security", "security"),
        ("known failure modes", "failure_modes"),
        ("rationale and decisions", "rationale"),
        ("other", "other"),
    ] {
        let section = pack[key].as_array().unwrap_or(&empty);
        if section.is_empty() {
            continue;
        }
        let _ = writeln!(out);
        let _ = writeln!(out, "{heading}:");
        for claim in section {
            let _ = writeln!(
                out,
                "  {} — {}",
                claim["symbol"].as_str().unwrap_or_else(|| claim["file"].as_str().unwrap_or("")),
                claim["claim"].as_str().unwrap_or("")
            );
        }
    }
    if pack["truncated"].as_bool().unwrap_or(false) {
        let _ = writeln!(out);
        let _ = writeln!(out, "truncated to the requested budget");
    }
}

fn render_doctor(payload: &Value, out: &mut String) {
    let verdict = text(payload, "verdict");
    let _ = writeln!(out, "{verdict} — {} records", count(payload, "records"));

    let blocking = &payload["blocking"];
    let advisory = &payload["advisory"];
    let _ = writeln!(out);
    let _ = writeln!(
        out,
        "  {:<22} {}",
        "chain intact",
        if blocking["integrity_intact"].as_bool().unwrap_or(false) { "yes" } else { "NO" }
    );
    for (label, value) in [
        ("detached anchors", &blocking["detached"]),
        ("broken citations", &blocking["broken_citations"]),
        ("drifted claims", &advisory["stale"]),
        ("apparent disagreements", &advisory["disagreements"]),
        ("records with no symbol", &advisory["unnameable"]),
    ] {
        let _ = writeln!(out, "  {label:<22} {}", value.as_u64().unwrap_or(0));
    }

    let empty = Vec::new();
    let next = payload["next"].as_array().unwrap_or(&empty);
    if !next.is_empty() {
        let _ = writeln!(out);
        for step in next {
            let _ = writeln!(out, "  → {}", step.as_str().unwrap_or(""));
        }
    }
}

fn render_evidence(payload: &Value, out: &mut String) {
    let cited = count(payload, "citations");
    let broken = count(payload, "broken");
    if cited == 0 {
        let _ = writeln!(out, "no records cite any evidence");
        return;
    }
    if broken == 0 {
        let _ = writeln!(out, "{cited} citations, all still resolving");
        return;
    }
    let _ = writeln!(out, "{broken} of {cited} citations no longer resolve");
    let _ = writeln!(out);
    let empty = Vec::new();
    for entry in payload["records"].as_array().unwrap_or(&empty) {
        let _ = writeln!(
            out,
            "  {:<10} {}",
            entry["standing"].as_str().unwrap_or(""),
            entry["citation"].as_str().unwrap_or("")
        );
        let _ = writeln!(out, "             {}", entry["claim"].as_str().unwrap_or(""));
    }
}

fn render_coverage(payload: &Value, out: &mut String) {
    let declared = count(payload, "declarations");
    if declared == 0 {
        let _ = writeln!(out, "no declarations found in the scanned paths");
        return;
    }
    let _ = writeln!(
        out,
        "{}% — {} of {} declarations carry a record, across {} files",
        count(payload, "percent"),
        count(payload, "documented"),
        declared,
        count(payload, "files")
    );

    let empty = Vec::new();
    let thinnest = payload["thinnest"].as_array().unwrap_or(&empty);
    if !thinnest.is_empty() {
        let _ = writeln!(out);
        let _ = writeln!(out, "thinnest coverage:");
        for entry in thinnest {
            let _ = writeln!(
                out,
                "  {:>3}/{:<3}  {}",
                entry["documented"].as_u64().unwrap_or(0),
                entry["declarations"].as_u64().unwrap_or(0),
                entry["file"].as_str().unwrap_or("")
            );
        }
    }
    let _ = writeln!(out);
    let _ = writeln!(out, "Coverage is a prompt, not a target. A codebase where every");
    let _ = writeln!(out, "declaration carries a record has mostly restated its own code.");
}

fn render_gaps(payload: &Value, out: &mut String) {
    if payload["git"].as_bool() != Some(true) {
        let _ = writeln!(out, "no git history here, so there is nothing to rank against");
        return;
    }
    let empty = Vec::new();
    let gaps = payload["gaps"].as_array().unwrap_or(&empty);
    if gaps.is_empty() {
        let _ = writeln!(
            out,
            "nothing undocumented in the last {} commits has been revisited",
            count(payload, "commits_scanned")
        );
        return;
    }

    let _ = writeln!(
        out,
        "{} undocumented declaration{} the last {} commits came back to:",
        gaps.len(),
        if gaps.len() == 1 { "" } else { "s" },
        count(payload, "commits_scanned")
    );
    for gap in gaps {
        let _ = writeln!(out);
        let _ = writeln!(
            out,
            "  {}:{} — {}",
            gap["file"].as_str().unwrap_or(""),
            gap["first_line"].as_u64().unwrap_or(0),
            gap["symbol"].as_str().unwrap_or("")
        );
        let corrections = gap["corrections"].as_u64().unwrap_or(0);
        let commits = gap["commits"].as_u64().unwrap_or(0);
        let authors = gap["authors"].as_u64().unwrap_or(0);
        let _ = writeln!(
            out,
            "    {commits} commit{}, {corrections} corrective, {authors} author{}",
            if commits == 1 { "" } else { "s" },
            if authors == 1 { "" } else { "s" }
        );
        if let Some(latest) = gap["latest_correction"].as_str() {
            let _ = writeln!(out, "    latest correction: {latest}");
        }
    }

    let _ = writeln!(out);
    let _ = writeln!(out, "A correction is evidence that the code did not say enough, because");
    let _ = writeln!(out, "code that said enough would not have needed correcting. Read those");
    let _ = writeln!(out, "commits rather than the code: the code is only what was left after.");
}

fn render_conflicts(payload: &Value, out: &mut String) {
    let empty = Vec::new();
    let findings = payload["findings"].as_array().unwrap_or(&empty);
    if findings.is_empty() {
        let _ = writeln!(out, "no conflicting records");
        return;
    }
    for finding in findings {
        let _ = writeln!(
            out,
            "{}  {}",
            finding["kind"].as_str().unwrap_or("").to_uppercase(),
            finding["anchor"].as_str().unwrap_or("")
        );
        if let Some(score) = finding.get("similarity").and_then(Value::as_u64) {
            let _ = writeln!(out, "  {score}% alike");
        }
        let _ = writeln!(out, "  {}", finding["left_claim"].as_str().unwrap_or(""));
        let _ = writeln!(out, "  {}", finding["right_claim"].as_str().unwrap_or(""));
        let _ = writeln!(out, "  {}", finding["describes"].as_str().unwrap_or(""));
        let _ = writeln!(out);
    }
    let _ = writeln!(out, "{} to review", findings.len());
    let _ = writeln!(out);
    let _ = writeln!(out, "These are signals, not verdicts. If one record replaces another,");
    let _ = writeln!(out, "supersede it so the link is recorded:");
    let _ = writeln!(out, "  codedoc supersede <record> --claim \"...\"");
}

fn render_repair(payload: &Value, out: &mut String) {
    let orphans = count(payload, "orphans");
    if orphans == 0 {
        let _ = writeln!(out, "{} records, chain intact", count(payload, "records"));
        return;
    }
    let _ = writeln!(out, "{orphans} of {} records are orphaned", count(payload, "records"));
    let _ = writeln!(out, "Their chain references records this ledger no longer holds, which");
    let _ = writeln!(out, "means it was edited outside codedoc, usually by rewriting git history.");
    let _ = writeln!(out);
    if payload["dry_run"].as_bool().unwrap_or(true) {
        let _ = writeln!(out, "Repair rebuilds the chain in timestamp order and remaps");
        let _ = writeln!(out, "supersession links. Record identities WILL change, because an");
        let _ = writeln!(out, "identity covers the chain it was written into.");
        let _ = writeln!(out);
        let _ = writeln!(out, "Commit or back up .codedoc first, then: codedoc repair --write");
    } else {
        let _ =
            writeln!(out, "rewrote {} records; chain is now intact", count(payload, "rewritten"));
    }
}

fn render_migrate(payload: &Value, out: &mut String) {
    let _ = writeln!(
        out,
        "{} records at schema {}",
        count(payload, "records"),
        count(payload, "schema")
    );
    let empty = Vec::new();
    let pending = payload["pending"].as_array().unwrap_or(&empty);
    if pending.is_empty() {
        let _ = writeln!(out, "nothing to migrate");
        return;
    }
    for step in pending {
        let _ = writeln!(out, "  {}", step.as_str().unwrap_or(""));
    }
    if payload["dry_run"].as_bool().unwrap_or(true) {
        let _ = writeln!(out, "dry run; re-run with --write to apply");
    } else {
        let _ = writeln!(out, "migrated {} records", count(payload, "migrated"));
    }
}

fn render_merge_driver(payload: &Value, out: &mut String) {
    if payload["already_present"].as_bool().unwrap_or(false) {
        let _ = writeln!(out, "merge driver already installed");
    } else {
        let _ = writeln!(out, "installed the codedoc ledger merge driver");
    }
    let _ = writeln!(out, "  {}", text(payload, "attribute_line"));
    let _ = writeln!(out, "  recorded in {}", text(payload, "attributes"));
    let _ = writeln!(out);
    let _ = writeln!(out, "Ledger shards now merge by union rather than by conflict.");
}

fn render_import(payload: &Value, out: &mut String) {
    let _ = writeln!(
        out,
        "scanned {} file{}, found {} documentable comment{}",
        count(payload, "files_scanned"),
        if count(payload, "files_scanned") == 1 { "" } else { "s" },
        count(payload, "candidates"),
        if count(payload, "candidates") == 1 { "" } else { "s" }
    );
    let unreadable = count(payload, "files_unreadable");
    if unreadable > 0 {
        let _ = writeln!(
            out,
            "  {unreadable} file{} could not be read as UTF-8 and {} skipped",
            if unreadable == 1 { "" } else { "s" },
            if unreadable == 1 { "was" } else { "were" }
        );
    }
    if let Some(kinds) = payload["by_kind"].as_object() {
        for (kind, number) in kinds {
            let _ = writeln!(out, "  {:<22} {}", kind, number.as_u64().unwrap_or(0));
        }
    }
    let candidates = count(payload, "candidates");
    let unnamed = count(payload, "unnamed");
    if unnamed > 0 && candidates > 0 {
        let share = unnamed * 100 / candidates;
        let _ = writeln!(out);
        let _ = writeln!(
            out,
            "{unnamed} of them ({share}%) attach to a construct this language adapter \
             cannot name."
        );
        let _ = writeln!(
            out,
            "Those resolve only while their file is byte-identical, and detach on the \
             first edit."
        );
    }

    let empty = Vec::new();
    let sample = payload["sample"].as_array().unwrap_or(&empty);
    if !sample.is_empty() {
        let _ = writeln!(out);
        let _ = writeln!(out, "sample:");
        for item in sample {
            let _ = writeln!(
                out,
                "  {}:{} [{}] {}",
                item["file"].as_str().unwrap_or(""),
                item["line"].as_u64().unwrap_or(0),
                item["kind"].as_str().unwrap_or(""),
                item["claim"].as_str().unwrap_or("")
            );
        }
    }
    let _ = writeln!(out);
    if payload["dry_run"].as_bool().unwrap_or(true) {
        let _ = writeln!(out, "dry run; nothing written. re-run with --write to record these.");
        let _ = writeln!(out, "your comments are left untouched either way.");
    } else {
        let _ = writeln!(out, "recorded {} entries into the ledger", count(payload, "written"));
    }
}

fn render_kinds(payload: &Value, out: &mut String) {
    let empty = Vec::new();
    for kind in payload["kinds"].as_array().unwrap_or(&empty) {
        let _ = writeln!(out, "{}", kind.as_str().unwrap_or(""));
    }
}
