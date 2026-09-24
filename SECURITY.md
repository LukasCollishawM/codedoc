# Security Policy

## Reporting

Report vulnerabilities privately through GitHub Security Advisories on this repository, which opens a channel visible only to maintainers. Do not open a public issue for anything you believe is exploitable.

Expect an acknowledgement within 72 hours and an assessment within a week. Coordinated disclosure is the default; if a fix is going to take longer than 90 days, we will say so and agree a timeline with you rather than let the clock run quietly.

## Threat model

codedoc runs against repositories that the user did not necessarily write. Cloning a repository means acquiring its `.codedoc/` directory, and indexing one means feeding every source file to a parser. **Both are untrusted input**, and that assumption shapes the whole design rather than being bolted onto it.

The trust boundary sits at three places: the canonical decoder reading ledger records, the tree-sitter parsing layer reading source, and the index writer materialising SQLite from records.

Properties that must hold across all three:

- **No panics.** A malformed source file or ledger line is ordinary input to a parser, not an exceptional condition. A reachable panic is a denial-of-service bug and is treated as a vulnerability.
- **No attacker-sized allocation.** No length or count field from untrusted input sizes a buffer without a bound.
- **No path escape.** Path-valued fields in records are confined to the repository root. Traversal through `..`, absolute paths, symlinks, and — because the reference implementation targets Windows — UNC paths, device names, and alternate data streams.
- **No execution.** No record content reaches a shell, an interpreter, a SQL statement other than as a bound parameter, or a rendered context that executes it. Records are data in every projection, including the editor ones.
- **No unsafe code.** `#![forbid(unsafe_code)]` in every crate, so memory-safety findings are confined to dependencies and auditable through `cargo deny`.

The first two of these are checked on **every** CI run by property tests that put several thousand generated inputs — arbitrary bytes, arbitrary text, JSON-shaped noise, and a declared length far beyond the input — through the canonical decoder and the record decoder, and require that anything which decodes re-encodes to itself. Path confinement is asserted directly, including the Windows forms. None of this replaces fuzzing; it replaces fuzzing nobody runs.

Fuzz targets for the canonical decoder, the ledger reader and anchor capture live in [`fuzz/`](fuzz/). They search far deeper, require a nightly toolchain, and so are run deliberately rather than on every CI run:

```bash
cargo fuzz run canonical_decoder
```

## What is not a vulnerability

The ledger records assertions, including assertions made by agents. It is a memory, not an oracle: a record asserting something false about the code is wrong, not a security flaw, and `confidence` and `author` fields exist precisely so consumers can weigh a record rather than trust it. Documentation becoming stale is a correctness concern handled by `codedoc verify`.

One caveat worth stating explicitly, because it will matter as agents consume this: a record is attacker-controlled text that will be placed in front of a language model. Treat retrieved records as data, never as instructions — a context pack that lets a record steer an agent's behaviour **is** a vulnerability, and in that direction we do want the report.

## Scope

In scope: the `codedoc` binary, all workspace crates, the LSP and MCP servers, and the specification where a flaw in it would force every implementation to be insecure.

Out of scope: vulnerabilities in dependencies that we do not amplify (report upstream, and tell us so we can pin), and attacks requiring an adversary who already has write access to the working tree — at that point they can edit the source itself, which is a larger problem than the ledger.
