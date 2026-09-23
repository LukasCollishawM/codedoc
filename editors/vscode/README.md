# codedoc for VS Code

A thin client over `codedoc-lsp`. It contains no anchoring or ledger logic; every feature is implemented in the server, so the same behaviour is available to any LSP-capable editor.

## Requirements

`codedoc-lsp` on your `PATH`:

```bash
cargo install --path crates/codedoc-cli   # installs codedoc
cargo build --release                     # builds codedoc-lsp
```

Then either put `target/release` on your `PATH` or set `codedoc.serverPath` to the absolute path of the binary.

The workspace must have a ledger (`codedoc init`), otherwise there is nothing to show.

## What it does

- **Hovers** render the records anchored to the construct under the cursor, grouped by kind.
- **Code lenses** mark lines carrying records.

Both are resolved live against the current buffer rather than read from the cached ranges in the ledger, so a record follows the code as you edit rather than pointing at where it used to be.

## Settings

| setting | default | meaning |
| --- | --- | --- |
| `codedoc.serverPath` | `codedoc-lsp` | path to the server executable |
| `codedoc.trace.server` | `off` | LSP message tracing |

## Status

Unpackaged. There is no marketplace listing yet; run it from source with `code --extensionDevelopmentPath=editors/vscode`.
