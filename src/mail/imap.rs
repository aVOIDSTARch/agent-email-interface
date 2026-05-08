use futures::TryStreamExt;
use mail_parser::MessageParser;

use super::{
    error::MailError,
    session::open_session,
    types::{AgentMessage, MailConfig},
};

fn parse_message(body: &[u8], uid: u32) -> Result<AgentMessage, MailError> {
    let parsed = MessageParser::default()
        .parse(body)
        .ok_or_else(|| MailError::Parse("Failed to parse RFC822 message".to_string()))?;

    Ok(AgentMessage {
        uid,
        subject: parsed.subject().map(|s| s.to_string()),
        from: parsed
            .from()
            .and_then(|f| f.first())
            .and_then(|a| a.address())
            .map(|s| s.to_string()),
        to: parsed
            .to()
            .map(|t| {
                t.iter()
                    .filter_map(|a| a.address())
                    .map(|s| s.to_string())
                    .collect()
            })
            .unwrap_or_default(),
        body: parsed.body_text(0).map(|s| s.into_owned()),
        html_body: parsed.body_html(0).map(|s| s.into_owned()),
        attachments: vec![],
    })
}

pub async fn fetch_unread(config: &MailConfig) -> Result<Vec<AgentMessage>, MailError> {
    let mut session = open_session(config).await?;
    session.select(&config.mailbox).await?;

    let uids = session.uid_search("UNSEEN").await?;
    if uids.is_empty() {
        session.logout().await?;
        return Ok(vec![]);
    }

    let uid_set = uids
        .iter()
        .map(|u| u.to_string())
        .collect::<Vec<_>>()
        .join(",");

    let stream = session.uid_fetch(&uid_set, "RFC822").await?;
    let fetches: Vec<_> = stream.try_collect().await?;

    let mut messages = Vec::new();
    for fetch in &fetches {
        if let Some(body) = fetch.body() {
            let uid = fetch.uid.unwrap_or(0);
            messages.push(parse_message(body, uid)?);
        }
    }

    session.logout().await?;
    Ok(messages)
}

pub async fn search(config: &MailConfig, query: &str) -> Result<Vec<AgentMessage>, MailError> {
    let mut session = open_session(config).await?;
    session.select(&config.mailbox).await?;

    let imap_query = format!("OR SUBJECT \"{}\" FROM \"{}\"", query, query);
    let uids = session.uid_search(&imap_query).await?;

    if uids.is_empty() {
        session.logout().await?;
        return Ok(vec![]);
    }

    let uid_set = uids
        .iter()
        .map(|u| u.to_string())
        .collect::<Vec<_>>()
        .join(",");

    let stream = session.uid_fetch(&uid_set, "RFC822").await?;
    let fetches: Vec<_> = stream.try_collect().await?;

    let mut messages = Vec::new();
    for fetch in &fetches {
        if let Some(body) = fetch.body() {
            let uid = fetch.uid.unwrap_or(0);
            messages.push(parse_message(body, uid)?);
        }
    }

    session.logout().await?;
    Ok(messages)
}

pub async fn mark_read(config: &MailConfig, uid: u32) -> Result<(), MailError> {
    let mut session = open_session(config).await?;
    session.select(&config.mailbox).await?;

    // Consume the store response stream to let the server confirm.
    let stream = session
        .uid_store(format!("{}", uid), "+FLAGS (\\Seen)")
        .await?;
    stream.try_for_each(|_| async { Ok(()) }).await?;

    session.logout().await?;
    Ok(())
}

pub async fn get_by_uid(config: &MailConfig, uid: u32) -> Result<Option<AgentMessage>, MailError> {
    let mut session = open_session(config).await?;
    session.select(&config.mailbox).await?;

    let stream = session.uid_fetch(format!("{}", uid), "RFC822").await?;
    let fetches: Vec<_> = stream.try_collect().await?;

    let result = fetches
        .first()
        .and_then(|f| f.body())
        .map(|body| parse_message(body, uid))
        .transpose()?;

    session.logout().await?;
    Ok(result)
}

pub async fn list_mailboxes(config: &MailConfig) -> Result<Vec<String>, MailError> {
    let mut session = open_session(config).await?;

    let stream = session.list(None, Some("*")).await?;
    let names: Vec<_> = stream.try_collect().await?;
    let mailboxes = names.iter().map(|n| n.name().to_string()).collect();

    session.logout().await?;
    Ok(mailboxes)
}
