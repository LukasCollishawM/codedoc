use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Child, ChildStdin, Command, Stdio};

use codedoc_ledger::Ledger;
use serde_json::{Value, json};

struct Editor {
    process: Child,
    input: ChildStdin,
    output: BufReader<std::process::ChildStdout>,
}

impl Editor {
    fn start(root: &std::path::Path) -> Self {
        let mut process = Command::new(env!("CARGO_BIN_EXE_codedoc-lsp"))
            .current_dir(root)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("the language server starts");
        let input = process.stdin.take().expect("stdin");
        let output = BufReader::new(process.stdout.take().expect("stdout"));
        Editor { process, input, output }
    }

    fn send(&mut self, message: &Value) {
        let body = message.to_string();
        write!(self.input, "Content-Length: {}\r\n\r\n{body}", body.len()).expect("write");
        self.input.flush().expect("flush");
    }

    fn receive(&mut self) -> Value {
        let mut length = 0usize;
        loop {
            let mut header = String::new();
            self.output.read_line(&mut header).expect("a header");
            let trimmed = header.trim();
            if trimmed.is_empty() {
                break;
            }
            if let Some(value) = trimmed.strip_prefix("Content-Length: ") {
                length = value.parse().expect("a length");
            }
        }
        let mut body = vec![0u8; length];
        self.output.read_exact(&mut body).expect("a body");
        serde_json::from_slice(&body).expect("JSON-RPC")
    }
}

impl Drop for Editor {
    fn drop(&mut self) {
        let _ = self.process.kill();
    }
}

#[test]
fn an_editor_can_connect_and_be_told_what_this_server_offers() {
    let root = tempfile::tempdir().expect("a temporary directory");
    std::fs::create_dir_all(root.path().join("src")).unwrap();
    std::fs::write(
        root.path().join("src/auth.rs"),
        "pub fn validate(token: &str) -> bool {\n    !token.is_empty()\n}\n",
    )
    .unwrap();
    Ledger::initialise(root.path()).unwrap();

    let mut editor = Editor::start(root.path());
    editor.send(&json!({
        "jsonrpc": "2.0", "id": 1, "method": "initialize",
        "params": {
            "processId": null,
            "rootUri": null,
            "capabilities": {}
        }
    }));
    let opened = editor.receive();
    let capabilities = &opened["result"]["capabilities"];

    assert_eq!(
        capabilities["hoverProvider"], true,
        "an editor decides what to ask for from this answer, so a capability that \
         is implemented and not advertised is one nobody will ever call: {capabilities}"
    );
    assert!(capabilities["codeLensProvider"].is_object(), "{capabilities}");
    assert!(capabilities["diagnosticProvider"].is_object(), "{capabilities}");

    editor.send(&json!({"jsonrpc": "2.0", "method": "initialized", "params": {}}));
    editor.send(&json!({"jsonrpc": "2.0", "id": 2, "method": "shutdown"}));
    let closed = editor.receive();
    assert!(closed["error"].is_null(), "shutdown should be clean: {closed}");
}

#[test]
fn the_server_exits_when_the_editor_closes_the_connection() {
    let root = tempfile::tempdir().expect("a temporary directory");
    std::fs::write(root.path().join("lib.rs"), "pub fn compute() -> u32 {\n    0\n}\n").unwrap();

    let mut process = Command::new(env!("CARGO_BIN_EXE_codedoc-lsp"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("the server starts");

    {
        let mut input = process.stdin.take().expect("stdin");
        let opening = json!({"jsonrpc": "2.0", "method": "initialized", "params": {}}).to_string();
        let body = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {"processId": null, "capabilities": {}},
        })
        .to_string();
        write!(input, "Content-Length: {}\r\n\r\n{body}", body.len()).expect("the handshake");
        write!(
            input,
            "Content-Length: {}

{opening}",
            opening.len()
        )
        .expect("the opening notification");
        input.flush().expect("flushed");
    }

    let started = std::time::Instant::now();
    loop {
        match process.try_wait().expect("the process can be polled") {
            Some(_) => return,
            None if started.elapsed() > std::time::Duration::from_secs(20) => {
                let _ = process.kill();
                panic!(
                    "the server outlived its editor. An editor restarts its server on a \
                     configuration change or a crash, and every restart would leave one \
                     of these behind"
                );
            }
            None => std::thread::sleep(std::time::Duration::from_millis(50)),
        }
    }
}
