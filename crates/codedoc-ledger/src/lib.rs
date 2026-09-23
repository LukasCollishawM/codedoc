#![forbid(unsafe_code)]

pub mod ledger;
pub mod record;

pub use ledger::{Ledger, LedgerError, Verification};
pub use record::{
    AnchorRole, Assurance, Author, Body, Evidence, Kind, Lifecycle, Record, RecordContent,
    RelationVerb, Role, SCHEMA_VERSION, Timestamp,
};
