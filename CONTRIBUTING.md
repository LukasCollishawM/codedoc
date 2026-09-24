# Contributing to codedoc

## This codebase contains no comments

`cargo xtask lint-comments` fails the build on `//` and `/* */` anywhere under `crates/**/src`. The rule is absolute rather than a preference for sparse commenting.

The reason is the premise of the project. Knowledge written as a comment cannot be queried, typed, superseded, contradicted, attached to two places at once, or checked for staleness when the code beneath it moves. A tool built on that argument cannot coherently store its own knowledge in comments.

Records replace them:

```bash
codedoc attach crates/codedoc-anchor/src/resolver.rs --symbol resolve_context \
  --kind rationale \
  --claim "Context rungs bracket the target rather than matching it, because a node that moved without changing has identical fingerprints at every position."
```

The record is written to `.codedoc/`, travels with the repository, appears on hover in an editor, and is flagged for review when the code it describes changes.

If you are mid-change and not ready to write records, put the prose in the pull request description and a maintainer will help convert it. A contribution will not be rejected for unfamiliarity with the ledger. The lint output states what to do; if it does not, that is a defect in the lint and worth reporting separately.

## Your agent gets codedoc on this repository

`.mcp.json` in the repository root registers `codedoc-mcp` against this checkout, so
an agent opening the project is handed the twenty-three tools and the ledger that
describes the code it is about to change. It runs the server through
`cargo run --release -p codedoc-mcp`, which needs one release build first —
`just check` does that, and so does `cargo build --release`.

Without it, an agent working on codedoc has to be told to use codedoc, which is the
position every other repository is in and the one this project exists to change.

## Standards are enforced by CI, not by reviewers

Every rule in `CLAUDE.md` is a mechanical check. This is deliberate: strict standards policed by human taste turn into gatekeeping, and a reviewer should never be the first to tell you something is disallowed. Run the checks before you push and review becomes a conversation about the design instead of the lint.

```bash
just check        # exactly what CI runs: lint, test, replay, cargo-deny, doctor
just lint         # clippy -D warnings, fmt, no-comments, docs, prose, cargo-deny
just test         # cargo test --workspace
```

If a reviewer asks for something no check enforces, that is a gap — either the check gets built or the request is only a suggestion. Say so; it is a fair thing to say.

## Known gaps are ignored tests

```bash
cargo test --workspace -- --ignored
```

Where something is known to be wrong and not yet fixed, the repository holds a test
asserting the behaviour it should have, marked `#[ignore]` with the reason, next to a
live test asserting the gap is still there. Running the ignored set tells you what is
broken and what fixing it would have to make true; the live one fails the day the gap
closes and tells you to delete both. A known gap that lives only in prose is one
nobody finds and nobody can check.

## The invariants

`CLAUDE.md` states four. One is repeated here because it is the one most often disputed in review:

**Never silently reattach.** When the resolver cannot identify where documentation moved to, the correct outcome is `DETACHED`, not a best guess. Ambiguity is failure, never a tiebreak. A patch that raises anchor-survival numbers by guessing will be rejected even though the numbers improve, because documentation attached to the wrong code is worse than documentation that admits it is lost. Survival rate is a quality metric. False reattachment is a hard zero, and the replay harness fails the build on a single instance.

If you believe you have a resolution strategy that is sound rather than merely effective on the available corpora, open an issue before implementing it.

## Changing the format

`.codedoc/` is a compatibility surface, not an implementation detail. Others' ledgers are years of accumulated institutional memory and are not reproducible.

Any change to the on-disk format requires, in order: an ADR in `docs/decisions/`, a corresponding change to `docs/spec/`, new vectors in `conformance/`, and a mechanical migration in `codedoc migrate`. The Rust code is the reference implementation, not the definition — if the spec and the code disagree, the spec is right and the code has a bug.

Before 1.0 the format may change with migration. After 1.0 it does not change incompatibly.

## Practicalities

Commits follow [Conventional Commits](https://www.conventionalcommits.org/). Sign off with `git commit -s` — the project uses the [DCO](https://developercertificate.org/), not a CLA, so you keep your copyright and we make no claim to relicense your work.

Code is dual-licensed `MIT OR Apache-2.0`; the specification and conformance vectors are `CC0-1.0`. Contributions are accepted under those terms.

Participation is governed by the [Code of Conduct](CODE_OF_CONDUCT.md).

Discussion happens in public issues. Design decisions are recorded as ADRs, including rejected ones and the reasoning for rejecting them.
