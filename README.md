# codedoc

codedoc is a semantic memory layer for source repositories, designed to be operated by
coding agents rather than by people. It stores what has been established about a
codebase — invariants, security properties, known failure modes, the reasoning behind a
decision — outside the source files, anchored to program structure rather than to line
numbers, and serves it back through MCP before an agent modifies anything.

Most use is indirect: the agent calls it, not the developer.

The problem it addresses is retention rather than effort. A coding agent working in an
unfamiliar repository routinely derives constraints that the source does not state. The
channels available for writing those down — a comment, a commit message, a chat
transcript — either do not survive the next refactor or are never read again, so the
next agent derives the same constraints from scratch.

## Install

Requires Rust 1.85 or later and a C toolchain for the tree-sitter grammars.

```bash
git clone https://github.com/LukasCollishawM/codedoc
cd codedoc
cargo install --path crates/codedoc-cli   # the codedoc command
cargo install --path crates/codedoc-mcp   # the MCP server
cargo install --path crates/codedoc-lsp   # optional: hovers and diagnostics in an editor
```

`codedoc-mcp` is a separate binary from `codedoc`, and it is the one the MCP
configuration below invokes.

## Point your agent at it

```bash
cd your-project
codedoc init
```

Then register the server with your MCP client:

```json
{
  "mcpServers": {
    "codedoc": {
      "command": "codedoc-mcp",
      "args": ["/absolute/path/to/your-project"],
      "env": {
        "CODEDOC_AGENT_MODEL": "your-model-id"
      }
    }
  }
}
```

The agent gets twenty-three tools, discovers them itself, and is told
when to use them. [AGENTS.md](AGENTS.md) documents the instructions the server sends
and how to get useful records out of an agent.

On a repository that already has comments, `codedoc import src/` anchors each comment
to the construct it documents and classifies it: `TODO` becomes a warning, `SAFETY:`
becomes a security record. It runs as a dry run by default and does not modify source
files.

On a repository with no existing documentation, `codedoc gaps` ranks undocumented
declarations by what the git history did to them — how many commits touched those
lines, how many of those commits were corrective, and what the most recent one said.
Repeated corrections indicate that the code as written was insufficient to work from,
which makes them a reasonable place to start.

## How it works

**Records rather than comments.** A record is a typed claim — `invariant`, `security`,
`known_failure_mode`, `rationale` and others — attached to a region of code. It carries
its author (a named human, or an agent and session) and an assurance level. An agent's
inference and a human's verified assertion remain distinguishable indefinitely.

**Anchors rather than line numbers.** An anchor stores several independent signals: a
symbol path, a fingerprint of the code's shape that ignores identifier names, a
fingerprint of its content that ignores formatting, and fingerprints of its immediate
neighbours. Inserting lines above a claim, or renaming locals within it, does not move
it.

**Detachment rather than approximation.** When the resolver exhausts its evidence, the
anchor detaches and waits for adjudication instead of binding to the nearest plausible
candidate. Attaching a claim to the wrong function is treated as strictly worse than
reporting that it could not be placed, so the two are not traded against each other:
anchor survival is a quality metric to be improved, and false attachment is held at
zero by a property test.

Replayed over the real history of seven codebases — zod, gson, ripgrep, httpx, cobra,
fmt and this repository — 5,371 anchors resolved with no anchor landing on the wrong
symbol, with detachments checked against the final revision rather than sampled.
Survival ranges from 99.6% to 82.9%. The spread is a property of the codebases rather
than of the language adapters: one deleted nineteen files over the window, another
declares dozens of identically named test macros per file. [CLAUDE.md](CLAUDE.md)
contains the full table.

```
$ codedoc verify
1493 unchanged   270 migrated   196 stale   136 detached
```

Unchanged and migrated anchors both held. Stale means the code beneath a claim changed
enough to warrant review, which includes a function that keeps its name and signature
while its body is rewritten. Detached means the anchor could not be placed and is
waiting for a decision.

Records are immutable. Revising one appends a superseding record; retracting one
appends a tombstone. Because nothing is edited or deleted, the state of belief at any
past date is a query rather than a reconstruction.

Relations are first-class: an agent can record that one function must execute after
another. Such a fact belongs to neither function individually and has no natural place
in a comment on either.

Knowledge is searchable by words as well as by location, since an agent arriving at
unfamiliar code does not yet know which file to ask about, and an entire task can be
briefed in a single call from the list of files it will touch.

## Where it lives

By default the ledger is written to `.codedoc/`, committed, and shared with the team.

For a repository you do not own or do not wish to modify:

```bash
codedoc init --scope local
```

This writes to `.git/codedoc/`, which git cannot track. There is no directory in the
working tree, no `.gitignore` entry and nothing in `git status`, so the repository
contains no evidence that codedoc is in use. `--scope global` stores the ledger outside
the repository entirely.

## Languages

Rust, Python, TypeScript, TSX, Go, Java, C# and C/C++, via tree-sitter. Adding a
language means writing an adapter rather than modifying the core.

A file no adapter understands can still carry a claim about the file itself — a
Dockerfile, a CI workflow, a migration, a Markdown page. Those anchors resolve by path
rather than by structure, so they survive the file changing and detach when it is
deleted. What they cannot do is name something inside the file, and they are never
reported stale, because there is no structure to measure drift against.

Two adapters have known gaps, each covered by a test that will fail when the gap is
closed (`cargo test --workspace -- --ignored`). In C#, a file-scoped namespace
(`namespace Acme;`, the default since C# 10) contributes nothing to a symbol path, so
types declared in such files are recorded unqualified. In TypeScript, an arrow function
or constant bound with `const` is not treated as a declaration, so a claim attached to
one resolves only while its file is unedited. Neither gap can cause a claim to attach
to the wrong code, since an anchor that cannot be named detaches instead, but both
reduce what can be tracked. [CLAUDE.md](CLAUDE.md) quantifies the effect.

## Status

Early, and the on-disk format is not yet stable. Before 1.0 it may change, always with
a mechanical `codedoc migrate` path; after 1.0 it will not change incompatibly. A
ledger accumulates over months and cannot be regenerated from the code, which is why
its compatibility guarantees are stricter than the API's.

Implemented: the ledger and its integrity checking, anchoring and resolution across
eight grammars, comment import, search, the full record lifecycle, relations, and the
CLI, MCP and LSP surfaces. CI verifies that the canonical encoding is byte-identical on
Linux, macOS and Windows, and runs `codedoc doctor` against this repository's own
ledger.

Outstanding: the VS Code extension installs from a local `.vsix` but is not published
to the marketplace.

## Documentation

| | |
| --- | --- |
| [AGENTS.md](AGENTS.md) | the instructions sent to agents, and how to get good records out of one |
| [docs/cli.md](docs/cli.md) | the command-line reference, for humans and CI |
| [docs/spec/format.md](docs/spec/format.md) | the normative format; the Rust implementation is the reference, not the definition |
| [CLAUDE.md](CLAUDE.md) | architecture and the invariants that govern changes to it |
| [CONTRIBUTING.md](CONTRIBUTING.md) | required reading before a PR, including the no-comments rule |
| [SECURITY.md](SECURITY.md) | threat model; a cloned repository's ledger is untrusted input |

## Licence

Code is `MIT OR Apache-2.0`. The specification and conformance vectors are `CC0-1.0`.
