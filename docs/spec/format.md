# codedoc format specification

Version 1.0. Licensed `CC0-1.0`.

This version is stable. The byte sequences it defines will not change incompatibly: a ledger written by a conforming implementation of 1.0 remains readable, and every identifier in it remains the identifier it was written with, under every later 1.x version. Section 10 states what that permits and forbids.

This document is normative. The Rust workspace in this repository is the reference implementation, not the definition; where the two disagree, the implementation has a bug. Conformance vectors live in `conformance/` and any implementation claiming compatibility must reproduce them exactly.

The key words MUST, MUST NOT, SHOULD and MAY are to be interpreted as in RFC 2119.

## 1. Canonical encoding

Every hash in this format is taken over canonical bytes. Two implementations that disagree about canonical form will disagree about every identifier, so this section is the foundation of interoperability.

A canonical value is one of: null, boolean, integer, string, array, or object.

**Floating point numbers are not representable.** An encoder MUST reject them rather than rounding. Implementations SHOULD make this a property of their value type rather than a validation step.

Integers MUST be representable in signed 64-bit range. Values outside it MUST be rejected.

Encoding rules:

- No insignificant whitespace. No space after `:` or `,`.
- Object keys MUST be sorted ascending by their UTF-8 byte sequence, which is equivalent to sorting by Unicode scalar value.
- Duplicate object keys MUST be rejected on decode. They are ambiguous under ordering and their presence indicates a malformed or hostile document.
- Strings are UTF-8. The characters `"` and `\` MUST be escaped. The control characters U+0008, U+000C, U+000A, U+000D and U+0009 MUST use the short forms `\b`, `\f`, `\n`, `\r`, `\t`. Remaining characters below U+0020 MUST use `\u00xx` with **lowercase** hexadecimal. All other characters, including all non-ASCII, MUST be emitted literally and MUST NOT be escaped.
- Trailing content after a complete value MUST be rejected.

Encoding MUST be idempotent: decoding canonical bytes and re-encoding MUST produce identical bytes.

Unrecognised object members MUST be preserved across a decode and re-encode cycle. A ledger is distributed and an older implementation MUST NOT silently strip members written by a newer one.

**An optional member MUST be omitted when it holds its default.** A member that has a default is one an older record may not carry, so a value and its absence denote the same thing; emitting it would give one value two encodings, which is what canonical form exists to prevent.

This rule carries more weight than tidiness. A record's identifier is the digest of its canonical encoding, and an implementation recomputes that identifier when it reads a record back. A member that serialises while holding its default therefore changes the identifier of **every record written before that member existed**, which breaks the hash chain linking them and every supersession reference pointing at them. The failure presents as a corrupt or tampered-with ledger rather than as a schema change, so it is not self-diagnosing.

A record's identifier is the digest of its **canonical re-encoding**, not of the bytes it was read from. A line that carries an optional member at its default therefore yields the same identifier as one that omits it, which is what allows a ledger written before that member existed to keep every identifier it was written with. An implementation MUST compute identity this way rather than over the stored bytes.

It follows that adding a member to this format is a compatible change only when the member is optional and omitted at its default. `conformance/records/identity.jsonl` freezes records carrying none of the optional members, with the identifiers they must still produce.

## 2. Digests

All digests are BLAKE3-256, rendered as 64 lowercase hexadecimal characters.

Digests are domain-separated. The pre-image is:

```
domain_string || 0x00 || payload
```

Domains defined by this version:

| purpose | domain |
| --- | --- |
| record identity | `codedoc.record.v1` |
| anchor identity | `codedoc.anchor.v1` |
| file identity | `codedoc.file.v1` |
| content fingerprint | `codedoc.fingerprint.content.v1` |
| structural fingerprint | `codedoc.fingerprint.structural.v1` |
| context fingerprint | `codedoc.fingerprint.context.v1` |
| ledger head | `codedoc.ledger.head.v1` |

Domain separation is mandatory: a file digest and a record digest over identical payloads MUST differ.

## 3. Anchors

An anchor identifies a region of program structure. It MUST NOT be interpreted as a line range; the `range` member is a cache and carries no authority.

Fingerprints are computed over a concrete syntax tree. Nodes the language adapter classifies as ignorable — comments — are excluded from every fingerprint, so annotating code does not disturb anchors attached to it.

Fingerprints are computed bottom-up, so that a subtree's digest depends only on that subtree:

**Structural fingerprint.** For a node, the pre-image is the node kind, `(`, then for each named non-ignorable child in order: the field name followed by `:` when the child occupies a named field, then that child's structural digest bytes; then `)`. Identifier and literal text is excluded, so renaming a local does not change it.

**Content fingerprint.** For a childless node the pre-image is its source text. Otherwise it is the concatenation of every non-ignorable child's content digest bytes, in order, including anonymous children. Whitespace is therefore excluded and punctuation is included.

**Context fingerprints.** The pre-image is the parent node kind (or `<root>`), `0x00`, then for up to two neighbouring named non-ignorable siblings in document order, that sibling's structural digest bytes each followed by `0x00`. Preceding and following contexts are computed separately.

**Symbol path.** `language://Segment/Segment`, built from the names of enclosing declarations, outermost first. The language component MUST be lowercase alphanumeric. Segments MUST NOT be empty.

Generic parameters and qualifying prefixes MUST NOT appear in a segment, so that renaming a lifetime or moving a type between modules does not move an anchor. A language MAY declare that its declaration names are *qualified*, in which case an out-of-line definition such as `Foo::bar` contributes both segments; otherwise only the terminal name is used.

**Which constructs name a segment is per-language, and is normative.** An implementation claiming support for a language MUST reproduce that language's naming vectors in `conformance/resolver/`, which state the segment a given declaration form contributes. Without that, two implementations reading the same source produce different symbol paths for the same construct, and a ledger written by one is degraded when read by the other.

Degraded, not corrupted. Rungs 2, 3, 4 and 6 all require the symbol path to match, so an implementation that names fewer declaration forms than the one which wrote a record fails to match rather than matching something else: the anchor falls to rung 1, and to detachment if its content has changed. A symbol path an implementation cannot construct is therefore lossy and safe, which is the same trade the rest of this format makes. An implementation MUST NOT invent a segment for a declaration form it does not recognise, and MUST NOT emit a segment whose text is a keyword of the language, which indicates a mis-parse rather than a name.

An anchor MAY carry no symbol path at all, when its construct sits in no naming declaration. Such an anchor can only reach rung 1, so it survives edits elsewhere in its file and is lost when its own construct changes.

**Symbol cardinality and ordinal.** A symbol path does not necessarily identify a unique declaration — an overload set is the common case. An anchor MUST therefore record how many declarations shared its symbol path when it was captured, and which of them it was, in document order.

Cardinality is counted **per declaration kind**, and the anchor records the kind it was counted under. Counting per symbol alone is too coarse: in Rust a type and its `impl` blocks all produce the same symbol path, so replacing a hand-written `impl` with a derive would change the count and detach every record on the type, even though the type itself never moved. Measured on ripgrep, counting per kind raised anchor survival from 97.5% to 98.2% with no loss of safety, because an overload set shares both a symbol and a kind and is still caught.

This is not bookkeeping. Without it, deleting one member of an overload set leaves exactly one declaration bearing that symbol, and a resolver that trusts the symbol will attach the deleted member's documentation to its surviving sibling, at high confidence. See section 4.

Implementations reading a record written before these members existed MUST treat the cardinality as 1 and the ordinal as 0.

Cardinality is not rare. A C++ overload set produces it, and so does an in-class declaration paired with its out-of-line definition, since both contribute the same symbol path by different routes. `conformance/resolver/` carries both shapes with the expected cardinality stated.

**Shape histogram.** An anchor records a count of node kinds in its subtree, used both to rank candidates at rung 6 and to measure drift. It excludes ignorable nodes.

**Subject.** An anchor declares what it is about: a `construct` within a file, or the `file` itself. The default is `construct`, and an implementation reading a record written before this member existed MUST assume it.

A `file` subject exists because not all knowledge about code is knowledge about a declaration. A module header stating what the file is for, an invariant every entry point in the file upholds, or a warning about the file's licence are claims whose subject genuinely is the file. Before this distinction existed such a claim had to be pinned to whichever declaration happened to sit nearest it, which was both wrong and unresolvable: measured over a 1.09M-line corpus, module-level documentation accounted for 620 of 1,036 anchors that carried no symbol and therefore could never resolve.

An anchor with subject `file` MUST record an empty node path and no symbol, and its range MUST span the file. Its fingerprints and shape histogram are those of the root node, so drift for a file anchor measures how much of the file changed.

A file anchor MAY be captured for a file no language adapter understands, because rung 0 resolves by path and never parses. Such an anchor is **opaque**: it MUST record an empty node kind, an empty shape histogram, and structural and context fingerprints over an empty payload, since it has no structure and no siblings. Its content fingerprint is the digest of the file's bytes, which is the childless-node rule of this section applied to a file that is one leaf. Its language SHOULD be recorded as `text`.

An implementation MUST NOT report drift for an opaque anchor. Drift compares shape histograms and there is none, so any figure would be invented; section 5 requires that a staleness report not carry noise.

An implementation MUST refuse to capture a construct anchor in a file it cannot parse, rather than approximating one.

## 4. Resolution

An anchor whose subject is `file` MUST be resolved by path alone, at rung 0, and MUST NOT be searched for within the file. It resolves if and only if the file exists — under the path recorded, or under a path rung 5 evidences it was renamed to. This is not a weaker form of the ladder but a stronger one: the identity of a file is its path, so the resolver checks it rather than inferring it.

It follows that an opaque anchor resolves exactly as any other file anchor does. An implementation MUST NOT detach one on the ground that the file has no adapter, because no step of rung 0 consults the language.

For every other anchor, an implementation MUST attempt the rungs in order and MUST stop at the first rung yielding exactly one candidate.

| rung | criterion | confidence |
| --- | --- | --- |
| 0 | the anchor's subject is the file, and the file exists | exact |
| 1 | content fingerprint and node kind match | exact |
| 2 | structural fingerprint, node kind and symbol path match | exact |
| 3 | the symbol path identifies the same number of declarations as when the anchor was captured, the recorded ordinal selects one, and the node path descends to a node of the recorded kind | high |
| 4 | symbol path, node kind and both context fingerprints match, the neighbourhood is non-empty and the same size, and shape similarity is above the floor | medium |
| 5 | migration through recorded version-control history: the file was renamed, or the construct is found in another file changed since the record's revision | medium |
| 6 | shape similarity within the same symbol, above the floor and ahead of the runner-up by the margin, and only when symbol cardinality still matches | low |

A rung yielding more than one candidate MUST NOT select among them. It MUST continue to the next rung, because a later rung may carry information that distinguishes them. If no rung yields exactly one candidate, the anchor MUST be reported as detached, citing the earliest rung at which candidates were ambiguous.

**Rung 4 MUST require the symbol path to match.** Context establishes position, not identity, and position alone cannot distinguish a construct renamed in place from one deleted and replaced by a similar construct in the same slot. Both present as the same kind of node, with the same neighbours, of a similar shape. Requiring the symbol narrows rung 4 to what it can actually evidence — the same named construct whose contents moved — and sends a genuine rename to rung 6, where it is adjudicated rather than accepted.

This was observed in generated testing: a function was deleted, a structurally identical one with a different name took its position, and the claim attached to the replacement at medium confidence.

Rung 4 MUST NOT fire for an anchor whose neighbourhood was empty when it was captured. A construct that is the only one of its kind in a file has, as context, nothing but its parent — and every other lone construct in every other file shares that. Treating it as evidence attaches a claim about a deleted function to whatever single function replaced it, which was observed in exactly that form: a function moved to another module, an unrelated one took its place, and the two matched because each was the only function in its file.

Rung 5 MUST bound its search to files changed since the revision recorded on the record. A repository-wide search would turn every detachment into a scan, and a construct that moved without appearing in the same range of history is not evidenced as the same construct. Where several files yield a candidate, the anchor MUST detach as ambiguous rather than choosing.

Rung 6 MUST NOT be accepted automatically by any consumer. The floor is 0.75 and the margin 0.15.

Record kinds `invariant`, `security`, `precondition` and `postcondition` require confidence `high` or better; all other kinds require `medium` or better. A resolution below the required confidence MUST be treated as detached. Consequently no record kind accepts a rung 6 result without adjudication.

**An implementation MUST NOT resolve an anchor onto a node it cannot distinguish from another candidate.** This is the central safety property of the format: documentation attached to the wrong construct is worse than documentation reported as lost.

The cardinality condition on rungs 3 and 6 exists to serve that property. A symbol shared by several declarations identifies none of them, and a count that has changed means the symbol no longer refers to what it referred to.

## 5. Staleness

Resolution answers where a claim's code went. It does not answer whether the claim is still true, and an implementation MUST NOT present the two as the same thing.

Conforming implementations SHOULD compute **drift**: the distance between the shape histogram recorded with the anchor and the shape of the node it resolved to, as a percentage. A construct that resolved perfectly but whose drift exceeds an implementation-defined threshold SHOULD be reported as stale rather than current, because a claim about a body that has since been rewritten may simply be false.

Because drift compares shape, it is blind to an edit that changes only identifiers or literal values. That is the edit a claim quoting a value is most likely to be falsified by. Conforming implementations SHOULD therefore also report, separately from drift, whether the content fingerprint recorded with an anchor is still present anywhere in the file it resolved in: when it is not, the construct beneath the claim was edited rather than merely matched by a weaker rung. This costs one lookup against the index a resolver already builds for rung 1. It MUST NOT be reported for an anchor whose subject is the file, which has no construct to compare, and it MUST NOT be reported for reformatting, which the content fingerprint excludes by construction.

Drift MUST be insensitive to identifier and literal text, so that renaming does not register as change. `conformance/resolver/` states an expected drift for every vector that resolves: reformatting and renaming measure zero in all seven covered languages, while a rewritten control flow measures well past any sensible threshold. An implementation whose drift is non-zero for a rename is reporting noise, and noise in a staleness report is worse than no report, because people learn to ignore it.

## 6. Records

A record is an immutable, content-addressed assertion. Its identifier is the record digest of its canonical bytes, computed over every member except the identifier itself.

Required members: `schema`, `kind`, `anchors`, `body`, `assurance`, `author`, `created`, `lifecycle`. Optional: `evidence`, `code_revision`, `parent`, `chain`, and any unrecognised members, which MUST be preserved.

`created` is an integer count of seconds since the Unix epoch. Sub-second precision is deliberately excluded to keep the encoding integral.

`kind` is drawn from a closed vocabulary: `explanation`, `rationale`, `invariant`, `precondition`, `postcondition`, `security`, `performance`, `assumption`, `workaround`, `specification`, `known_failure_mode`, `ownership`, `decision`, `warning`, `tombstone`, and `relation.<verb>` where verb is one of `must_execute_after`, `guarded_by`, `constrained_by`, `invalidates`, `tested_by`, `derived_from`, `contradicts`, `supersedes`, `owns`.

Extending the vocabulary is a schema change. Discriminants MUST NOT be renumbered or reused. `conformance/vocabulary/wire.json` freezes the wire string of every closed vocabulary in this format — record kinds, relation verbs, assurance, lifecycle, anchor roles, author kinds, evidence forms, anchor subjects, resolver rungs and confidence levels. An entry may be added to that file; an entry already in it MUST continue to encode and decode as written, because a ledger cannot be regenerated from the code and a renamed discriminant strands every record that used it.

`assurance` is `asserted`, `inferred` or `speculative`, and is orthogonal to `author`, which is `human`, `agent`, `analyzer` or `runtime`. Implementations MUST keep them distinguishable at query time.

Records are never modified. A revised belief is a new record whose `parent` names the record it supersedes. A retraction is a record of kind `tombstone` whose `parent` names its target. A record named as the parent of a non-tombstone record is superseded; a record named as the parent of a tombstone is retracted. Neither appears in the active set.

## 7. Ledger

Records are stored as canonical JSON, one per line, terminated by `0x0A`, in files under `.codedoc/ledger/` named by the first two hexadecimal characters of the record identifier with a `.jsonl` extension.

Sharding means file order carries no meaning. Order is expressed by the `chain` member, which names the ledger head observed when the record was appended. Implementations reconstruct sequence by following those references, not by reading files in order. This makes concurrent appends on divergent branches a union operation, and a merge of two ledgers is the union of their lines.

A record whose `chain` names an identifier not present in the ledger is **orphaned**, and the ledger is not intact. Because identifiers are computed from content, editing any record in place changes its identifier and orphans everything chained to it. This is the tamper-evidence mechanism; there is no separate signature.

A record referenced by no other record's `chain` is a **tip**. Multiple tips are permitted and indicate a merge.

Derived state — indexes, caches, projections — MUST be reconstructible from the ledger alone and MUST NOT be committed. An implementation that versions its derived state MUST rebuild rather than fail when that version does not match.

## 8. Scopes

A repository MAY carry more than one ledger, and an implementation SHOULD support at least:

| scope | location | visibility |
| --- | --- | --- |
| shared | `.codedoc/` in the working tree | committed, shared with everyone who clones |
| local | inside the git directory | present only in this clone, untracked by construction |
| global | outside the repository, keyed by its canonical path | present only on this machine |

The local and global scopes exist so that codedoc can be used on a repository the user does not own or does not wish to modify. An implementation offering them **MUST NOT write anything to the working tree for those scopes**, including ignore files: a scope that announces itself in `git status` has failed at its only distinguishing purpose.

Reads MUST merge every scope present, deduplicating by record identifier. An interface MAY offer to narrow a listing to one scope — asking what is recorded in the untracked ledger is the question that ledger exists to make askable — but anything deciding whether recorded knowledge still holds MUST read them all, because a claim recorded in one scope is no less true while another is being checked.

Writes MUST target exactly one scope. A record derived from an existing one — a revision, an affirmation, a tombstone, a relocation — MUST default to the scope holding the record it derives from, rather than to whichever scope is the default for new records. Writing it elsewhere would copy into a shared ledger the content of a record deliberately kept out of one, and a tombstone quotes the claim it retires, so retraction would publish exactly the text the local scope was keeping private.

This clause is filesystem behaviour rather than a property of any byte sequence, so it cannot be expressed as a vector in `conformance/`. The reference implementation covers it with integration tests against a real repository, including that `git add -A` followed by a commit cannot capture a local ledger. An implementation claiming scope support should test the same property the same way; a vector set that passes says nothing about it.

## 9. Threat model

A cloned repository's ledger is untrusted input, as is every source file presented to a parser. Conforming implementations MUST NOT panic or abort on malformed input, MUST NOT size an allocation from an untrusted length, MUST confine path-valued members to the repository root, and MUST NOT allow record content to reach an executed context.

Record content is attacker-controlled text that will frequently be placed in front of a language model. Implementations that assemble context for an agent MUST treat records as data and MUST NOT allow a record to be interpreted as instructions to that agent.

## 10. Versioning and compatibility

This specification carries a major and a minor version. The major version is the compatibility boundary; the minor version admits only additions that every earlier implementation of the same major version can already read.

**The `schema` member.** Every record carries `schema`, an integer naming the major version of this specification that the record was written against. Records described here carry `1`.

An implementation MUST read a record whose `schema` equals a major version it implements. It MUST NOT reject a record whose `schema` is greater: the members it does not recognise are preserved unaltered by section 1, so such a record round-trips and keeps its identifier even where its meaning is not understood. An implementation SHOULD report that it is reading records written against a later version rather than presenting a partial understanding as a complete one.

An implementation MUST NOT write a record whose `schema` names a version it does not implement.

**What a minor version may add.** An optional member, omitted at its default, as section 1 requires. An entry in a closed vocabulary. A resolver rung above those defined, provided it is attempted after them. A new digest domain.

**What a minor version MUST NOT change.** Any rule in section 1, because every identifier in every ledger depends on all of them. Any domain string in section 2. The pre-image of any fingerprint in section 3. The wire string of any vocabulary entry already frozen in `conformance/vocabulary/wire.json`. The meaning of a rung already defined, or its position in the ladder. Whether a member is required.

**Domain strings do not track this version.** The domains in section 2 end in `v1` because that is their own revision, not the specification's. A domain string is frozen once defined; changing a digest's pre-image requires a new domain, which is an addition rather than a change, and leaves digests taken under the old one valid.

**A major version is a new format.** It does not amend this one. An implementation supporting both reads each according to its own rules, and a mechanical migration path MUST exist before any record can be rewritten from one to the other. Rewriting is the only way a record's identifier may change, because a record is its bytes.

**Conformance.** An implementation claims a version by reproducing the vectors in `conformance/` for it exactly. The vectors are part of the specification rather than a test suite belonging to any implementation: where a vector and this prose disagree, the prose governs and the vector has a bug.
