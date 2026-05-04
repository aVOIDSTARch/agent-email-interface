pub mod error;
pub mod handlers;
pub mod state;

use std::sync::Arc;

use axum::{middleware, routing::get, routing::post, Router};
use tower_http::trace::TraceLayer;

use crate::{cloak::middleware::cloak_auth, config::AppConfig, mail::PanoramaMail};

use state::AppState;

pub async fn serve(config: &AppConfig, mail: Arc<PanoramaMail>) {
    let state = AppState { mail };

    let app = Router::new()
        .route("/health", get(handlers::health))
        .route("/mail/send", post(handlers::send))
        .route("/mail/unread", get(handlers::fetch_unread))
        .route("/mail/search", get(handlers::search))
        .route("/mail/messages/:uid", get(handlers::get_message))
        .route("/mail/messages/:uid/read", post(handlers::mark_read))
        .route("/mail/mailboxes", get(handlers::list_mailboxes))
        .layer(middleware::from_fn(cloak_auth))
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let addr = format!("0.0.0.0:{}", config.http_port);
    tracing::info!("panorama-mail HTTP server listening on {}", addr);
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .expect("failed to bind listener");
    axum::serve(listener, app).await.expect("server error");
}
