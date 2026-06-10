use axum::{
    extract::{Request, State},
    http::{StatusCode, header::AUTHORIZATION},
    middleware::Next,
    response::{IntoResponse, Response},
};
use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode, decode_header, jwk::JwkSet};
use serde::Deserialize;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Clone)]
pub struct AuthState {
    verifier: Option<Arc<AuthVerifier>>,
}

impl AuthState {
    pub fn disabled() -> Self {
        Self { verifier: None }
    }

    pub fn from_env() -> Self {
        std::env::var("AUTH_ISSUER").map_or_else(|_| Self::disabled(), Self::enabled)
    }

    pub fn enabled(issuer: String) -> Self {
        Self {
            verifier: Some(Arc::new(AuthVerifier::new(issuer))),
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.verifier.is_some()
    }
}

pub async fn require_auth(
    State(auth): State<AuthState>,
    request: Request,
    next: Next,
) -> Result<Response, AuthError> {
    let Some(verifier) = auth.verifier else {
        return Ok(next.run(request).await);
    };
    let token = bearer_token(&request)?;

    verifier.verify(token).await?;

    Ok(next.run(request).await)
}

fn bearer_token(request: &Request) -> Result<&str, AuthError> {
    let value = request
        .headers()
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .ok_or(AuthError::MissingBearerToken)?;

    value
        .strip_prefix("Bearer ")
        .filter(|token| !token.is_empty())
        .ok_or(AuthError::MissingBearerToken)
}

struct AuthVerifier {
    issuer: String,
    jwks: RwLock<Option<JwkSet>>,
}

impl AuthVerifier {
    fn new(issuer: String) -> Self {
        Self {
            issuer: issuer.trim_end_matches('/').to_owned(),
            jwks: RwLock::new(None),
        }
    }

    async fn verify(&self, token: &str) -> Result<TokenClaims, AuthError> {
        if let Ok(claims) = self.verify_with_cached_jwks(token).await {
            return Ok(claims);
        }

        self.refresh_jwks().await?;
        self.verify_with_cached_jwks(token).await
    }

    async fn verify_with_cached_jwks(&self, token: &str) -> Result<TokenClaims, AuthError> {
        let header = decode_header(token).map_err(|_| AuthError::InvalidToken)?;
        let kid = header.kid.ok_or(AuthError::InvalidToken)?;
        let jwks = self.jwks.read().await;
        let jwk = jwks
            .as_ref()
            .and_then(|set| set.find(&kid))
            .ok_or(AuthError::InvalidToken)?;
        let key = DecodingKey::from_jwk(jwk).map_err(|_| AuthError::InvalidToken)?;
        let mut validation = Validation::new(Algorithm::EdDSA);

        validation.set_issuer(&[self.issuer.as_str()]);

        let token =
            decode::<TokenClaims>(token, &key, &validation).map_err(|_| AuthError::InvalidToken)?;

        let claims = token.claims;
        let _ = (&claims.sub, &claims.iss, claims.exp, &claims.amr);

        Ok(claims)
    }

    async fn refresh_jwks(&self) -> Result<(), AuthError> {
        let response = reqwest::get(format!("{}/jwks", self.issuer))
            .await
            .map_err(|_| AuthError::JwksUnavailable)?;
        let jwks = response
            .error_for_status()
            .map_err(|_| AuthError::JwksUnavailable)?
            .json::<JwkSet>()
            .await
            .map_err(|_| AuthError::JwksUnavailable)?;

        *self.jwks.write().await = Some(jwks);

        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize)]
struct TokenClaims {
    sub: String,
    iss: String,
    exp: usize,
    amr: Option<Vec<String>>,
}

#[derive(Debug)]
pub enum AuthError {
    MissingBearerToken,
    InvalidToken,
    JwksUnavailable,
}

impl IntoResponse for AuthError {
    fn into_response(self) -> Response {
        let message = match self {
            AuthError::MissingBearerToken => "missing bearer token",
            AuthError::InvalidToken => "invalid access token",
            AuthError::JwksUnavailable => "identity provider jwks unavailable",
        };

        (StatusCode::UNAUTHORIZED, message).into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{Router, routing::get};
    use tower::ServiceExt;

    #[test]
    fn disabled_auth_allows_request() {
        assert!(!AuthState::disabled().is_enabled());
    }

    #[tokio::test]
    async fn enabled_auth_requires_bearer_token() {
        let app = Router::new()
            .route("/protected", get(|| async { "ok" }))
            .route_layer(axum::middleware::from_fn_with_state(
                AuthState::enabled("http://127.0.0.1:7777".to_owned()),
                require_auth,
            ));
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/protected")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }
}
