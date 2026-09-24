# ADR-0011: A renamed declaration detaches

Status: Accepted (reluctantly)

## Context

After rung 4 was narrowed to require a matching symbol path — see the false reattachment
it was producing, recorded in `docs/spec/format.md` — a declaration that is renamed
while its body is unchanged no longer resolves. It detaches and waits for
adjudication.

Renaming is routine, so this is a real cost. `codedoc resolve <record> --to-symbol
<new>` clears it in one command, but it is a command someone has to run.

## What was tried

The body of a renamed function is byte-identical, which looks like strong evidence:
stronger than position, and cheap to check. An implementation added `body_content` to
the anchor — the content fingerprint of the declaration's body subtree — and let rung 2
match on it when unique, on the reasoning that two constructs with byte-identical
bodies are the same code under a different name.

The conformance suite rejected it within a minute:

```
cpp_deleting_an_overload_with_an_identical_twin_detaches: expected detachment
  left: "detached"   right: "located"
```

## Why it does not work

Overloads routinely have identical bodies. `Gate::admit(int)` and `Gate::admit(long)`
that both do `int scaled = value; return scaled;` are different functions with the same
body, so body identity attached a claim about a deleted overload to its surviving
sibling — the exact failure fixed hours earlier, reintroduced through a different door.

The general shape: **byte-identical bodies do not imply the same construct.** They
imply the same *code*, which is not the same claim. Overloads, trait implementations,
platform-gated variants and generated code all produce distinct constructs with
identical bodies.

## Decision

Renames detach. There is no evidence available at resolution time that distinguishes
a rename from a deletion followed by a similar addition, and every candidate signal —
position, shape, body — has been shown to conflate them.

## Consequences

`codedoc resolve` is the answer, and the review output names the detached record so
the person who did the renaming is the one prompted, while they still know what they
renamed it to.

If this is revisited, the evidence to reach for is **version control**, not structure.
Git already knows a rename happened, at a similarity threshold, and rung 5 already
consults it for files. Extending that to constructs would be evidence about history
rather than a guess about shape, which is the distinction that matters.
