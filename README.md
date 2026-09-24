# codedoc

codedoc stores what people and agents work out about a codebase — constraints, traps,
the reasoning behind a decision — outside the source files, attached to the code it
describes. An agent about to modify that code is served the relevant claims first,
over MCP.

A comment cannot be searched by what it says, marked as superseded, attributed, or
checked against the code beneath it. A record can.

## Installation

Requires Rust 1.85 or later and a C toolchain for the tree-sitter grammars.

```bash
cargo install --path crates/codedoc-cli   # the codedoc command
cargo install --path crates/codedoc-mcp   # the MCP server
cargo install --path crates/codedoc-lsp   # optional: editor hovers and diagnostics
```

## Usage

```bash
cd your-project
codedoc init                 # --scope local to keep it out of the working tree
codedoc import src/          # optional: seed from comments already present
```

Register the server with an MCP client:

```json
{
  "mcpServers": {
    "codedoc": {
      "command": "codedoc-mcp",
      "args": ["/absolute/path/to/your-project"],
      "env": { "CODEDOC_AGENT_MODEL": "your-model-id" }
    }
  }
}
```

The agent gets twenty-three tools, discovers them itself, and is told
when to use them. [AGENTS.md](AGENTS.md) documents what it is told.

Recording a claim:

```
codedoc_attach {
  "file":   "crates/codedoc-verify/src/history.rs",
  "symbol": "rust://is_ancestor",
  "kind":   "invariant",
  "claim":  "Only git exit code 1 means not an ancestor; every other failure
             must be read as already present.",
  "detail": "git exits 0 for ancestor, 1 for not, and 128 for a revision it
             does not know. The only caller decides whether a record was
             written during a change, so reading 128 as 'not an ancestor'
             would credit an author with recording work they did not do."
}
```

Retrieving it, at any later point, from `codedoc_context`:

```json
{
  "target": { "file": "crates/codedoc-verify/src/history.rs",
              "symbol": "rust://is_ancestor" },
  "invariants": [
    {
      "kind": "invariant",
      "claim": "Only git exit code 1 means not an ancestor; every other failure
                must be read as already present.",
      "assurance": "asserted"
    }
  ]
}
```

Every tool has a command-line equivalent emitting the same JSON under `--json`.
See [docs/cli.md](docs/cli.md).

## Anchoring

Line numbers are not stored. Each claim is attached to an anchor holding four
independent descriptions of its target:

| signal | survives |
| --- | --- |
| symbol path, from the enclosing declarations | insertion, reformatting |
| structural fingerprint, identifiers excluded | renaming locals |
| content fingerprint, formatting excluded | reformatting, comment edits |
| context fingerprints of adjacent siblings | the target moving within a file |

Resolution is a seven-rung ladder, strongest evidence first, and each resolution
records which rung matched. Two candidates at one rung is a failure, not a tiebreak:
the anchor detaches and waits for adjudication rather than binding to the likelier
candidate. A claim attached to the wrong construct is treated as strictly worse than
one reported as lost.

```
$ codedoc verify
71 unchanged   18 migrated   0 stale   0 detached
```

| state | meaning |
| --- | --- |
| unchanged | resolved, target unmoved |
| migrated | resolved elsewhere; the code moved and the claim followed |
| stale | resolved, but the target changed enough to warrant re-reading |
| detached | unresolvable without guessing; awaiting a decision |

Exit codes are `0`, `1` for stale, `2` for detached and `3` for an integrity failure,
so CI can branch on them. A stale claim is answered with `codedoc affirm`,
`codedoc supersede` or `codedoc retract`.

## Records

Immutable and content-addressed. A revision appends a superseding record; a retraction
appends a tombstone. Nothing is edited or deleted, so the state of belief at any past
date is queryable.

Each record carries its author — a named person, or an agent and session — and an
assurance level of `asserted`, `inferred` or `speculative`. Retrieval weights them
differently.

Records may also relate two constructs rather than describe one: *must execute after*,
*guarded by*, *invalidates*. Such facts belong to neither endpoint alone.

## Anchor survival

Replayed over the history of seven codebases — zod, gson, ripgrep, httpx, cobra, fmt
and this repository — **5,371 anchors, zero resolved onto the wrong symbol.** Survival
ranges from 99.6% to 82.9%. Detachments were checked against the final revision rather
than sampled, and name code that is genuinely absent.

The spread tracks the corpora rather than the adapters: one deleted nineteen files over
the window, another declares dozens of identically named test macros per file, and a
symbol shared by fifty declarations identifies none of them. [CLAUDE.md](CLAUDE.md)
carries the per-corpus table.

Retrieval on a 1.09M-line corpus of 27,642 records: `context` 40ms, `brief` 38ms,
`search` 54ms, `verify` 3.6s.

## Storage

| scope | location | visibility |
| --- | --- | --- |
| shared | `.codedoc/` | committed, shared with the team |
| local | inside `.git/` | this clone only, untracked by construction |
| global | outside the repository | this machine only |

`--scope local` writes nothing to the working tree: no directory, no `.gitignore`
entry, nothing in `git status`. Reads merge every scope present.

The ledger is append-only JSONL, sharded by record id, with a merge driver making
concurrent branches a union.

## Language support

Rust, Python, TypeScript, TSX, Go, Java, C# and C/C++, via tree-sitter. A new language
is an adapter rather than a core change.

A file no parser handles — Dockerfile, CI workflow, migration, Markdown — carries
claims about the file as a whole, resolved by path.

Two known adapter gaps, each with a test that fails when closed
(`cargo test --workspace -- --ignored`): C# file-scoped namespaces contribute nothing
to a symbol path, and TypeScript `const`-bound arrow functions are not treated as
declarations. Neither can misattach a claim. Both narrow what is trackable: a claim on
an unnameable construct is matched by its content alone, so it survives edits elsewhere
in the file and is lost when that construct changes. Across three repositories, counting
only constructs the adapter could not name: got 7%, click 3%, gorilla/mux 0%.

## Status

Pre-1.0. The on-disk format may change before 1.0, always with a mechanical
`codedoc migrate` path, and not incompatibly after it.

Implemented: ledger and integrity checking, anchoring and resolution across eight
grammars, comment import, search, the full record lifecycle, relations, and the CLI,
MCP and LSP surfaces. CI verifies byte-identical encoding on Linux, macOS and Windows.

Outstanding: the VS Code extension installs from a local `.vsix` and is not published.

## Documentation

| | |
| --- | --- |
| [AGENTS.md](AGENTS.md) | what the agent is told, and how to get usable records from one |
| [docs/cli.md](docs/cli.md) | command reference |
| [docs/spec/format.md](docs/spec/format.md) | on-disk format, normative |
| [CLAUDE.md](CLAUDE.md) | architecture and the invariants governing changes |
| [CONTRIBUTING.md](CONTRIBUTING.md) | required before a PR, including the no-comments rule |
| [SECURITY.md](SECURITY.md) | threat model |
| [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md) | Contributor Covenant, and who to contact |

## Licence

`MIT OR Apache-2.0`. Specification and conformance vectors are `CC0-1.0`.
