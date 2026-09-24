# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

codedoc is a **persistent semantic memory protocol for source repositories**. Documentation is one projection of it, not the point of it.

Comments are knowledge trapped in the one medium that cannot be queried, versioned independently, typed, contradicted, superseded, or attached to more than one place at once. codedoc moves that knowledge into an append-only ledger of immutable, content-addressed records anchored to **program structure** — not to line numbers, not to files. The source file keeps only executable source. Everything else lives beside it and is projected back on demand.

The consumer is an agent. Human legibility is a rendering concern, delivered by the LSP server and `codedoc render`. Design every interface for the agent first; if the human projection is inconvenient, fix the renderer, never the protocol.

The thesis this repository exists to prove:

```
today:     repo -> agent infers -> agent acts -> understanding discarded
codedoc:   repo + accumulated understanding -> agent acts -> repo + understanding + delta
```

`.codedoc/` is the mechanism by which a repository develops institutional memory.

## The four invariants

These are not guidelines. A change that violates one is wrong regardless of what it enables. They are enforced in CI and each has a dedicated test suite.

**I1 — The ledger is the only truth.** Every index, cache, and projection is derived and must be reconstructible from the ledger alone. `codedoc reindex` deletes the index and rebuilds it from the ledger, and must produce byte-identical output every time. If a piece of state cannot be rebuilt, it does not belong outside the ledger.

**I2 — Never silently reattach.** A resolution either clears its evidence threshold or the anchor becomes `DETACHED` and waits for adjudication. **Ambiguity is failure, not a tiebreak.** Two candidates at the same rung means DETACHED, always. Low survival rates are a quality problem and get iterated on; a single false reattachment is a corruption event, because it makes the corpus confidently wrong, which is worse than empty. Target anchor survival is soft. **False-reattachment rate is a hard zero.**

**I3 — Records are immutable.** No record is ever edited or deleted. Change emits a superseding record; removal emits a tombstone. History is queryable by construction — "what did we believe about this code at commit X" is a primary query, not an archaeology exercise.

**I4 — Canonical encoding is law.** Identical input produces identical bytes and therefore identical hashes, on every platform, forever. Sorted keys, no floats, UTF-8 NFC, explicit integer widths, `BTreeMap` never `HashMap` anywhere a value reaches serialization. Unknown fields survive a decode/encode round-trip untouched — the ledger is distributed, and an old binary must not silently strip what a new one wrote. **An optional field must be omitted when it holds its default**, and this is not a style preference. A record's id is the hash of its canonical encoding, recomputed on read, so a field that serialises at its default re-identifies every record written before it existed — breaking the hash chain and every supersession link into them, and presenting as a corrupt ledger rather than as a schema change. Four `#[serde(default)]` fields on `Anchor` did exactly that here and orphaned 43 of 44 records. `conformance/records/identity.jsonl` freezes records that carry none of the optional fields; if you add a field and that test fails, the field serialises at its default and the format change is not backward compatible.

## Architecture

```
┌───────────────────────────────────────────────────────────────┐
│ projections   LSP hovers/gutters · rustdoc · mermaid · site   │  codedoc-render
│               PR comments · onboarding docs · arch diagrams   │  codedoc-lsp
├───────────────────────────────────────────────────────────────┤
│ agent API     attach · supersede · tombstone · query          │  codedoc-mcp
│               traverse · verify · adjudicate · context        │  codedoc-cli
├───────────────────────────────────────────────────────────────┤
│ context       retrieval + ranking + packing to a token budget │  codedoc-context
├───────────────────────────────────────────────────────────────┤
│ graph         typed records · relations · lifecycle           │  codedoc-graph
│               provenance · confidence · supersession chains   │  codedoc-verify
├───────────────────────────────────────────────────────────────┤
│ query         derived SQLite projection, rebuildable (I1)     │  codedoc-index
├───────────────────────────────────────────────────────────────┤
│ ledger        CAS objects · hash chain · git integration      │  codedoc-ledger
├───────────────────────────────────────────────────────────────┤
│ anchor        tree-sitter CST · fingerprints · resolver ladder│  codedoc-anchor
│               per-language normalization adapters             │  codedoc-lang
├───────────────────────────────────────────────────────────────┤
│ primitives    ids · hashes · canonical codec · newtypes       │  codedoc-core
└───────────────────────────────────────────────────────────────┘
                              source code
```

Dependencies flow strictly downward. `core <- anchor <- lang`; `core <- ledger <- index`; `graph` over core+ledger; `context` over graph+index+anchor; cli/lsp/mcp/render at the top only. An upward or lateral dependency between peer crates is a design error — resolve it by moving the shared concept down, never by adding the edge.

`editors/vscode/` is a thin JSON-RPC client over `codedoc-lsp`, and any other editor client is held to the same shape. They contain **no** anchor logic, no ledger logic, and no schema knowledge beyond the wire types. Any editor feature requiring new intelligence is implemented in the server.

## Anchors

An anchor is a durable reference to program structure. It carries several independent signals, each of which fails differently, which is what lets the resolver distinguish "this moved" from "this changed":

- `symbol_path` — language-normalized, e.g. `csharp://PaymentService/AuthorizeAsync`. Coarse and stable.
- `node_path` — structural path within the symbol over **named** tree-sitter nodes only, e.g. `body/if[2]/consequence`. Skipping anonymous nodes makes it immune to formatting.
- `structural_fingerprint` — subtree shape: node kinds and field names, identifiers and literals excluded. Survives renames; detects reshaping.
- `content_fingerprint` — normalized token stream including identifiers, whitespace and comments stripped. Near-exact identity.
- `context_fingerprints` — preceding and following sibling subtrees. Relocates a node that moved without changing.
- `range` — cached `line:col`. **A cache. Never authoritative.** Code that treats it as identity is a bug.

The resolver is a ladder. Each rung yields a confidence, and the rung that fired is **recorded in the resolution** — a `Resolution` value cannot exist without its provenance, because the type makes that unrepresentable:

```
0  subject is the file, and the file exists            -> Exact      (module-level claims)
1  content_fingerprint unique match                    -> Exact
2  structural_fingerprint + symbol_path unique         -> Exact      (identifier renames)
3  symbol_path + node_path                             -> High
4  context fingerprints bracket a unique region        -> Medium
5  git rename/hunk migration from recorded revision    -> Medium
6  normalized-token similarity, unique best by margin  -> Low        (never auto-accepted)
7  otherwise                                           -> Detached
```

Rung 0 is not a fallback and does not participate in the ladder. An anchor either says it is about a construct, in which case rungs 1-6 search for that construct, or it says it is about the file, in which case its identity is the path and the resolver checks the path exists. Not all knowledge about code is knowledge about a declaration: a module header, or an invariant every entry point in a file upholds, has the file as its genuine subject. Pinning such a claim to the nearest declaration is both wrong and unresolvable — on a 1.09M-line corpus it was 620 of the 1,036 anchors that carried no symbol and so could never resolve.

Rung 6 never auto-accepts. Anything landing at `Low` is queued for adjudication. Kinds carrying safety weight — `invariant`, `security`, `precondition`, `postcondition` — require `High` or better and are otherwise detached even when a plausible candidate exists. An invariant is the kind of claim a reader will act on without re-deriving, so attaching one to the wrong construct does more damage than attaching an explanation there.

**Language coverage, first wave:** Rust, C#, TypeScript, Python, Go, Java. Rust because the core is Rust and dogfooding is a gate. C# because it is the origin context. Python is **mandatory in the first wave specifically because it is structurally alien** — a resolver tuned only on brace languages silently encodes brace assumptions into its normalization, and that must surface in week one rather than year one. Each language is an adapter implementing one trait; a language-specific branch anywhere outside `codedoc-lang` is a leak.

## Records

Immutable, content-addressed by blake3 over canonical bytes. Every record carries `id`, `parent` (supersession chain), `chain` (ledger head at append), `kind`, `anchors`, `body`, `evidence`, `confidence`, `author`, `code_revision`, `created`, `lifecycle`.

Two shapes, and the distinction is load-bearing:

**Assertions** attach to one anchor: `explanation`, `rationale`, `invariant`, `precondition`, `postcondition`, `security`, `performance`, `assumption`, `workaround`, `specification`, `known_failure_mode`, `ownership`, `decision`, `warning`.

**Relations** connect two role-tagged anchors and are the reason this is a graph rather than a comment store: `must_execute_after`, `guarded_by`, `constrained_by`, `invalidates`, `tested_by`, `derived_from`, `contradicts`, `supersedes`, `owns`. These encode facts belonging to neither endpoint, which therefore have no home in a comment — the clearest single argument for the whole format.

`confidence` is `asserted | inferred | speculative`, orthogonal to `author`, which is `human | agent{model, session} | analyzer | runtime`. An agent's speculation and a human's assertion must never be indistinguishable at query time, and the context packer weights them differently.

The kind vocabulary is **closed and versioned**. Extending it is a schema change with a decision record. Never renumber, never reuse a retired discriminant.

## Storage

```
.codedoc/
  ledger/*.jsonl     append-only, canonical JSON, one record per line, hash-prefix sharded
  objects/           content-addressed blobs for large bodies
  index.sqlite       derived, gitignored, rebuildable (I1)
  config.toml
```

The ledger is committed text, not binary. Reviewability in a PR diff and greppability without tooling beat encoding efficiency; the SQLite projection carries query performance, so the ledger does not have to. Sharding by hash prefix plus append-only semantics makes merges a union operation — ship the driver via `codedoc git install-merge-driver` and never make a user hand-resolve a conflict in an append-only log.

## Adoption

codedoc must be addable to a codebase on day 40,000, not day one. A tool that only works if adopted before the first commit has no users. This is a design constraint with teeth, and several parts of the system exist only to satisfy it.

**Bootstrapping is `codedoc import`.** It reads the comments a repository already has, determines which construct each documents, anchors it there, and classifies it by marker — `TODO` to `warning`, `SAFETY:` to `security`, "because…" to `rationale`. It is dry-run by default and requires `--write`. It is **additive and never edits source**: whether a comment is later deleted is the adopting team's decision, and a tool that rewrites source files on first contact does not get a second chance.

**The zero-comments rule is this repository's dogfooding standard, not a precondition of using codedoc.** Nothing in the tool requires it, `cargo xtask lint-comments` scans only our own crates, and a codebase can hold records and comments side by side forever. If that ever reads as a requirement, adoption dies; say so explicitly wherever the rule appears.

**Partial coverage is the normal state.** Most of a mature repository will have no records. `verify` reports only on anchors that exist and must never fail because coverage is low.

**Bulk paths must not be quadratic.** `Ledger::append` resolves the head by reading every record, so loops over it are O(n²); bulk work goes through `append_batch`, which resolves the head once. `Index::append` adds a single record rather than rebuilding the projection. Assume every operation will one day meet a repository with a hundred thousand records.

## Code standards

Written for a repository whose entire purpose is the claim that code should carry no prose.

### No comments. None.

No `//`, no `/* */`, anywhere under `crates/**/src`. This is the thesis, enforced by `cargo xtask lint-comments` in CI. It binds this repository only — see **Adoption** — and the lint's failure output prints the `codedoc attach` command that replaces the comment, because a rule that only rejects is gatekeeping.

Explanation goes in the ledger. Until the ledger can hold it, it goes in `docs/decisions/`. `///` doc comments are permitted on `pub` items only. No `pub` item in this repository currently carries one: the sixty-six that exist are all in `crates/codedoc-mcp/src/main.rs`, on `JsonSchema` argument structs, where `schemars` compiles them into the tool schema the server sends to agents. Those are an interface rather than documentation and cannot be moved to the ledger.

`unsafe` is banned outright, so `// SAFETY:` never arises. Every `#[allow(...)]` must have a corresponding `workaround` record anchored to that item; `cargo xtask lint-allows` fails CI otherwise. The suppression and its justification are linked by structure rather than by adjacency.

If you feel the urge to write a comment, that urge is the product's input signal. Emit a record.

### Types

Newtype every identifier: `RecordId`, `AnchorId`, `SymbolPath`, `FileId`, `GitRev`, `Blake3`, `LedgerHead`. A bare `String` crossing a module boundary as an identifier is a defect. Parse, don't validate — the constructor is the only door into a valid value, and nothing past it re-checks.

Make illegal states unrepresentable. `Resolution` carries its rung. A `Record` cannot exist without provenance. A detached anchor cannot be read as though resolved. Prefer the compiler refusing over a runtime guard, always.

No booleans in signatures — `resolve(path, true, false)` is unreadable at the call site and an enum costs nothing.

`Canonical` is the deliberate exception and must stay one: the specification closes the set of canonical values at null, boolean, integer, string, array and object, so a consumer matching on it should be forced to handle every variant rather than given a wildcard to hide behind. Marking it `#[non_exhaustive]` would let a seventh kind of value slip silently through every downstream match, which is the opposite of what the attribute is for here.

Every public enum and struct that can grow — record kinds, relation verbs, error types, resolver rungs — carries `#[non_exhaustive]`. This produces a deliberate asymmetry, and it is the right one: the attribute does not apply within the defining crate, so adding a record kind still breaks our own build until every site handles it, while downstream consumers keep compiling. Growth in the vocabulary must be free for the ecosystem and expensive for us.

Two rules follow from that asymmetry, and the second one matters more than it looks:

**Inside the defining crate, no catch-all `_ =>`.** Exhaustiveness is how a newly added record kind finds every site obliged to handle it. When another of our crates needs a projection of an enum — a string form, a category — add the method next to the enum, where the match is exhaustive, rather than matching across the boundary. `Kind::as_str` and `Role::as_str` exist for exactly this reason.

**Across a crate boundary the wildcard is mandatory, so it must fail safe.** A wildcard over a variant that did not exist when the code was written is a prediction about the future, and the only honest prediction is the conservative one. `codedoc-verify` maps an unrecognised resolver rung to `Detached`, never to `Fresh`: a rung added by a later version must degrade into adjudication rather than silently present itself as verified. Every cross-crate wildcard picks the arm that would be safe if the unknown variant turned out to be the most dangerous one. `codedoc review` had one pointing the other way: an unrecognised verification status counted as unchanged, so a future status would have been reported to a reviewer as a claim that still holds. It now falls into the list of claims worth re-reading, which is the arm that is safe if the unknown status turns out to mean something is wrong. **Within the defining crate there is no excuse for a wildcard at all** — exhaustiveness is the compiler forcing the decision, which is the whole reason for the attribute.

### Errors

Libraries use `thiserror` with typed, exhaustive enums. `anyhow`/`miette` appear only at binary edges. No `Box<dyn Error>` in a library signature — it discards precisely the information the caller needs.

`unwrap()` outside tests is a defect. `expect()` is permitted only where the invariant is guaranteed by construction **and** carries an `invariant` record. No `panic!` on any path reachable from user input or file contents — a malformed source file is an expected input to a parser, not an exceptional one.

### Structure

One level of abstraction per function. Early return over nesting; depth 3 is the ceiling. No `utils`, `helpers`, `common`, or `misc` modules — a name that does not say what is inside will collect anything. No `Manager`, `Handler`, `Service`, or `Processor` suffixes unless that word is genuinely the domain term. `pub(crate)` by default; widening visibility is a deliberate API decision.

Do not add a trait with one implementation. Do not add indirection for a second case that does not yet exist. The crate boundaries above already encode the extension points that were worth predicting.

### Documentation is part of the change, not a follow-up

**Read `notes/WRITING.md` before editing any prose file in this repository.** It is untracked, it is binding, and it is strict: no temporal framing, no narration of the document by itself, no rhetorical beats, no slogans, no aphorisms, terms defined at first use, transcripts verbatim. Every rule in it was broken here first and the fix reverted into it. The final section is a checklist to run against the diff.

A change that alters behaviour and leaves the documentation describing the old behaviour is incomplete, and it is incomplete in the worst way: the repository asserts something false, with authority. Update the docs in the same commit as the code.

What has to stay true, and who it is for:

- **`README.md`** is the only thing most people will read. If you add, rename or remove a command, change a flag, change install steps, or change what is or is not implemented, it changes here in the same commit. The **Status** section is a standing promise about what works — when you implement something it listed as missing, remove it from that list.
- **`docs/spec/format.md`** is normative and binds other implementations. Any change to on-disk bytes, digest domains, the resolver ladder, or the kind vocabulary changes the spec *and* `conformance/` first, then the code.
- **`CLAUDE.md`** (this file) carries architecture, invariants and standards. Correct it when reality diverges — including recording measurements that disprove an earlier claim, rather than quietly restating the claim.
- **`CONTRIBUTING.md`**, **`SECURITY.md`** track the contributor workflow and the threat model; a new trust boundary belongs in the latter.
- **`CHANGELOG.md`** gets an entry for anything user-visible.

Two habits that keep this honest. **Never document an intention as though it were a fact** — if a command is planned, it belongs in Status as missing, not in the command table as though it runs. And **run what you document**: the install instructions, the quickstart and the examples are claims, so execute them before committing them. The README once described a tool nobody could install because no install step had ever been run.

`cargo run -p xtask -- lint-docs` mechanically checks the part that can be checked: every CLI subcommand appears in the README command table, and every command the README advertises actually exists. Prose accuracy is still yours to maintain.

### CLI and agent surface

Every command emits stable JSON under `--json`, and the human renderer is written **over** that JSON, never the reverse. Divergence between the two output paths is a bug, and snapshot tests cover both from the one source. Exit codes are semantic: `0` clean, `1` stale documentation present, `2` detached anchors requiring adjudication, `3` ledger integrity failure. Agents branch on these.

### Testing

Property tests are the primary instrument for the resolver, because the core claim is universally quantified: **for any tree and any edit script, the resolution is correct or `Detached` — never wrong.** That is one `proptest` property, and it is the most important test in the repository.

The replay harness (`cargo xtask replay`) walks real git history in real open-source repositories, migrating anchors commit by commit, reporting survival and false reattachment. Survival is tracked and improved. False reattachment is a hard zero and fails the build.

`insta` snapshots cover canonical encodings and CLI output; a golden-file suite pins cross-platform encoding determinism (I4). No mocks of our own code — test through public APIs, and when that is awkward the API is the problem. Every bug fix starts with the failing test, and that test gets a `known_failure_mode` record so the next agent inherits the lesson instead of rediscovering it.

Behaviour that the specification mandates is tested from `conformance/` vectors rather than from Rust-native fixtures, so that the same assertions bind any implementation.

### Performance budgets

Measured, not aspirational, and on the large corpus only — a figure from a small one has repeatedly failed to predict anything. The corpus is 120 crates from the cargo registry: **1,910 files, 1.09M LOC, 27,642 records**, imported from the comments already in them. Release builds, best of three.

| operation | 1.09M LOC | budget |
| --- | --- | --- |
| `import --write` | 28s | — |
| `doctor` | 5.0s | 60s |
| `verify` | 3.6s | 60s |
| `reindex` | 2.1s | 60s |
| `coverage` | 0.73s | 60s |
| `conflicts` | 0.59s | 60s |
| `list` | 0.36s | 60s |
| `evidence` | 0.27s | 60s |
| `search` | 54ms | 100ms |
| `context` (depth 2) | 40ms | 100ms |
| `brief` | 38ms | 100ms |

`gaps` is missing from that table on purpose. The scale corpus is 120 vendored crates with no git history, and `gaps` reads nothing else, so measuring it there would measure nothing. On codedoc's own repository it is **4.8s** for 502 declarations across 24 files — one `git log -L` process per declaration, in parallel. The cost is bounded by `FILES_EXAMINED`, not by corpus size, so a million-line repository costs the same as this one; what moves it is history depth. It is not yet bounded the way it could be: a range cannot be corrected more often than the file containing it, so once enough results are in hand whose correction counts beat the next file's, the remaining files need not be opened at all. That bound holds only where the file was not renamed inside the window, because `git log -L` follows a range across a rename and the `--name-only` pass that ranks files does not. Making it sound means following renames in the ranking pass too. Unimplemented, and not to be implemented as though the condition were not there.

Everything is inside budget at a million lines, but only after the scale test found something a smaller corpus could not. `context` was **1.98s** at 1M LOC while passing comfortably at 50k, because relation lookup ran one query per symbol, used a `LIKE 'relation.%'` that the kind index cannot serve, and an unindexed `NOT IN` subquery — a cost invisible until a file carried enough claims for the per-symbol loop to matter. One query with a bound `IN` list, a range predicate the index can use, and an index on `parent` took it to 33ms.

The lesson is worth more than the number: a budget met on a small corpus says nothing about an algorithm that is linear in the wrong variable. Measure on the large corpus before claiming a budget is met.

It happened again, and the second time the lesson had already been written down. `conflicts` compared every pair of active records, recomputing each one's subject symbol inside the inner loop, when only pairs sharing a symbol and a kind can ever match. On 43 records that is invisible. On 27,642 it is **164 seconds** — and it went unnoticed until `doctor` called it and a whole-corpus run took nearly three minutes. Bucketing by symbol and kind first, then comparing within buckets, took it to **0.6s**: the same findings, 271 times faster. Anything pairwise needs the large corpus before it is believed, and adding a command that calls three others means re-measuring all three.

**A green local gate says nothing about what you committed.** Every check here runs against the working tree, so none of them can see a file that was edited but never staged. That happened: three commits carried a module that existed only on one disk, because `git add` was given a pathspec for its old location, failed atomically, and its error was discarded. Local builds passed throughout; CI failed three times saying exactly what was wrong. Before trusting a push, either read CI or clone the pushed commit somewhere clean and build it — and never silence `git add`.

**An optimisation that is not one gets removed, not shipped.** A verification cache was built, measured at 25% on a case that barely occurs, and reverted — see [ADR-0010](docs/decisions/0010-no-verification-cache.md). Measure before and after; if the after is not clearly better, delete the code rather than keeping it because it was work.

**Always measure release builds.** Debug figures for this workload are five to twenty times worse and will send you optimising the wrong thing; an early `verify` reading of two minutes was mostly `-O0`.

## Open source

This ships as OSS, and for a format project that is a design constraint rather than a distribution choice.

### The compatibility surface is the format, not the API

Once a second implementation exists, `.codedoc/` bytes are a contract forever. Everything below follows from that.

`docs/spec/` holds a normative, implementation-independent specification, versioned separately from the crates. The Rust workspace is the **reference implementation, not the definition** — when code and spec disagree, one of them is a bug and the spec decides which. `conformance/` holds language-neutral vectors (input to canonical bytes to expected hash; anchor plus edit script to expected rung and confidence). The Rust test suite is one consumer of those vectors; a Go or TypeScript implementation must be able to run them without reading a line of Rust.

Before 1.0 the format may change, but never without a mechanical `codedoc migrate` path. After 1.0 it does not change incompatibly at all. A user's ledger is their institutional memory, accumulated over years and not reproducible — corrupting it is unforgivable in a way that breaking an API never is. When a format change and an ergonomics win are in tension, the format wins.

### Semver and published surface

Publish narrowly. A crate on crates.io is a permanent obligation, so crates whose API has not settled carry `publish = false` rather than a `0.x` promise nobody intends to keep. Every crate carries `publish = false` today, so nothing is on crates.io and `cargo-semver-checks` has nothing to check; it joins CI with the first publish. MSRV is declared in `rust-version`, tested in the matrix, and raised only in a minor release.

### Rules are enforced by machines, never by reviewers

Standards this strict, enforced by human review, become gatekeeping — and drive off precisely the contributors a young project needs. **Every rule in this file is a CI check or it is not a rule.** A reviewer must never be the first person to tell a contributor that their comment is not allowed; the lint says so locally, before the PR, and prints the command that fixes it. Lint messages carry the remedy, not merely the violation. If a standard cannot be checked mechanically, either build the check or drop the standard.

The no-comments rule in particular will startle every drive-by contributor. That is a documentation and tooling problem, and it is ours, not theirs.

### Licensing and provenance

Code is dual `MIT OR Apache-2.0`, the Rust ecosystem default, and Apache's explicit patent grant matters disproportionately for a format hoping to attract independent implementations. The spec and conformance vectors are `CC0-1.0` — nobody adopts a format whose specification is encumbered. Contributions are DCO sign-off, not a CLA; on a protocol project a CLA reads as a retained relicensing option and costs goodwill that is worth more than the option. `cargo deny` gates advisories, licences, and banned or duplicated crates.

### Security posture

A cloned repository's `.codedoc/` is attacker-controlled input, and so is every source file handed to the parser. The canonical decoder, the ledger reader, and the index writer all sit on that trust boundary: no panics, no allocation sized by an untrusted length field, no path in any record field escaping the repository root, and no record content reaching an executed context. `#![forbid(unsafe_code)]` in every crate. The no-panic property that section 9 of the spec requires is carried by proptest, in `crates/codedoc-core/tests/malformed.rs` and `crates/codedoc-ledger/tests/malformed.rs`, which throw arbitrary bytes at the canonical decoder and the record reader on every push. `fuzz/` holds libFuzzer targets for the same three entry points, run by hand for longer campaigns; they are not in CI, because a sixty-second run from an empty corpus re-explores what proptest already covers and the corpus is not persisted. The threat model belongs in the spec, since it binds other implementations too.

### Governance

Decisions happen in public. `docs/decisions/` carries ADRs, and any format change requires one. Conventional Commits drive a generated CHANGELOG. The CI matrix covers Linux, macOS, and Windows, because G1 is a cross-platform claim and only a matrix can substantiate it.

The README is a product surface, not a formality — this idea is unfamiliar enough that adoption depends on it being legible in sixty seconds.

## Commands

`just` is a convenience; `cargo` and `cargo xtask` are the real interface and work without it.

```bash
cargo build --workspace --all-targets
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check
cargo run -p xtask -- lint-comments          # the no-comments gate
cargo run -p xtask -- vectors                # regenerate conformance vectors
cargo run --release -p xtask -- replay --commits 200

cargo test -p codedoc-anchor --test resolver_invariant       # the I2 property test
cargo test -p codedoc-core --test conformance                # encoding vectors
cargo test -p codedoc-index --test rebuildable               # I1
cargo test -p codedoc-anchor resolver -- --nocapture         # one module, with output
```

The binaries, which must be built `--release` for any measurement:

```bash
codedoc init
codedoc import src/ [--write]                # adoption path; dry-run by default
codedoc attach <file> --symbol <s> --kind invariant --claim "..."
codedoc verify [--json]                      # 0 clean, 1 stale, 2 detached, 3 integrity
codedoc context <file>[:line] [--symbol s] [--depth n] [--budget n]
codedoc list | history <record> | stats | kinds | reindex

codedoc-mcp [root]                           # stdio MCP server
codedoc-lsp                                  # stdio LSP server
```

## Dependencies

Curated deliberately; do not introduce alternatives without a decision record.

`tree-sitter` + six grammars · `blake3` · `serde` + `serde_json` · `rusqlite` (bundled) · `clap` · `anyhow` · `thiserror` · `rmcp` · `tokio` · `schemars` · `lsp-server` + `lsp-types` · `time` · `walkdir` · `proptest` · `tempfile` · `cargo-deny`.

`rusqlite` is bundled so the binary carries no system dependencies; agents install this into arbitrary environments.

Three of these differ from the list this file carried before any code existed, and the reasons are worth keeping:

**`rmcp` rather than a hand-rolled JSON-RPC loop.** The transport is trivial, but MCP is a moving specification and lifecycle is where hand-rolled servers rot. The hand-written version this replaced already hardcoded the protocol version instead of negotiating it, dropped `notifications/initialized`, and returned the wrong error code for an unknown tool. The MCP surface is a product surface, not a convenience.

**`lsp-server` + `lsp-types` rather than `tower-lsp`.** `tower-lsp` 0.20 has not shipped since 2023. `lsp-server` is rust-analyzer's, maintained, and synchronous, which suits work that is CPU-bound anyway.

**No `gix`.** Reading `HEAD` for provenance is a few lines of file reading, and the replay harness shells out to `git`. A full git library earns its place when rung 5 (history migration) is implemented, not before.

## Gates

Correctness properties, not feature counts. All tracks proceed concurrently; these gate merges.

- **G1** Canonical encoding byte-identical across Linux, macOS, and Windows. Vectors in `conformance/encoding/`, run on a three-OS matrix.
- **G2** Zero false reattachments. Measured by replaying real history over seven corpora in seven languages, **5,371 anchors, zero suspicious**:

| corpus | language | commits | anchors | survived | detached |
| --- | --- | --- | --- | --- | --- |
| [zod](https://github.com/colinhacks/zod) | TypeScript, TSX | 400 | 1,496 | **99.6%** | 6 |
| [gson](https://github.com/google/gson) | Java | 400 | 218 | **99.5%** | 1 |
| [ripgrep](https://github.com/BurntSushi/ripgrep) | Rust | 400 | 1,100 | **98.2%** | 20 |
| this repository | Rust | 86 | 277 | **97.5%** | 7 |
| [httpx](https://github.com/encode/httpx) | Python | 300 | 667 | **92.5%** | 50 |
| [cobra](https://github.com/spf13/cobra) | Go | 400 | 369 | **89.4%** | 39 |
| [fmt](https://github.com/fmtlib/fmt) | C++ | 400 | 1,177 | **82.9%** | 201 |
| [serilog](https://github.com/serilog/serilog) | C# | 400 | 163 | *not measurable* | 163 |

  **The spread is a property of the corpora and of how their languages name things, not of the adapters.** Detachments on httpx (all 50), cobra (38 of 39) and ripgrep (all 20) were checked against the final revision and name a declaration that is genuinely absent; cobra's exception, `argsMinusFirstX`, became a method on `Command`, so its symbol path legitimately changed. C++ is lower because fmt's test files declare dozens of `TEST(...)` per file and its headers carry many `formatter` template specialisations: a symbol shared by fifty declarations identifies none of them, so adding or removing any one changes the cardinality and correctly detaches the rest. That is the overload rule from section 4 operating at scale rather than a tracking failure. Not all of the shared C++ symbols are real overloading, though: `FMT_BEGIN_NAMESPACE` opens a namespace that tree-sitter-cpp parses as a function named `namespace`, and 6.6% of fmt's imported anchors are rooted at that one symbol.

  **C# is not measurable until a defect in `codedoc-lang` is fixed.** `codedoc-lang` does not treat `file_scoped_namespace_declaration` as a naming declaration, so `namespace Acme.Widgets;` contributes nothing to a symbol path where the braced form contributes `Acme.Widgets`. serilog migrated between the two forms across this range and every anchor detached. `crates/codedoc-anchor/tests/csharp_namespaces.rs` holds an ignored test asserting the behaviour it should have. Do not quote the 0% as an adapter quality figure; it measures one bug.

  **Four adapter naming gaps were found by measuring, and all are reported rather than fixed because `codedoc-lang` is not this workspace's to change.** None affects the hard zero — an unnamed or mis-named construct detaches rather than misattaching — but all narrow what can be tracked. C# is above; C++ macro-opened namespaces are with the corpora that surfaced them. Go is the same shape as TypeScript one language over: a `const_declaration` or `var_declaration` is not a naming declaration, so `const TestMode = "test"` carries no symbol while `func` and `type` do, and a documented package-level constant is exactly what Go projects use for modes, environment names and error types. Counting only constructs the adapter could not name: **gin 6.4% of 1,079, cobra 1.7% of 776, viper 0.5%, gorilla/mux 0.3%**. `crates/codedoc-anchor/tests/go_declarations.rs` holds the ignored test. The fourth is TypeScript: a lexical declaration is not a naming declaration, so `export const validate = (raw) => {}` and `export const TIMEOUT = 5000` carry no symbol, while `export function` does. Binding an arrow function to a const is the ordinary way to declare a function in modern TypeScript, and over zod's sources **16.4% of imported anchors carried no symbol, against 0.0% for Java, 1.2% for C++ and 1.6% for Go**. An anchor with no symbol cannot reach rungs 2, 3, 4 or 6, so the only evidence left to it is the content fingerprint at rung 1: it survives any edit elsewhere in its file, including reformatting, and is lost the moment its own construct changes. Measured again on three repositories nobody here had touched, counting only constructs the adapter could not name and excluding file-subject anchors, which correctly carry no symbol: **got 7%, click 3%, gorilla/mux 0%**. `crates/codedoc-anchor/tests/typescript_declarations.rs` holds the ignored tests.

  **A fifth gap is larger than any of them and is about import rather than resolution: Python docstrings are not documentation to codedoc.** A docstring is an expression statement, not a comment, so `Adapter::is_ignorable` never sees one and `import` walks past. django carries **3,990 docstrings across 12,513 declarations** and codedoc imports none of them, while taking 6,217 records from the `#` comments, which in Python are the incidental notes rather than the documentation. Rust and C# triple-slash comments do import, because both are comment nodes in their grammars, so this is specific to Python and to any language documenting itself in something other than a comment. Closing it needs a hook in `codedoc-lang` for documentation that is not a comment node; a language-specific branch in `codedoc-ops` would be the leak this file forbids. `crates/codedoc-ops/tests/python_docstrings.rs` holds the ignored test and the live one asserting the gap is still there.

  **Naming is what makes an anchor durable, and this is the measurement that shows it.** Verifying a freshly imported corpus against the code it was captured from should resolve everything. Java and Go detach nothing; Rust detaches 22 of 27,642 (0.08%), C++ 17 of 5,261 (0.32%), TypeScript 71 of 4,147 (1.71%). In all five corpora **every single one of those anchors has no symbol** — without one a construct can only be matched by content, so two identical bodies are indistinguishable, and ambiguity is failure rather than a tiebreak. TypeScript's rate is twenty-one times Rust's for exactly the reason above. An adapter that fails to name a declaration form is a correctness problem, not a cosmetic one.

  **Seven more corpora, measured after the harness stopped being quadratic**, none of them in the table above and none chosen by anyone here. 6,407 anchors, five flagged for inspection:

| corpus | language | commits | anchors | survived | detached |
| --- | --- | --- | --- | --- | --- |
| [gin](https://github.com/gin-gonic/gin) | Go | 150 | 846 | **100.0%** | 0 |
| [gson](https://github.com/google/gson) | Java | 150 | 128 | **100.0%** | 0 |
| [click](https://github.com/pallets/click) | Python | 150 | 576 | **97.4%** | 15 |
| [traefik](https://github.com/traefik/traefik) | Go, TypeScript, TSX | 150 | 3,722 | **96.7%** | 121 |
| [json](https://github.com/nlohmann/json) | C++ | 150 | 831 | **96.4%** | 30 |
| [got](https://github.com/sindresorhus/got) | TypeScript | 150 | 136 | **94.9%** | 7 |
| [mux](https://github.com/gorilla/mux) | Go | 150 | 168 | **94.0%** | 10 |

  traefik is the first corpus here that is more than one language, and the largest single replay: 3,722 anchors across Go, TypeScript and TSX in one ledger, resolving in one pass.

  **All five flagged results are on nlohmann and all five are one adapter defect.** `value_t` and `operator` resolved at content identity onto the declarations they were captured from, and every one kept its leaf declaration, so nothing reattached to the wrong construct. What changed was the symbol path, which gained a segment named `namespace`: tree-sitter-cpp cannot expand `NLOHMANN_JSON_NAMESPACE_BEGIN`, so it reads that macro, the namespace it opens and the body beneath it as one `function_definition` whose return type is the macro and whose name is the token `namespace`. The symbol therefore depends on whether the preprocessor context happened to parse, and moves when that changes. It also collides: **33.0% of nlohmann's 1,987 imported anchors are rooted at `cpp://namespace` and 33 of them are that bare symbol**, which by the overload rule identifies none of them, against **6.6% of fmt's 1,032**. Reported to `codedoc-lang`, which owns the fix — a declarator that is a reserved word is a mis-parse, and refusing it costs nothing real. `crates/codedoc-anchor/tests/cpp_namespace_macros.rs` pins the behaviour it should have. Replay exits non-zero whenever it flags anything; a sweep that loses that exit code is how the count above was first recorded as a zero.

  nlohmann could not be measured at all before that: replaying twenty of its commits did not finish in one hundred and ten seconds, and it now takes seven for a hundred and fifty. The harness rebuilt the whole-file fingerprint table and the whole `FileIndex` once **per declaration**, so a single 25,000-line header cost both a few hundred times over. That is the third time this shape of mistake has been found here, and it had the worst consequence of the three: it silently restricted the corpus that substantiates G2 to repositories small enough to tolerate it, which excludes exactly the single-header C++ shape that stresses the resolver hardest.

  **Rung 5 is exercised**: 27 anchors on ripgrep and 12 here followed a file across a git rename. The harness twice flattered these numbers by measuring less — it skipped renamed files entirely, and it captured only direct children of the root, so TypeScript, where nearly every declaration sits inside an `export` statement, contributed 143 anchors instead of 1,496.

  **The hard zero is carried by the property test**, which has ground truth by construction: for any tree and any edit script, resolution is correct or `Detached`. Replay over real history cannot label outcomes automatically, so it measures survival and *flags* confident rungs landing on a different symbol for human inspection. Do not claim replay proves the invariant; it evidences it.
- **G3** Ledger verifies from genesis; index rebuilds byte-identically from it, and rebuilds rather than fails when its schema version moves or the file is corrupt. **Met** — asserted by `crates/codedoc-index/tests/rebuildable.rs`.
- **G4** Context retrieval within budget on a 1M-LOC repository. **Met** — 40ms for `context`, 38ms for `brief` and 54ms for `search` at 1.09M LOC and 27,642 records.
- **G5** **Dogfood.** `.mcp.json` registers the server against this checkout, so an agent opening the repository is handed its own tools rather than told about them. This repository contains zero comments, carries its own architecture in its own ledger, and CI runs `codedoc doctor` against that ledger on every push. 84 records over 868 declarations. The percentage is not the point; what matters is that every invariant argued for in this file is also recorded against the code it governs, and that the tool is used here the way it asks others to use it.
- **G6** **Independence.** A second implementation, written in another language against `docs/spec/` alone and never reading the Rust, passes `conformance/`. Until that happens this is a tool with a data directory; afterwards it is a format.
