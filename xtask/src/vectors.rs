use std::fs;
use std::process::ExitCode;

use codedoc_core::{Canonical, RecordId};

const CASES: &[(&str, &str)] = &[
    ("keys_sort_ascending", r#"{"c":3,"a":1,"b":2}"#),
    ("nested_keys_sort", r#"{"z":{"y":1,"x":{"b":2,"a":1}},"a":[3,2,1]}"#),
    ("non_ascii_emitted_raw", r#"{"greek":"αβ","emoji":"😀"}"#),
    ("control_characters_escape_lowercase", r#"{"bell":"\u0007","unit":"\u001f"}"#),
    ("short_escape_forms", r#"{"s":"\b\f\n\r\t\"\\"}"#),
    ("integer_bounds", r#"{"max":9223372036854775807,"min":-9223372036854775808}"#),
    ("empty_containers", r#"{"array":[],"object":{}}"#),
    ("scalars", r#"{"null":null,"true":true,"false":false,"zero":0}"#),
    ("array_order_is_preserved", r#"{"ordered":[3,1,2,"a","A"]}"#),
    ("key_ordering_is_bytewise", r#"{"Z":1,"a":2,"A":3,"z":4,"é":5}"#),
];

pub fn run() -> ExitCode {
    let mut entries = Vec::new();
    for (name, input) in CASES {
        let Ok(value) = Canonical::decode(input.as_bytes()) else {
            eprintln!("vector {name} does not decode");
            return ExitCode::from(1);
        };
        let canonical = value.encode();
        let Ok(rendered) = String::from_utf8(canonical.clone()) else {
            eprintln!("vector {name} is not valid utf-8");
            return ExitCode::from(1);
        };
        let digest = RecordId::of(&canonical);
        entries.push(format!(
            "    {{\n      \"name\": {},\n      \"input\": {},\n      \"canonical\": {},\n      \"record_digest\": \"{}\"\n    }}",
            json_string(name),
            json_string(input),
            json_string(&rendered),
            digest
        ));
    }

    let document = format!(
        "{{\n  \"version\": 1,\n  \"description\": \"Canonical encoding vectors. Any implementation must reproduce canonical and record_digest exactly from input.\",\n  \"vectors\": [\n{}\n  ]\n}}\n",
        entries.join(",\n")
    );

    if let Err(failure) = fs::write("conformance/encoding/vectors.json", document) {
        eprintln!("writing vectors: {failure}");
        return ExitCode::from(1);
    }
    println!("wrote {} canonical encoding vectors", CASES.len());
    ExitCode::SUCCESS
}

fn json_string(raw: &str) -> String {
    String::from_utf8(Canonical::Text(raw.to_owned()).encode()).unwrap_or_default()
}
