pub mod middleware;

#[allow(dead_code)]
/// Stub matching the shape of cloak-sdk's CloakState.
/// Swap in the real cloak-sdk type at Panorama workspace integration time:
///   - session_id: String
///   - signing_key: Vec<u8> (HMAC-SHA256 key for local token verification)
///   - halt_stream_url: String
#[derive(Clone, Default)]
pub struct CloakState {
    pub stub: bool,
}
