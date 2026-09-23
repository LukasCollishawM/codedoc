#![forbid(unsafe_code)]

mod operations;

use std::path::PathBuf;

use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{
    CallToolResult, ContentBlock, Implementation, ProtocolVersion, ServerCapabilities, ServerConfig,
};
use rmcp::transport::stdio;
use rmcp::{ErrorData as McpError, ServerHandler, ServiceExt, tool, tool_handler, tool_router};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ContextArgs {
    pub file: String,
    pub line: Option<u32>,
    pub symbol: Option<String>,
    pub depth: Option<u8>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct AttachArgs {
    pub file: String,
    pub symbol: Option<String>,
    pub line: Option<u32>,
    pub kind: String,
    pub claim: String,
    pub detail: Option<String>,
    pub assurance: Option<String>,
    pub model: Option<String>,
    pub session: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListArgs {
    pub file: Option<String>,
    pub symbol: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct NoArgs {}

#[derive(Clone)]
pub struct Codedoc {
    root: PathBuf,
    tool_router: ToolRouter<Codedoc>,
}

impl Codedoc {
    pub fn new(root: PathBuf) -> Self {
        Codedoc { root, tool_router: Self::tool_router() }
    }

    pub fn router(&self) -> &ToolRouter<Codedoc> {
        &self.tool_router
    }
}

fn respond(outcome: Result<Value, String>) -> Result<CallToolResult, McpError> {
    match outcome {
        Ok(payload) => {
            let rendered = serde_json::to_string_pretty(&payload).unwrap_or_default();
            Ok(CallToolResult::success(vec![ContentBlock::text(rendered)]))
        }
        Err(message) => Err(McpError::invalid_params(message, None)),
    }
}

#[tool_router]
impl Codedoc {
    #[tool(
        description = "Retrieve accumulated knowledge anchored to a location in the source: invariants, rationale, security properties, known failure modes and relations. Call this before changing unfamiliar code."
    )]
    async fn codedoc_context(
        &self,
        Parameters(args): Parameters<ContextArgs>,
    ) -> Result<CallToolResult, McpError> {
        let arguments = json!({
            "file": args.file,
            "line": args.line,
            "symbol": args.symbol,
            "depth": args.depth,
        });
        respond(operations::context(&self.root, &arguments))
    }

    #[tool(
        description = "Record a durable claim about a piece of code so that it survives refactoring and remains available to future agents. Use this instead of writing a comment."
    )]
    async fn codedoc_attach(
        &self,
        Parameters(args): Parameters<AttachArgs>,
    ) -> Result<CallToolResult, McpError> {
        let arguments = json!({
            "file": args.file,
            "symbol": args.symbol,
            "line": args.line,
            "kind": args.kind,
            "claim": args.claim,
            "detail": args.detail,
            "assurance": args.assurance,
            "model": args.model,
            "session": args.session,
        });
        respond(operations::attach(&self.root, &arguments))
    }

    #[tool(
        description = "Resolve every recorded anchor against the current working tree and report which claims are fresh, migrated, stale or detached."
    )]
    async fn codedoc_verify(
        &self,
        Parameters(_args): Parameters<NoArgs>,
    ) -> Result<CallToolResult, McpError> {
        respond(operations::verify(&self.root))
    }

    #[tool(description = "List active records, optionally filtered by file or symbol.")]
    async fn codedoc_list(
        &self,
        Parameters(args): Parameters<ListArgs>,
    ) -> Result<CallToolResult, McpError> {
        let arguments = json!({"file": args.file, "symbol": args.symbol});
        respond(operations::list(&self.root, &arguments))
    }
}

#[tool_handler]
impl ServerHandler for Codedoc {
    fn get_info(&self) -> ServerConfig {
        let mut identity = Implementation::from_build_env();
        identity.name = "codedoc".to_owned();
        identity.version = env!("CARGO_PKG_VERSION").to_owned();
        identity.title = Some("codedoc semantic memory".to_owned());
        identity.description =
            Some("Durable, AST anchored knowledge about this repository.".to_owned());

        let mut config = ServerConfig::new(ServerCapabilities::builder().enable_tools().build());
        config.protocol_version = ProtocolVersion::default();
        config.server_info = identity;
        config.instructions = Some(
            "Call codedoc_context before modifying unfamiliar code, and codedoc_attach to record              anything you learned that the source does not already state. Records survive              refactoring; comments do not."
                .to_owned(),
        );
        config
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = std::env::args().nth(1).map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."));
    let service = Codedoc::new(root).serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}
