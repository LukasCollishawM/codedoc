use std::fs;
use std::path::PathBuf;

use codedoc_ledger::Record;

fn fixtures() -> String {
    let path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../conformance/records/identity.jsonl");
    fs::read_to_string(path).expect("record identity fixtures are missing")
}

#[test]
fn a_record_keeps_the_identity_it_was_written_with() {
    let raw = fixtures();
    let mut checked = 0usize;
    for line in raw.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (expected, encoded) = line.split_once(' ').expect("each fixture is `<id> <line>`");
        let record = Record::decode_line(encoded.as_bytes())
            .unwrap_or_else(|error| panic!("fixture does not decode: {error}"));
        assert_eq!(
            record.id().to_string(),
            expected,
            "a record written by an earlier version of codedoc must keep the identity it \
             was written with. A record's id is the hash of its canonical encoding, so a \
             member that serialises when it holds its default silently re-identifies every \
             record ever written, breaking the hash chain and every supersession link that \
             points at them. An optional member MUST be omitted when it holds its default."
        );
        if !encoded.contains("symbol_cardinality") {
            for entry in &record.content().anchors {
                assert_eq!(
                    entry.anchor.symbol_cardinality, 1,
                    "a record written before cardinality existed must read as one                      declaration, or every anchor in it silently stops matching"
                );
                assert_eq!(entry.anchor.symbol_ordinal, 0);
            }
        }

        let reencoded = record.encode_line().expect("a decoded record re-encodes");
        assert_eq!(
            String::from_utf8(reencoded).unwrap(),
            encoded,
            "re-encoding a stored record must reproduce its bytes exactly"
        );
        checked += 1;
    }
    assert!(checked >= 2, "the identity fixture set must not be empty");
}
