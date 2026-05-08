pub mod protocol;
pub mod server;
pub mod tools;

use std::sync::Arc;

use crate::{logger::SharedLogger, mail::PanoramaMail};

use server::McpServer;

pub async fn run(mail: Arc<PanoramaMail>, logger: SharedLogger) {
    McpServer::new(mail, logger).run().await;
}
