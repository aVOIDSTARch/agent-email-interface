//! End-to-end tests against live running services.
//!
//! Prerequisites:
//!   - Proton Bridge running
//!   - .env populated with valid credentials
//!   - `cargo build --bin panorama-mail` completed (MCP tests spawn the binary)
//!
//! HTTP tests connect to an already-running `panorama-mail serve` instance.
//!   Default: http://127.0.0.1:3500 — override with HTTP_BASE_URL env var.
//!   Tests skip silently if the server is not reachable.
//!
//! MCP tests spawn a fresh `panorama-mail mcp` subprocess with
//!   STORE_PATH=/tmp/panorama-e2e-test to avoid Tantivy lock conflicts.
//!
//! Run with:
//!   cargo test --test e2e_test -- --nocapture --test-threads=1

use std::time::Duration;

use reqwest::Client;
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

// ─── Helpers ──────────────────────────────────────────────────────────────────

fn load_username() -> String {
    dotenvy::dotenv().ok();
    std::env::var("PROTON_USERNAME").expect("PROTON_USERNAME not set in .env")
}

fn unique_marker(prefix: &str) -> String {
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    format!("{prefix}-{ts}")
}

// ═══════════════════════════════════════════════════════════════════════════════
// HTTP live-server tests
// ═══════════════════════════════════════════════════════════════════════════════

/// Returns the live HTTP server base URL, or None if not reachable.
async fn live_http_base() -> Option<String> {
    dotenvy::dotenv().ok();
    let base = std::env::var("HTTP_BASE_URL")
        .unwrap_or_else(|_| "http://127.0.0.1:3500".to_string());
    match Client::new()
        .get(format!("{base}/health"))
        .timeout(Duration::from_secs(3))
        .send()
        .await
    {
        Ok(_) => Some(base),
        Err(_) => {
            println!("[SKIP] HTTP server not reachable at {base} — start `panorama-mail serve` first");
            None
        }
    }
}

#[tokio::test]
async fn test_live_http_health() {
    let Some(base) = live_http_base().await else { return };
    let client = Client::new();

    let resp = client
        .get(format!("{base}/health"))
        .send()
        .await
        .expect("request failed");

    assert_eq!(resp.status(), 200);
    let body: Value = resp.json().await.expect("json parse failed");
    assert_eq!(body["status"], "ok", "unexpected body: {body}");
    println!("\nHealth: {body}");
}

#[tokio::test]
async fn test_live_http_list_mailboxes() {
    let Some(base) = live_http_base().await else { return };
    let client = Client::new();

    let resp = client
        .get(format!("{base}/mail/mailboxes"))
        .send()
        .await
        .expect("request failed");

    assert_eq!(resp.status(), 200);
    let mailboxes: Vec<String> = resp.json().await.expect("json parse failed");
    assert!(!mailboxes.is_empty(), "expected at least one mailbox");
    assert!(
        mailboxes.iter().any(|m| m.to_uppercase().contains("INBOX")),
        "expected INBOX in list, got: {mailboxes:?}"
    );
    println!("\nMailboxes ({}):", mailboxes.len());
    for mb in &mailboxes {
        println!("  {mb}");
    }
}

#[tokio::test]
async fn test_live_http_fetch_unread() {
    let Some(base) = live_http_base().await else { return };
    let client = Client::new();

    let resp = client
        .get(format!("{base}/mail/unread"))
        .send()
        .await
        .expect("request failed");

    assert_eq!(resp.status(), 200);
    let messages: Vec<Value> = resp.json().await.expect("json parse failed");
    println!("\nUnread messages: {}", messages.len());
    for m in messages.iter().take(5) {
        println!(
            "  UID={} Subject={}",
            m["uid"],
            m["subject"].as_str().unwrap_or("(none)")
        );
    }
}

#[tokio::test]
async fn test_live_http_search() {
    let Some(base) = live_http_base().await else { return };
    let username = load_username();
    let client = Client::new();

    let resp = client
        .get(format!("{base}/mail/search"))
        .query(&[("q", &username)])
        .send()
        .await
        .expect("request failed");

    assert_eq!(resp.status(), 200);
    let results: Vec<Value> = resp.json().await.expect("json parse failed");
    println!("\nSearch for '{username}': {} result(s)", results.len());
    for m in results.iter().take(5) {
        println!(
            "  UID={} Subject={}",
            m["uid"],
            m["subject"].as_str().unwrap_or("(none)")
        );
    }
}

#[tokio::test]
async fn test_live_http_send_email() {
    let Some(base) = live_http_base().await else { return };
    let to = load_username();
    let client = Client::new();

    let subject = unique_marker("pano-http-send");

    println!("\n[1] POST /mail/send to={to} subject={subject}...");
    let resp = client
        .post(format!("{base}/mail/send"))
        .json(&json!({ "to": to, "subject": subject, "body": "HTTP send e2e test. Safe to delete." }))
        .send()
        .await
        .expect("request failed");

    assert_eq!(
        resp.status(),
        200,
        "send failed: {}",
        resp.text().await.unwrap_or_default()
    );
    println!("    OK");
}

#[tokio::test]
async fn test_live_http_full_flow() {
    let Some(base) = live_http_base().await else { return };
    let to = load_username();
    let client = Client::new();

    let marker = unique_marker("pano-http-flow");

    // 1. Send to self.
    println!("\n[1] POST /mail/send ({marker})...");
    let send_resp = client
        .post(format!("{base}/mail/send"))
        .json(&json!({
            "to": &to,
            "subject": &marker,
            "body": "HTTP full-flow e2e test. Safe to delete."
        }))
        .send()
        .await
        .expect("send request failed");
    assert_eq!(
        send_resp.status(),
        200,
        "send failed: {}",
        send_resp.text().await.unwrap_or_default()
    );
    println!("    Sent OK");

    // 2. Wait for IMAP delivery.
    println!("[2] Waiting 4s for delivery...");
    tokio::time::sleep(Duration::from_secs(4)).await;

    // 3. GET /mail/unread — look for our marker.
    println!("[3] GET /mail/unread...");
    let unread: Vec<Value> = client
        .get(format!("{base}/mail/unread"))
        .send()
        .await
        .expect("unread request failed")
        .json()
        .await
        .expect("json parse failed");
    println!("    {} unread message(s)", unread.len());

    let uid_from_unread = unread.iter().find_map(|m| {
        (m["subject"].as_str() == Some(marker.as_str())).then(|| m["uid"].as_u64()).flatten()
    });

    // 4. Search as fallback.
    let uid = match uid_from_unread {
        Some(uid) => uid,
        None => {
            println!("[4] GET /mail/search?q={marker}...");
            let results: Vec<Value> = client
                .get(format!("{base}/mail/search"))
                .query(&[("q", &marker)])
                .send()
                .await
                .expect("search request failed")
                .json()
                .await
                .expect("json parse failed");
            println!("    {} result(s)", results.len());

            match results.iter().find_map(|m| {
                (m["subject"].as_str() == Some(marker.as_str())).then(|| m["uid"].as_u64()).flatten()
            }) {
                Some(uid) => uid,
                None => {
                    println!("    WARN: message not delivered yet — skipping get/mark-read");
                    return;
                }
            }
        }
    };

    println!("    Found UID={uid}");

    // 5. GET /mail/messages/:uid
    println!("[5] GET /mail/messages/{uid}...");
    let fetched: Value = client
        .get(format!("{base}/mail/messages/{uid}"))
        .send()
        .await
        .expect("get_message request failed")
        .json()
        .await
        .expect("json parse failed");
    assert_eq!(
        fetched["uid"].as_u64(),
        Some(uid),
        "uid mismatch in fetched message"
    );
    println!(
        "    Subject: {}",
        fetched["subject"].as_str().unwrap_or("(none)")
    );

    // 6. POST /mail/messages/:uid/read
    println!("[6] POST /mail/messages/{uid}/read...");
    let mark: Value = client
        .post(format!("{base}/mail/messages/{uid}/read"))
        .send()
        .await
        .expect("mark_read request failed")
        .json()
        .await
        .expect("json parse failed");
    assert_eq!(mark["ok"], true, "mark_read response: {mark}");

    println!("[✓] HTTP full flow passed");
}

// ═══════════════════════════════════════════════════════════════════════════════
// MCP subprocess tests
// ═══════════════════════════════════════════════════════════════════════════════

struct McpProcess {
    child: tokio::process::Child,
    stdin: tokio::process::ChildStdin,
    reader: BufReader<tokio::process::ChildStdout>,
    next_id: u64,
}

impl McpProcess {
    async fn spawn() -> Self {
        dotenvy::dotenv().ok();

        let bin = env!("CARGO_BIN_EXE_panorama-mail");
        let mut child = tokio::process::Command::new(bin)
            .arg("mcp")
            .env("STORE_PATH", "/tmp/panorama-e2e-test")
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::inherit())
            .spawn()
            .unwrap_or_else(|e| panic!("failed to spawn {bin}: {e}"));

        let stdin = child.stdin.take().expect("no stdin handle");
        let stdout = child.stdout.take().expect("no stdout handle");

        Self {
            child,
            stdin,
            reader: BufReader::new(stdout),
            next_id: 1,
        }
    }

    /// Send a JSON-RPC request and read one response line.
    async fn request(&mut self, method: &str, params: Value) -> Value {
        let id = self.next_id;
        self.next_id += 1;

        let msg = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        });

        let mut line = serde_json::to_string(&msg).unwrap();
        line.push('\n');
        self.stdin
            .write_all(line.as_bytes())
            .await
            .expect("stdin write failed");
        self.stdin.flush().await.expect("stdin flush failed");

        let mut response = String::new();
        tokio::time::timeout(Duration::from_secs(30), self.reader.read_line(&mut response))
            .await
            .expect("MCP response timed out after 30s")
            .expect("stdout read failed");

        serde_json::from_str(&response)
            .unwrap_or_else(|e| panic!("JSON parse failed ({e}): {response:?}"))
    }

    /// Send a JSON-RPC notification (no id, no response expected).
    async fn notify(&mut self, method: &str) {
        let msg = json!({ "jsonrpc": "2.0", "method": method });
        let mut line = serde_json::to_string(&msg).unwrap();
        line.push('\n');
        self.stdin
            .write_all(line.as_bytes())
            .await
            .expect("stdin write failed");
        self.stdin.flush().await.expect("stdin flush failed");
    }

    /// MCP handshake: initialize + notifications/initialized.
    async fn initialize(&mut self) {
        let resp = self
            .request(
                "initialize",
                json!({
                    "protocolVersion": "2024-11-05",
                    "capabilities": {},
                    "clientInfo": { "name": "e2e-test", "version": "0.1.0" }
                }),
            )
            .await;
        assert_eq!(
            resp["result"]["protocolVersion"], "2024-11-05",
            "unexpected initialize response: {resp}"
        );
        self.notify("notifications/initialized").await;
    }

    /// Call a tool and return the full JSON-RPC response.
    async fn call_tool(&mut self, name: &str, args: Value) -> Value {
        self.request("tools/call", json!({ "name": name, "arguments": args }))
            .await
    }

    /// Extract the text content from a tool result.
    fn tool_text(resp: &Value) -> &str {
        resp["result"]["content"][0]["text"]
            .as_str()
            .unwrap_or("")
    }

    async fn kill(mut self) {
        let _ = self.child.kill().await;
    }
}

// ─── MCP tests ────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_mcp_handshake() {
    let mut mcp = McpProcess::spawn().await;

    println!("\n[1] MCP initialize...");
    let resp = mcp
        .request(
            "initialize",
            json!({
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": { "name": "e2e-test", "version": "0.1.0" }
            }),
        )
        .await;

    println!("    Response: {resp}");
    assert_eq!(resp["result"]["protocolVersion"], "2024-11-05");
    assert_eq!(resp["result"]["serverInfo"]["name"], "panorama-mail");

    println!("[2] notifications/initialized...");
    mcp.notify("notifications/initialized").await;

    println!("[3] ping...");
    let pong = mcp.request("ping", json!({})).await;
    assert!(pong.get("error").is_none(), "ping returned error: {pong}");

    println!("[✓] MCP handshake passed");
    mcp.kill().await;
}

#[tokio::test]
async fn test_mcp_tools_list() {
    let mut mcp = McpProcess::spawn().await;
    mcp.initialize().await;

    println!("\n[1] tools/list...");
    let resp = mcp.request("tools/list", json!({})).await;

    let tools = resp["result"]["tools"].as_array().expect("tools is not array");
    println!("    {} tool(s) registered", tools.len());
    for t in tools {
        println!("  - {}", t["name"].as_str().unwrap_or("?"));
    }

    let names: Vec<&str> = tools
        .iter()
        .filter_map(|t| t["name"].as_str())
        .collect();

    for expected in &[
        "mail_send",
        "mail_fetch_unread",
        "mail_search",
        "mail_mark_read",
        "mail_get_by_uid",
        "mail_list_mailboxes",
    ] {
        assert!(
            names.contains(expected),
            "missing tool '{expected}', got: {names:?}"
        );
    }

    println!("[✓] tools/list passed");
    mcp.kill().await;
}

#[tokio::test]
async fn test_mcp_list_mailboxes() {
    let mut mcp = McpProcess::spawn().await;
    mcp.initialize().await;

    println!("\n[1] mail_list_mailboxes...");
    let resp = mcp.call_tool("mail_list_mailboxes", json!({})).await;
    assert!(
        resp.get("error").is_none(),
        "RPC error: {resp}"
    );
    assert!(
        !resp["result"]["isError"].as_bool().unwrap_or(false),
        "tool error: {resp}"
    );

    let text = McpProcess::tool_text(&resp);
    println!("    Result: {text}");

    let mailboxes: Vec<String> =
        serde_json::from_str(text).expect("expected JSON array of mailbox names");
    assert!(!mailboxes.is_empty(), "expected at least one mailbox");
    assert!(
        mailboxes.iter().any(|m| m.to_uppercase().contains("INBOX")),
        "expected INBOX, got: {mailboxes:?}"
    );

    println!("[✓] mail_list_mailboxes passed");
    mcp.kill().await;
}

#[tokio::test]
async fn test_mcp_fetch_unread() {
    let mut mcp = McpProcess::spawn().await;
    mcp.initialize().await;

    println!("\n[1] mail_fetch_unread...");
    let resp = mcp.call_tool("mail_fetch_unread", json!({})).await;
    assert!(
        resp.get("error").is_none(),
        "RPC error: {resp}"
    );
    assert!(
        !resp["result"]["isError"].as_bool().unwrap_or(false),
        "tool error: {resp}"
    );

    let text = McpProcess::tool_text(&resp);
    let messages: Vec<Value> = serde_json::from_str(text).expect("expected JSON array of messages");
    println!("    {} unread message(s)", messages.len());
    for m in messages.iter().take(5) {
        println!(
            "  UID={} Subject={}",
            m["uid"],
            m["subject"].as_str().unwrap_or("(none)")
        );
    }

    println!("[✓] mail_fetch_unread passed");
    mcp.kill().await;
}

#[tokio::test]
async fn test_mcp_search() {
    let mut mcp = McpProcess::spawn().await;
    mcp.initialize().await;

    let username = load_username();

    println!("\n[1] mail_search query={username}...");
    let resp = mcp
        .call_tool("mail_search", json!({ "query": username }))
        .await;
    assert!(
        resp.get("error").is_none(),
        "RPC error: {resp}"
    );
    assert!(
        !resp["result"]["isError"].as_bool().unwrap_or(false),
        "tool error: {resp}"
    );

    let text = McpProcess::tool_text(&resp);
    let results: Vec<Value> = serde_json::from_str(text).expect("expected JSON array");
    println!("    {} result(s)", results.len());

    println!("[✓] mail_search passed");
    mcp.kill().await;
}

#[tokio::test]
async fn test_mcp_full_flow() {
    let mut mcp = McpProcess::spawn().await;
    mcp.initialize().await;

    let to = load_username();
    let marker = unique_marker("pano-mcp-flow");

    // 1. mail_send
    println!("\n[1] mail_send to={to} subject={marker}...");
    let send_resp = mcp
        .call_tool(
            "mail_send",
            json!({ "to": &to, "subject": &marker, "body": "MCP full-flow e2e test. Safe to delete." }),
        )
        .await;
    assert!(
        send_resp.get("error").is_none(),
        "RPC error on send: {send_resp}"
    );
    assert!(
        !send_resp["result"]["isError"].as_bool().unwrap_or(false),
        "tool error on send: {}",
        McpProcess::tool_text(&send_resp)
    );
    println!("    OK: {}", McpProcess::tool_text(&send_resp));

    // 2. Wait for IMAP delivery.
    println!("[2] Waiting 4s for delivery...");
    tokio::time::sleep(Duration::from_secs(4)).await;

    // 3. mail_fetch_unread — look for our marker.
    println!("[3] mail_fetch_unread...");
    let unread_resp = mcp.call_tool("mail_fetch_unread", json!({})).await;
    assert!(
        !unread_resp["result"]["isError"].as_bool().unwrap_or(false),
        "fetch_unread error: {}",
        McpProcess::tool_text(&unread_resp)
    );
    let unread: Vec<Value> =
        serde_json::from_str(McpProcess::tool_text(&unread_resp)).expect("expected JSON array");
    println!("    {} unread message(s)", unread.len());

    let uid_from_unread = unread.iter().find_map(|m| {
        (m["subject"].as_str() == Some(marker.as_str())).then(|| m["uid"].as_u64()).flatten()
    });

    // 4. mail_search as fallback.
    let uid = match uid_from_unread {
        Some(uid) => uid,
        None => {
            println!("[4] mail_search query={marker}...");
            let search_resp = mcp
                .call_tool("mail_search", json!({ "query": marker }))
                .await;
            assert!(
                !search_resp["result"]["isError"].as_bool().unwrap_or(false),
                "search error: {}",
                McpProcess::tool_text(&search_resp)
            );
            let results: Vec<Value> =
                serde_json::from_str(McpProcess::tool_text(&search_resp)).expect("expected JSON array");
            println!("    {} result(s)", results.len());

            match results.iter().find_map(|m| {
                (m["subject"].as_str() == Some(marker.as_str())).then(|| m["uid"].as_u64()).flatten()
            }) {
                Some(uid) => uid,
                None => {
                    println!("    WARN: message not delivered yet — skipping get/mark-read");
                    mcp.kill().await;
                    return;
                }
            }
        }
    };

    println!("    Found UID={uid}");

    // 5. mail_get_by_uid
    println!("[5] mail_get_by_uid uid={uid}...");
    let get_resp = mcp
        .call_tool("mail_get_by_uid", json!({ "uid": uid }))
        .await;
    assert!(
        !get_resp["result"]["isError"].as_bool().unwrap_or(false),
        "get_by_uid error: {}",
        McpProcess::tool_text(&get_resp)
    );
    let fetched: Value =
        serde_json::from_str(McpProcess::tool_text(&get_resp)).expect("expected JSON message");
    assert_eq!(
        fetched["uid"].as_u64(),
        Some(uid),
        "uid mismatch in fetched message"
    );
    println!(
        "    Subject: {}",
        fetched["subject"].as_str().unwrap_or("(none)")
    );

    // 6. mail_mark_read
    println!("[6] mail_mark_read uid={uid}...");
    let mark_resp = mcp
        .call_tool("mail_mark_read", json!({ "uid": uid }))
        .await;
    assert!(
        !mark_resp["result"]["isError"].as_bool().unwrap_or(false),
        "mark_read error: {}",
        McpProcess::tool_text(&mark_resp)
    );
    println!("    OK: {}", McpProcess::tool_text(&mark_resp));

    println!("[✓] MCP full flow passed");
    mcp.kill().await;
}
