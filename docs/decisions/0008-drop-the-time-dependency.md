# ADR-0008: Remove the time dependency

Status: Accepted

## Context

`cargo deny` reported RUSTSEC-2026-0009, a denial of service via stack exhaustion, against `time` 0.3.45. The fix landed in 0.3.47, but current releases require Rust 1.88 against a declared MSRV of 1.85.

## Decision

Remove the dependency. It was used for exactly one thing: rendering an `i64` as RFC 3339.

## Alternatives rejected

**Raise the MSRV to 1.88.** Disproportionate; a documentation tool should not demand a recent toolchain from the projects adopting it.

**Ignore the advisory.** Our exposure was probably nil, since we format timestamps rather than parse untrusted ones. Rejected because an ignored advisory becomes a standing exception the next person must re-evaluate, and because the gate exists precisely so the decision is deliberate.

## Consequences

A short civil-from-days conversion in `codedoc-ledger`, tested at the epoch, a leap day, a pre-epoch timestamp and 2100. One fewer dependency, the advisory closed, and the MSRV held.
