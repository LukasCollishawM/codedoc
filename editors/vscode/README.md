# codedoc for VS Code

A thin client over `codedoc-lsp`. It contains no anchoring or ledger logic; every feature is implemented in the server, so the same behaviour is available to any LSP-capable editor.

## Requirements

`codedoc-lsp` on your `PATH`:

```bash
cargo install --path crates/codedoc-lsp
```

That puts `codedoc-lsp` on your `PATH`. Alternatively run `cargo build --release` and set `codedoc.serverPath` to the absolute path of `target/release/codedoc-lsp`.

The workspace must have a ledger (`codedoc init`), otherwise there is nothing to show.

## What it does

- **Hovers** render the records anchored to the construct under the cursor, grouped by kind.
- **Code lenses** mark lines carrying records.
- **Diagnostics** warn where a claim may no longer describe the code beneath it, carrying the drift percentage, and note where an anchor detached and is waiting for a decision.

Both are resolved live against the current buffer rather than read from the cached ranges in the ledger, so a record follows the code as you edit rather than pointing at where it used to be.

## Settings

| setting | default | meaning |
| --- | --- | --- |
| `codedoc.serverPath` | `codedoc-lsp` | path to the server executable |
| `codedoc.trace.server` | `off` | LSP message tracing |

## Building and installing

```bash
cd editors/vscode
npm install
npx @vscode/vsce package --allow-missing-repository --skip-license
code --install-extension codedoc-1.0.0.vsix
```

Or run it from source without packaging:

```bash
code --extensionDevelopmentPath=editors/vscode
```

## Status

Not on the marketplace. The `.vsix` above installs locally and is what you want
for trying it; publishing needs a publisher account and a decision about who owns
it, which is not one to make quietly.
