# ADR-0006: Use the official MCP SDK rather than hand-rolled JSON-RPC

Status: Accepted

## Context

The MCP server was first written as a hand-rolled JSON-RPC loop over stdio, justified as avoiding a dependency.

That justification did not survive scrutiny. The workspace already commits to an LSP library, so the async runtime was not avoidable, and the hand-written server had already drifted from the specification: it hardcoded the protocol version instead of negotiating it, silently dropped `notifications/initialized`, and returned `-32603` where `-32602` is correct for invalid parameters.

## Decision

`rmcp`, the official Rust MCP SDK. Tool logic stays in a transport-independent `operations` module; `rmcp` owns the protocol.

## Consequences

Protocol conformance, schemas generated from typed argument structs, and correct lifecycle handling stop being ours to maintain. MCP is a moving specification and this is a product surface, not a convenience. Keeping `operations` separate means a future transport change does not touch the logic.
