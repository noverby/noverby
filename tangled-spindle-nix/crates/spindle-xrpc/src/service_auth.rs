//! Service authentication extractor for XRPC requests.
//!
//! Validates the `Authorization: Bearer <token>` header against the configured
//! spindle token. For v1, this is a simple bearer token check. Full AT Protocol
//! JWT-based service authentication (DID resolution, signature verification)
//! is deferred to Phase 8 hardening.

use axum::extract::FromRequestParts;
use axum::http::StatusCode;
use axum::http::request::Parts;
use axum::response::{IntoResponse, Response};

use crate::XrpcContext;

/// Extracted service authentication information.
///
/// If this extractor succeeds, the request has been authenticated.
#[derive(Debug, Clone)]
pub struct ServiceAuth {
    /// The authenticated caller's DID.
    ///
    /// In v1 (bearer token mode), this is always the spindle owner's DID.
    /// In a future AT Protocol JWT mode, this will be the `iss` claim from
    /// the JWT.
    pub did: String,
}

/// Error returned when authentication fails.
#[derive(Debug)]
pub struct AuthError {
    pub status: StatusCode,
    pub message: String,
}

impl IntoResponse for AuthError {
    fn into_response(self) -> Response {
        let body = serde_json::json!({
            "error": "AuthenticationRequired",
            "message": self.message,
        });
        (self.status, axum::Json(body)).into_response()
    }
}

impl FromRequestParts<std::sync::Arc<XrpcContext>> for ServiceAuth {
    type Rejection = AuthError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &std::sync::Arc<XrpcContext>,
    ) -> Result<Self, Self::Rejection> {
        let auth_header = parts
            .headers
            .get("authorization")
            .and_then(|v| v.to_str().ok());

        let token = match auth_header {
            Some(header) if header.starts_with("Bearer ") => &header[7..],
            _ => {
                return Err(AuthError {
                    status: StatusCode::UNAUTHORIZED,
                    message: "missing or invalid Authorization header".into(),
                });
            }
        };

        if token != state.token {
            return Err(AuthError {
                status: StatusCode::UNAUTHORIZED,
                message: "invalid bearer token".into(),
            });
        }

        // In v1, authenticated caller is always the owner.
        // TODO: Phase 8 — Parse JWT `iss` claim for the actual caller DID.
        Ok(ServiceAuth {
            did: state.owner.clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auth_error_response_format() {
        let err = AuthError {
            status: StatusCode::UNAUTHORIZED,
            message: "test error".into(),
        };
        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }
}
