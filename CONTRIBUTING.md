# Contributing to codedoc

## The rule that will surprise you

**This codebase contains no comments.** Not sparse comments, not comments-where-needed. None. `cargo xtask lint-comments` fails the build on `//` and `/* */` anywhere under `crates/**/src`.

This is not asceticism, and it is not a style preference. codedoc exists because knowledge written as a comment cannot be queried, typed, superseded, contradicted, attached to two places at once, or checked for staleness when the code beneath it moves. The project would be incoherent if it stored its own knowledge that way. So it does not.

What replaces comments is a command:

```bash
codedoc attach crates/codedoc-anchor/src/resolver.rs --symbol resolve_context \
  --kind rationale \
  --claim "Context rungs bracket the target rather than matching it, because a node that moved without changing has identical fingerprints at every position."
```

That record lands in `.codedoc/`, travels with the repository, appears on hover in your editor, and — unlike a comment — gets flagged when the code it describes changes underneath it.

If you are mid-change and not ready to write records, put the prose in your PR description and a maintainer will help convert it. **A first contribution will never be rejected for not knowing the ledger.** The lint tells you what to do, and if it fails to, that is a bug in the lint worth reporting on its own.

## Standards are enforced by CI, not by reviewers

Every rule in `CLAUDE.md` is a mechanical check. This is deliberate: strict standards policed by human taste turn into gatekeeping, and a reviewer should never be the first to tell you something is disallowed. Run the checks before you push and review becomes a conversation about the design instead of the lint.

```bash
just check        # exactly what CI runs: lint + test + replay
just lint         # clippy -D warnings, fmt, no-comments, cargo-deny
just test         # cargo nextest run --workspace
```

If a reviewer asks for something no check enforces, that is a gap — either the check gets built or the request is only a suggestion. Say so; it is a fair thing to say.

## The invariants are not negotiable

`CLAUDE.md` states four. One deserves repeating here because it is the most likely to be argued with in a PR:

**Never silently reattach.** When the resolver cannot identify where documentation moved to, the correct outcome is `DETACHED`, not a best guess. Ambiguity is failure, never a tiebreak. A patch that raises anchor-survival numbers by guessing will be rejected even though the numbers improve, because documentation attached to the wrong code is worse than documentation that admits it is lost. Survival rate is a quality metric. False reattachment is a hard zero, and the replay harness fails the build on a single instance.

If you think you have a resolution strategy that is genuinely sound rather than merely lucky, open an issue before writing it. That conversation is one of the more interesting ones this project has.

## Changing the format

`.codedoc/` is a compatibility surface, not an implementation detail. Others' ledgers are years of accumulated institutional memory and are not reproducible.

Any change to the on-disk format requires, in order: an ADR in `docs/decisions/`, a corresponding change to `docs/spec/`, new vectors in `conformance/`, and a mechanical migration in `codedoc migrate`. The Rust code is the reference implementation, not the definition — if the spec and the code disagree, the spec is right and the code has a bug.

Before 1.0 the format may change with migration. After 1.0 it does not change incompatibly.

## Practicalities

Commits follow [Conventional Commits](https://www.conventionalcommits.org/). Sign off with `git commit -s` — the project uses the [DCO](https://developercertificate.org/), not a CLA, so you keep your copyright and we make no claim to relicense your work.

Code is dual-licensed `MIT OR Apache-2.0`; the specification and conformance vectors are `CC0-1.0`. Contributions are accepted under those terms.

Discussion happens in public issues. Design decisions land as ADRs, including the ones that were rejected and why — a decision record that only contains accepted decisions is a press release.
