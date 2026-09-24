use std::fs;

use codedoc_ledger::{Ledger, Record};
use proptest::prelude::*;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(400))]

    #[test]
    fn a_record_line_of_arbitrary_bytes_is_rejected_rather_than_fatal(
        raw in proptest::collection::vec(any::<u8>(), 0..400)
    ) {
        let _ = Record::decode_line(&raw);
    }

    #[test]
    fn a_record_line_of_plausible_json_is_rejected_rather_than_fatal(
        raw in r#"\{[\{\}\[\]",:0-9a-z_\ -]{0,300}\}"#
    ) {
        let _ = Record::decode_line(raw.as_bytes());
    }
}

#[test]
fn a_corrupt_shard_names_the_line_rather_than_bringing_the_process_down() {
    let workspace = tempfile::tempdir().expect("a temporary directory");
    let ledger = Ledger::initialise(workspace.path()).expect("a ledger");
    let shard = ledger.base().join("ledger").join("aa.jsonl");
    fs::write(&shard, b"{\"not\": \"a record\"}\n").expect("a shard is written");

    let failure = ledger.records().expect_err("a malformed shard is an error");
    let rendered = failure.to_string();
    assert!(
        rendered.contains("line 1"),
        "a cloned repository's ledger is untrusted input, so a bad line has to be \
         reported precisely enough to find rather than crashed on: {rendered}"
    );
}
