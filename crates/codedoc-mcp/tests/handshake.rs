use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, Command, Stdio};

use codedoc_ledger::Ledger;
use serde_json::{Value, json};

struct Server {
    process: Child,
    input: ChildStdin,
    output: BufReader<std::process::ChildStdout>,
}

impl Server {
    fn start(root: &std::path::Path) -> Self {
        let mut process = Command::new(env!("CARGO_BIN_EXE_codedoc-mcp"))
            .arg(root)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("the server starts");
        let input = process.stdin.take().expect("stdin");
        let output = BufReader::new(process.stdout.take().expect("stdout"));
        Server { process, input, output }
    }

    fn send(&mut self, message: Value) {
        writeln!(self.input, "{message}").expect("the server accepts a message");
        self.input.flush().expect("flush");
    }

    fn receive(&mut self) -> Value {
        let mut line = String::new();
        self.output.read_line(&mut line).expect("the server answers");
        serde_json::from_str(&line).unwrap_or_else(|_| panic!("not JSON-RPC: {line}"))
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.process.kill();
    }
}

fn workspace() -> tempfile::TempDir {
    let root = tempfile::tempdir().expect("a temporary directory");
    std::fs::create_dir_all(root.path().join("src")).unwrap();
    std::fs::write(
        root.path().join("src/auth.rs"),
        "pub fn validate(token: &str) -> bool {\n    !token.is_empty()\n}\n",
    )
    .unwrap();
    Ledger::initialise(root.path()).unwrap();
    root
}

#[test]
fn an_agent_can_connect_list_the_tools_and_record_something() {
    let root = workspace();
    let mut server = Server::start(root.path());

    server.send(json!({
        "jsonrpc": "2.0", "id": 1, "method": "initialize",
        "params": {
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": {"name": "handshake-test", "version": "0"}
        }
    }));
    let opened = server.receive();
    let instructions = opened["result"]["instructions"].as_str().unwrap_or_default();
    assert!(
        instructions.contains("codedoc_context") && instructions.contains("DATA"),
        "the instructions are the only thing most agents will ever read about this \
         server, and they have to arrive at initialize: {instructions}"
    );

    server.send(json!({"jsonrpc": "2.0", "method": "notifications/initialized"}));
    server.send(json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list", "params": {}}));
    let listed = server.receive();
    let tools = listed["result"]["tools"].as_array().expect("a tool list");
    assert!(tools.len() >= 20, "expected the full surface, got {}", tools.len());
    let names: Vec<&str> = tools.iter().filter_map(|tool| tool["name"].as_str()).collect();
    for required in ["codedoc_search", "codedoc_attach", "codedoc_verify", "codedoc_init"] {
        assert!(names.contains(&required), "{required} is not advertised: {names:?}");
    }

    server.send(json!({
        "jsonrpc": "2.0", "id": 3, "method": "tools/call",
        "params": {
            "name": "codedoc_attach",
            "arguments": {
                "file": "src/auth.rs",
                "symbol": "rust://validate",
                "kind": "invariant",
                "claim": "Validation must precede tenant resolution."
            }
        }
    }));
    let attached = server.receive();
    let rendered = attached["result"]["content"][0]["text"].as_str().expect("a text payload");
    let payload: Value = serde_json::from_str(rendered).expect("the payload is JSON");
    assert_eq!(payload["command"], "attach", "{rendered}");

    server.send(json!({
        "jsonrpc": "2.0", "id": 4, "method": "tools/call",
        "params": {"name": "codedoc_search", "arguments": {"query": "tenant resolution"}}
    }));
    let found = server.receive();
    let rendered = found["result"]["content"][0]["text"].as_str().expect("a text payload");
    let payload: Value = serde_json::from_str(rendered).expect("the payload is JSON");
    assert_eq!(
        payload["count"], 1,
        "a record written through the protocol has to be findable through it: {rendered}"
    );
}
