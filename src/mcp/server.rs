use std::sync::Arc;

use serde_json::json;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

use crate::mail::PanoramaMail;

use super::{
    protocol::{JsonRpcRequest, JsonRpcResponse},
    tools::{build_tool_definitions, execute_tool},
};

pub struct McpServer {
    mail: Arc<PanoramaMail>,
}

impl McpServer {
    pub fn new(mail: Arc<PanoramaMail>) -> Self {
        Self { mail }
    }

    pub async fn run(&mut self) {
        let mut reader = BufReader::new(tokio::io::stdin());
        let mut stdout = tokio::io::stdout();
        let mut line = String::new();

        loop {
            line.clear();
            match reader.read_line(&mut line).await {
                Ok(0) => break,
                Ok(_) => {}
                Err(e) => {
                    eprintln!("stdin error: {e}");
                    break;
                }
            }

            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            let response = match serde_json::from_str::<JsonRpcRequest>(trimmed) {
                Ok(req) => self.handle(req).await,
                Err(e) => JsonRpcResponse::err(None, -32700, format!("Parse error: {e}")),
            };

            let mut out = serde_json::to_string(&response).unwrap_or_default();
            out.push('\n');
            let _ = stdout.write_all(out.as_bytes()).await;
            let _ = stdout.flush().await;
        }
    }

    async fn handle(&self, req: JsonRpcRequest) -> JsonRpcResponse {
        match req.method.as_str() {
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
            "notifications/initialized" | "ping" => JsonRpcResponse::ok(req.id, json!({})),
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
        }
    }
}
