use std::sync::Arc;

use serde_json::json;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

use crate::{logger::SharedLogger, mail::PanoramaMail};

use super::{
    protocol::{JsonRpcRequest, JsonRpcResponse},
    tools::{build_tool_definitions, execute_tool},
};

pub struct McpServer {
    mail: Arc<PanoramaMail>,
    logger: SharedLogger,
}

impl McpServer {
    pub fn new(mail: Arc<PanoramaMail>, logger: SharedLogger) -> Self {
        Self { mail, logger }
    }

    pub async fn run(&mut self) {
        self.logger
            .info("panorama-mail MCP server running (JSON-RPC on stdio)");

        let mut reader = BufReader::new(tokio::io::stdin());
        let mut stdout = tokio::io::stdout();
        let mut line = String::new();

        loop {
            line.clear();
            match reader.read_line(&mut line).await {
                Ok(0) => break,
                Ok(_) => {}
                Err(e) => {
                    self.logger.error(&format!("stdin error: {e}"));
                    break;
                }
            }

            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            let response = match serde_json::from_str::<JsonRpcRequest>(trimmed) {
                Ok(req) => self.handle(req).await,
                Err(e) => Some(JsonRpcResponse::err(None, -32700, format!("Parse error: {e}"))),
            };

            if let Some(resp) = response {
                let mut out = serde_json::to_string(&resp).unwrap_or_default();
                out.push('\n');
                let _ = stdout.write_all(out.as_bytes()).await;
                let _ = stdout.flush().await;
            }
        }
    }

    async fn handle(&self, req: JsonRpcRequest) -> Option<JsonRpcResponse> {
        // JSON-RPC 2.0: notifications (no id) must never receive a response.
        if req.id.is_none() {
            return None;
        }
        Some(match req.method.as_str() {
            "initialize" => JsonRpcResponse::ok(
                req.id,
                json!({
                    "protocolVersion": "2024-11-05",
                    "capabilities": { "tools": {} },
                    "serverInfo": {
                        "name": "panorama-mail",
                        "version": env!("CARGO_PKG_VERSION")
                    }
                }),
            ),
            "ping" => JsonRpcResponse::ok(req.id, json!({})),
            "tools/list" => JsonRpcResponse::ok(
                req.id,
                json!({ "tools": build_tool_definitions() }),
            ),
            "tools/call" => {
                let params = req.params.unwrap_or(json!({}));
                let name = params["name"].as_str().unwrap_or("").to_string();
                let args = params.get("arguments").cloned();
                let result = execute_tool(&name, args, &self.mail).await;
                JsonRpcResponse::ok(req.id, serde_json::to_value(result).unwrap_or(json!(null)))
            }
            method => JsonRpcResponse::err(
                req.id,
                -32601,
                format!("Method not found: {}", method),
            ),
        })
    }
}
