# ADR-0003: A committed JSONL ledger with a derived index

Status: Accepted

## Context

The ledger is repository state that must survive review, merge and inspection by tools we do not control, while queries need to be fast.

## Decision

Records are canonical JSON, one per line, under `.codedoc/ledger/`, sharded by the first two hex characters of the record identifier. A SQLite projection at `.codedoc/index.sqlite` is derived, gitignored and rebuildable.

## Alternatives rejected

**Binary encoding (CBOR, protobuf).** More compact and faster to parse. Rejected because a ledger diff is something humans review in pull requests, and a format requiring our tooling to read is a format that loses the adoption argument. The SQLite projection carries query performance, so the ledger does not have to.

**A single append-only file.** Simpler ordering, but every concurrent branch would conflict on the final line.

## Consequences

Sharding means file order carries no meaning, so sequence is expressed by the `chain` member and reconstructed by following references. That is what makes a merge a union operation and the merge driver tractable. Everything derived must be reconstructible from the ledger alone.
