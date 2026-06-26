use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use serde::{Deserialize, Serialize};
use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use password_hash::SaltString;
use uuid::Uuid;
use chrono::Utc;
use jsonwebtoken::{encode, Header, EncodingKey};

use crate::{db::DbHelper, middleware::auth::Claims};

#[derive(Deserialize)]
pub struct RegisterRequest {
    pub email: String,
    pub password: String,
}

#[derive(Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

#[derive(Serialize)]
pub struct AuthResponse {
    pub token: String,
    pub user_id: String,
    pub role: String,
}

pub async fn register(
    State(db): State<DbHelper>,
    Json(payload): Json<RegisterRequest>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let conn = db.get_conn();

    let email = payload.email.trim().to_lowercase();
    if email.is_empty() || payload.password.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "Email and password are required".to_string()));
    }

    let user_id = Uuid::new_v4().to_string();

    let salt = SaltString::generate(&mut rand::thread_rng());
    let password_hash = Argon2::default()
        .hash_password(payload.password.as_bytes(), &salt)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Hashing error: {}", e)))?
        .to_string();

    conn.execute(
        "INSERT INTO users (id, email, password_hash, role, created_at) VALUES (?1, ?2, ?3, 'student', ?4)",
        libsql::params![
            user_id.clone(),
            email.clone(),
            password_hash,
            Utc::now().to_rfc3339()
        ],
    )
    .await
    .map_err(|e| {
        if e.to_string().contains("UNIQUE") {
            (StatusCode::CONFLICT, "An account with this email already exists".to_string())
        } else {
            (StatusCode::INTERNAL_SERVER_ERROR, format!("Registration failed: {}", e))
        }
    })?;

    conn.execute(
        "INSERT OR IGNORE INTO student_ratings (student_id, display_name, rating, rating_deviation, volatility, games_played) VALUES (?1, ?2, 1500.0, 350.0, 0.06, 0)",
        libsql::params![user_id.clone(), email.clone()],
    )
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Rating seed failed: {}", e)))?;

    let token = sign_jwt(&user_id, &email, "student")?;
    Ok(Json(AuthResponse {
        token,
        user_id,
        role: "student".to_string(),
    }))
}

pub async fn login(
    State(db): State<DbHelper>,
    Json(payload): Json<LoginRequest>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let conn = db.get_conn();
    let email = payload.email.trim().to_lowercase();

    let mut stmt = conn
        .prepare("SELECT id, email, password_hash, role FROM users WHERE email = ?1")
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let mut rows = stmt
        .query(libsql::params![email])
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    if let Some(row) = rows
        .next()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    {
        let id: String = row.get(0).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        let db_email: String = row.get(1).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        let hash_str: String = row.get(2).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        let role: String = row.get(3).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

        let parsed_hash = PasswordHash::new(&hash_str)
            .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Invalid stored credential".to_string()))?;

        if Argon2::default()
            .verify_password(payload.password.as_bytes(), &parsed_hash)
            .is_ok()
        {
            let token = sign_jwt(&id, &db_email, &role)?;
            return Ok(Json(AuthResponse {
                token,
                user_id: id,
                role,
            }));
        }
    }

    Err((StatusCode::UNAUTHORIZED, "Invalid email or password".to_string()))
}

fn sign_jwt(id: &str, email: &str, role: &str) -> Result<String, (StatusCode, String)> {
    let secret = std::env::var("JWT_SECRET")
        .unwrap_or_else(|_| "super_secret_key_change_me".to_string());
    let claims = Claims {
        sub: id.to_string(),
        email: email.to_string(),
        role: role.to_string(),
        exp: (Utc::now().timestamp() + 86400 * 7) as usize,
    };
    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("JWT signing error: {}", e)))
}
