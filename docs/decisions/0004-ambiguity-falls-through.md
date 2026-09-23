# ADR-0004: Ambiguity falls through the ladder rather than terminating

Status: Accepted

## Context

The first resolver treated multiple candidates at any rung as immediate detachment, reasoning that selecting among equals violates the never-guess invariant (I2).

Measured against a real corpus of 2095 records imported from the `rmcp` source, this detached 602 anchors: 29 percent of the corpus.

## Decision

A rung yielding more than one candidate neither selects nor terminates. It falls through to the next rung. Detachment happens only when no rung yields exactly one candidate, and the reported reason cites the earliest ambiguous rung.

## Rationale

The original reasoning conflated two things. I2 forbids *selecting* among indistinguishable candidates; it says nothing about continuing to look. Later rungs carry strictly more information: content identity cannot separate two textually identical functions, but the symbol path can. Terminating early discarded that information and produced detachments the evidence did not warrant.

On the same corpus, falling through reduced detachment from 602 to 121 while never selecting among equals.

## Consequences

The specification states this explicitly, because an implementation that terminates early looks conforming while being far less useful. A conformance vector covers the case where candidates genuinely cannot be distinguished, the anchoring symbol deleted with identical twins remaining, and requires detachment there.
