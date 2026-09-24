# ADR-0013: A file with no language adapter can still carry a claim

Status: Accepted

## Context

ADR-0012 gave a claim the option of having a file as its subject, and section 4 of the
specification resolves such a claim by path alone at rung 0: it holds if and only if
the file exists, and is never searched for within the file.

That rung needs no parse. The implementation demanded one anyway. `attach` asked the
registry for a language adapter before it looked at what was being anchored, so every
path through it was gated on tree-sitter understanding the file:

```
$ codedoc attach README.md --kind decision --claim "..."
codedoc: no language adapter is registered for README.md
```

The set of files this excludes is not a fringe. A Dockerfile, a CI workflow, a SQL
migration, a lockfile, a `.toml`, a Makefile and the documentation itself are all
places where a reader meets a decision with no explanation and no way to add one.
Why a base image is pinned, why a matrix excludes a target, what a backfill assumes
about ordering — none of it could be recorded, in a tool whose entire claim is that a
repository should accumulate institutional memory.

The gap was found by using the tool: an attempt to record the documentation-register
convention against `README.md` failed, and the claim had to be anchored to
`xtask/src/docs.rs` instead, which is not what it is about.

## What was considered

**Writing a text adapter in `codedoc-lang`.** A trivial grammar producing one root
node spanning the file would make everything work with no changes anywhere else, and
it is the tidiest option. It was rejected on ownership rather than design:
`codedoc-lang` belongs to another workstream. It remains the better long-term shape and
is worth proposing there.

**Leaving it and documenting the limitation.** Honest, and it was the state of things.
But the limitation is not inherent — rung 0 explicitly does not need a parse — so
documenting it would be recording an implementation accident as though it were a
property of the format.

**Fingerprinting the file as a single leaf.** Adopted. Section 3 computes a childless
node's content fingerprint over its source text. A file with no structure is exactly
one leaf, so the existing rule applied to it gives the digest of the file's bytes, and
no new rule is required.

## Decision

An anchor whose subject is the file may be captured for any readable path, whether or
not a language adapter exists for it.

Where no adapter exists the anchor is **opaque**: its language is `text`, its node kind
is empty, its structural and context fingerprints are the digests of empty payloads
because there is no structure and no siblings to describe, its shape histogram is
empty, its content fingerprint is the digest of the file's bytes under the existing
content domain, and its range spans the file.

Resolution is unchanged in substance. An opaque anchor takes rung 0 and resolves if and
only if its path is present, so editing the file does not move it and deleting the file
detaches it.

Naming a construct inside such a file is refused rather than approximated, and the
refusal says what to do instead.

No member is added to the format and no member changes type, so a reader written before
this decision reads an opaque anchor without modification and no migration is required.

## Consequences

**Drift is not measurable for an opaque anchor, and is reported as absent rather than
as a number.** Drift compares shape histograms, and there is no shape. The content
fingerprint would support a changed-or-not answer, but a claim that went fully stale on
a one-character edit would be noise, and section 5 is explicit that noise in a
staleness report is worse than no report. The consequence is real: a claim about a
Dockerfile will not be flagged when the Dockerfile is rewritten. It is recorded as a
known failure mode against the capture function rather than left to be discovered.

**`coverage` and `gaps` are unaffected.** Both enumerate declarations, an opaque file
has none, and neither counts files it cannot parse.

**Every opaque anchor in a repository shares a structural fingerprint**, since all are
the digest of an empty payload. This is harmless because rung 0 never consults it, and
it is the truthful value: two files with no structure are not distinguishable by
structure.
