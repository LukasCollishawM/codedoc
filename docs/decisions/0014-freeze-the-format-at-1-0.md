# ADR-0014: Freeze the format at 1.0

Status: Accepted

## Context

`.codedoc/` holds knowledge a team accumulates over years and cannot regenerate. Once a
second implementation exists, those bytes are a contract. The specification has carried
"Version 1 (draft)" since it was written, which left every implementer waiting for the
draft to settle before committing to it, and left this repository free to change bytes
without saying so.

Three things were unspecified, and each would have let two implementations disagree
about bytes rather than about behaviour:

- `schema` was required of every record and defined nowhere. Its value, its meaning,
  and what an implementation does with a value it does not recognise were all absent.
- Which constructs contribute a symbol path segment was left to the implementation.
  Two implementations reading the same source would produce different symbol paths,
  so a ledger written by one is degraded when read by the other.
- Six of the seven digest domains had no conformance vector. Only the record domain was
  covered, through `record_digest` in the encoding vectors, so an implementation could
  get domain separation wrong for the other six and still pass.

## Decision

The format is 1.0 and stable. A ledger written by a conforming implementation of 1.0
remains readable under every later 1.x, and every identifier in it remains the
identifier it was written with.

Section 10 of the specification states what a minor version may add — an optional
member omitted at its default, a vocabulary entry, a rung below the existing ones, a
new digest domain — and what it may never change: any rule in section 1, any domain
string, any fingerprint pre-image, any frozen wire string, the meaning or position of a
rung, or whether a member is required.

A major version is a new format rather than an amendment to this one, and a mechanical
migration must exist before any record is rewritten from one to the other. Rewriting is
the only circumstance in which a record's identifier may change, because a record is its
bytes.

## What 1.0 does not do

**It does not publish the crates.** The compatibility surface is the format, not the
Rust API. Every crate keeps `publish = false`: a crates.io release is a permanent
obligation, and no consumer has asked for the libraries. `cargo-semver-checks` joins CI
with the first publish, as it always would have.

**It does not claim G6.** No second implementation has been written against the
specification alone and passed `conformance/`. Until one has, 1.0 is a promise this
project is making rather than one that has been tested from outside. Freezing before
that is deliberate: an implementer cannot reasonably be asked to build against a draft,
so the draft had to end first for G6 to become possible at all.

**It does not fix the adapter naming gaps.** Four declaration forms carry no symbol, or
the wrong one, in `codedoc-lang`: C# file-scoped namespaces, C++ namespaces opened by a
macro, Go `const` and `var`, and TypeScript lexical declarations. Section 3 now forbids
the C++ case explicitly, as a segment whose text is a language keyword, so the reference
implementation has a known deviation from the specification it publishes. These cost
durability rather than safety — an anchor that cannot be named detaches rather than
misattaching — and each is pinned by an ignored test naming the form it should produce.
They are additions to what an implementation names, which section 10 permits within 1.x.

## Consequences

The three holes are closed in `docs/spec/format.md`. `conformance/encoding/domains.json`
pins all seven domains against five payloads, including an empty one and one whose text
opens with a domain string followed by NUL, which is what separates an implementation
that inserts the separator from one that concatenates.

Regenerating the vectors was itself deleting four resolver vectors, including the rung 4
safety property the specification cites. CI now regenerates and fails on any diff.

The cost of freezing early is that a defect in the format is now expensive to fix. The
cost of not freezing is that nobody else can build against it, and a format with one
implementation is a data directory.
