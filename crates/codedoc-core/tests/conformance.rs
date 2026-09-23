use std::fs;
use std::path::PathBuf;

use codedoc_core::{Canonical, RecordId};

fn vectors_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../conformance/encoding/vectors.json")
}

fn load() -> Canonical {
    let raw = fs::read(vectors_path()).expect("conformance vectors are missing");
    Canonical::decode(&raw).expect("conformance vectors are not canonical JSON")
}

fn field<'a>(value: &'a Canonical, name: &str) -> &'a str {
    value
        .field(name)
        .and_then(Canonical::as_text)
        .unwrap_or_else(|| panic!("vector is missing the {name} member"))
}

#[test]
fn every_vector_reproduces_its_canonical_form_and_digest() {
    let document = load();
    let Some(Canonical::Array(vectors)) = document.field("vectors") else {
        panic!("vectors member must be an array");
    };
    assert!(!vectors.is_empty(), "the vector set must not be empty");

    for vector in vectors {
        let name = field(vector, "name");
        let input = field(vector, "input");
        let expected_canonical = field(vector, "canonical");
        let expected_digest = field(vector, "record_digest");

        let decoded = Canonical::decode(input.as_bytes())
            .unwrap_or_else(|failure| panic!("{name}: input does not decode: {failure}"));
        let encoded = decoded.encode();
        let rendered = String::from_utf8(encoded.clone())
            .unwrap_or_else(|_| panic!("{name}: canonical form is not valid utf-8"));

        assert_eq!(rendered, expected_canonical, "{name}: canonical form diverged");
        assert_eq!(
            RecordId::of(&encoded).to_string(),
            expected_digest,
            "{name}: record digest diverged"
        );
    }
}

#[test]
fn canonical_form_is_a_fixed_point() {
    let document = load();
    let Some(Canonical::Array(vectors)) = document.field("vectors") else {
        panic!("vectors member must be an array");
    };
    for vector in vectors {
        let name = field(vector, "name");
        let canonical = field(vector, "canonical");
        let again = Canonical::decode(canonical.as_bytes())
            .unwrap_or_else(|failure| panic!("{name}: canonical form does not decode: {failure}"))
            .encode();
        assert_eq!(
            String::from_utf8(again).unwrap(),
            canonical,
            "{name}: re-encoding canonical form must be identity"
        );
    }
}

#[test]
fn the_vector_file_is_itself_canonical_json() {
    let raw = fs::read(vectors_path()).expect("conformance vectors are missing");
    assert!(
        Canonical::decode(&raw).is_ok(),
        "the vector file must parse under the same rules it describes"
    );
}
