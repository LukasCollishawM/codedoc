use std::path::Path;
use std::process::Command;

use serde_json::{Value, json};

use crate::{Outcome, evidence, query};

const ATTRIBUTE_REQUEST: &str = "merge=codedoc-ledger";
const DRIVER_SETTING: &str = "merge.codedoc-ledger.driver";

fn merge_driver_unregistered(root: &Path) -> bool {
    let Ok(attributes) = std::fs::read_to_string(root.join(".gitattributes")) else {
        return false;
    };
    let requested = attributes.lines().any(|line| {
        let line = line.trim();
        !line.starts_with('#') && line.contains(ATTRIBUTE_REQUEST)
    });
    if !requested {
        return false;
    }
    let Ok(output) =
        Command::new("git").arg("-C").arg(root).args(["config", "--get", DRIVER_SETTING]).output()
    else {
        return false;
    };
    !output.status.success() || output.stdout.iter().all(|byte| byte.is_ascii_whitespace())
}

pub fn doctor(root: &Path) -> Result<(Value, i32), crate::OpsError> {
    let (verified, _) = query::verify(root)?;
    let (contradictions, _) = query::conflicts(root)?;
    let (citations, _) = evidence::evidence(root)?;

    let count = |value: &Value, path: &[&str]| -> u64 {
        let mut current = value;
        for step in path {
            current = &current[*step];
        }
        current.as_u64().unwrap_or(0)
    };

    let found = crate::workspace(root)?;
    let graph = codedoc_graph::Graph::across(&found)?;
    let active = graph.active();
    let unnameable = active
        .iter()
        .filter(|record| {
            record.subject().is_some_and(|anchor| {
                anchor.symbol.is_none() && !matches!(anchor.subject, codedoc_anchor::Subject::File)
            })
        })
        .count() as u64;

    let intact = verified["integrity_intact"].as_bool().unwrap_or(false);
    let detached = count(&verified, &["counts", "detached"]);
    let stale = count(&verified, &["counts", "stale"]);
    let broken_citations = count(&citations, &["broken"]);
    let disagreements = count(&contradictions, &["count"]);

    let unregistered_driver = merge_driver_unregistered(root);
    let blocking = !intact || detached > 0 || broken_citations > 0;
    let advisory = stale > 0 || disagreements > 0 || unnameable > 0 || unregistered_driver;

    let mut needs: Vec<&str> = Vec::new();
    if !intact {
        needs.push("the hash chain is broken; run codedoc repair");
    }
    if detached > 0 {
        needs.push("anchors detached; codedoc detached suggests where the code went");
    }
    if broken_citations > 0 {
        needs.push("cited evidence no longer resolves; codedoc evidence lists it");
    }
    if stale > 0 {
        needs.push("code beneath claims changed; re-read and codedoc affirm or supersede");
    }
    if disagreements > 0 {
        needs.push("records appear to disagree; codedoc conflicts lists them");
    }
    if unnameable > 0 {
        needs.push(
            "some records sit on a construct the adapter cannot name; they hold only \
             while their file is unedited",
        );
    }
    if unregistered_driver {
        needs.push(
            ".gitattributes asks git to merge the ledger with a driver this clone has \
             not registered; run codedoc git install-merge-driver before the next merge",
        );
    }

    let verdict = if blocking {
        "unhealthy"
    } else if advisory {
        "needs reading"
    } else {
        "healthy"
    };

    Ok((
        json!({
            "command": "doctor",
            "verdict": verdict,
            "records": verified["records"],
            "blocking": {
                "integrity_intact": intact,
                "detached": detached,
                "broken_citations": broken_citations,
            },
            "advisory": {
                "stale": stale,
                "disagreements": disagreements,
                "unnameable": unnameable,
                "merge_driver_unregistered": unregistered_driver,
            },
            "next": needs,
        }),
        i32::from(blocking) * 2,
    ))
}

pub fn health(root: &Path) -> Outcome {
    doctor(root).map(|(payload, _)| payload)
}
