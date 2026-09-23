# codedoc

**Agents won't stop documenting. So let them.**

codedoc is memory for a codebase, built for agents to use. It stores what has been
worked out about your code — invariants, traps, why an ordering matters — *outside*
your source files, anchored to program structure rather than line numbers, and
serves it back to an agent through MCP before that agent changes anything.

You mostly do not run codedoc. Your agent does.

---

Every coding agent that touches your repository works out how it fits together,
notices the constraint that isn't obvious, spots the trap in the retry path — then
writes it into a comment or a chat message, and loses it. The next session derives
it again, from scratch, and pays for it again.

The problem was never that agents under-document. It's that the only channel
available throws the work away.

## Install

Requires Rust 1.85+ and a C toolchain (for the tree-sitter grammars).

```bash
git clone https://github.com/LukasCollishawM/codedoc
cd codedoc
cargo build --release
cargo install --path crates/codedoc-cli
```

## Point your agent at it

```bash
cd your-project
codedoc init
```

Then add the server to your MCP client:

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

That's the setup. The agent gets eleven tools, discovers them itself, and is told
when to use them. See [AGENTS.md](AGENTS.md) for what it is told and how to make
your agent use it well.

**Adopting on an existing codebase?** `codedoc import src/` reads the comments you
already have, anchors each to the construct it documents, and classifies it — `TODO`
becomes a warning, `SAFETY:` becomes a security record. Dry run by default. It never
modifies your source.

## How it works

Three ideas, and the third is the one that matters.

**Records, not comments.** A record is a typed claim — `invariant`, `security`,
`known_failure_mode`, `rationale` — attached to a piece of code. It carries who
made it (which human, or which agent and session) and how sure they were. An
agent's guess and a human's verified assertion stay distinguishable forever.

**Anchors, not line numbers.** An anchor holds several independent signals: a
symbol path, a fingerprint of the code's *shape* that ignores identifiers, a
fingerprint of its *content* that ignores formatting, and fingerprints of its
neighbours. Insert fifty lines above a claim and it still points at the same code.
Rename the locals inside it and it still points at the same code.

**It refuses to guess.** When a resolver runs out of evidence, the anchor
**detaches** and waits for a decision, rather than attaching to whatever looks
closest. Documentation confidently pointing at the wrong function is worse than
documentation that admits it is lost — so anchor survival is a quality metric we
work to improve, and false attachment is a hard zero enforced by a property test.

```
$ codedoc verify
1493 unchanged   270 migrated   196 stale   136 detached
```

*Unchanged* and *migrated* held. *Stale* means the code changed enough to be worth a
look — including the case where a function keeps its name and signature but its body
is rewritten underneath a claim, which resolves perfectly and is still flagged.
*Detached* means codedoc will not guess, and is asking.

Records are immutable. Revising one writes a superseding record; retracting writes
a tombstone. Nothing is edited and nothing is deleted, so "what did we believe about
this six months ago" is a query rather than an archaeology exercise.

Relations are first-class: an agent can record that one function must execute after
another. That fact belongs to neither function, and has nowhere to live in a comment.

## Where it lives

By default the ledger is committed at `.codedoc/` and shared with your team.

If you want to use codedoc on a repository you don't own or don't want to change:

```bash
codedoc init --scope local
```

That puts it in `.git/codedoc/`, which git structurally cannot track. No directory
in the working tree, no `.gitignore` entry, nothing in `git status`. There is no
evidence in the repository that you are using it. `--scope global` puts it outside
the repository entirely.

## Languages

Rust, C#, TypeScript, Python, Go, Java — via tree-sitter. Adding one is an adapter,
not a rewrite.

## Status

Early, and the format is **not yet stable**. Before 1.0 it may change, but never
without a mechanical `codedoc migrate` path; after 1.0 it will not change
incompatibly. A ledger is accumulated memory and cannot be regenerated, so it is
treated that way.

Working today: the ledger and its integrity checking, anchoring and resolution
across six languages, comment import, the full record lifecycle, relations, and the
CLI, MCP and LSP surfaces. Canonical encoding is verified byte-identical on Linux,
macOS and Windows in CI.

Not done: no packaged VS Code extension (the LSP server it would wrap works);
performance is measured at 50k LOC, not at the million-line scale the budgets target.

## Documentation

| | |
| --- | --- |
| [AGENTS.md](AGENTS.md) | what your agent is told, and how to get good records out of it |
| [docs/cli.md](docs/cli.md) | the CLI, for humans and CI |
| [docs/spec/format.md](docs/spec/format.md) | the normative format — the Rust implementation is the reference, not the definition |
| [CLAUDE.md](CLAUDE.md) | architecture and the invariants that govern changes |
| [CONTRIBUTING.md](CONTRIBUTING.md) | read before a PR; the no-comments rule will surprise you |
| [SECURITY.md](SECURITY.md) | threat model — a cloned repository's ledger is untrusted input |

## Licence

Code is `MIT OR Apache-2.0`. The specification and conformance vectors are `CC0-1.0`.
