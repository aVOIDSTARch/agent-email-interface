pub mod protocol;
pub mod server;
pub mod tools;

use std::sync::Arc;

use crate::mail::PanoramaMail;

use server::McpServer;

pub async fn run(mail: Arc<PanoramaMail>) {
    McpServer::new(mail).run().await;
}
