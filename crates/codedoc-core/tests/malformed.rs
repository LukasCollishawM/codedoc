use codedoc_core::Canonical;
use proptest::prelude::*;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(600))]

    #[test]
    fn arbitrary_bytes_are_rejected_rather_than_fatal(raw in proptest::collection::vec(any::<u8>(), 0..400)) {
        let _ = Canonical::decode(&raw);
    }

    #[test]
    fn arbitrary_text_is_rejected_rather_than_fatal(raw in ".{0,400}") {
        let _ = Canonical::decode(raw.as_bytes());
    }

    #[test]
    fn json_shaped_noise_is_rejected_rather_than_fatal(
        raw in r#"[\{\}\[\]",:0-9a-z\ eE.+-]{0,300}"#
    ) {
        let _ = Canonical::decode(raw.as_bytes());
    }

    #[test]
    fn a_declared_length_far_beyond_the_input_does_not_reserve_it(size in 1u64..u64::MAX) {
        let claimed = format!(r#"{{"length":{size},"body":"short"}}"#);
        let _ = Canonical::decode(claimed.as_bytes());
    }

    #[test]
    fn whatever_decodes_re_encodes_to_itself(raw in r#"[\{\}\[\]",:0-9a-z ]{0,200}"#) {
        if let Ok(value) = Canonical::decode(raw.as_bytes()) {
            let encoded = value.encode();
            let again = Canonical::decode(&encoded)
                .expect("canonical bytes decode");
            prop_assert_eq!(
                again.encode(),
                encoded,
                "encoding is idempotent by specification, and a value that re-encodes \
                 differently changes the identity of every record containing it"
            );
        }
    }
}
