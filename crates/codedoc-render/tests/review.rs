use codedoc_render::{ReviewInput, StaleClaim, review_markdown};

fn input<'a>(base: &'a str) -> ReviewInput<'a> {
    ReviewInput {
        base,
        files: &[],
        stale: Vec::new(),
        detached: Vec::new(),
        unchanged: 0,
        moved: 0,
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
    assert!(rendered.contains("4 recorded claims still resolve"));
    assert!(!rendered.contains("may no longer hold"));
}

#[test]
fn the_singular_case_reads_as_english() {
    let mut given = input("origin/main");
    given.unchanged = 1;
    assert!(review_markdown(&given).contains("1 recorded claim still resolves"));
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
    given.stale = vec![StaleClaim {
        file: "src/pay.rs".to_owned(),
        symbol: "rust://settle".to_owned(),
        claim: "The fee is deducted.".to_owned(),
        drift: Some(36),
        relocated_to: None,
    }];
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
    given.stale = vec![StaleClaim {
        file: "src/auth.rs".to_owned(),
        symbol: "rust://validate".to_owned(),
        claim: "Validation precedes tenant resolution.".to_owned(),
        drift: Some(41),
        relocated_to: None,
    }];
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

#[test]
fn a_claim_that_shifted_below_the_threshold_is_mentioned_rather_than_called_unaffected() {
    let mut given = input("origin/main");
    given.unchanged = 66;
    given.moved = 2;
    given.detached = vec![(
        "route.go".to_owned(),
        "go://Host".to_owned(),
        "Host adds a matcher.".to_owned(),
        None,
    )];

    let rendered = review_markdown(&given);
    assert!(rendered.contains("66 other claims still resolve"), "{rendered}");
    assert!(
        !rendered.contains("unaffected"),
        "a claim that drifted is not unaffected, and saying so is a false statement \
         about the reviewer's own change: {rendered}"
    );
    assert!(rendered.contains("2 of those sit on code this change touched"), "{rendered}");
}

#[test]
fn nothing_is_said_about_drift_when_nothing_drifted() {
    let mut given = input("origin/main");
    given.unchanged = 5;
    given.moved = 0;
    assert!(!review_markdown(&given).contains("sit on code this change touched"));
}

#[test]
fn a_claim_whose_code_moved_names_the_file_it_moved_to() {
    let mut given = input("origin/main");
    given.stale = vec![StaleClaim {
        file: "auth.go".to_owned(),
        symbol: "go://BasicAuthForProxy".to_owned(),
        claim: "If the realm is empty, Proxy Authorization Required is used.".to_owned(),
        drift: Some(0),
        relocated_to: Some("proxyauth.go".to_owned()),
    }];
    let rendered = review_markdown(&given);

    assert!(
        rendered.contains("proxyauth.go"),
        "a reviewer sent to auth.go for a function that is no longer in auth.go stops \
         trusting the comment. verify already resolved it across the file boundary and \
         reported where it went: {rendered}"
    );
    assert!(
        !rendered.contains("(0% changed)"),
        "a construct lifted into another file has not changed, and saying it changed by \
         zero percent under a heading that says it changed reads as a defect: {rendered}"
    );
    assert!(
        !rendered.contains("the code beneath them changed"),
        "moving is not editing, and the two need different headings or the reviewer \
         goes looking for an edit that was never made: {rendered}"
    );
}

#[test]
fn a_claim_whose_code_was_edited_in_place_still_reads_as_edited() {
    let mut given = input("origin/main");
    given.stale = vec![StaleClaim {
        file: "auth.go".to_owned(),
        symbol: "go://BasicAuthForRealm".to_owned(),
        claim: "Search user in the slice of allowed credentials.".to_owned(),
        drift: Some(36),
        relocated_to: None,
    }];
    let rendered = review_markdown(&given);
    assert!(rendered.contains("the code beneath them changed"), "{rendered}");
    assert!(rendered.contains("(36% changed)"), "{rendered}");
    assert!(!rendered.contains("different file"), "{rendered}");
}

#[test]
fn a_clean_change_says_the_claims_resolved_not_that_they_are_true() {
    let mut given = input("origin/main");
    given.unchanged = 11;
    let rendered = review_markdown(&given);

    assert!(rendered.contains("11 recorded claims still resolve"), "{rendered}");
    assert!(
        !rendered.contains("still hold against"),
        "resolving an anchor says the construct was found, not that what was recorded about it is true. Changing a string literal a claim quotes leaves the node kinds identical, so drift is zero and the claim resolves, while the claim is now false. Asserting it holds is more than codedoc knows: {rendered}"
    );
}
