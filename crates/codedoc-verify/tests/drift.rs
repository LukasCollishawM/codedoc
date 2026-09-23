use std::collections::BTreeMap;

use codedoc_verify::{DRIFT_STALE_THRESHOLD, drift_between};

fn shape(pairs: &[(&str, u32)]) -> BTreeMap<String, u32> {
    pairs.iter().map(|(kind, count)| ((*kind).to_owned(), *count)).collect()
}

#[test]
fn identical_shapes_have_no_drift() {
    let recorded = shape(&[("block", 1), ("let_declaration", 2), ("identifier", 4)]);
    assert_eq!(drift_between(&recorded, &recorded), 0);
}

#[test]
fn an_empty_pair_has_no_drift() {
    assert_eq!(drift_between(&shape(&[]), &shape(&[])), 0);
}

#[test]
fn renaming_a_local_does_not_register_as_drift() {
    let before = shape(&[("block", 1), ("let_declaration", 2), ("identifier", 4)]);
    let after = before.clone();
    assert!(
        drift_between(&before, &after) < DRIFT_STALE_THRESHOLD,
        "a rename changes identifier text, not tree shape, so it must not read as drift"
    );
}

#[test]
fn a_rewritten_body_registers_as_drift() {
    let before = shape(&[("block", 1), ("let_declaration", 2), ("identifier", 4)]);
    let after = shape(&[
        ("block", 2),
        ("if_expression", 1),
        ("return_expression", 1),
        ("binary_expression", 2),
        ("identifier", 6),
    ]);
    assert!(
        drift_between(&before, &after) >= DRIFT_STALE_THRESHOLD,
        "replacing a body's control flow must exceed the staleness threshold, because a \
         claim about what the old body did may simply be false of the new one"
    );
}

#[test]
fn drift_is_symmetric() {
    let left = shape(&[("block", 1), ("identifier", 3)]);
    let right = shape(&[("block", 2), ("if_expression", 1), ("identifier", 5)]);
    assert_eq!(drift_between(&left, &right), drift_between(&right, &left));
}

#[test]
fn drift_is_bounded_and_total_when_nothing_is_shared() {
    let left = shape(&[("block", 1)]);
    let right = shape(&[("match_expression", 9)]);
    let value = drift_between(&left, &right);
    assert!(value <= 100);
    assert_eq!(value, 100, "sharing nothing is total drift");
}

#[test]
fn growing_a_body_slightly_stays_under_the_threshold() {
    let before = shape(&[("block", 1), ("let_declaration", 4), ("identifier", 8)]);
    let after = shape(&[("block", 1), ("let_declaration", 5), ("identifier", 9)]);
    assert!(
        drift_between(&before, &after) < DRIFT_STALE_THRESHOLD,
        "adding one statement is ordinary maintenance and must not flag every record \
         on the function"
    );
}

#[test]
fn findings_are_ordered_independently_of_how_work_was_scheduled() {
    let statuses = [
        codedoc_verify::Status::Detached,
        codedoc_verify::Status::Stale,
        codedoc_verify::Status::Migrated,
        codedoc_verify::Status::Fresh,
    ];
    for pair in statuses.windows(2) {
        let [worse, better] = pair else { continue };
        assert!(
            worse > better,
            "verification runs files in parallel and sorts afterwards, so the ordering \
             must be total and must put what needs attention first"
        );
    }
}
