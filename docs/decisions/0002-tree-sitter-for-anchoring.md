# ADR-0002: tree-sitter for anchoring

Status: Accepted

## Context

Anchors must attach to program structure across many languages without building a compiler front end per language.

## Decision

tree-sitter, with a per-language adapter in `codedoc-lang` supplying declaration node kinds, name fields and ignorable (comment) kinds.

## Alternatives rejected

**Per-language LSP servers.** Would give real name resolution and types. Rejected as the anchoring foundation because it requires a working toolchain for every language in the repository, which is exactly the fragility a documentation tool cannot afford. It stays viable as later enrichment.

**Regex and heuristics.** Rejected outright; the resolver's safety property depends on structural identity being meaningful.

## Consequences

Anchors are structural and syntactic, not semantic. Two identical functions are genuinely indistinguishable to us, which is precisely why the resolver must be able to detach rather than guess. Adding a language is an adapter, not a rewrite.
