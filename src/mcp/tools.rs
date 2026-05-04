use std::sync::Arc;

use serde_json::{json, Value};

use crate::mail::{AgentMailTransport, PanoramaMail};

use super::protocol::{ContentBlock, ToolCallResult, ToolDefinition};

pub fn build_tool_definitions() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition {
            name: "mail_send".to_string(),
            description: "Send an email via Proton Bridge".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "to":      { "type": "string", "description": "Recipient email address" },
                    "subject": { "type": "string", "description": "Email subject" },
                    "body":    { "type": "string", "description": "Email body (plain text)" }
                },
                "required": ["to", "subject", "body"]
            }),
        },
        ToolDefinition {
            name: "mail_fetch_unread".to_string(),
            description: "Fetch all unread messages from the configured mailbox".to_string(),
            input_schema: json!({ "type": "object", "properties": {} }),
        },
        ToolDefinition {
            name: "mail_search".to_string(),
            description: "Search messages by subject or sender".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "query": { "type": "string", "description": "Term to match against subject or from address" }
                },
                "required": ["query"]
            }),
        },
        ToolDefinition {
            name: "mail_mark_read".to_string(),
            description: "Mark a message as read by UID".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "uid": { "type": "integer", "description": "IMAP UID of the message" }
                },
                "required": ["uid"]
            }),
        },
        ToolDefinition {
            name: "mail_get_by_uid".to_string(),
            description: "Fetch a single message by its IMAP UID".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "uid": { "type": "integer", "description": "IMAP UID of the message" }
                },
                "required": ["uid"]
            }),
        },
        ToolDefinition {
            name: "mail_list_mailboxes".to_string(),
            description: "List all available IMAP mailboxes (folders)".to_string(),
            input_schema: json!({ "type": "object", "properties": {} }),
        },
    ]
}

pub async fn execute_tool(name: &str, args: Option<Value>, mail: &Arc<PanoramaMail>) -> ToolCallResult {
    let text = match dispatch(name, args, mail).await {
        Ok(t) => return ToolCallResult {
            content: vec![ContentBlock { block_type: "text".to_string(), text: t }],
            is_error: false,
        },
        Err(e) => e,
    };
    ToolCallResult {
        content: vec![ContentBlock { block_type: "text".to_string(), text }],
        is_error: true,
    }
}

async fn dispatch(
    name: &str,
    args: Option<Value>,
    mail: &Arc<PanoramaMail>,
) -> Result<String, String> {
    let args = args.unwrap_or(json!({}));

    match name {
        "mail_send" => {
            let to = args["to"].as_str().ok_or("missing 'to'")?;
            let subject = args["subject"].as_str().ok_or("missing 'subject'")?;
            let body = args["body"].as_str().ok_or("missing 'body'")?;
            mail.send(to, subject, body).await.map_err(|e| e.to_string())?;
            Ok("Message sent".to_string())
        }
        "mail_fetch_unread" => {
            let messages = mail.fetch_unread().await.map_err(|e| e.to_string())?;
            serde_json::to_string(&messages).map_err(|e| e.to_string())
        }
        "mail_search" => {
            let query = args["query"].as_str().ok_or("missing 'query'")?;
            let messages = mail.search(query).await.map_err(|e| e.to_string())?;
            serde_json::to_string(&messages).map_err(|e| e.to_string())
        }
        "mail_mark_read" => {
            let uid = args["uid"].as_u64().ok_or("missing or invalid 'uid'")? as u32;
            mail.mark_read(uid).await.map_err(|e| e.to_string())?;
            Ok(format!("UID {} marked as read", uid))
        }
        "mail_get_by_uid" => {
            let uid = args["uid"].as_u64().ok_or("missing or invalid 'uid'")? as u32;
            match mail.get_by_uid(uid).await.map_err(|e| e.to_string())? {
                Some(msg) => serde_json::to_string(&msg).map_err(|e| e.to_string()),
                None => Err(format!("Message UID {} not found", uid)),
            }
        }
        "mail_list_mailboxes" => {
            let mailboxes = mail.list_mailboxes().await.map_err(|e| e.to_string())?;
            serde_json::to_string(&mailboxes).map_err(|e| e.to_string())
        }
        other => Err(format!("Unknown tool: {}", other)),
    }
}
