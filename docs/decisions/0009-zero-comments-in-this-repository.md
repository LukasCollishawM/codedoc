# ADR-0009: This repository contains no comments

Status: Accepted

## Context

codedoc exists because knowledge written as a comment cannot be queried, typed, superseded or checked for staleness. A project making that argument while carrying comments would be incoherent.

## Decision

No `//` or `/* */` anywhere under `crates/**/src`, enforced by `cargo xtask lint-comments` in CI. Doc comments are permitted on `pub` items only, and only until `codedoc render rustdoc` can generate them from the ledger.

## Consequences

Every `#[allow(...)]` must carry a `workaround` record, linking a suppression to its justification structurally rather than by adjacency.

**This binds this repository only.** It is a dogfooding standard, not a precondition of using codedoc, and nothing in the tool requires it; `codedoc import` is additive and never edits source. If the rule ever reads as a requirement, adoption dies, so it is stated as ours wherever it appears, and the lint's failure output prints the `codedoc attach` command that replaces the comment rather than merely rejecting.
