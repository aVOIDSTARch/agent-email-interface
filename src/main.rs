mod cli;
mod cloak;
mod config;
mod http;
mod mail;
mod mcp;

use std::sync::Arc;

use clap::Parser;

use cli::Commands;
use config::AppConfig;
use mail::PanoramaMail;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let cli = cli::Cli::parse();

    let config = AppConfig::from_env().unwrap_or_else(|e| {
        eprintln!("Config error: {e}");
        std::process::exit(1);
    });

    let mail = Arc::new(PanoramaMail::new(config.mail.clone()));

    match cli.command {
        Commands::Serve => http::serve(&config, Arc::clone(&mail)).await,
        Commands::Mcp => mcp::run(Arc::clone(&mail)).await,
        Commands::Send { to, subject, body, json } => {
            cli::handlers::send(&mail, &to, &subject, &body, json).await;
        }
        Commands::Fetch { json } => {
            cli::handlers::fetch(&mail, json).await;
        }
        Commands::Search { query, json } => {
            cli::handlers::search(&mail, &query, json).await;
        }
        Commands::Get { uid, json } => {
            cli::handlers::get(&mail, uid, json).await;
        }
        Commands::Read { uid } => {
            cli::handlers::read(&mail, uid).await;
        }
        Commands::Mailboxes { json } => {
            cli::handlers::mailboxes(&mail, json).await;
        }
    }
}
