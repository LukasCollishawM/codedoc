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

Attributes, decorators and annotations between a comment and what it documents are
stepped over, so a doc comment above `#[cfg(...)]` above `pub fn` anchors to the
function. Anchoring to the attribute instead produces a record that cannot be
resolved, because attribute text repeats throughout a file and carries no symbol.

### `codedoc git install-merge-driver`

Registers a union merge driver for `.codedoc/ledger/*.jsonl`, so concurrent branches
merge their records instead of conflicting. The driver refuses to write if any input
line is not a valid record.

## Recording

### `codedoc attach <file> --kind <kind> --claim "..."`

Anchors a claim. Locate with `--symbol <path>` or `--line <n>`.

Omit both and the claim is recorded against **the file itself**. Use this when what you
know is true of the whole file rather than one declaration — what the module is for, or
an invariant every entry point in it upholds. A file claim is returned to anyone asking
about any symbol in that file, and it reports as stale once the file has been
substantially rewritten.

```bash
codedoc attach src/auth.rs --symbol rust://validate_token \
  --kind invariant \
  --claim "Signature validation must precede tenant resolution." \
  --detail "Resolving a tenant from an unvalidated token allows tenant confusion."
```

```bash
codedoc attach src/auth.rs   --kind invariant   --claim "Every handler in this module assumes the request is already authenticated."
```

Options: `--detail`, `--assurance asserted|inferred|speculative`, `--author
human|agent|analyzer|runtime`, `--identity`, `--session`, `--evidence` (repeatable,
as `git:<rev>`, `test:<name>`, `doc:<path>`, `record:<id>` or a URL), `--supersedes`.

### `codedoc search <words...>`

Finds recorded claims by what they say rather than by where they are. Use it when you
do not yet know which file to ask about.

```bash
codedoc search tenant isolation --limit 5
codedoc search retry --kind known_failure_mode --file src/http/
```

Terms are matched independently and results ranked, so a broad query returns something
useful rather than nothing. Ranking combines textual relevance with how much the record
is trusted, using the same assurance, authorship and age weighting as `codedoc context`.
Options: `--kind`, `--file` (path prefix), `--limit` (default 20).

Once you know the file or symbol, `codedoc context` is the sharper tool: it returns
everything that applies to a location, grouped by kind, rather than what matched a word.

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

### `codedoc coverage [<path>...] [--limit N]`

What fraction of declarations carry at least one active record, overall and by file,
with the thinnest files listed first. Tests, examples, benches and vendored
directories are excluded, since they are not what you document.

```
$ codedoc coverage crates
3% — 21 of 686 declarations carry a record, across 41 files
```

**Coverage is a prompt, not a target.** A codebase where every declaration carries a
record has mostly restated its own code, which is the failure mode this project
exists to avoid. It is useful for the opposite question: after importing a few
thousand comments, which parts of the system got nothing?

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

Each entry carries up to three **suggestions**: constructs of the same kind, ranked by
how closely their shape matches what was recorded. A renamed function usually appears
at 100%. These are proposals for you to confirm, never applied automatically — the
whole point of detaching is that the evidence was not sufficient to decide, and a
suggestion does not change that. It just saves you finding the candidate yourself.

### `codedoc review [<base>] [--out <path>]`

Renders the claims a change has put in doubt, as markdown suitable for posting on a
pull request. `base` defaults to `HEAD`; in CI use the merge base.

```bash
codedoc review origin/main --out review.md
```

It lists claims whose anchors detached (the code they described could not be found)
and claims that resolved but whose code drifted, with the percentage. Exit codes are
the same as `verify`, so a workflow can fail or comment on `1` and `2`.

It closes with a footnote counting the declarations the change touched that carry no
recorded knowledge. That is a prompt at the one moment someone has the context to act
on it, not a demand: the note disappears when there is nothing to say.

A claim appearing here is not necessarily wrong. It means the code it describes moved
or changed enough to be worth re-reading before merge, which is the moment that
knowledge is most worth having and least likely to be looked up.

`.github/workflows/codedoc-review.yml` in this repository runs it on every pull
request and posts the result as a single comment, updated in place rather than
appended to on each push. Copy it into your own repository; it needs
`pull-requests: write`.

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

**Renaming a declaration detaches its records**, and this is how you reattach them.
Nothing available at resolution time distinguishes a rename from a deletion followed
by a similar addition — position, shape and body identity were each tried and each
conflated the two. See [ADR-0011](decisions/0011-renames-detach.md).

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

It mends every ledger in the workspace, because `codedoc verify` reports across every
scope and a cure narrower than the diagnosis leaves you repairing something that still
reports broken. Pass `--scope` to confine it to one; the output reports per scope
either way.

The better answer is not to rewrite history across a ledger. Orphaning is detected,
not prevented.

### `codedoc migrate [--write]`

Reports the schema distribution of a ledger and applies any pending migrations.
Refuses to operate on records written by a newer build, rather than discarding
members it cannot represent.
