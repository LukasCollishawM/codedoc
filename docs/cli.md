# The codedoc CLI

For humans and CI. Agents normally use the MCP server instead — see
[AGENTS.md](../AGENTS.md).

Every command accepts `--json`, which emits a stable schema. The human output is
rendered *from* that JSON, never in parallel to it, so the two cannot disagree.

Global flags: `--root <path>` (defaults to the working directory, searching upward
for a ledger), `--json`, `--scope shared|local|global`.

`--scope` chooses which ledger a command **writes** to. It also narrows the three
commands that enumerate records — `list`, `search` and `stats` — which is how you ask
what you have recorded locally in a repository you do not own.

`supersede`, `affirm`, `retract` and `resolve` write to the ledger holding the record
they act on, not to the default one, unless you name a scope. A record you kept in the
untracked local ledger stays there when you revise or retire it — otherwise affirming a
local note would copy it into the repository, and a tombstone quotes the claim it
retires, so retracting one would publish the very text the local scope was keeping out.

It deliberately does not narrow `verify`, `context`, `detached`, `conflicts` or
`evidence`. Those answer questions about whether your knowledge still holds, and a
claim you recorded locally is no less true while you are checking it, so reading only
one ledger would make them answer the wrong question.

## Exit codes

| code | meaning |
| --- | --- |
| `0` | clean |
| `1` | stale documentation present |
| `2` | something needs a decision: a detached anchor from `verify`, `detached` or `review`; a citation that stopped resolving from `evidence`; either of those from `doctor` |
| `3` | ledger integrity failure |
| `4` | the command itself failed |

`doctor` exits `0` for findings that need someone to read code and decide — drifted
claims, apparent disagreements — and `2` only for what a machine can settle. A build
that fails on "somebody should re-read this" is a build people learn to route around.

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

The result reports `restates_the_symbol` when every word of the claim is already in
the name it is attached to — "Validates the token" on `validate_token`. This project
exists because comments restate code; a record that does it has the same problem and
costs a reader the same time. Measured over 26,305 imported comments from real
codebases it fires on 0.21% of them, so it is a rare signal rather than a nag.

The result also carries a `similar` list: active claims on the same code that this one
largely restates, worst first. The record is written regardless — a near-duplicate is a
prompt, not a refusal — but a claim listed there is usually better superseded than
duplicated. The threshold is the same one `codedoc conflicts` uses, so what is flagged
here is what would be flagged there later.

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

It searches what a claim says and also what it is about, so the name of a function or
a file finds the records attached to it even when the claim never mentions them. What a
claim says is weighted above where it lives.

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

Options: `--claim` (one is generated from the verb if omitted), `--detail`,
`--assurance`, `--author human|agent|analyzer|runtime`, `--identity`, `--session`.

### `codedoc kinds`

Prints the record kind vocabulary.

## Reading

### `codedoc context <file>[:line] [--symbol <path>] [--depth N] [--budget N] [--as-of <date>]`

Retrieves what is known about a location: invariants, security, known failure modes,
rationale, and relations at `--depth` (default 2).

Claims are ranked by trust — assurance, then authorship, then age — and `--budget`
caps the assembled size by dropping the least trustworthy first. `--as-of` answers as
of a past moment; see `codedoc list` below.

### `codedoc list [--file <path>] [--symbol <path>] [--as-of <date>]`

Active records, optionally narrowed.

`--as-of` answers what was recorded as of a moment rather than now: pass `2026-03-01`
or a full RFC 3339 instant. Records written later are excluded and revisions made later
are undone, so a claim that has since been superseded comes back in the wording it had
then. `codedoc context` takes the same argument.

A date that cannot be parsed is refused rather than read as the epoch, which would
answer "nothing was known" to a question that was only mistyped.

### `codedoc history <record|symbol>`

What was believed before, and when it changed.

Given a record identifier — abbreviable to any unambiguous prefix — it returns that
record's supersession chain, from whichever revision you name: the whole chain, not
only what came before the one you happened to have. Entries restating their parent
word for word are marked `affirmation: true`, which is how a chain distinguishes
someone checking again from someone changing their mind.

Given a symbol path such as `rust://validate_token`, it returns everything ever
recorded about that symbol in chronological order, including records since superseded
or retracted, each marked `believed`, `withdrawn` or `retraction`. This is the "how did
this come to be the way it is" question: the claims that were made, which were revised
away, and which still stand.

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

Each entry in the thinnest list also reports `about_the_file`: claims recorded against
the file itself rather than any declaration in it. They do not count toward the
percentage, because a module header does not document a function — but a file that
already carries one is not the blank page the percentage makes it look like.

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

### `codedoc brief <files...> [--since <rev>]`

Everything recorded about a set of files, as one answer, before you change them.

```bash
codedoc brief src/auth.rs src/tenancy.rs --budget 4000
codedoc brief --since main --depth 1
```

This is the counterpart to `codedoc review`: a brief is what you should know before
starting, a review is what you may have invalidated after finishing.

`--budget` is spent **across the whole set**, not per file. Asking for context on six
files separately returns the top claims from each and six times the intended size; a
brief returns what matters most about the change. `--depth` follows relations that many
hops. Each claim names the file and symbol it belongs to, which a single-file context
pack does not need to.

Naming no files and no revision is refused rather than answered with the whole ledger.

### `codedoc doctor`

One answer to whether the recorded knowledge in a repository is in good order.

```bash
codedoc doctor
```

It reports two classes, and the split is the point.

**Blocking** — a broken hash chain, a detached anchor, a citation that no longer
resolves. Each of these is settled without anyone's judgement: the chain is broken or
it is not, the code is there or it is gone. Exit code 2.

**Advisory** — claims whose code drifted, and records that appear to disagree. Settling
these needs someone to read the code and decide, and a build that fails on "somebody
should re-read this" is a build people learn to ignore, which costs more than it
catches. Exit code 0, reported in the output.

The `next` field names the command to run for each finding.

### `codedoc evidence`

Checks that the support records cite still exists.

```bash
codedoc evidence
```

A claim citing a document that was deleted, a record that was retracted, a revision no
longer in the repository or a test that is gone still reads as well evidenced. That is
worse than citing nothing, because the citation lends an authority nothing is holding
up any more. Exit code 2 when anything is broken.

URLs are recorded and never checked. Fetching one would mean codedoc making a network
request out of someone's repository, which it does not do.

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

### `codedoc affirm <record>`

Records that a claim still holds against the code as it now is. Use it when `codedoc
verify` reports a record as stale, you have re-read the code, and the claim is still
true.

```bash
codedoc affirm 638bd987 --author human --identity "a reviewer"
```

It re-anchors the claim to the current shape, so the drift that made it stale clears,
and it records who did the re-reading and at which revision. The claim itself is not
reworded — an affirmation that changed the words would be a supersede. `codedoc
history` marks these entries `affirmation: true`, which is how a chain distinguishes
"someone checked this again" from "someone changed their mind".

A detached record cannot be affirmed: affirming means the code was re-read, and if the
code cannot be found there was nothing to read. Place it with `codedoc resolve` first.

Options: `--assurance` (defaults to the original's), `--author`, `--identity`,
`--session`.

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
