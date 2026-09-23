use serde_json::Value;

pub fn human(payload: &Value) {
    match payload.get("command").and_then(Value::as_str) {
        Some("init") => render_init(payload),
        Some("attach") => render_attach(payload),
        Some("verify") => render_verify(payload),
        Some("context") => render_context(payload),
        Some("reindex") => render_reindex(payload),
        Some("list") => render_list(payload),
        Some("history") => render_history(payload),
        Some("stats") => render_stats(payload),
        Some("kinds") => render_kinds(payload),
        Some("import") => render_import(payload),
        _ => println!("{payload}"),
    }
}

fn text(payload: &Value, key: &str) -> String {
    payload.get(key).and_then(Value::as_str).unwrap_or("").to_owned()
}

fn count(payload: &Value, key: &str) -> u64 {
    payload.get(key).and_then(Value::as_u64).unwrap_or(0)
}

fn render_init(payload: &Value) {
    println!("initialised codedoc in {}", text(payload, "root"));
    println!("run `codedoc attach` to record the first claim");
}

fn render_attach(payload: &Value) {
    println!("recorded {}", &text(payload, "record")[..16]);
    println!("  kind    {}", text(payload, "kind"));
    println!("  anchor  {} {}", text(payload, "file"), text(payload, "range"));
    if let Some(symbol) = payload.get("symbol").and_then(Value::as_str) {
        println!("  symbol  {symbol}");
    }
}

fn render_verify(payload: &Value) {
    let counts = &payload["counts"];
    let fresh = counts.get("fresh").and_then(Value::as_u64).unwrap_or(0);
    let migrated = counts.get("migrated").and_then(Value::as_u64).unwrap_or(0);
    let stale = counts.get("stale").and_then(Value::as_u64).unwrap_or(0);
    let detached = counts.get("detached").and_then(Value::as_u64).unwrap_or(0);

    if !payload["integrity_intact"].as_bool().unwrap_or(true) {
        println!("ledger integrity FAILED");
        for orphan in payload["orphaned_records"].as_array().unwrap_or(&Vec::new()) {
            println!("  orphaned {}", orphan.as_str().unwrap_or_default());
        }
        println!();
    }

    println!("{fresh} unchanged   {migrated} migrated   {stale} stale   {detached} detached");

    let empty = Vec::new();
    let findings = payload["findings"].as_array().unwrap_or(&empty);
    for finding in findings {
        let status = finding["status"].as_str().unwrap_or("");
        if status == "fresh" || status == "migrated" {
            continue;
        }
        println!();
        println!("{} {}", status.to_uppercase(), finding["file"].as_str().unwrap_or(""));
        if let Some(symbol) = finding.get("symbol").and_then(Value::as_str) {
            println!("  {symbol}");
        }
        println!(
            "  {} :: {}",
            finding["kind"].as_str().unwrap_or(""),
            finding["claim"].as_str().unwrap_or("")
        );
        println!("  {}", &finding["record"].as_str().unwrap_or("")[..16]);
    }

    if stale > 0 || detached > 0 {
        println!();
        println!("run `codedoc verify --json` for the full resolution detail");
    }
}

fn render_context(payload: &Value) {
    let pack = &payload["pack"];
    let target = &pack["target"];
    println!("TARGET");
    println!(
        "  {}{}",
        target["file"].as_str().unwrap_or(""),
        target
            .get("line")
            .and_then(Value::as_u64)
            .map(|line| format!(":{line}"))
            .unwrap_or_default()
    );
    if let Some(symbol) = target.get("symbol").and_then(Value::as_str) {
        println!("  {symbol}");
    }

    if payload["empty"].as_bool().unwrap_or(false) {
        println!();
        println!("no recorded knowledge covers this location");
        return;
    }

    render_section(pack, "invariants", "INVARIANTS");
    render_section(pack, "security", "SECURITY");
    render_section(pack, "failure_modes", "KNOWN FAILURE MODES");
    render_section(pack, "rationale", "RATIONALE");
    render_section(pack, "other", "OTHER");

    let empty = Vec::new();
    let relations = pack.get("relations").and_then(Value::as_array).unwrap_or(&empty);
    if !relations.is_empty() {
        println!();
        println!("RELATIONS");
        for relation in relations {
            println!(
                "  {} {} {}",
                relation["subject"].as_str().unwrap_or("?"),
                relation["verb"].as_str().unwrap_or(""),
                relation["object"].as_str().unwrap_or("?")
            );
        }
    }

    if pack["truncated"].as_bool().unwrap_or(false) {
        println!();
        println!("(truncated to fit the requested budget)");
    }
}

fn render_section(pack: &Value, key: &str, heading: &str) {
    let empty = Vec::new();
    let claims = pack.get(key).and_then(Value::as_array).unwrap_or(&empty);
    if claims.is_empty() {
        return;
    }
    println!();
    println!("{heading}");
    for claim in claims {
        println!("  - {}", claim["claim"].as_str().unwrap_or(""));
        if let Some(detail) = claim.get("detail").and_then(Value::as_str) {
            for line in detail.lines() {
                println!("    {line}");
            }
        }
        println!("    [{}]", claim["assurance"].as_str().unwrap_or(""));
    }
}

fn render_reindex(payload: &Value) {
    println!("indexed {} records", count(payload, "records"));
    println!("digest {}", &text(payload, "digest")[..16]);
}

fn render_list(payload: &Value) {
    let empty = Vec::new();
    let records = payload["records"].as_array().unwrap_or(&empty);
    if records.is_empty() {
        println!("no records");
        return;
    }
    for record in records {
        println!(
            "{}  {:<20} {}",
            &record["record"].as_str().unwrap_or("")[..12],
            record["kind"].as_str().unwrap_or(""),
            record["claim"].as_str().unwrap_or("")
        );
        if let Some(symbol) = record.get("symbol").and_then(Value::as_str) {
            println!("              {symbol}");
        }
    }
    println!();
    println!("{} records", records.len());
}

fn render_history(payload: &Value) {
    let empty = Vec::new();
    let chain = payload["chain"].as_array().unwrap_or(&empty);
    for (position, entry) in chain.iter().enumerate() {
        let marker = if position == 0 { "current" } else { "superseded" };
        println!("{} {}", &entry["record"].as_str().unwrap_or("")[..12], marker);
        println!("  {}", entry["claim"].as_str().unwrap_or(""));
        println!("  {}", entry["created"].as_str().unwrap_or(""));
        if let Some(revision) = entry.get("code_revision").and_then(Value::as_str) {
            println!("  code {revision}");
        }
        println!();
    }
}

fn render_stats(payload: &Value) {
    println!(
        "{} active of {} total records",
        count(payload, "active_records"),
        count(payload, "total_records")
    );
    if let Some(kinds) = payload["by_kind"].as_object() {
        for (kind, number) in kinds {
            println!("  {:<22} {}", kind, number.as_u64().unwrap_or(0));
        }
    }
    println!();
    println!(
        "integrity {}",
        if payload["integrity_intact"].as_bool().unwrap_or(false) { "intact" } else { "BROKEN" }
    );
}

fn render_import(payload: &Value) {
    println!(
        "scanned {} files, found {} documentable comments",
        count(payload, "files_scanned"),
        count(payload, "candidates")
    );
    if let Some(kinds) = payload["by_kind"].as_object() {
        for (kind, number) in kinds {
            println!("  {:<22} {}", kind, number.as_u64().unwrap_or(0));
        }
    }
    let empty = Vec::new();
    let sample = payload["sample"].as_array().unwrap_or(&empty);
    if !sample.is_empty() {
        println!();
        println!("sample:");
        for item in sample {
            println!(
                "  {}:{} [{}] {}",
                item["file"].as_str().unwrap_or(""),
                item["line"].as_u64().unwrap_or(0),
                item["kind"].as_str().unwrap_or(""),
                item["claim"].as_str().unwrap_or("")
            );
        }
    }
    println!();
    if payload["dry_run"].as_bool().unwrap_or(true) {
        println!("dry run; nothing written. re-run with --write to record these.");
        println!("your comments are left untouched either way.");
    } else {
        println!("recorded {} entries into the ledger", count(payload, "written"));
    }
}

fn render_kinds(payload: &Value) {
    let empty = Vec::new();
    for kind in payload["kinds"].as_array().unwrap_or(&empty) {
        println!("{}", kind.as_str().unwrap_or(""));
    }
}
