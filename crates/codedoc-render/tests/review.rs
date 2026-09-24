use codedoc_render::{ReviewInput, review_markdown};

fn input<'a>(base: &'a str) -> ReviewInput<'a> {
    ReviewInput {
        base,
        files: &[],
        stale: Vec::new(),
        detached: Vec::new(),
        unchanged: 0,
        recorded_here: 0,
        touched_declarations: 0,
        undocumented_declarations: 0,
    }
}

#[test]
fn a_change_that_breaks_nothing_says_so_plainly() {
    let mut given = input("origin/main");
    given.unchanged = 4;
    let rendered = review_markdown(&given);
    assert!(rendered.contains("4 recorded claims still hold"));
    assert!(!rendered.contains("may no longer hold"));
}

#[test]
fn the_singular_case_reads_as_english() {
    let mut given = input("origin/main");
    given.unchanged = 1;
    assert!(review_markdown(&given).contains("1 recorded claim still holds"));
}

#[test]
fn detached_claims_say_that_codedoc_refused_to_guess() {
    let mut given = input("origin/main");
    given.detached = vec![(
        "src/auth.rs".to_owned(),
        "rust://validate".to_owned(),
        "Validation precedes resolution.".to_owned(),
        None,
    )];
    let rendered = review_markdown(&given);
    assert!(rendered.contains("could not be found"));
    assert!(rendered.contains("will not guess"));
    assert!(rendered.contains("rust://validate"));
}

#[test]
fn a_detached_claim_offers_its_best_candidate() {
    let mut given = input("origin/main");
    given.detached = vec![(
        "src/auth.rs".to_owned(),
        "rust://parse_header".to_owned(),
        "Whitespace-only headers count as empty.".to_owned(),
        Some("rust://parse_headers".to_owned()),
    )];
    let rendered = review_markdown(&given);
    assert!(rendered.contains("possibly now `rust://parse_headers`"));
    assert!(
        rendered.contains("confirm with"),
        "a suggestion must read as something to confirm, not something already done"
    );
}

#[test]
fn stale_claims_carry_their_drift() {
    let mut given = input("origin/main");
    given.stale = vec![(
        "src/pay.rs".to_owned(),
        "rust://settle".to_owned(),
        "The fee is deducted.".to_owned(),
        Some(36),
    )];
    let rendered = review_markdown(&given);
    assert!(rendered.contains("(36% changed)"));
    assert!(
        rendered.contains("not necessarily wrong"),
        "the wording must not assert more than it knows, or reviewers learn to dismiss it"
    );
}

#[test]
fn undocumented_declarations_prompt_without_nagging() {
    let mut given = input("origin/main");
    given.unchanged = 1;
    given.touched_declarations = 5;
    given.undocumented_declarations = 4;
    let rendered = review_markdown(&given);
    assert!(rendered.contains("touched 5 declarations, 4 of which"));
    assert!(rendered.contains("<sub>"), "the prompt is a footnote, not a headline");
}

#[test]
fn fully_documented_changes_get_no_prompt() {
    let mut given = input("origin/main");
    given.unchanged = 2;
    given.touched_declarations = 3;
    given.undocumented_declarations = 0;
    assert!(!review_markdown(&given).contains("carry no recorded knowledge"));
}

#[test]
fn a_flagged_claim_names_the_three_ways_to_answer_for_it() {
    let mut given = input("origin/main");
    given.stale = vec![(
        "src/auth.rs".to_owned(),
        "rust://validate".to_owned(),
        "Validation precedes tenant resolution.".to_owned(),
        Some(41),
    )];
    let rendered = review_markdown(&given);

    for action in ["codedoc affirm", "codedoc supersede", "codedoc retract"] {
        assert!(
            rendered.contains(action),
            "this comment is where a person meets the tool, and telling them a claim \
             needs re-reading without naming what to do afterwards leaves the work \
             undone and the comment repeating itself forever: {action} missing"
        );
    }
}

#[test]
fn a_clean_change_is_not_lectured_about_what_to_do_next() {
    let mut given = input("origin/main");
    given.unchanged = 3;
    let rendered = review_markdown(&given);
    assert!(
        !rendered.contains("codedoc affirm"),
        "there is nothing to answer for, so advice on answering is noise: {rendered}"
    );
}

#[test]
fn a_change_no_claim_covers_says_that_rather_than_counting_to_zero() {
    let given = input("origin/main");
    let rendered = review_markdown(&given);
    assert!(
        rendered.contains("Nothing recorded covers"),
        "\"0 recorded claims still hold\" reads as though claims were checked and \
         none survived, which is the opposite of what happened: {rendered}"
    );
    assert!(!rendered.contains("0 recorded"), "{rendered}");
}

#[test]
fn a_change_that_recorded_something_gets_credit_for_it() {
    let mut given = input("origin/main");
    given.unchanged = 2;
    given.recorded_here = 3;
    let rendered = review_markdown(&given);
    assert!(
        rendered.contains("recorded 3 new claims"),
        "a review that only ever reports what a change broke teaches people that \
         this tool is a complaint. Recording something is the behaviour it is \
         trying to produce, so it should be visible in the same place: {rendered}"
    );
}
