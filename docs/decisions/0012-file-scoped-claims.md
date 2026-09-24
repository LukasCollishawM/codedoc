# ADR-0012: A claim may have a file as its subject

Status: Accepted

## Context

Every anchor pointed at a construct. The resolver ladder is built for that: it searches
a file for the declaration a record describes, and detaches when it cannot find it
without guessing.

Importing a 1.09M-line corpus showed what that costs. Of 27,420 imported anchors,
1,036 carried no symbol, and an anchor with no symbol cannot reach rungs 2, 3, 4 or 6 —
it is permanently unresolvable. Sampling them, 620 were module-level documentation:
Rust `//!` headers, and plain header comments above a file's imports. Because the
importer had to name a construct, it named whichever node followed the comment, which
was an inner attribute or a `use` statement.

That is wrong twice over. The record could never resolve, and it was not true: a
comment saying what a module is for is not a claim about the `use` statement beneath
it.

## What was considered

**Dropping them.** Honest, and it was the behaviour for blocks with no adjacent node.
But it discards the highest-level knowledge in a codebase — what a file is *for* — which
is exactly what a newcomer, human or agent, needs first.

**Anchoring to the first real declaration in the file.** Resolvable, but it makes a
claim about the module look like a claim about one function, and it moves when that
function is deleted. It trades an unresolvable record for a confidently wrong one,
which I2 exists to prevent.

## Decision

An anchor declares its subject: a `construct`, or the `file`. A file anchor records no
symbol and an empty node path, and its range spans the file.

A file anchor resolves at **rung 0**, by path. It resolves if and only if the file
exists — under the recorded path, or under a path rung 5 evidences it was renamed to.
It is never searched for within a file.

Rung 0 sits above rung 1 rather than below rung 6 because it is not a weaker form of
the ladder. The ladder exists to infer which construct a record describes; the identity
of a file is its path, which is checked rather than inferred. There is nothing to be
ambiguous about.

## Consequences

Drift for a file anchor is measured against the root node's shape, so a module doc
reports stale once the file it describes has been substantially rewritten. The
conformance vector measures 60% for a function body rewritten in place, against a stale
threshold of 25%.

Retrieval unions file claims into symbol queries. A claim about a file is a claim about
every declaration in it, so an agent asking about one function sees the module's
invariants without having to know to ask for them separately.

`codedoc attach` with neither `--symbol` nor `--line` records one. That was previously
an error, `TargetUnspecified`, which is the right shape for the feature: the target
grammar is a file, optionally narrowed to a symbol or a line, and the un-narrowed case
means what it says.

Unresolvable imported anchors fell from 1,036 to 106 on the same corpus.

## What this does not do

It does not introduce anchors for directories, packages or the repository. Those are
plausible subjects and the `subject` member leaves room for them, but nothing measured
demanded them, and a vocabulary that grows ahead of evidence is a vocabulary nobody can
resolve against.
