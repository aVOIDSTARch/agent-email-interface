use axum::{body::Body, http::Request, middleware::Next, response::Response};

/// Pass-through auth middleware stub.
///
/// Real impl: extract "Authorization: Bearer <token>" header, verify
/// HMAC-SHA256 against CloakState.signing_key, check claims.expires_at,
/// insert TokenClaims into request extensions.
pub async fn cloak_auth(req: Request<Body>, next: Next) -> Response {
    next.run(req).await
}
