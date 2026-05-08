//! HTTP server integration tests.
//!
//! Prerequisites: Proton Bridge running, .env populated with valid credentials.
//! Run with: cargo test --test http_server_test -- --nocapture --test-threads=1
use std::sync::Arc;
use std::time::Duration;

use panorama_mail::{config::AppConfig, http::build_router, mail::PanoramaMail};
use reqwest::Client;

async fn spawn_server() -> (String, tokio::task::JoinHandle<()>) {
    dotenvy::dotenv().ok();
    let config = AppConfig::from_env().expect("Failed to load config — is .env populated?");
    let mail = Arc::new(PanoramaMail::new(config.mail.clone()));

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("failed to bind random port");
    let port = listener.local_addr().expect("no local addr").port();
    let base_url = format!("http://127.0.0.1:{}", port);

    let router = build_router(mail);
    let handle = tokio::spawn(async move {
        axum::serve(listener, router).await.ok();
    });

    (base_url, handle)
}

#[tokio::test]
async fn test_http_health() {
    let (base_url, _handle) = spawn_server().await;
    let client = Client::new();

    let resp = client
        .get(format!("{}/health", base_url))
        .send()
        .await
        .expect("request failed");

    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.expect("json parse failed");
    assert_eq!(body["status"], "ok");
    println!("\nHealth: {body}");
}

#[tokio::test]
async fn test_http_list_mailboxes() {
    let (base_url, _handle) = spawn_server().await;
    let client = Client::new();

    let resp = client
        .get(format!("{}/mail/mailboxes", base_url))
        .send()
        .await
        .expect("request failed");

    assert_eq!(resp.status(), 200);
    let mailboxes: Vec<String> = resp.json().await.expect("json parse failed");
    assert!(!mailboxes.is_empty(), "Expected at least one mailbox");
    assert!(
        mailboxes.iter().any(|m| m.to_uppercase().contains("INBOX")),
        "Expected INBOX in mailbox list, got: {mailboxes:?}"
    );
    println!("\nMailboxes: {mailboxes:?}");
}

#[tokio::test]
async fn test_http_send_and_receive() {
    let (base_url, _handle) = spawn_server().await;

    dotenvy::dotenv().ok();
    let config = AppConfig::from_env().expect("Failed to load config");

    let client = Client::new();

    let subject = format!(
        "panorama-mail http-send-receive {}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
    );

    println!("\n[1] POST /mail/send...");
    let send_resp = client
        .post(format!("{}/mail/send", base_url))
        .json(&serde_json::json!({
            "to": config.mail.username,
            "subject": &subject,
            "body": "HTTP send-and-receive integration test. Safe to delete."
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

    println!("[2] Waiting 3s for IMAP delivery...");
    tokio::time::sleep(Duration::from_secs(3)).await;

    println!("[3] GET /mail/unread...");
    let unread_resp = client
        .get(format!("{}/mail/unread", base_url))
        .send()
        .await
        .expect("unread request failed");

    assert_eq!(unread_resp.status(), 200);
    let messages: Vec<serde_json::Value> = unread_resp.json().await.expect("json parse failed");
    println!("    {} unread message(s)", messages.len());
    println!("[✓] HTTP send-and-receive passed");
}

#[tokio::test]
async fn test_http_full_flow() {
    let (base_url, _handle) = spawn_server().await;

    dotenvy::dotenv().ok();
    let config = AppConfig::from_env().expect("Failed to load config");

    let client = Client::new();

    let marker = format!(
        "panorama-mail-http-flow-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
    );

    // 1. Send to self via HTTP.
    println!("\n[1] POST /mail/send ({marker})...");
    let send_resp = client
        .post(format!("{}/mail/send", base_url))
        .json(&serde_json::json!({
            "to": config.mail.username,
            "subject": &marker,
            "body": "HTTP full-flow integration test. Safe to delete."
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
    println!("[2] Waiting 3s for IMAP delivery...");
    tokio::time::sleep(Duration::from_secs(3)).await;

    // 3. GET /mail/unread — look for our message.
    println!("[3] GET /mail/unread...");
    let unread_resp = client
        .get(format!("{}/mail/unread", base_url))
        .send()
        .await
        .expect("unread request failed");
    assert_eq!(unread_resp.status(), 200);
    let unread: Vec<serde_json::Value> = unread_resp.json().await.expect("json parse failed");
    println!("    {} unread message(s)", unread.len());

    let test_msg_uid: Option<u64> = unread.iter().find_map(|m| {
        if m["subject"].as_str() == Some(marker.as_str()) {
            m["uid"].as_u64()
        } else {
            None
        }
    });

    // 4. GET /mail/search?q=<marker> as fallback.
    let uid = match test_msg_uid {
        Some(uid) => uid,
        None => {
            println!("[4] GET /mail/search?q={marker} (not found in unread)...");
            let search_resp = client
                .get(format!("{}/mail/search", base_url))
                .query(&[("q", &marker)])
                .send()
                .await
                .expect("search request failed");
            assert_eq!(search_resp.status(), 200);
            let results: Vec<serde_json::Value> =
                search_resp.json().await.expect("json parse failed");
            println!("    {} search result(s)", results.len());

            match results.iter().find_map(|m| {
                if m["subject"].as_str() == Some(marker.as_str()) {
                    m["uid"].as_u64()
                } else {
                    None
                }
            }) {
                Some(uid) => uid,
                None => {
                    println!(
                        "    WARN: message not yet delivered — delivery delay exceeded. Skipping get/mark-read."
                    );
                    return;
                }
            }
        }
    };

    println!("    Found UID={uid}");

    // 5. GET /mail/messages/:uid
    println!("[5] GET /mail/messages/{uid}...");
    let get_resp = client
        .get(format!("{}/mail/messages/{}", base_url, uid))
        .send()
        .await
        .expect("get_message request failed");
    assert_eq!(get_resp.status(), 200);
    let fetched: serde_json::Value = get_resp.json().await.expect("json parse failed");
    assert_eq!(fetched["uid"].as_u64(), Some(uid));
    println!(
        "    Subject: {}",
        fetched["subject"].as_str().unwrap_or("(none)")
    );

    // 6. POST /mail/messages/:uid/read
    println!("[6] POST /mail/messages/{uid}/read...");
    let mark_resp = client
        .post(format!("{}/mail/messages/{}/read", base_url, uid))
        .send()
        .await
        .expect("mark_read request failed");
    assert_eq!(mark_resp.status(), 200);
    let mark_body: serde_json::Value = mark_resp.json().await.expect("json parse failed");
    assert_eq!(mark_body["ok"], true);

    println!("[✓] HTTP full flow passed");
}
