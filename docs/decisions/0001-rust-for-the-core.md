# ADR-0001: Rust for the core

Status: Accepted

## Context

codedoc must parse many languages, fingerprint every node of large files, ship as something a person or an agent can install anywhere, and serve three protocols (CLI, MCP, LSP) from one codebase.

## Decision

Rust, as a Cargo workspace.

## Alternatives rejected

**TypeScript.** Fastest to prototype and would make the VS Code extension native. Rejected because whole-repository fingerprinting is CPU-bound, tree-sitter via WASM is materially slower, and a JetBrains plugin would still need a sidecar process, so the native-extension advantage evaporates for half the targets.

**C#.** Matches the origin context and Roslyn gives real semantic symbols. Rejected because the tree-sitter story is weak, which undermines the language-agnostic premise the whole format rests on.

## Consequences

A single static binary with no system dependencies, which matters because agents install this into arbitrary environments; `rusqlite` is bundled for the same reason. The cost is that editor plugins are thin clients over a server rather than native implementations.
