use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

use codedoc_core::{
    AnchorId, Canonical, ContentFingerprint, ContextFingerprint, FileId, LedgerHead, RecordId,
    StructuralFingerprint,
};

fn load() -> Canonical {
    let path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../conformance/encoding/domains.json");
    let raw = fs::read(path).expect("digest domain vectors are missing");
    Canonical::decode(&raw).expect("digest domain vectors are not canonical JSON")
}

fn field<'a>(value: &'a Canonical, name: &str) -> &'a str {
    value
        .field(name)
        .and_then(Canonical::as_text)
        .unwrap_or_else(|| panic!("vector is missing the {name} member"))
}

fn digest_under(domain: &str, payload: &[u8]) -> String {
    match domain {
        RecordId::DOMAIN => RecordId::of(payload).to_string(),
        AnchorId::DOMAIN => AnchorId::of(payload).to_string(),
        FileId::DOMAIN => FileId::of(payload).to_string(),
        ContentFingerprint::DOMAIN => ContentFingerprint::of(payload).to_string(),
        StructuralFingerprint::DOMAIN => StructuralFingerprint::of(payload).to_string(),
        ContextFingerprint::DOMAIN => ContextFingerprint::of(payload).to_string(),
        LedgerHead::DOMAIN => LedgerHead::of(payload).to_string(),
        other => panic!(
            "the vectors name a domain this implementation does not have: {other}. Section 2 \
             of the specification lists the domains, and one appearing here that the code \
             cannot produce means the two disagree about the format"
        ),
    }
}

fn vectors(document: &Canonical) -> &[Canonical] {
    let Some(Canonical::Array(entries)) = document.field("vectors") else {
        panic!("vectors member must be an array");
    };
    entries
}

#[test]
fn every_domain_vector_reproduces_its_digest() {
    let document = load();
    let entries = vectors(&document);
    assert!(!entries.is_empty(), "the vector set must not be empty");

    for vector in entries {
        let name = field(vector, "name");
        let domain = field(vector, "domain");
        let payload = field(vector, "payload_utf8");
        let expected = field(vector, "digest");
        assert_eq!(
            digest_under(domain, payload.as_bytes()),
            expected,
            "vector {name} does not reproduce. Every identifier in every ledger is taken \
             under one of these domains, so an implementation that disagrees here agrees \
             about nothing else"
        );
    }
}

#[test]
fn every_domain_in_the_specification_is_covered() {
    let document = load();
    let covered: BTreeSet<&str> =
        vectors(&document).iter().map(|vector| field(vector, "domain")).collect();

    for domain in [
        RecordId::DOMAIN,
        AnchorId::DOMAIN,
        FileId::DOMAIN,
        ContentFingerprint::DOMAIN,
        StructuralFingerprint::DOMAIN,
        ContextFingerprint::DOMAIN,
        LedgerHead::DOMAIN,
    ] {
        assert!(
            covered.contains(domain),
            "section 2 defines {domain} and no vector pins it. A second implementation \
             could get its separation wrong and still pass conformance, which is what \
             these vectors exist to prevent. Regenerate with `cargo run -p xtask -- vectors`"
        );
    }
}

#[test]
fn identical_payloads_under_different_domains_differ() {
    let document = load();
    let mut digests: Vec<(&str, &str)> = Vec::new();
    for vector in vectors(&document) {
        let name = field(vector, "name");
        let Some((_, label)) = name.rsplit_once('/') else {
            panic!("a vector name is domain/label: {name}");
        };
        digests.push((label, field(vector, "digest")));
    }

    for (label, digest) in &digests {
        let collisions =
            digests.iter().filter(|(other, taken)| other == label && taken == digest).count();
        assert_eq!(
            collisions, 1,
            "two domains produced the same digest for payload {label}, so domain \
             separation is not doing anything and a file digest could be read as a \
             record digest"
        );
    }
}
