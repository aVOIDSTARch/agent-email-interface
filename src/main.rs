use std::sync::Arc;

use clap::Parser;
use dotenvy::dotenv;
use panorama_mail::{
    cli, config::AppConfig, http, log_info, log_error,
    logger::{LoguruLogger, SharedLogger},
    mail::PanoramaMail, mcp,
    store::{EmailStore, SqliteTantivyStore},
};

use cli::Commands;

#[tokio::main]
async fn main() {
    dotenv().ok();

    // ── Logger — swap LoguruLogger for any type that implements Logger ────────
    let logger: SharedLogger = Arc::new(LoguruLogger::new());

    tracing_subscriber::fmt::init();

    let cli = cli::Cli::parse();

    let config = AppConfig::from_env().unwrap_or_else(|e| {
        log_error!(logger, "Config error: {e}");
        std::process::exit(1);
    });

    let store: Arc<dyn EmailStore> = Arc::new(
        SqliteTantivyStore::open(&config.store_path)
            .await
            .unwrap_or_else(|e| {
                log_error!(logger, "Store init error: {e}");
                std::process::exit(1);
            }),
    );

    let mail = Arc::new(
        PanoramaMail::new(config.mail.clone()).with_store(Arc::clone(&store)),
    );

    match cli.command {
        Commands::Serve => {
            http::serve(&config, Arc::clone(&mail), Arc::clone(&logger)).await;
        }
        Commands::Mcp => {
            mcp::run(Arc::clone(&mail), Arc::clone(&logger)).await;
        }
        Commands::Send { to, subject, body, json } => {
            cli::handlers::send(&mail, &to, &subject, &body, json).await;
        }
        Commands::Fetch { json, fetch_timer } => {
            match fetch_timer {
                Some(secs) => cli::handlers::fetch_loop(&mail, json, secs).await,
                None => cli::handlers::fetch(&mail, json).await,
            }
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

    log_info!(logger, "panorama-mail exiting");
}
