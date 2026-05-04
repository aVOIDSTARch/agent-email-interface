use async_imap::Client;
use tokio::net::TcpStream;
use tokio_util::compat::TokioAsyncReadCompatExt;

use super::{error::MailError, types::MailConfig};

pub type ImapSession = async_imap::Session<tokio_util::compat::Compat<TcpStream>>;

pub async fn open_session(config: &MailConfig) -> Result<ImapSession, MailError> {
    let tcp = TcpStream::connect((config.imap_host.as_str(), config.imap_port)).await?;
    let client = Client::new(tcp.compat());
    client
        .login(&config.username, &config.password)
        .await
        .map_err(|(e, _)| MailError::Imap(e))
}
