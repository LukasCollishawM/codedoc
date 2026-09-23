# The codedoc CLI

For humans and CI. Agents normally use the MCP server instead — see
[AGENTS.md](../AGENTS.md).

Every command accepts `--json`, which emits a stable schema. The human output is
rendered *from* that JSON, never in parallel to it, so the two cannot disagree.

Global flags: `--root <path>` (defaults to the working directory, searching upward
for a ledger), `--json`, `--scope shared|local|global`.

## Exit codes

| code | meaning |
| --- | --- |
| `0` | clean |
| `1` | stale documentation present |
| `2` | detached anchors requiring adjudication |
| `3` | ledger integrity failure |
| `4` | the command itself failed |

## Setting up

### `codedoc init [--scope shared\|local\|global]`

Creates a ledger. `shared` (the default) lives at `.codedoc/` and is committed.
`local` lives in `.git/codedoc/` and leaves no evidence in the repository. `global`
lives outside the repository, keyed by canonical path, and survives a reclone.

Reads merge across every scope present; writes go to one.

### `codedoc import <path>... [--write] [--limit N]`

Harvests existing comments, anchors each to the construct it documents, and infers a
kind from markers (`TODO` → warning, `SAFETY:` → security, "because…" → rationale).
Dry run unless `--write`. **Never modifies source.**

### `codedoc git install-merge-driver`

Registers a union merge driver for `.codedoc/ledger/*.jsonl`, so concurrent branches
merge their records instead of conflicting. The driver refuses to write if any input
line is not a valid record.

## Recording

### `codedoc attach <file> --kind <kind> --claim "..."`

Anchors a claim. Locate with `--symbol <path>` or `--line <n>`.

```bash
codedoc attach src/auth.rs --symbol rust://validate_token \
  --kind invariant \
  --claim "Signature validation must precede tenant resolution." \
  --detail "Resolving a tenant from an unvalidated token allows tenant confusion."
```

Options: `--detail`, `--assurance asserted|inferred|speculative`, `--author
human|agent|analyzer|runtime`, `--identity`, `--session`, `--evidence` (repeatable,
as `git:<rev>`, `test:<name>`, `doc:<path>`, `record:<id>` or a URL), `--supersedes`.

### `codedoc relate <subject> <verb> <object>`

Records a fact about the link between two places. Targets are `path@symbol` or
`path:line`.

```bash
codedoc relate src/auth.rs@rust://validate_signature must_execute_after \
               src/auth.rs@rust://resolve_tenant \
  --claim "Tenant resolution must not precede signature validation."
```

Verbs: `must_execute_after`, `guarded_by`, `constrained_by`, `invalidates`,
`tested_by`, `derived_from`, `contradicts`, `supersedes`, `owns`.

### `codedoc kinds`

Prints the record kind vocabulary.

## Reading

### `codedoc context <file>[:line] [--symbol <path>] [--depth N] [--budget N]`

Retrieves what is known about a location: invariants, security, known failure modes,
rationale, and relations at `--depth` (default 2).

Claims are ranked by trust — assurance, then authorship, then age — and `--budget`
caps the assembled size by dropping the least trustworthy first.

### `codedoc list [--file <path>] [--symbol <path>]`

Active records.

### `codedoc history <record>`

The supersession chain for a record: what was believed before, and when it changed.
Record identifiers may be abbreviated to any unambiguous prefix.

### `codedoc render [markdown|mermaid] [--title "..."] [--out <path>]`

Projects the ledger into something other than a terminal.

- **markdown** — every active record grouped by file, in source order. An onboarding
  document that cannot go stale independently of the code, because it is generated
  from anchors that are verified against it.
- **mermaid** — the relation graph as a diagram. Only relations appear, since they
  are the part of the ledger that is genuinely a graph.

Writes to stdout, or to `--out`.

### `codedoc stats`

Record counts by kind, relation count, ledger integrity, and which scopes are present.

## Maintaining

### `codedoc verify [<file>...] [--since <rev>]`

With no arguments, re-resolves every anchor against the working tree. Reports fresh, migrated, stale and
detached counts, and sets the exit code accordingly.

A record is **stale** either because its anchor only resolved through a weak signal,
or because the code it points at has *drifted*: the shape of the construct changed
enough that a claim about the old one may simply be false of the new one. A function
whose body is rewritten but whose name and signature survive resolves perfectly and
is still reported stale, with the drift percentage in `--json`.

Given files, or `--since <rev>` to derive them from what changed in git, verification
is scoped to those files and answers from the index rather than the whole ledger. At
1.09M lines that is the difference between 30 seconds and 67 milliseconds, which is
what makes checking your own change after each edit practical. A scoped run does not
perform the whole-ledger integrity scan and says so rather than implying it passed.

### `codedoc detached`

Lists anchors that could not be located. These are awaiting a decision, not errors —
an anchor detaches rather than attaching to the wrong code.

### `codedoc conflicts`

Reports records that appear to disagree:

- **declared** — a `contradicts` relation states outright that two records disagree.
- **near_duplicate** — two active records on the same code say almost the same thing.
  At volume this usually means one was meant to replace the other and was attached
  instead of superseded. Exact duplicates are not reported; they are collapsed on
  retrieval.
- **opposite_assurance** — the same code carries both an `asserted` and a
  `speculative` claim of the same kind.

These are signals for a human or an agent to adjudicate, not verdicts. Exits `1`
when anything is reported.

### `codedoc resolve <record> --to-symbol <path> | --to-line <n> [--in-file <path>]`

Places a detached record explicitly.

### `codedoc supersede <record> [--claim "..."] [--detail "..."] [--kind <kind>]`

Revises a record. Writes a superseding record and re-anchors it to the code's current
position; the original stays in history.

### `codedoc retract <record> [--reason "..."]`

Retires a record. Writes a tombstone; the claim stays queryable in history but leaves
the active set.

### `codedoc reindex`

Rebuilds the SQLite projection from the ledger. The index is derived state and is
never committed; this is only needed if it is deleted or corrupted.

### `codedoc repair [--write]`

Rebuilds a broken hash chain. A record is orphaned when its `chain` names a record the
ledger no longer holds, which does not happen through normal use — it means the ledger
was edited outside codedoc, in practice by rewriting git history across it.

Repair re-chains every record in timestamp order and remaps supersession links.
**Record identities change**, because an identity covers the chain the record was
written into, so this is a recovery tool rather than routine maintenance. Dry run
unless `--write`; back up `.codedoc` first.

The better answer is not to rewrite history across a ledger. Orphaning is detected,
not prevented.

### `codedoc migrate [--write]`

Reports the schema distribution of a ledger and applies any pending migrations.
Refuses to operate on records written by a newer build, rather than discarding
members it cannot represent.
