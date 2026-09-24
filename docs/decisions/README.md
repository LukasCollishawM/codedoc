# Decision records

Architectural decisions, including the alternatives that were rejected and why. A decision record containing only accepted decisions is a press release.

Any change to the on-disk format requires a record here, then a change to `docs/spec/format.md`, then vectors in `conformance/`, then code.

| | |
| --- | --- |
| [ADR-0001](0001-rust-for-the-core.md) | Rust for the core |
| [ADR-0002](0002-tree-sitter-for-anchoring.md) | tree-sitter for anchoring |
| [ADR-0003](0003-committed-jsonl-ledger.md) | A committed JSONL ledger with a derived index |
| [ADR-0004](0004-ambiguity-falls-through.md) | Ambiguity falls through the ladder rather than terminating |
| [ADR-0005](0005-merkle-fingerprints.md) | Bottom-up Merkle fingerprints |
| [ADR-0006](0006-official-mcp-sdk.md) | Use the official MCP SDK rather than hand-rolled JSON-RPC |
| [ADR-0007](0007-lsp-server-over-tower-lsp.md) | lsp-server rather than tower-lsp |
| [ADR-0008](0008-drop-the-time-dependency.md) | Remove the time dependency |
| [ADR-0009](0009-zero-comments-in-this-repository.md) | This repository contains no comments |
| [ADR-0010](0010-no-verification-cache.md) | No verification cache *(rejected)* |
| [ADR-0011](0011-renames-detach.md) | A renamed declaration detaches |
| [ADR-0012](0012-file-scoped-claims.md) | Claims whose subject is the file |
| [ADR-0013](0013-opaque-file-anchors.md) | Opaque anchors for files no adapter parses |
| [ADR-0014](0014-freeze-the-format-at-1-0.md) | Freeze the format at 1.0 |
