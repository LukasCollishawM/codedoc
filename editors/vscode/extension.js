const vscode = require("vscode");
const { LanguageClient, TransportKind } = require("vscode-languageclient/node");

let client;

function serverOptions() {
  const configured = vscode.workspace
    .getConfiguration("codedoc")
    .get("serverPath", "codedoc-lsp");
  const run = { command: configured, transport: TransportKind.stdio };
  return { run, debug: run };
}

function clientOptions() {
  return {
    documentSelector: [
      { scheme: "file", language: "rust" },
      { scheme: "file", language: "csharp" },
      { scheme: "file", language: "typescript" },
      { scheme: "file", language: "typescriptreact" },
      { scheme: "file", language: "python" },
      { scheme: "file", language: "go" },
      { scheme: "file", language: "java" },
    ],
    synchronize: {
      fileEvents: vscode.workspace.createFileSystemWatcher("**/.codedoc/ledger/*.jsonl"),
    },
  };
}

async function start(context) {
  client = new LanguageClient(
    "codedoc",
    "codedoc",
    serverOptions(),
    clientOptions()
  );
  try {
    await client.start();
  } catch (failure) {
    vscode.window.showErrorMessage(
      `codedoc: could not start the language server (${failure.message}). ` +
        `Set codedoc.serverPath, or install it with: cargo install --path crates/codedoc-lsp`
    );
    client = undefined;
    return;
  }
  context.subscriptions.push(client);
}

function activate(context) {
  context.subscriptions.push(
    vscode.commands.registerCommand("codedoc.showRecord", (record) => {
      vscode.window.showInformationMessage(`codedoc record ${record}`);
    })
  );

  context.subscriptions.push(
    vscode.commands.registerCommand("codedoc.restart", async () => {
      if (client) {
        await client.stop();
        client = undefined;
      }
      await start(context);
    })
  );

  return start(context);
}

async function deactivate() {
  if (client) {
    await client.stop();
    client = undefined;
  }
}

module.exports = { activate, deactivate };
