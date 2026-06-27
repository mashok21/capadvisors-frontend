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

/// Extractor that requires the authenticated user to hold the "admin" or
/// "super_admin" role.  Use it in any handler that should be admin-only:
///
/// ```rust
/// async fn admin_handler(RequireAdmin(claims): RequireAdmin) { ... }
/// ```
#[derive(Clone)]
pub struct RequireAdmin(pub Claims);

impl<S> FromRequestParts<S> for RequireAdmin
where
    S: Send + Sync,
{
    type Rejection = Response;

    async fn from_request_parts<'a, 'b>(
        parts: &'a mut axum::http::request::Parts,
        state: &'b S,
    ) -> Result<Self, Self::Rejection> {
        let AuthUser(claims) = AuthUser::from_request_parts(parts, state).await?;

        if claims.role == "admin" || claims.role == "super_admin" {
            Ok(RequireAdmin(claims))
        } else {
            Err((
                StatusCode::FORBIDDEN,
                "Access Denied: Admin or Super Admin privileges required.".to_string(),
            )
                .into_response())
        }
    }
}

fn unauthorized_response(msg: &str) -> Response {
    (StatusCode::UNAUTHORIZED, msg.to_string()).into_response()
}
