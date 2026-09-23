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
        Some("resolve") => render_resolve(payload, &mut out),
        Some("retract") => render_retract(payload, &mut out),
        Some("detached") => render_detached(payload, &mut out),
        Some("conflicts") => render_conflicts(payload, &mut out),
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
        let _ = writeln!(out, "    [{}]", claim["assurance"].as_str().unwrap_or(""));
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
    for (position, entry) in chain.iter().enumerate() {
        let marker = if position == 0 { "current" } else { "superseded" };
        let _ = writeln!(out, "{} {}", &entry["record"].as_str().unwrap_or("")[..12], marker);
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

fn render_resolve(payload: &Value, out: &mut String) {
    let _ = writeln!(out, "reattached {}", &text(payload, "readopts")[..16]);
    let _ = writeln!(out, "  as {}", &text(payload, "record")[..16]);
    let _ = writeln!(out, "  {} {}", text(payload, "file"), text(payload, "range"));
    if let Some(symbol) = payload.get("symbol").and_then(Value::as_str) {
        let _ = writeln!(out, "  {symbol}");
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
        "scanned {} files, found {} documentable comments",
        count(payload, "files_scanned"),
        count(payload, "candidates")
    );
    if let Some(kinds) = payload["by_kind"].as_object() {
        for (kind, number) in kinds {
            let _ = writeln!(out, "  {:<22} {}", kind, number.as_u64().unwrap_or(0));
        }
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
