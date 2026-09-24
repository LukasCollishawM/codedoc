use std::fs;

use codedoc_core::{
    AnchorId, ContentFingerprint, ContextFingerprint, FileId, LedgerHead, RecordId,
    StructuralFingerprint,
};

const PAYLOADS: &[(&str, &str)] = &[
    ("empty", ""),
    ("ascii", "payload"),
    ("non_ascii", "café αβ 😀"),
    ("embedded_nul", "a\u{0}b\u{1f}c"),
    ("domain_prefix_lookalike", "codedoc.record.v1\u{0}payload"),
];

fn json_string(value: &str) -> String {
    let mut out = String::from("\"");
    for glyph in value.chars() {
        match glyph {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            other if (other as u32) < 0x20 => {
                out.push_str(&format!("\\u{:04x}", other as u32));
            }
            other => out.push(other),
        }
    }
    out.push('"');
    out
}

pub fn generate() -> Result<usize, String> {
    let mut entries: Vec<String> = Vec::new();
    for (label, payload) in PAYLOADS {
        let bytes = payload.as_bytes();
        let taken = [
            (RecordId::DOMAIN, RecordId::of(bytes).to_string()),
            (AnchorId::DOMAIN, AnchorId::of(bytes).to_string()),
            (FileId::DOMAIN, FileId::of(bytes).to_string()),
            (ContentFingerprint::DOMAIN, ContentFingerprint::of(bytes).to_string()),
            (StructuralFingerprint::DOMAIN, StructuralFingerprint::of(bytes).to_string()),
            (ContextFingerprint::DOMAIN, ContextFingerprint::of(bytes).to_string()),
            (LedgerHead::DOMAIN, LedgerHead::of(bytes).to_string()),
        ];
        for (domain, digest) in taken {
            entries.push(format!(
                "    {{\n      \"name\": {},\n      \"domain\": {},\n      \"payload_utf8\": {},\n      \"digest\": \"{digest}\"\n    }}",
                json_string(&format!("{domain}/{label}")),
                json_string(domain),
                json_string(payload),
            ));
        }
    }

    let document = format!(
        "{{\n  \"version\": 1,\n  \"description\": {},\n  \"vectors\": [\n{}\n  ]\n}}\n",
        json_string(
            "Digest domain vectors. The pre-image is the domain string, then one 0x00 byte, \
             then the payload bytes, hashed with BLAKE3-256 and rendered as 64 lowercase \
             hexadecimal characters. Identical payloads under different domains must differ."
        ),
        entries.join(",\n")
    );

    let count = entries.len();
    fs::write("conformance/encoding/domains.json", document)
        .map_err(|failure| failure.to_string())?;
    Ok(count)
}
