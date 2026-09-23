# ADR-0007: lsp-server rather than tower-lsp

Status: Accepted

## Context

`CLAUDE.md` originally named `tower-lsp`, chosen before any code existed.

## Decision

`lsp-server` and `lsp-types`.

## Rationale

`tower-lsp` 0.20 has not shipped since 2023. `lsp-server` is maintained by the rust-analyzer project and is synchronous, which suits work that is CPU-bound anyway: resolution is parsing and hashing, not IO.

## Consequences

No async runtime is required by the LSP path. `lsp-types` 0.97 replaced `url::Url` with a URI type that has no `to_file_path`, so the server carries a small percent-decoding conversion that handles Windows drive letters.
