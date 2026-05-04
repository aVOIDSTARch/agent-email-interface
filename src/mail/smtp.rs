use lettre::{
    message::Mailbox, transport::smtp::authentication::Credentials, AsyncSmtpTransport,
    AsyncTransport, Message, Tokio1Executor,
};

use super::{error::MailError, types::MailConfig};

pub async fn send(config: &MailConfig, to: &str, subject: &str, body: &str) -> Result<(), MailError> {
    let from: Mailbox = config
        .username
        .parse()
        .map_err(|e: lettre::address::AddressError| MailError::Parse(format!("Invalid from address: {e}")))?;

    let to: Mailbox = to
        .parse()
        .map_err(|e: lettre::address::AddressError| MailError::Parse(format!("Invalid to address: {e}")))?;

    let email = Message::builder()
        .from(from)
        .to(to)
        .subject(subject)
        .body(body.to_string())
        .map_err(|e| MailError::Parse(format!("Failed to build message: {e}")))?;

    let mailer = AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(&config.smtp_host)
        .port(config.smtp_port)
        .credentials(Credentials::new(
            config.username.clone(),
            config.password.clone(),
        ))
        .build();

    mailer.send(email).await?;
    Ok(())
}
