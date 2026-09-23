# codedoc

**Agents won't stop documenting. So let them — somewhere the next refactor can't destroy.**

Every coding agent that touches your repository works out how it fits together, discovers the non-obvious constraint, notices the trap in the retry path — and then writes that understanding into a comment, or a chat message, and loses it. The next session starts from zero and derives it again.

The problem was never that agents under-document. It's that the only channel available throws the work away.

codedoc gives that knowledge somewhere durable to land: an append-only ledger of typed, content-addressed records anchored to **program structure** rather than to line numbers. Insert fifty lines above a claim and it still points at the same code. Rename the variables inside it and it still points at the same code. Delete the function and it says so, loudly, instead of quietly sliding onto the function underneath.

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

None of that is in the source file. The source file contains source.

## Adding it to a codebase that already exists

codedoc is built to be adopted on day 40,000 of a project, not day one.

```bash
codedoc init
codedoc import src/          # dry run: shows what it found
codedoc import src/ --write  # record it
```

`import` reads the comments you already have, works out which construct each one is attached to, anchors it there, and classifies it — `TODO` becomes a warning, `SAFETY:` becomes a security record, "because…" becomes rationale. On a mid-sized crate that is a few thousand records in a couple of seconds.

**It does not touch your source.** Your comments stay exactly where they are. codedoc is additive; whether you ever delete a comment is your team's decision, not the tool's.

> The rule that this repository keeps zero comments is *our* dogfooding standard, not a condition of using codedoc. Nothing in the tool requires it, and the comment lint is ours, not yours.

## What it is underneath

A record is immutable and content-addressed. Changing your mind emits a superseding record; retracting emits a tombstone. Nothing is edited and nothing is deleted, so "what did we believe about this code six months ago" is a query rather than an archaeology exercise.

An anchor carries several independent signals — a symbol path, a structural fingerprint that ignores identifiers, a content fingerprint that ignores formatting, and the fingerprints of its neighbours. A resolver walks them in order of strength, and the rule that matters most is what it does when they run out:

```
$ codedoc verify

0 unchanged   0 migrated   0 stale   1 detached

DETACHED src/auth.rs
  rust://validate_signature
  invariant :: Signature validation must precede tenant resolution.
```

**It never guesses.** When the evidence is ambiguous the anchor detaches and waits for a human or an agent to adjudicate. Documentation confidently attached to the wrong code is worse than documentation that admits it is lost, so survival rate is a quality metric we improve and false attachment is a hard zero enforced by a property test.

## For agents

An MCP server exposes the whole thing:

```bash
codedoc-mcp /path/to/repo
```

| tool | purpose |
| --- | --- |
| `codedoc_context` | retrieve what is known about a location before changing it |
| `codedoc_attach` | record something learned, instead of writing a comment |
| `codedoc_verify` | check which claims survived a change |
| `codedoc_list` | enumerate active records |

Agent-authored records default to `assurance: inferred` and carry the model and session that produced them, so an agent's guess and a human's assertion are never indistinguishable at query time.

There is also an LSP server (`codedoc-lsp`) that renders records as hovers and code lenses, resolved live against the buffer, so the editor shows where a claim is *now* rather than where it was recorded.

## Languages

Rust, C#, TypeScript, Python, Go, Java — via tree-sitter, so adding a language is an adapter, not a rewrite.

## Status

Early. The format is not yet stable; before 1.0 it may change, but never without a mechanical `codedoc migrate` path, and after 1.0 not incompatibly at all. A ledger is accumulated institutional memory and is not reproducible, so it gets treated accordingly.

See `CLAUDE.md` for the architecture and the invariants, `CONTRIBUTING.md` before opening a PR, and `docs/spec/` for the normative format — the Rust implementation is the reference, not the definition.

## Licence

Code is `MIT OR Apache-2.0`. The specification and conformance vectors are `CC0-1.0`.
