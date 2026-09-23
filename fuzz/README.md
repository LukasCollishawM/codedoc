# Fuzz targets

A cloned repository's ledger is untrusted input, and so is every source file handed
to the parser. These targets sit on that boundary.

```bash
cargo install cargo-fuzz
cargo fuzz run canonical_decoder
cargo fuzz run ledger_reader
cargo fuzz run anchor_capture
```

Requires a nightly toolchain, which is why these are not in the default CI matrix.

| target | property |
| --- | --- |
| `canonical_decoder` | anything that decodes must re-encode to itself, and must never panic |
| `ledger_reader` | a record's identity survives a decode and encode cycle, including members this build does not recognise |
| `anchor_capture` | capturing and resolving an anchor over arbitrary source never panics |

A panic here is a denial-of-service bug, not a curiosity: `SECURITY.md` treats a
reachable panic on malformed input as a vulnerability, because a malformed source
file is an ordinary input to a parser.
