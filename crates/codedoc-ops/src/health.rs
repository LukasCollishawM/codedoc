use std::path::Path;

use serde_json::{Value, json};

use crate::{Outcome, evidence, query};

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

    let intact = verified["integrity_intact"].as_bool().unwrap_or(false);
    let detached = count(&verified, &["counts", "detached"]);
    let stale = count(&verified, &["counts", "stale"]);
    let broken_citations = count(&citations, &["broken"]);
    let disagreements = count(&contradictions, &["count"]);

    let blocking = !intact || detached > 0 || broken_citations > 0;
    let advisory = stale > 0 || disagreements > 0;

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
            },
            "next": needs,
        }),
        i32::from(blocking) * 2,
    ))
}

pub fn health(root: &Path) -> Outcome {
    doctor(root).map(|(payload, _)| payload)
}
