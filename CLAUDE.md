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

**I1 — The ledger is the only truth.** Every index, cache, and projection is derived and must be reconstructible from the ledger alone. `codedoc reindex --from-scratch` after `rm -rf .codedoc/index.sqlite` must produce a byte-identical index. If a piece of state cannot be rebuilt, it does not belong outside the ledger.

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

`editors/vscode/` and `editors/rider/` are thin JSON-RPC clients over `codedoc-lsp`. They contain **no** anchor logic, no ledger logic, and no schema knowledge beyond the wire types. Any editor feature requiring new intelligence is implemented in the server.

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

Rung 6 never auto-accepts. Anything landing at `Low` is queued for adjudication. Kinds carrying safety weight — `invariant`, `security`, `precondition`, `postcondition` — require `High` or better and are otherwise detached even when a plausible candidate exists. Being wrong about an invariant is the failure mode that ends the project.

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

Explanation goes in the ledger. Until the ledger can hold it, it goes in `docs/decisions/`. The arc is deliberate: `///` doc comments are permitted **only on `pub` items**, and only until `codedoc render rustdoc` can generate them from the ledger — at which point they become build artifacts and the sources lose them too. The day this repository's public API documentation is emitted from its own ledger is the day the product is real.

`unsafe` is banned outright, so `// SAFETY:` never arises. Every `#[allow(...)]` must have a corresponding `workaround` record anchored to that item; `codedoc lint allows` fails CI otherwise. The suppression and its justification are linked by structure rather than by adjacency — the entire pitch, applied to ourselves.

If you feel the urge to write a comment, that urge is the product's input signal. Emit a record.

### Types

Newtype every identifier: `RecordId`, `AnchorId`, `SymbolPath`, `FileId`, `GitRev`, `Blake3`, `LedgerHead`. A bare `String` crossing a module boundary as an identifier is a defect. Parse, don't validate — the constructor is the only door into a valid value, and nothing past it re-checks.

Make illegal states unrepresentable. `Resolution` carries its rung. A `Record` cannot exist without provenance. A detached anchor cannot be read as though resolved. Prefer the compiler refusing over a runtime guard, always.

No booleans in signatures — `resolve(path, true, false)` is unreadable at the call site and an enum costs nothing.

Every public enum and struct that can grow — record kinds, relation verbs, error types, resolver rungs — carries `#[non_exhaustive]`. This produces a deliberate asymmetry, and it is the right one: the attribute does not apply within the defining crate, so adding a record kind still breaks our own build until every site handles it, while downstream consumers keep compiling. Growth in the vocabulary must be free for the ecosystem and expensive for us.

Two rules follow from that asymmetry, and the second one matters more than it looks:

**Inside the defining crate, no catch-all `_ =>`.** Exhaustiveness is how a newly added record kind finds every site obliged to handle it. When another of our crates needs a projection of an enum — a string form, a category — add the method next to the enum, where the match is exhaustive, rather than matching across the boundary. `Kind::as_str` and `Role::as_str` exist for exactly this reason.

**Across a crate boundary the wildcard is mandatory, so it must fail safe.** A wildcard over a variant that did not exist when the code was written is a prediction about the future, and the only honest prediction is the conservative one. `codedoc-verify` maps an unrecognised resolver rung to `Detached`, never to `Fresh`: a rung added by a later version must degrade into adjudication rather than silently present itself as verified. Every cross-crate wildcard picks the arm that would be safe if the unknown variant turned out to be the most dangerous one.

### Errors

Libraries use `thiserror` with typed, exhaustive enums. `anyhow`/`miette` appear only at binary edges. No `Box<dyn Error>` in a library signature — it discards precisely the information the caller needs.

`unwrap()` outside tests is a defect. `expect()` is permitted only where the invariant is guaranteed by construction **and** carries an `invariant` record. No `panic!` on any path reachable from user input or file contents — a malformed source file is an expected input to a parser, not an exceptional one.

### Structure

One level of abstraction per function. Early return over nesting; depth 3 is the ceiling. No `utils`, `helpers`, `common`, or `misc` modules — a name that does not say what is inside will collect anything. No `Manager`, `Handler`, `Service`, or `Processor` suffixes unless that word is genuinely the domain term. `pub(crate)` by default; widening visibility is a deliberate API decision.

Do not add a trait with one implementation. Do not add indirection for a second case that does not yet exist. The crate boundaries above already encode the extension points that were worth predicting.

### Documentation is part of the change, not a follow-up

A change that alters behaviour and leaves the documentation describing the old behaviour is incomplete, and it is incomplete in the worst way: the repository now asserts something false, with authority. Update the docs in the same commit as the code.

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
| `import --write` | 27s | — |
| `reindex` | 2.4s | 60s |
| `verify` | 3.8s | 60s |
| `doctor` | 5.1s | 60s |
| `conflicts` | 0.6s | 60s |
| `coverage` | 0.8s | 60s |
| `context` (depth 2) | 45ms | 100ms |
| `brief` | 46ms | 100ms |
| `search` | 68ms | 100ms |

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

Publish narrowly. A crate on crates.io is a permanent obligation, so crates whose API has not settled carry `publish = false` rather than a `0.x` promise nobody intends to keep. `cargo-semver-checks` runs in CI on everything published. MSRV is declared in `rust-version`, tested in the matrix, and raised only in a minor release.

### Rules are enforced by machines, never by reviewers

Standards this strict, enforced by human review, become gatekeeping — and drive off precisely the contributors a young project needs. **Every rule in this file is a CI check or it is not a rule.** A reviewer must never be the first person to tell a contributor that their comment is not allowed; the lint says so locally, before the PR, and prints the command that fixes it. Lint messages carry the remedy, not merely the violation. If a standard cannot be checked mechanically, either build the check or drop the standard.

The no-comments rule in particular will startle every drive-by contributor. That is a documentation and tooling problem, and it is ours, not theirs.

### Licensing and provenance

Code is dual `MIT OR Apache-2.0`, the Rust ecosystem default, and Apache's explicit patent grant matters disproportionately for a format hoping to attract independent implementations. The spec and conformance vectors are `CC0-1.0` — nobody adopts a format whose specification is encumbered. Contributions are DCO sign-off, not a CLA; on a protocol project a CLA reads as a retained relicensing option and costs goodwill that is worth more than the option. `cargo deny` gates advisories, licences, and banned or duplicated crates.

### Security posture

A cloned repository's `.codedoc/` is attacker-controlled input, and so is every source file handed to the parser. The canonical decoder, the ledger reader, and the index writer all sit on that trust boundary: no panics, no allocation sized by an untrusted length field, no path in any record field escaping the repository root, and no record content reaching an executed context. `#![forbid(unsafe_code)]` in every crate. Fuzz targets for the canonical decoder and the ledger reader are part of the suite, not an aspiration. The threat model belongs in the spec, since it binds other implementations too.

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
- **G2** Zero false reattachments. Measured over **ripgrep**, 400 commits and 1,100 anchors: **98.2% survival, 0 suspicious, and all 20 detachments audited as correct**. Six are constructs whose symbol was shared by two `#[cfg]`-gated definitions in `pathutil.rs` and now has one, so the survivor is not evidenced to be the one a claim described; three are the `is_hidden` definitions, renamed to `is_hidden_path`, `is_hidden_entry` and `is_hidden_path_only`; seven are `lowargs.rs` enums whose `impl Default` blocks became derives; `Worker` gained a `Drop` impl, so two impls now share that symbol and kind; `hyperlink_aliases` stopped being a `mod` declaration in that file; and `TEMPLATE_CHOICES` and `version_pcre2` were deleted outright. Every anchor either found its code or correctly reported it gone. Also over **httpx**, 300 commits of Python and 502 anchors: **92.8% survival, 0 suspicious, and all 36 detachments audited as correct** — every one names a `def` or `class` that is absent from that file at the final revision, checked mechanically rather than sampled. httpx emptied most of `_utils.py` over that range and rewrote several test modules, so the lower figure is a codebase that deleted more, not an adapter that tracks worse. Also over this repository's own history — 86 commits including a crate extraction and three modules moved between crates — **277 anchors, 97.5% survival, 0 suspicious**, and all seven detachments name a declaration that is genuinely gone. **Rung 5 is exercised by all of this**: 27 anchors on ripgrep and 12 here followed a file across a git rename, which is the first evidence from real history rather than unit tests — the harness used to skip renamed files entirely, counting their anchors as neither survival nor detachment and quietly flattering the figure. It now replays across the rename and reports how many files it skipped as deleted, so the denominator is visible. **The hard zero is carried by the property test**, which has ground truth by construction: for any tree and any edit script, resolution is correct or `Detached`. Replay over real history cannot label outcomes automatically, so it measures survival and *flags* confident rungs landing on a different symbol for human inspection. Do not claim replay proves the invariant; it evidences it.
- **G3** Ledger verifies from genesis; index rebuilds byte-identically from it. **Met** — asserted by `crates/codedoc-index/tests/rebuildable.rs`.
- **G4** Context retrieval within budget on a 1M-LOC repository. **Met** — 33ms at 1.09M LOC and 27,420 records.
- **G5** **Dogfood.** This repository contains zero comments, carries its own architecture in its own ledger, and `codedoc verify` runs green in its own CI. The project is not real until it is its own first user.
- **G6** **Independence.** A second implementation, written in another language against `docs/spec/` alone and never reading the Rust, passes `conformance/`. Until that happens this is a tool with a data directory; afterwards it is a format.
