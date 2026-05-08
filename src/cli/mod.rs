pub mod handlers;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "panorama-mail",
    about = "Proton Bridge email interface for Panorama agents",
    version
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Start HTTP server with Cloak-stub auth (default port 8420)
    Serve,
    /// Start MCP JSON-RPC stdio server
    Mcp,
    /// Send an email
    Send {
        #[arg(long, help = "Recipient email address")]
        to: String,
        #[arg(long, help = "Email subject")]
        subject: String,
        #[arg(long, default_value = "", help = "Email body (plain text)")]
        body: String,
        #[arg(long, help = "Output result as JSON")]
        json: bool,
    },
    /// Fetch all unread messages
    Fetch {
        #[arg(long, help = "Output as JSON")]
        json: bool,
        #[arg(long, value_name = "SECONDS", help = "Poll for new mail every N seconds (default 300 when flag is present)")]
        fetch_timer: Option<u64>,
    },
    /// Search messages by subject or sender
    Search {
        #[arg(help = "Search term (matches subject or from address)")]
        query: String,
        #[arg(long, help = "Output as JSON")]
        json: bool,
    },
    /// Fetch a single message by UID
    Get {
        #[arg(help = "IMAP UID of the message")]
        uid: u32,
        #[arg(long, help = "Output as JSON")]
        json: bool,
    },
    /// Mark a message as read
    Read {
        #[arg(help = "IMAP UID of the message")]
        uid: u32,
    },
    /// List all IMAP mailboxes
    Mailboxes {
        #[arg(long, help = "Output as JSON")]
        json: bool,
    },
}
