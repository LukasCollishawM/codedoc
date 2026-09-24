# codedoc

**codedoc gives a codebase a memory.** When a coding agent works something out about
your code that the source does not say, it writes it down here, and the next agent to
touch that code is told about it before it changes anything.

## The problem

Somebody once spent an afternoon discovering that a function must run before another,
or that an error code means the opposite of what it looks like, or that a timeout is 30
seconds because anything lower breaks a downstream service. That knowledge went into a
commit message, a pull request comment, or a chat window, and is now gone.

Coding agents make this worse and better at the same time. Worse, because an agent
re-derives that knowledge on every task and then discards it. Better, because an agent
will happily write down what it learns, if there is somewhere to put it — and a comment
is not somewhere, because the next refactor deletes it or moves the code out from under
it.

codedoc is that somewhere.

## What it looks like

An agent working on your code finds something non-obvious and records it:

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
             would credit an author with recording work they did not do.
             Under-crediting is the smaller wrong."
}
```

Nothing changes in the source file. Weeks later, a different agent is asked to touch
that function. It calls `codedoc_context` before reading the code, and gets back:

```json
{
  "target": {
    "file": "crates/codedoc-verify/src/history.rs",
    "symbol": "rust://is_ancestor"
  },
  "invariants": [
    {
      "kind": "invariant",
      "claim": "Only git exit code 1 means not an ancestor; every other failure
                must be read as already present.",
      "detail": "git exits 0 for ancestor, 1 for not, and 128 for a revision it
                 does not know. The only caller decides whether a record was
                 written during a change, so reading 128 as 'not an ancestor'
                 would credit an author with recording work they did not do.
                 Under-crediting is the smaller wrong.",
      "assurance": "asserted"
    }
  ]
}
```

Every tool has a command-line equivalent that emits the same JSON under `--json`,
documented in [docs/cli.md](docs/cli.md).

## Install

Requires Rust 1.85 or later and a C toolchain for the tree-sitter grammars.

```bash
git clone https://github.com/LukasCollishawM/codedoc
cd codedoc
cargo install --path crates/codedoc-cli   # the codedoc command
cargo install --path crates/codedoc-mcp   # the MCP server
cargo install --path crates/codedoc-lsp   # optional: hovers and diagnostics in an editor
```

`codedoc-mcp` is a separate binary from `codedoc`, and it is the one your agent runs.

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
when to use them. [AGENTS.md](AGENTS.md) covers what it is told and how to get useful
records out of an agent rather than restatements of the code.

Adopting on a codebase that already has comments? `codedoc import src/` reads them,
works out which construct each one describes, and files it — `TODO` becomes a warning,
`SAFETY:` becomes a security note. It is a dry run by default and never edits your
source.

## How it survives the code changing

A note pinned to "line 47" is wrong the moment someone adds an import. So codedoc does
not store line numbers. Each claim is attached to an **anchor**, which records several
independent descriptions of the code it points at:

- the **name** — `rust://is_ancestor`, built from the enclosing declarations
- the **shape** of the code, ignoring what things are called
- the **content**, ignoring formatting and comments
- the **neighbours** on either side

Insert fifty lines above it and every one of those still matches. Rename the variables
inside it and the shape still matches. Reformat it and the content still matches. The
claim follows the code.

**When the evidence runs out, codedoc stops rather than guesses.** If a function was
deleted and a similar one now sits where it was, codedoc will not quietly move the
claim across — it marks the anchor **detached** and asks a human. Documentation
attached to the wrong function is worse than documentation that admits it is lost, so
the two are not traded off against each other.

Asking what state things are in looks like this:

```
$ codedoc verify
71 unchanged   18 migrated   0 stale   0 detached
```

- **unchanged** — found it, nothing about it moved
- **migrated** — found it somewhere else; the code moved and the claim came with it
- **stale** — found it, but the code underneath changed enough to be worth re-reading.
  It does not mean the claim is wrong
- **detached** — could not find it without guessing, so it is waiting for a decision

Anything stale or detached is listed underneath with the claim and where it was, and
the exit code says which happened, so CI can branch on it.

A stale claim is a question put to whoever changed the code: `codedoc affirm` if it
still holds, `codedoc supersede` if it needs rewording, `codedoc retract` if it is gone.
A claim nobody answers keeps being reported until somebody does.

## Records are never edited

Revising a claim writes a new one that supersedes the old. Retracting one writes a
tombstone. Nothing is deleted, so "what did we believe about this six months ago" is a
question you can ask.

Each record carries who made it — a named person, or a specific agent and session — and
how sure they were. An agent's guess and a human's verified assertion stay
distinguishable, and the two are weighted differently when context is assembled.

Claims can also join two pieces of code rather than describe one: *this function must
run after that one*, *this is guarded by that check*. Such a fact belongs to neither
function on its own and has nowhere to live in a comment on either.

## Does it actually work

Replayed over the real history of seven codebases — zod, gson, ripgrep, httpx, cobra,
fmt and this repository — **5,371 anchors, and not one landed on the wrong symbol.**
Survival ranges from 99.6% to 82.9%, and the detachments were checked against the final
revision rather than sampled: they name code that is genuinely gone.

The spread is a property of the codebases rather than the language support. One of them
deleted nineteen files over the window; another declares dozens of identically named
test macros per file, and a name shared by fifty declarations identifies none of them.
[CLAUDE.md](CLAUDE.md) has the table and what each number means.

## Where the claims live

By default they go in `.codedoc/`, committed, and shared with your team.

For a repository you do not own or do not want to modify:

```bash
codedoc init --scope local
```

That writes inside `.git/`, which git cannot track. No directory in your working tree,
no `.gitignore` entry, nothing in `git status` — the repository contains no evidence
you are using it. `--scope global` keeps them outside the repository entirely.

## Languages

Rust, Python, TypeScript, TSX, Go, Java, C# and C/C++, via tree-sitter. Adding a
language means writing an adapter rather than changing the core.

A file no parser understands — a Dockerfile, a CI workflow, a migration, a Markdown
page — can still carry a claim about the file as a whole. Those follow the file's path
rather than its structure, so they survive it being edited and detach when it is
deleted. What they cannot do is point at something inside the file.

Two adapters have known gaps, each with a test that will fail when the gap closes
(`cargo test --workspace -- --ignored`). In C#, a file-scoped namespace
(`namespace Acme;`) contributes nothing to a name, so types in those files are recorded
unqualified. In TypeScript, `export const validate = () => {}` is not treated as a
declaration, so a claim on one holds only while its file is unedited. Neither can
attach a claim to the wrong code — an anchor that cannot be named detaches instead —
but both narrow what can be tracked.

## Status

Early, and the storage format is not settled. Before 1.0 it may change, always with a
mechanical `codedoc migrate` path; after 1.0 it will not change incompatibly. Claims
accumulate over months and cannot be regenerated from the code, which is why the
compatibility rules here are stricter than for the API.

Working: the store and its integrity checking, anchoring and resolution across eight
grammars, comment import, search, the full claim lifecycle, relations, and the CLI, MCP
and LSP surfaces. CI checks that the storage encoding is byte-identical on Linux, macOS
and Windows, and runs codedoc against this repository's own claims.

Not done: the VS Code extension installs from a local `.vsix` but is not on the
marketplace.

## Documentation

| | |
| --- | --- |
| [AGENTS.md](AGENTS.md) | what your agent is told, and how to get good records out of one |
| [docs/cli.md](docs/cli.md) | every command, for humans and CI |
| [docs/spec/format.md](docs/spec/format.md) | the storage format, written to bind other implementations |
| [CLAUDE.md](CLAUDE.md) | architecture and the rules that govern changes to it |
| [CONTRIBUTING.md](CONTRIBUTING.md) | read before a PR, including the no-comments rule |
| [SECURITY.md](SECURITY.md) | threat model; a cloned repository's claims are untrusted input |

## Licence

Code is `MIT OR Apache-2.0`. The specification and conformance vectors are `CC0-1.0`.
