use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result, bail};
use codedoc_ledger::Record;
use serde_json::{Value, json};

const ATTRIBUTE_LINE: &str = ".codedoc/ledger/*.jsonl merge=codedoc-ledger";
const DRIVER_NAME: &str = "merge.codedoc-ledger";

pub fn install_merge_driver(root: &Path) -> Result<(Value, i32)> {
    let configured = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["config", &format!("{DRIVER_NAME}.name"), "codedoc append-only ledger union merge"])
        .status()
        .context("running git config")?;
    if !configured.success() {
        bail!("git config failed; is {} a git repository?", root.display());
    }

    let driver = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["config", &format!("{DRIVER_NAME}.driver"), "codedoc git merge-driver %O %A %B"])
        .status()
        .context("running git config")?;
    if !driver.success() {
        bail!("could not register the merge driver");
    }

    let attributes = root.join(".gitattributes");
    let existing = fs::read_to_string(&attributes).unwrap_or_default();
    let already = existing.lines().any(|line| line.trim() == ATTRIBUTE_LINE);
    if !already {
        let mut updated = existing;
        if !updated.is_empty() && !updated.ends_with('\n') {
            updated.push('\n');
        }
        updated.push_str(ATTRIBUTE_LINE);
        updated.push('\n');
        fs::write(&attributes, updated).context("writing .gitattributes")?;
    }

    Ok((
        json!({
            "command": "git-install-merge-driver",
            "driver": DRIVER_NAME,
            "attributes": attributes.display().to_string(),
            "attribute_line": ATTRIBUTE_LINE,
            "already_present": already,
        }),
        0,
    ))
}

pub fn merge_driver(base: &str, ours: &str, theirs: &str) -> Result<(Value, i32)> {
    let mut merged: BTreeMap<String, (i64, String)> = BTreeMap::new();
    let mut conflicts = 0usize;

    for source in [base, ours, theirs] {
        let raw = fs::read_to_string(source).unwrap_or_default();
        for line in raw.lines() {
            if line.trim().is_empty() {
                continue;
            }
            match Record::decode_line(line.as_bytes()) {
                Ok(record) => {
                    merged.insert(
                        record.id().to_string(),
                        (record.content().created.unix_seconds(), line.to_owned()),
                    );
                }
                Err(_) => conflicts += 1,
            }
        }
    }

    if conflicts > 0 {
        bail!(
            "{conflicts} line(s) in the merge inputs are not valid ledger records; refusing to \
             write a ledger that would fail verification"
        );
    }

    let mut ordered: Vec<(&String, &(i64, String))> = merged.iter().collect();
    ordered.sort_by(|left, right| left.1.0.cmp(&right.1.0).then_with(|| left.0.cmp(right.0)));

    let mut output = String::new();
    for (_, (_, line)) in &ordered {
        output.push_str(line);
        output.push('\n');
    }
    fs::write(ours, output).context("writing the merged ledger")?;

    Ok((
        json!({
            "command": "git-merge-driver",
            "records": ordered.len(),
            "written": ours,
        }),
        0,
    ))
}
