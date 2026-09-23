# codedoc

**Agents won't stop documenting. So let them.**

codedoc stores documentation *outside* your source files — in a versioned ledger anchored to code structure rather than line numbers — and serves it back through a CLI, an MCP server for agents, and an LSP server for editors.

---

Every coding agent that touches your repository works out how it fits together, notices the constraint that isn't obvious, spots the trap in the retry path — then writes it into a comment or a chat message and loses it. The next session derives it again.

The problem was never that agents under-document. It's that the only channel available throws the work away.

## Install

Requires Rust 1.85+ and a C toolchain (for the tree-sitter grammars).

```bash
git clone https://github.com/LukasCollishawM/codedoc
cd codedoc
cargo install --path crates/codedoc-cli   # installs `codedoc`
cargo build --release                     # also builds codedoc-mcp, codedoc-lsp
```

Not yet on crates.io.

## Quickstart

```bash
cd your-project
codedoc init
codedoc import src/            # dry run: shows what it found in your existing comments
codedoc import src/ --write    # record it
codedoc context src/auth.rs:83 # ask what's known about a location
```

`import` is how you adopt this on a project that already exists. It reads the comments you already have, works out which construct each one documents, anchors it there, and classifies it — `TODO` becomes a warning, `SAFETY:` becomes a security record, "because…" becomes rationale. On a mid-sized crate that is a few thousand records in about a second.

**It does not modify your source.** Your comments stay where they are. Whether you ever delete one is your team's call.

> The rule that *this* repository contains zero comments is our own dogfooding standard. It is not a condition of using codedoc, and nothing in the tool requires it.

## What you get back

```
$ codedoc context src/auth.rs:83

TARGET
  src/auth.rs:83
  rust://AuthService/validate_token

INVARIANTS
  - Signature validation must precede tenant resolution.
    Resolving a tenant from an unvalidated token allows tenant
    confusion across trust boundaries.
    [asserted]

KNOWN FAILURE MODES
  - A timeout after reservation can double-charge on retry.
    [inferred]

RELATIONS
  rust://JwtSignature/validate must execute before rust://TenantContext/create
```

None of that is in the source file.

## Why anchors instead of line numbers

Insert fifty lines above a claim and it still points at the same code. Rename the locals inside it and it still points at the same code. Delete the function and it tells you, instead of quietly sliding onto the function underneath:

```
$ codedoc verify
1493 unchanged   270 migrated   196 stale   136 detached

DETACHED src/auth.rs
  rust://validate_signature
  invariant :: Signature validation must precede tenant resolution.
```

An anchor carries a symbol path, a structural fingerprint that ignores identifiers, a content fingerprint that ignores formatting, and fingerprints of its neighbours. A resolver tries them in order of strength. When they run out it **detaches rather than guessing** — documentation attached to the wrong code is worse than documentation that admits it is lost.

Then you adjudicate:

```bash
codedoc detached                                     # what needs a decision
codedoc resolve <record> --to-symbol rust://new_name # place it explicitly
codedoc retract <record> --reason "no longer true"   # or drop it
```

Records are immutable. Changing your mind writes a superseding record; retracting writes a tombstone. Nothing is edited and nothing is deleted, so `codedoc history <record>` answers what you believed six months ago.

## For agents (MCP)

```bash
codedoc-mcp /path/to/repo
```

Add it to any MCP client:

```json
{
  "mcpServers": {
    "codedoc": {
      "command": "codedoc-mcp",
      "args": ["/path/to/your/repo"]
    }
  }
}
```

| tool | purpose |
| --- | --- |
| `codedoc_context` | what's known about a location, before changing it |
| `codedoc_attach` | record something learned, instead of writing a comment |
| `codedoc_verify` | which claims survived a change |
| `codedoc_list` | enumerate active records |

Agent-authored records default to `assurance: inferred` and carry the model and session that produced them, so an agent's guess and a human's assertion stay distinguishable at query time.

## For editors (LSP)

`codedoc-lsp` speaks stdio LSP and provides hovers and code lenses, resolved live against the buffer — so the editor shows where a claim is *now*, not where it was recorded. Point any LSP client at the binary.

A VS Code client lives in `editors/vscode/`. It is unpackaged; run it from source with `code --extensionDevelopmentPath=editors/vscode`.

## Commands

| | |
| --- | --- |
| `codedoc init` | create the ledger |
| `codedoc import <path> [--write]` | adopt existing comments; dry run by default |
| `codedoc attach <file> --symbol <s> --kind <k> --claim "…"` | record a claim |
| `codedoc context <file>[:line] [--depth n] [--budget n]` | retrieve knowledge |
| `codedoc verify` | re-resolve every anchor against the working tree |
| `codedoc detached` | anchors awaiting a decision |
| `codedoc resolve <record> --to-symbol \| --to-line` | reattach explicitly |
| `codedoc supersede <record> --claim "…"` | revise a claim |
| `codedoc retract <record>` | retire a claim |
| `codedoc history <record>` | the full supersession chain |
| `codedoc list [--file \| --symbol]` | active records |
| `codedoc stats` | record counts by kind, ledger integrity |
| `codedoc kinds` | the record kind vocabulary |
| `codedoc reindex` | rebuild the SQLite projection from the ledger |
| `codedoc migrate [--write]` | report and apply ledger schema migrations |
| `codedoc git install-merge-driver` | make ledger shards merge by union |

Every command accepts `--json` with a stable schema; the human output is rendered from that JSON, never in parallel to it. Exit codes are meaningful: `0` clean, `1` stale, `2` detached, `3` ledger integrity failure.

## Languages

Rust, C#, TypeScript, Python, Go, Java — via tree-sitter. Adding one is an adapter, not a rewrite.

## Status

Working today: the ledger and its integrity checking, anchoring and resolution across six languages, comment import, the full record lifecycle, and the CLI, MCP and LSP surfaces. Canonical encoding is verified byte-identical on Linux, macOS and Windows in CI.

Not done yet: the VS Code client is unpackaged and has no marketplace listing; the resolver's similarity rung is deliberately never auto-accepted, so low-confidence matches always require adjudication; and performance is measured at 50k LOC rather than at the million-line scale the budgets target.

The format is **not yet stable**. Before 1.0 it may change, but never without a mechanical migration path; after 1.0 it will not change incompatibly. A ledger is accumulated institutional memory and cannot be regenerated, so it gets treated that way.

## Documentation

- `docs/spec/format.md` — the normative format. The Rust implementation is the reference, not the definition.
- `CLAUDE.md` — architecture and the invariants that govern changes.
- `CONTRIBUTING.md` — read before opening a PR; the no-comments rule will surprise you.
- `SECURITY.md` — threat model. A cloned repository's ledger is untrusted input.

## Licence

Code is `MIT OR Apache-2.0`. The specification and conformance vectors are `CC0-1.0`.
