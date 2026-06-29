use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use serde::{Deserialize, Serialize};
use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use password_hash::SaltString;
use uuid::Uuid;
use chrono::Utc;
use jsonwebtoken::{encode, Header, EncodingKey};

use crate::{
    db::DbHelper,
    middleware::auth::{Claims, RequireSuperAdmin},
};

// ─────────────────────────────────────────────────────────────────────────────
// Request / response types
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct RegisterRequest {
    pub name: String,
    pub email: String,
    pub password: String,
}

/// Used only by the `POST /api/auth/admin/create` route — the `role` field is
/// always hardcoded to `'admin'` server-side and never taken from the payload.
#[derive(Deserialize)]
pub struct CreateAdminRequest {
    pub name: String,
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
    pub name: String,
    pub email: String,
    pub role: String,
}

// ─────────────────────────────────────────────────────────────────────────────
// POST /api/auth/register  — public, always creates quiz_taker
// ─────────────────────────────────────────────────────────────────────────────

pub async fn register(
    State(db): State<DbHelper>,
    Json(payload): Json<RegisterRequest>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let conn = db.get_conn();

    let name = payload.name.trim().to_string();
    let email = payload.email.trim().to_lowercase();
    if name.is_empty() || email.is_empty() || payload.password.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "Name, email and password are required".to_string()));
    }

    let user_id = Uuid::new_v4().to_string();
    let password_hash = hash_password(&payload.password)?;

    let tx = conn
        .transaction()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    tx.execute(
        "INSERT INTO users (id, name, email, password_hash, role)
         VALUES (?1, ?2, ?3, ?4, 'quiz_taker')",
        libsql::params![user_id.clone(), name.clone(), email.clone(), password_hash],
    )
    .await
    .map_err(|e| {
        if e.to_string().contains("UNIQUE") {
            (StatusCode::CONFLICT, "An account with this email already exists".to_string())
        } else {
            (StatusCode::INTERNAL_SERVER_ERROR, format!("Registration failed: {}", e))
        }
    })?;

    // Seed a Glicko-2 rating record for quiz participants.
    tx.execute(
        "INSERT OR IGNORE INTO student_ratings
             (student_id, display_name, rating, rating_deviation, volatility, games_played)
         VALUES (?1, ?2, 1500.0, 350.0, 0.06, 0)",
        libsql::params![user_id.clone(), name.clone()],
    )
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Rating seed failed: {}", e)))?;

    tx.commit()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let token = sign_jwt(&user_id, &email, "quiz_taker")?;
    Ok(Json(AuthResponse {
        token,
        user_id,
        name,
        email,
        role: "quiz_taker".to_string(),
    }))
}

// ─────────────────────────────────────────────────────────────────────────────
// POST /api/auth/admin/create  — protected: RequireSuperAdmin only
// ─────────────────────────────────────────────────────────────────────────────

pub async fn create_admin(
    RequireSuperAdmin(_): RequireSuperAdmin,
    State(db): State<DbHelper>,
    Json(payload): Json<CreateAdminRequest>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let conn = db.get_conn();

    let name = payload.name.trim().to_string();
    let email = payload.email.trim().to_lowercase();
    if name.is_empty() || email.is_empty() || payload.password.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "Name, email and password are required".to_string()));
    }

    let user_id = Uuid::new_v4().to_string();
    let password_hash = hash_password(&payload.password)?;

    // Role is always 'admin' — never sourced from the request payload to prevent
    // privilege escalation via payload injection.
    conn.execute(
        "INSERT INTO users (id, name, email, password_hash, role)
         VALUES (?1, ?2, ?3, ?4, 'admin')",
        libsql::params![user_id.clone(), name.clone(), email.clone(), password_hash],
    )
    .await
    .map_err(|e| {
        if e.to_string().contains("UNIQUE") {
            (StatusCode::CONFLICT, "An account with this email already exists".to_string())
        } else {
            (StatusCode::INTERNAL_SERVER_ERROR, format!("Admin creation failed: {}", e))
        }
    })?;

    let token = sign_jwt(&user_id, &email, "admin")?;
    Ok(Json(AuthResponse {
        token,
        user_id,
        name,
        email,
        role: "admin".to_string(),
    }))
}

// ─────────────────────────────────────────────────────────────────────────────
// POST /api/auth/login  — public
// ─────────────────────────────────────────────────────────────────────────────

pub async fn login(
    State(db): State<DbHelper>,
    Json(payload): Json<LoginRequest>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let conn = db.get_conn();
    let email = payload.email.trim().to_lowercase();

    let mut stmt = conn
        .prepare(
            "SELECT id, name, email, password_hash, role
             FROM   users
             WHERE  email = ?1",
        )
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
        let name: String = row.get(1).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        let db_email: String = row.get(2).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        let hash_str: String = row.get(3).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        let mut role: String = row.get(4).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

        let parsed_hash = PasswordHash::new(&hash_str)
            .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Invalid stored credential".to_string()))?;

        if Argon2::default()
            .verify_password(payload.password.as_bytes(), &parsed_hash)
            .is_ok()
        {
            if let Ok(bootstrap_email) = std::env::var("BOOTSTRAP_SUPER_ADMIN") {
                if db_email.to_lowercase() == bootstrap_email.to_lowercase() {
                    role = "super_admin".to_string();
                }
            }

            let token = sign_jwt(&id, &db_email, &role)?;
            return Ok(Json(AuthResponse {
                token,
                user_id: id,
                name,
                email: db_email,
                role,
            }));
        }
    }

    Err((StatusCode::UNAUTHORIZED, "Invalid email or password".to_string()))
}

// ─────────────────────────────────────────────────────────────────────────────
// Private helpers
// ─────────────────────────────────────────────────────────────────────────────

fn hash_password(password: &str) -> Result<String, (StatusCode, String)> {
    let salt = SaltString::generate(&mut rand::thread_rng());
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Hashing error: {}", e)))
}

fn sign_jwt(id: &str, email: &str, role: &str) -> Result<String, (StatusCode, String)> {
    let secret = std::env::var("JWT_SECRET")
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "JWT_SECRET is not configured".to_string()))?;
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
