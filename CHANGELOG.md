# Changelog

Notable changes to codedoc. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project will follow [Semantic Versioning](https://semver.org/) from 1.0.

Before 1.0 the on-disk format may change, but never without a mechanical `codedoc migrate` path.

## [Unreleased]

### Added

- **Record lifecycle from the command line.** `codedoc supersede` revises a claim, `codedoc retract` retires one, and `codedoc resolve` places a detached anchor explicitly. `codedoc detached` lists anchors awaiting a decision. Records remain immutable; revision writes a superseding record and retraction writes a tombstone, so `codedoc history` still answers what was believed earlier.
- **`codedoc import`** harvests the comments an existing codebase already has, anchors each to the construct it documents, and classifies it by marker. Dry run by default, additive, and never edits source.
- **Rung 5 of the resolver.** A file renamed in git no longer detaches every anchor in it; the verifier consults history from the revision recorded on the record and re-resolves in the new path at medium confidence.
- **`codedoc migrate`** reports the schema distribution of a ledger, applies registered migrations, and refuses to operate on records written by a newer build.
- **`codedoc git install-merge-driver`** and the driver behind it, so concurrent branches union their ledger shards instead of conflicting. The driver refuses to write if any input line is not a valid record.
- **MCP server** (`codedoc-mcp`) built on the official `rmcp` SDK, exposing context retrieval, attach, verify and list.
- **LSP server** (`codedoc-lsp`) providing hovers and code lenses resolved live against the buffer.
- **Conformance vectors** for canonical encoding and for the resolver, under `conformance/`, licensed CC0 so other implementations are unencumbered.
- **`cargo xtask lint-docs`** fails the build when a CLI subcommand is missing from the README, because documentation drift is a defect like any other.

### Changed

- **The resolver no longer terminates on ambiguity.** A rung with more than one candidate falls through to rungs carrying more information rather than detaching immediately. On a 2095-record corpus this reduced detachment from 602 anchors to 121, without ever selecting among indistinguishable candidates. See [ADR-0004](docs/decisions/0004-ambiguity-falls-through.md).
- **Fingerprints are computed bottom-up and memoised**, making whole-file fingerprinting linear in node count rather than quadratic in tree size. See [ADR-0005](docs/decisions/0005-merkle-fingerprints.md).
- **`codedoc context` answers from the SQLite projection** instead of parsing the whole ledger: 230ms to 74ms.
- **`codedoc verify` reuses a per-file index** and accumulates symbol paths down the tree walk: 6.4s to 3.0s over 2095 anchors.
- Symbol paths no longer include generic parameters or path qualifiers, so renaming a lifetime does not move an anchor.

### Fixed

- Writing to a closed stdout no longer panics. Piping `verify` to `head` is ordinary use.
- `#[non_exhaustive]` wildcards across crate boundaries now fail safe: an unrecognised resolver rung classifies as detached, never as fresh.

### Security

- **RUSTSEC-2026-0009** closed by removing the `time` dependency rather than raising the MSRV. It was used only to render an `i64` as RFC 3339. See [ADR-0008](docs/decisions/0008-drop-the-time-dependency.md).
