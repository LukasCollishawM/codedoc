use std::collections::BTreeMap;
use std::fmt;

use codedoc_anchor::{Anchor, Confidence as AnchorConfidence};
use codedoc_core::{Canonical, CanonicalError, GitRev, LedgerHead, RecordId};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Timestamp(i64);

impl Timestamp {
    pub fn now() -> Self {
        Timestamp(OffsetDateTime::now_utc().unix_timestamp())
    }

    pub fn from_unix_seconds(seconds: i64) -> Self {
        Timestamp(seconds)
    }

    pub fn unix_seconds(self) -> i64 {
        self.0
    }

    pub fn to_rfc3339(self) -> String {
        OffsetDateTime::from_unix_timestamp(self.0)
            .ok()
            .and_then(|moment| moment.format(&Rfc3339).ok())
            .unwrap_or_else(|| self.0.to_string())
    }
}

impl fmt::Display for Timestamp {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.to_rfc3339())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum RelationVerb {
    MustExecuteAfter,
    GuardedBy,
    ConstrainedBy,
    Invalidates,
    TestedBy,
    DerivedFrom,
    Contradicts,
    Supersedes,
    Owns,
}

impl RelationVerb {
    pub fn parse(text: &str) -> Option<RelationVerb> {
        let candidate = text.trim().to_ascii_lowercase().replace('-', "_");
        [
            RelationVerb::MustExecuteAfter,
            RelationVerb::GuardedBy,
            RelationVerb::ConstrainedBy,
            RelationVerb::Invalidates,
            RelationVerb::TestedBy,
            RelationVerb::DerivedFrom,
            RelationVerb::Contradicts,
            RelationVerb::Supersedes,
            RelationVerb::Owns,
        ]
        .into_iter()
        .find(|verb| verb.as_str() == candidate)
    }

    pub fn vocabulary() -> Vec<&'static str> {
        vec![
            "must_execute_after",
            "guarded_by",
            "constrained_by",
            "invalidates",
            "tested_by",
            "derived_from",
            "contradicts",
            "supersedes",
            "owns",
        ]
    }

    pub fn as_str(self) -> &'static str {
        match self {
            RelationVerb::MustExecuteAfter => "must_execute_after",
            RelationVerb::GuardedBy => "guarded_by",
            RelationVerb::ConstrainedBy => "constrained_by",
            RelationVerb::Invalidates => "invalidates",
            RelationVerb::TestedBy => "tested_by",
            RelationVerb::DerivedFrom => "derived_from",
            RelationVerb::Contradicts => "contradicts",
            RelationVerb::Supersedes => "supersedes",
            RelationVerb::Owns => "owns",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum Kind {
    Explanation,
    Rationale,
    Invariant,
    Precondition,
    Postcondition,
    Security,
    Performance,
    Assumption,
    Workaround,
    Specification,
    KnownFailureMode,
    Ownership,
    Decision,
    Warning,
    Relation(RelationVerb),
    Tombstone,
}

impl Kind {
    pub fn as_str(self) -> String {
        match self {
            Kind::Explanation => "explanation".to_owned(),
            Kind::Rationale => "rationale".to_owned(),
            Kind::Invariant => "invariant".to_owned(),
            Kind::Precondition => "precondition".to_owned(),
            Kind::Postcondition => "postcondition".to_owned(),
            Kind::Security => "security".to_owned(),
            Kind::Performance => "performance".to_owned(),
            Kind::Assumption => "assumption".to_owned(),
            Kind::Workaround => "workaround".to_owned(),
            Kind::Specification => "specification".to_owned(),
            Kind::KnownFailureMode => "known_failure_mode".to_owned(),
            Kind::Ownership => "ownership".to_owned(),
            Kind::Decision => "decision".to_owned(),
            Kind::Warning => "warning".to_owned(),
            Kind::Relation(verb) => format!("relation.{}", verb.as_str()),
            Kind::Tombstone => "tombstone".to_owned(),
        }
    }

    pub fn parse(text: &str) -> Option<Kind> {
        let candidate = text.trim().to_ascii_lowercase().replace('-', "_");
        if let Some(verb) = candidate.strip_prefix("relation.") {
            return RelationVerb::parse(verb).map(Kind::Relation);
        }
        [
            Kind::Explanation,
            Kind::Rationale,
            Kind::Invariant,
            Kind::Precondition,
            Kind::Postcondition,
            Kind::Security,
            Kind::Performance,
            Kind::Assumption,
            Kind::Workaround,
            Kind::Specification,
            Kind::KnownFailureMode,
            Kind::Ownership,
            Kind::Decision,
            Kind::Warning,
            Kind::Tombstone,
        ]
        .into_iter()
        .find(|kind| kind.as_str() == candidate)
    }

    pub fn vocabulary() -> Vec<String> {
        let mut names: Vec<String> = [
            Kind::Explanation,
            Kind::Rationale,
            Kind::Invariant,
            Kind::Precondition,
            Kind::Postcondition,
            Kind::Security,
            Kind::Performance,
            Kind::Assumption,
            Kind::Workaround,
            Kind::Specification,
            Kind::KnownFailureMode,
            Kind::Ownership,
            Kind::Decision,
            Kind::Warning,
        ]
        .into_iter()
        .map(Kind::as_str)
        .collect();
        names.extend(RelationVerb::vocabulary().into_iter().map(|verb| format!("relation.{verb}")));
        names
    }

    pub fn required_confidence(self) -> AnchorConfidence {
        match self {
            Kind::Invariant | Kind::Security | Kind::Precondition | Kind::Postcondition => {
                AnchorConfidence::High
            }
            _ => AnchorConfidence::Medium,
        }
    }

    pub fn is_safety_weighted(self) -> bool {
        self.required_confidence() >= AnchorConfidence::High
    }

    pub fn is_relation(self) -> bool {
        matches!(self, Kind::Relation(_))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum Role {
    Subject,
    Object,
}

impl Role {
    pub fn as_str(self) -> &'static str {
        match self {
            Role::Subject => "subject",
            Role::Object => "object",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnchorRole {
    pub role: Role,
    pub anchor: Anchor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum Assurance {
    Asserted,
    Inferred,
    Speculative,
}

impl Assurance {
    pub fn parse(text: &str) -> Option<Assurance> {
        match text.trim().to_ascii_lowercase().as_str() {
            "asserted" => Some(Assurance::Asserted),
            "inferred" => Some(Assurance::Inferred),
            "speculative" => Some(Assurance::Speculative),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Assurance::Asserted => "asserted",
            Assurance::Inferred => "inferred",
            Assurance::Speculative => "speculative",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "authority")]
#[non_exhaustive]
pub enum Author {
    Human { identity: String },
    Agent { model: String, session: String },
    Analyzer { name: String },
    Runtime { name: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "form", content = "value")]
#[non_exhaustive]
pub enum Evidence {
    GitRevision(GitRev),
    Test(String),
    Document(String),
    Record(RecordId),
    Url(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum Lifecycle {
    Active,
    Superseded,
    Tombstoned,
}

impl Lifecycle {
    pub fn as_str(self) -> &'static str {
        match self {
            Lifecycle::Active => "active",
            Lifecycle::Superseded => "superseded",
            Lifecycle::Tombstoned => "tombstoned",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Body {
    pub claim: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecordContent {
    pub schema: u32,
    pub kind: Kind,
    pub anchors: Vec<AnchorRole>,
    pub body: Body,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evidence: Vec<Evidence>,
    pub assurance: Assurance,
    pub author: Author,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code_revision: Option<GitRev>,
    pub created: Timestamp,
    pub lifecycle: Lifecycle,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<RecordId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chain: Option<LedgerHead>,
    #[serde(flatten, default, skip_serializing_if = "BTreeMap::is_empty")]
    pub unrecognised: BTreeMap<String, Canonical>,
}

pub const SCHEMA_VERSION: u32 = 1;

impl RecordContent {
    pub fn canonical(&self) -> Result<Canonical, CanonicalError> {
        Canonical::from_serializable(self)
    }

    pub fn compute_id(&self) -> Result<RecordId, CanonicalError> {
        Ok(RecordId::of(&self.canonical()?.encode()))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Record {
    id: RecordId,
    content: RecordContent,
}

impl Record {
    pub fn seal(content: RecordContent) -> Result<Self, CanonicalError> {
        let id = content.compute_id()?;
        Ok(Record { id, content })
    }

    pub fn id(&self) -> RecordId {
        self.id
    }

    pub fn content(&self) -> &RecordContent {
        &self.content
    }

    pub fn kind(&self) -> Kind {
        self.content.kind
    }

    pub fn subject(&self) -> Option<&Anchor> {
        self.content
            .anchors
            .iter()
            .find(|entry| entry.role == Role::Subject)
            .map(|entry| &entry.anchor)
    }

    pub fn object(&self) -> Option<&Anchor> {
        self.content
            .anchors
            .iter()
            .find(|entry| entry.role == Role::Object)
            .map(|entry| &entry.anchor)
    }

    pub fn encode_line(&self) -> Result<Vec<u8>, CanonicalError> {
        Ok(self.content.canonical()?.encode())
    }

    pub fn decode_line(line: &[u8]) -> Result<Self, CanonicalError> {
        let canonical = Canonical::decode(line)?;
        let content: RecordContent = canonical.into_deserializable()?;
        let recomputed = content.compute_id()?;
        Ok(Record { id: recomputed, content })
    }

    pub fn verify_identity(&self) -> Result<bool, CanonicalError> {
        Ok(self.content.compute_id()? == self.id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use codedoc_core::RepoPath;
    use codedoc_lang::Registry;

    fn sample_anchor() -> Anchor {
        let adapter = Registry::by_name("rust").unwrap();
        let source = "fn authorize() -> bool { true }";
        let tree = adapter.parse(source).unwrap();
        let node = tree.root_node().child(0).unwrap();
        Anchor::capture(RepoPath::parse("src/auth.rs").unwrap(), adapter, source, node)
    }

    fn sample_content() -> RecordContent {
        RecordContent {
            schema: SCHEMA_VERSION,
            kind: Kind::Invariant,
            anchors: vec![AnchorRole { role: Role::Subject, anchor: sample_anchor() }],
            body: Body { claim: "Authorization precedes reservation.".to_owned(), detail: None },
            evidence: Vec::new(),
            assurance: Assurance::Asserted,
            author: Author::Human { identity: "maintainer".to_owned() },
            code_revision: None,
            created: Timestamp::from_unix_seconds(1_700_000_000),
            lifecycle: Lifecycle::Active,
            parent: None,
            chain: None,
            unrecognised: BTreeMap::new(),
        }
    }

    #[test]
    fn sealing_is_deterministic() {
        let left = Record::seal(sample_content()).unwrap();
        let right = Record::seal(sample_content()).unwrap();
        assert_eq!(left.id(), right.id());
    }

    #[test]
    fn record_round_trips_through_its_canonical_line() {
        let record = Record::seal(sample_content()).unwrap();
        let line = record.encode_line().unwrap();
        let restored = Record::decode_line(&line).unwrap();
        assert_eq!(restored.id(), record.id());
        assert_eq!(restored.content(), record.content());
    }

    #[test]
    fn changing_any_field_changes_the_identity() {
        let baseline = Record::seal(sample_content()).unwrap();
        let mut altered = sample_content();
        altered.body.claim = "Something else entirely.".to_owned();
        let altered = Record::seal(altered).unwrap();
        assert_ne!(baseline.id(), altered.id());
    }

    #[test]
    fn unrecognised_fields_survive_a_round_trip() {
        let mut content = sample_content();
        content
            .unrecognised
            .insert("future_extension".to_owned(), Canonical::Text("preserved".to_owned()));
        let record = Record::seal(content).unwrap();
        let restored = Record::decode_line(&record.encode_line().unwrap()).unwrap();
        assert_eq!(
            restored.content().unrecognised.get("future_extension"),
            Some(&Canonical::Text("preserved".to_owned()))
        );
        assert_eq!(restored.id(), record.id());
    }

    #[test]
    fn safety_weighted_kinds_demand_high_confidence() {
        assert!(Kind::Invariant.is_safety_weighted());
        assert!(Kind::Security.is_safety_weighted());
        assert!(!Kind::Explanation.is_safety_weighted());
        assert_eq!(Kind::Explanation.required_confidence(), AnchorConfidence::Medium);
    }
}
