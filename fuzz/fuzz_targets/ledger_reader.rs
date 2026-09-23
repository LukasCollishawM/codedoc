#![no_main]

use codedoc_ledger::Record;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    for line in data.split(|byte| *byte == b'\n') {
        if line.is_empty() {
            continue;
        }
        if let Ok(record) = Record::decode_line(line) {
            let reencoded = record.encode_line().expect("a decoded record must re-encode");
            let restored =
                Record::decode_line(&reencoded).expect("a re-encoded record must decode");
            assert_eq!(
                restored.id(),
                record.id(),
                "identity must survive a decode and encode cycle, including members this \
                 build does not recognise"
            );
        }
    }
});
