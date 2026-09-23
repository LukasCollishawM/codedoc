#![forbid(unsafe_code)]

mod locate;

use std::error::Error;
use std::path::{Path, PathBuf};

use lsp_server::{Connection, ExtractError, Message, Request, RequestId, Response};
use lsp_types::request::{CodeLensRequest, HoverRequest, Request as RequestTrait};
use lsp_types::{
    CodeLens, CodeLensOptions, CodeLensParams, Hover, HoverContents, HoverParams,
    HoverProviderCapability, InitializeParams, MarkupContent, MarkupKind, Position, Range,
    ServerCapabilities,
};
use serde_json::Value;

fn main() -> Result<(), Box<dyn Error + Sync + Send>> {
    let (connection, io_threads) = Connection::stdio();

    let capabilities = serde_json::to_value(ServerCapabilities {
        hover_provider: Some(HoverProviderCapability::Simple(true)),
        code_lens_provider: Some(CodeLensOptions { resolve_provider: Some(false) }),
        ..ServerCapabilities::default()
    })?;

    let initialization = connection.initialize(capabilities)?;
    let root = workspace_root(&initialization);
    serve(&connection, root)?;
    io_threads.join()?;
    Ok(())
}

fn uri_to_path(uri: &lsp_types::Uri) -> Option<PathBuf> {
    let text = uri.as_str();
    let remainder = text.strip_prefix("file://")?;
    let trimmed = remainder.strip_prefix('/').unwrap_or(remainder);
    let decoded = percent_decode(trimmed);
    let windows_drive = decoded.len() >= 2
        && decoded.as_bytes()[1] == b':'
        && decoded.as_bytes()[0].is_ascii_alphabetic();
    if windows_drive {
        return Some(PathBuf::from(decoded));
    }
    Some(PathBuf::from(format!("/{decoded}")))
}

fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' && index + 2 < bytes.len() {
            let pair = std::str::from_utf8(&bytes[index + 1..index + 3]).unwrap_or("");
            if let Ok(value) = u8::from_str_radix(pair, 16) {
                out.push(value);
                index += 3;
                continue;
            }
        }
        out.push(bytes[index]);
        index += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn workspace_root(initialization: &Value) -> PathBuf {
    let parsed: Option<InitializeParams> = serde_json::from_value(initialization.clone()).ok();
    parsed
        .and_then(|params| {
            #[allow(deprecated)]
            params.root_uri.as_ref().and_then(uri_to_path)
        })
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
}

fn serve(connection: &Connection, root: PathBuf) -> Result<(), Box<dyn Error + Sync + Send>> {
    for message in &connection.receiver {
        match message {
            Message::Request(request) => {
                if connection.handle_shutdown(&request)? {
                    return Ok(());
                }
                let response = dispatch(root.as_path(), request);
                connection.sender.send(Message::Response(response))?;
            }
            Message::Response(_) | Message::Notification(_) => continue,
        }
    }
    Ok(())
}

fn dispatch(root: &Path, request: Request) -> Response {
    let id = request.id.clone();
    match request.method.as_str() {
        HoverRequest::METHOD => match cast::<HoverRequest>(request) {
            Ok((id, params)) => reply(id, hover(root, &params)),
            Err(failure) => failed(id, failure),
        },
        CodeLensRequest::METHOD => match cast::<CodeLensRequest>(request) {
            Ok((id, params)) => reply(id, code_lenses(root, &params)),
            Err(failure) => failed(id, failure),
        },
        _ => Response::new_ok(id, Value::Null),
    }
}

fn cast<R>(request: Request) -> Result<(RequestId, R::Params), ExtractError<Request>>
where
    R: RequestTrait,
    R::Params: serde::de::DeserializeOwned,
{
    request.extract(R::METHOD)
}

fn reply<T: serde::Serialize>(id: RequestId, payload: T) -> Response {
    match serde_json::to_value(payload) {
        Ok(value) => Response::new_ok(id, value),
        Err(failure) => Response::new_err(id, -32603, failure.to_string()),
    }
}

fn failed(id: RequestId, failure: ExtractError<Request>) -> Response {
    Response::new_err(id, -32602, format!("{failure:?}"))
}

fn hover(root: &Path, params: &HoverParams) -> Option<Hover> {
    let position = params.text_document_position_params.position;
    let uri = &params.text_document_position_params.text_document.uri;
    let path = uri_to_path(uri)?;

    let located = locate::records_at(root, &path, position.line + 1)?;
    if located.markdown.is_empty() {
        return None;
    }

    Some(Hover {
        contents: HoverContents::Markup(MarkupContent {
            kind: MarkupKind::Markdown,
            value: located.markdown,
        }),
        range: Some(Range {
            start: Position { line: located.start_line.saturating_sub(1), character: 0 },
            end: Position { line: located.end_line.saturating_sub(1), character: 0 },
        }),
    })
}

fn code_lenses(root: &Path, params: &CodeLensParams) -> Vec<CodeLens> {
    let Some(path) = uri_to_path(&params.text_document.uri) else {
        return Vec::new();
    };
    locate::lenses_for(root, &path)
        .into_iter()
        .map(|entry| CodeLens {
            range: Range {
                start: Position { line: entry.line.saturating_sub(1), character: 0 },
                end: Position { line: entry.line.saturating_sub(1), character: 0 },
            },
            command: Some(lsp_types::Command {
                title: entry.title,
                command: "codedoc.showRecord".to_owned(),
                arguments: Some(vec![Value::String(entry.record)]),
            }),
            data: None,
        })
        .collect()
}
