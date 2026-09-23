# ADR-0010: No verification cache

Status: Rejected

## Context

After parallelising verification, a full run over 1.09M lines and 27,420 anchors took
7.7 seconds. Most of that is parsing 1,910 files and computing their fingerprints,
work that is identical between runs when the files have not changed.

The obvious optimisation is to cache each file's findings against a key derived from
its content and the set of records anchored in it, and skip the work when the key
matches.

## Decision

Rejected. It was built, measured, and removed.

## Measurements

| | cold | warm |
| --- | --- | --- |
| parallel, no cache | 7.7s | 7.7s |
| parallel, with cache | 8.0s | 5.8s |
| scoped to one file | 0.067s | 0.067s |

The first attempt was worse than useless: 18.2s cold, because the pre-pass read and
hashed every file serially and the cache wrote 1,910 individual SQLite inserts.
Parallelising the key computation and batching the writes into one transaction
produced the figures above.

## Why it was rejected

**The win is 25%, and it applies to a case that barely happens.** CI verifies from a
fresh checkout, so its cache is always cold and it pays the 0.3s penalty. Interactive
and agent use goes through scoped verification, which answers in 67 milliseconds
because it queries the index instead of walking the repository. The warm full-run sits
between two cases that are each better served by something else.

**The floor is not much lower anyway.** Even with every parse cached, a warm run still
reads and hashes every file to compute the keys, and still reads the whole ledger to
build the graph. That is roughly five seconds of the seven, so the cache was reaching
for about two.

**The risk is asymmetric.** A cache on the operation that decides whether documentation
still matches the code fails in the direction of reporting a stale claim as verified.
Everything else in this project is arranged so that uncertainty degrades into
adjudication; a wrong cache key degrades into false confidence instead. Two seconds is
not worth buying that.

## Consequences

Full verification stays parallel and uncached. The performance answer for repeated
checking is `codedoc verify <files>` or `--since <rev>`, which is faster than any cache
could be because it never touches the files it was not asked about.

If this is revisited, the thing to attack is the five seconds of file reading and
ledger parsing, not the two seconds of resolution.
