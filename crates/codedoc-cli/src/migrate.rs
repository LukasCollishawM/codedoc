use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{Context, Result, bail};
use codedoc_ledger::{Ledger, Record, SCHEMA_VERSION};
use serde_json::{Value, json};

type Step = fn(&Record) -> Option<Record>;

struct Migration {
    from: u32,
    to: u32,
    describe: &'static str,
    apply: Step,
}

const MIGRATIONS: &[Migration] = &[];

pub fn run(root: &Path, write: bool) -> Result<(Value, i32)> {
    let ledger = Ledger::discover(root).context("opening the ledger")?;
    let records = ledger.records().context("reading the ledger")?;

    let mut by_schema: BTreeMap<u32, usize> = BTreeMap::new();
    for record in &records {
        *by_schema.entry(record.content().schema).or_insert(0) += 1;
    }

    let ahead: Vec<u32> =
        by_schema.keys().copied().filter(|schema| *schema > SCHEMA_VERSION).collect();
    if !ahead.is_empty() {
        bail!(
            "this ledger contains records at schema {ahead:?}, newer than this build understands \
             (schema {SCHEMA_VERSION}). Upgrade codedoc rather than migrating downward; writing \
             with an older build would discard members it cannot represent."
        );
    }

    let pending: Vec<&Migration> =
        MIGRATIONS.iter().filter(|migration| by_schema.contains_key(&migration.from)).collect();

    if pending.is_empty() {
        return Ok((
            json!({
                "command": "migrate",
                "schema": SCHEMA_VERSION,
                "records": records.len(),
                "by_schema": by_schema.iter().map(|(k, v)| (k.to_string(), *v)).collect::<BTreeMap<_, _>>(),
                "pending": Vec::<String>::new(),
                "migrated": 0,
                "dry_run": !write,
            }),
            0,
        ));
    }

    let described: Vec<String> = pending
        .iter()
        .map(|migration| format!("{} -> {}: {}", migration.from, migration.to, migration.describe))
        .collect();

    if !write {
        return Ok((
            json!({
                "command": "migrate",
                "schema": SCHEMA_VERSION,
                "records": records.len(),
                "by_schema": by_schema.iter().map(|(k, v)| (k.to_string(), *v)).collect::<BTreeMap<_, _>>(),
                "pending": described,
                "migrated": 0,
                "dry_run": true,
            }),
            0,
        ));
    }

    let mut migrated = 0usize;
    let mut rewritten = Vec::new();
    for record in &records {
        let mut current = record.clone();
        for migration in &pending {
            if current.content().schema != migration.from {
                continue;
            }
            match (migration.apply)(&current) {
                Some(next) => {
                    current = next;
                    migrated += 1;
                }
                None => bail!(
                    "migration {} -> {} could not be applied to record {}",
                    migration.from,
                    migration.to,
                    record.id()
                ),
            }
        }
        rewritten.push(current);
    }

    ledger.replace_all(&rewritten).context("rewriting the ledger with migrated records")?;
    codedoc_index::Index::rebuild(&ledger).context("rebuilding the index")?;

    Ok((
        json!({
            "command": "migrate",
            "schema": SCHEMA_VERSION,
            "records": records.len(),
            "pending": described,
            "migrated": migrated,
            "dry_run": false,
        }),
        0,
    ))
}
