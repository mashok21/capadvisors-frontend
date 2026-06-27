use axum::{
    extract::FromRequestParts,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use jsonwebtoken::{decode, Algorithm, DecodingKey, Validation};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    pub sub: String,
    pub email: String,
    pub role: String,
    pub exp: usize,
}

#[derive(Clone)]
pub struct AuthUser(pub Claims);

impl<S> FromRequestParts<S> for AuthUser
where
    S: Send + Sync,
{
    type Rejection = Response;

    async fn from_request_parts<'a, 'b>(
        parts: &'a mut axum::http::request::Parts,
        _state: &'b S,
    ) -> Result<Self, Self::Rejection> {
        let auth_header = parts
            .headers
            .get(axum::http::header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .ok_or_else(|| unauthorized_response("Missing Authorization header"))?;

        let token = auth_header
            .strip_prefix("Bearer ")
            .ok_or_else(|| unauthorized_response("Invalid Bearer token format"))?;

        let secret = std::env::var("JWT_SECRET")
            .unwrap_or_else(|_| "super_secret_key_change_me".to_string());

        let mut validation = Validation::new(Algorithm::HS256);
        validation.validate_exp = true;

        match decode::<Claims>(
            token,
            &DecodingKey::from_secret(secret.as_bytes()),
            &validation,
        ) {
            Ok(data) => Ok(AuthUser(data.claims)),
            Err(_) => Err(unauthorized_response("Invalid or expired token")),
        }
    }
}

fn unauthorized_response(msg: &str) -> Response {
    (StatusCode::UNAUTHORIZED, msg.to_string()).into_response()
}
