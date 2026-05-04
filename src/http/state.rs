use std::sync::Arc;

use crate::mail::PanoramaMail;

#[derive(Clone)]
pub struct AppState {
    pub mail: Arc<PanoramaMail>,
}
