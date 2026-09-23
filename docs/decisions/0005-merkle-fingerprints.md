# ADR-0005: Bottom-up Merkle fingerprints

Status: Accepted

## Context

The first implementation computed a node's fingerprint by serialising its entire subtree, making whole-file fingerprinting O(nodes x depth) with large intermediate buffers. Verifying 2095 anchors took over two minutes.

## Decision

Fingerprints are computed bottom-up in a single memoised pass: a node's digest combines its kind, field names and its children's digests, so each subtree is hashed once.

## Consequences

Whole-file fingerprinting is O(nodes) and the same workload fell to seconds. Digest values changed, which was acceptable only because nothing had been released; after 1.0 it would have required a migration. The single-node entry points delegate to the same pass, so there is one implementation and no way for the two to disagree.
