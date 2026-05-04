use thiserror::Error;

#[derive(Error, Debug)]
pub enum MailError {
    #[error("IMAP error: {0}")]
    Imap(#[from] async_imap::error::Error),
    #[error("SMTP error: {0}")]
    Smtp(#[from] lettre::transport::smtp::Error),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("UTF-8 error: {0}")]
    Utf8(#[from] std::string::FromUtf8Error),
    #[error("Parse error: {0}")]
    Parse(String),
}
