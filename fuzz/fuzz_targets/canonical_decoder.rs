#![no_main]

use codedoc_core::Canonical;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let Ok(decoded) = Canonical::decode(data) else {
        return;
    };
    let encoded = decoded.encode();
    let again = Canonical::decode(&encoded)
        .expect("canonical output must always decode");
    assert_eq!(
        encoded,
        again.encode(),
        "canonical encoding must be a fixed point for every input that decodes"
    );
});
