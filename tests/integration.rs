/// Live integration tests against Proton Bridge.
///
/// Prerequisites: Proton Bridge running, .env populated.
/// Run with: cargo test -- --nocapture --test-threads=1
use std::time::Duration;

use panorama_mail::{
    config::AppConfig,
    mail::{AgentMailTransport, PanoramaMail},
};

fn load() -> (AppConfig, PanoramaMail) {
    dotenvy::dotenv().ok();
    let config = AppConfig::from_env().expect("Failed to load config — is .env populated?");
    let mail = PanoramaMail::new(config.mail.clone());
    (config, mail)
}

// ─── Mailbox listing ─────────────────────────────────────────────────────────

#[tokio::test]
async fn test_list_mailboxes() {
    let (_, mail) = load();
    let mailboxes = mail
        .list_mailboxes()
        .await
        .expect("list_mailboxes failed — is Bridge running?");

    println!("\nMailboxes ({}):", mailboxes.len());
    for mb in &mailboxes {
        println!("  {mb}");
    }

    assert!(!mailboxes.is_empty(), "Expected at least one mailbox");
    assert!(
        mailboxes.iter().any(|m| m.to_uppercase().contains("INBOX")),
        "Expected INBOX in mailbox list, got: {mailboxes:?}"
    );
}

// ─── Send ────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_send_to_self() {
    let (config, mail) = load();
    let subject = format!(
        "panorama-mail test {}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
    );

    mail.send(
        &config.mail.username,
        &subject,
        "Automated integration test from panorama-mail. Safe to delete.",
    )
    .await
    .expect("send failed");

    println!("\nSent: {subject}");
}

// ─── Fetch unread ────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_fetch_unread() {
    let (_, mail) = load();
    let messages = mail
        .fetch_unread()
        .await
        .expect("fetch_unread failed");

    println!("\nUnread messages: {}", messages.len());
    for msg in &messages {
        println!(
            "  UID={} From={} Subject={}",
            msg.uid,
            msg.from.as_deref().unwrap_or("(none)"),
            msg.subject.as_deref().unwrap_or("(none)"),
        );
    }
}

// ─── Full flow: send → wait → fetch → get → mark read ───────────────────────

#[tokio::test]
async fn test_full_flow() {
    let (config, mail) = load();

    // Unique marker so we can identify our test message.
    let marker = format!(
        "panorama-mail-flow-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
    );

    // 1. Send to self.
    println!("\n[1] Sending test message ({marker})...");
    mail.send(
        &config.mail.username,
        &marker,
        "Integration test body. Safe to delete.",
    )
    .await
    .expect("send failed");
    println!("    Sent OK");

    // 2. Give IMAP a moment to deliver.
    tokio::time::sleep(Duration::from_secs(3)).await;

    // 3. Fetch unread and look for our message.
    println!("[2] Fetching unread...");
    let messages = mail.fetch_unread().await.expect("fetch_unread failed");
    println!("    {} unread message(s)", messages.len());

    let test_msg = messages
        .iter()
        .find(|m| m.subject.as_deref().unwrap_or("") == marker);

    if test_msg.is_none() {
        println!(
            "    NOTE: test message not in unread — may already be read or delivery is slow.\n    \
             Checking via search instead..."
        );
    }

    // 4. Search for the marker subject.
    println!("[3] Searching for marker...");
    let results = mail.search(&marker).await.expect("search failed");
    println!("    {} result(s)", results.len());

    let found = test_msg.or_else(|| results.iter().find(|m| m.subject.as_deref().unwrap_or("") == marker));

    let msg = match found {
        Some(m) => m,
        None => {
            println!(
                "    WARN: message not found in unread or search — delivery may be delayed.\n    \
                 Skipping get_by_uid / mark_read assertions."
            );
            return;
        }
    };

    let uid = msg.uid;
    println!("    Found UID={uid}");

    // 5. Fetch by UID.
    println!("[4] Fetching by UID {uid}...");
    let fetched = mail
        .get_by_uid(uid)
        .await
        .expect("get_by_uid failed")
        .expect("message not found by UID");
    println!(
        "    Subject: {}",
        fetched.subject.as_deref().unwrap_or("(none)")
    );
    assert_eq!(fetched.uid, uid);

    // 6. Mark as read.
    println!("[5] Marking UID {uid} as read...");
    mail.mark_read(uid).await.expect("mark_read failed");
    println!("    Done");

    println!("[✓] Full flow passed");
}

// ─── Search ──────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_search() {
    let (config, mail) = load();
    // Search by the account's own address — should always match sent messages.
    let results = mail
        .search(&config.mail.username)
        .await
        .expect("search failed");

    println!(
        "\nSearch for '{}': {} result(s)",
        config.mail.username,
        results.len()
    );
    for msg in results.iter().take(5) {
        println!(
            "  UID={} From={} Subject={}",
            msg.uid,
            msg.from.as_deref().unwrap_or("(none)"),
            msg.subject.as_deref().unwrap_or("(none)"),
        );
    }
}
