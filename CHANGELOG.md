# Changelog

Notable changes to codedoc. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project will follow [Semantic Versioning](https://semver.org/) from 1.0.

Before 1.0 the on-disk format may change, but never without a mechanical `codedoc migrate` path.

## [Unreleased]

### Added

- **Relations are writable.** `codedoc relate` and `codedoc_relate` create records about the link between two pieces of code — `must_execute_after`, `guarded_by`, `constrained_by` and the rest. The vocabulary, the graph traversal and the context rendering all existed; nothing could create one, because `attach` took a single anchor.
- **`codedoc repair`** rebuilds a hash chain broken by editing a ledger outside codedoc, re-chaining in timestamp order and remapping supersession links. Found by breaking this repository's own ledger with a git rebase and a force push, which orphaned 15 of 34 records.
- **`--scope` is no longer ignored.** The global flag was parsed and never passed to any operation, so every write went to the default scope regardless. A flag that is accepted and silently ignored is worse than one that does not exist.
- **The VS Code client packages to an installable `.vsix`.** It was source-only before, which meant the human-facing half of the project could not actually be installed by a human.
- **LSP diagnostics.** The editor now warns where a claim may no longer describe the code beneath it, carrying the drift percentage, and notes where an anchor detached. Staleness was computable and invisible unless you ran `verify` in a terminal.
- **`codedoc_render`, `codedoc_import`, `codedoc_review`, `codedoc_conflicts` and `codedoc_coverage` for agents**, bringing the MCP surface to sixteen tools. An agent can now report what its own change put in doubt rather than leaving a reviewer to find out. Asked for onboarding notes, an agent can now generate them from verified anchors instead of from its own reading of the code. The five remaining CLI-only commands are administrative by intent.
- **Agent parity, and then some.** The MCP server exposes eleven tools, including the full record lifecycle. Both the CLI and MCP now sit on a shared `codedoc-ops` layer so the two surfaces cannot drift apart.
- **Semantic staleness.** Verification previously detected that an anchor had *moved*. It now also detects that the code *changed*: a function whose body is rewritten while its name and signature survive used to resolve perfectly and report as cleanly migrated, even when the claim attached to it had become false. Drift is measured against the recorded tree shape and reported as a percentage.
- **Trust weighting in retrieval.** Context is ranked by a score combining assurance, authorship and age, and a budget now drops the least trustworthy claims rather than whatever came last. Recency is floored so age alone cannot let a fresh agent guess displace a human assertion.
- **A pull request workflow** that runs `codedoc review` and posts one comment, updated in place across pushes rather than appended to.
- **`codedoc coverage`** reports what fraction of declarations carry a record, thinnest files first. Adoption needed a number: a team that imports two thousand comments had no way to ask whether that covered their auth layer or their string helpers. 1.8s over 114,633 declarations.
- **`codedoc review` also counts what the change touched that has no records**, as a footnote. Nothing previously prompted anyone to record anything at the moment they had the understanding to do it.
- **`codedoc review`** renders the claims a change has put in doubt as a pull request comment. Staleness was only visible to whoever ran `verify` in a terminal, which during review is nobody.
- **`codedoc render`** projects the ledger to markdown (an onboarding document grouped by file) or mermaid (the relation graph as a diagram). "Documentation is one projection" was the thesis; until now there was one projection and it was a terminal.
- **`codedoc conflicts`** reports declared contradictions, near-duplicate claims on the same code, and same-kind claims that disagree about certainty. Ledger hygiene for a corpus that agents write to continuously.
- **Ledger scopes.** `--scope local` keeps the ledger in `.git/codedoc/`, which git cannot track, so codedoc can be used on a repository you do not own without leaving evidence in it. `--scope global` keeps it outside the repository entirely.
- `AGENTS.md` and `docs/cli.md`, so the README can explain what codedoc is rather than double as a command reference.
- **Record lifecycle from the command line.** `codedoc supersede` revises a claim, `codedoc retract` retires one, and `codedoc resolve` places a detached anchor explicitly. `codedoc detached` lists anchors awaiting a decision. Records remain immutable; revision writes a superseding record and retraction writes a tombstone, so `codedoc history` still answers what was believed earlier.
- **`codedoc import`** harvests the comments an existing codebase already has, anchors each to the construct it documents, and classifies it by marker. Dry run by default, additive, and never edits source.
- **Rung 5 of the resolver.** A file renamed in git no longer detaches every anchor in it; the verifier consults history from the revision recorded on the record and re-resolves in the new path at medium confidence.
- **`codedoc migrate`** reports the schema distribution of a ledger, applies registered migrations, and refuses to operate on records written by a newer build.
- **`codedoc git install-merge-driver`** and the driver behind it, so concurrent branches union their ledger shards instead of conflicting. The driver refuses to write if any input line is not a valid record.
- **MCP server** (`codedoc-mcp`) built on the official `rmcp` SDK, exposing context retrieval, attach, verify and list.
- **LSP server** (`codedoc-lsp`) providing hovers and code lenses resolved live against the buffer.
- **Drift is now a conformance property.** Every vector that resolves states its expected drift, and the suite asserts it: zero for reformatting and renaming in all seven languages, 44 for a rewritten control flow. The spec had a normative staleness clause with nothing binding it.
- **Resolver vectors for every shipped language.** Java, Go, C# and TypeScript had adapters and no coverage at all; each now has reformat-holds and rename-holds, and the vectors carry the expected `symbol_cardinality` so the overload invariant is asserted rather than implied. A C++ case covers an in-class declaration and its out-of-line definition resolving to one symbol.
- **Conformance vectors** for canonical encoding and for the resolver, under `conformance/`, licensed CC0 so other implementations are unencumbered.
- **`cargo xtask lint-docs`** fails the build when a CLI subcommand is missing from the README, because documentation drift is a defect like any other.

### Changed

- **The replay harness reports why anchors detached**, not just how many. Over this repository's own history that turned "5 detached" into five constructs that were genuinely deleted, which is a different fact entirely.

- **The specification covers what the implementation does.** Symbol cardinality, drift-based staleness, scopes, and the rule that generic parameters never appear in a symbol path were all implemented before being written down, which inverts the rule this project states for itself. `docs/spec/format.md` now binds them, including the requirement that a local or global scope writes nothing to the working tree.

- **Verification runs files in parallel**: 30s to 7.7s at 1.09M lines on 32 cores. Per-file resolution shares nothing, and findings are sorted after collection, so output is byte-identical across runs.
- **Verification can be scoped to what you changed.** `codedoc verify <files>` or `--since <rev>` answers from the index instead of the whole ledger: 67ms against 30s at 1.09M lines. A scoped run skips the whole-ledger integrity scan and reports that it did, rather than implying it passed.
- **Context retrieval is 60x faster at scale.** Relation lookup issued one query per symbol, with a `LIKE` the kind index cannot serve and an unindexed subquery. At 1.09M LOC and 27,420 records that cost 1.98s against a 100ms budget, while passing comfortably at 50k. A single bound query, an index-usable range predicate and an index on `parent` bring it to 33ms.

- **The resolver no longer terminates on ambiguity.** A rung with more than one candidate falls through to rungs carrying more information rather than detaching immediately. On a 2095-record corpus this reduced detachment from 602 anchors to 121, without ever selecting among indistinguishable candidates. See [ADR-0004](docs/decisions/0004-ambiguity-falls-through.md).
- **Fingerprints are computed bottom-up and memoised**, making whole-file fingerprinting linear in node count rather than quadratic in tree size. See [ADR-0005](docs/decisions/0005-merkle-fingerprints.md).
- **`codedoc context` answers from the SQLite projection** instead of parsing the whole ledger: 230ms to 74ms.
- **`codedoc verify` reuses a per-file index** and accumulates symbol paths down the tree walk: 6.4s to 3.0s over 2095 anchors.
- Symbol paths no longer include generic parameters or path qualifiers, so renaming a lifetime does not move an anchor.

### Fixed

- **A false reattachment at high confidence.** Deleting one of two overloads reattached its record to the surviving overload, because the resolver assumed a symbol path uniquely identifies a declaration. Anchors now record how many declarations shared their symbol path, and both the symbol and similarity rungs refuse when that count has changed. Found while adding C++, where overloading is idiomatic, but the defect was language-agnostic and reproduced in Java.
- The index rebuilds itself when its schema version does not match, rather than failing on a stale column.
- Relations now resolve for the symbol being queried, not only for symbols of records already matched, so a symbol carrying only a relation is no longer reported as having none.
- Writing to a closed stdout no longer panics. Piping `verify` to `head` is ordinary use.
- `#[non_exhaustive]` wildcards across crate boundaries now fail safe: an unrecognised resolver rung classifies as detached, never as fresh.

### Security

- `RepoPath` rejects `CONIN$` and `CONOUT$`. The Windows device-name guard was missing the two console handles, and a path naming one is exactly what it exists to refuse.

- **Fuzz targets** for the canonical decoder, the ledger reader and anchor capture, covering the three places untrusted input crosses into the system. A reachable panic there is a denial-of-service bug, since a malformed source file is an ordinary input to a parser.

- **RUSTSEC-2026-0009** closed by removing the `time` dependency rather than raising the MSRV. It was used only to render an `i64` as RFC 3339. See [ADR-0008](docs/decisions/0008-drop-the-time-dependency.md).
