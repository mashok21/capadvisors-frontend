use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use serde::{Deserialize, Serialize};

use crate::{db::DbHelper, middleware::auth::AuthUser};

const VALID_FIRMS: &[&str] = &[
    "Deloitte",
    "KPMG",
    "EY",
    "PwC",
    "Top-20 Mid-Firm",
    "Individual Practice",
    "None / Other",
];
const VALID_YEARS: &[&str] = &["1st Year", "2nd Year", "Completed", "Direct Entry"];

// ─────────────────────────────────────────────────────────────────────────────
// Shapes
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct ProfileResponse {
    pub user_id: String,
    pub avatar_url: String,
    pub articleship_firm: String,
    pub articleship_year: String,
    pub firm_location: String,
    pub updated_at: String,
}

#[derive(Deserialize)]
pub struct UpdateProfileRequest {
    /// Passing avatar_url through the profile PUT is optional — the dedicated
    /// upload endpoint is preferred.  When omitted the existing avatar is kept.
    pub avatar_url: Option<String>,
    pub articleship_firm: String,
    pub articleship_year: String,
    pub firm_location: String,
}

// ─────────────────────────────────────────────────────────────────────────────
// GET /api/user/profile
// Returns the authenticated student's professional profile.
// If no profile row exists yet, returns default values without creating a row.
// ─────────────────────────────────────────────────────────────────────────────

pub async fn get_profile(
    AuthUser(claims): AuthUser,
    State(db): State<DbHelper>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let conn = db.get_conn();

    let mut stmt = conn
        .prepare(
            "SELECT user_id, avatar_url, articleship_firm, articleship_year,
                    firm_location, COALESCE(updated_at, '')
             FROM   student_profiles
             WHERE  user_id = ?1",
        )
        .await
        .map_err(ie)?;

    let mut rows = stmt
        .query(libsql::params![claims.sub.clone()])
        .await
        .map_err(ie)?;

    if let Some(row) = rows.next().await.map_err(ie)? {
        Ok(Json(row_to_profile(&row)?))
    } else {
        Ok(Json(ProfileResponse {
            user_id:          claims.sub,
            avatar_url:       String::new(),
            articleship_firm: "None / Other".to_string(),
            articleship_year: "1st Year".to_string(),
            firm_location:    String::new(),
            updated_at:       String::new(),
        }))
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PUT /api/user/profile
// Upserts the professional profile fields.  Skips avatar if not supplied so
// the upload endpoint remains the single writer for avatar_url.
// ─────────────────────────────────────────────────────────────────────────────

pub async fn update_profile(
    AuthUser(claims): AuthUser,
    State(db): State<DbHelper>,
    Json(payload): Json<UpdateProfileRequest>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    if !VALID_FIRMS.contains(&payload.articleship_firm.as_str()) {
        return Err((
            StatusCode::UNPROCESSABLE_ENTITY,
            format!(
                "Invalid articleship_firm '{}'. Valid values: {}",
                payload.articleship_firm,
                VALID_FIRMS.join(", ")
            ),
        ));
    }
    if !VALID_YEARS.contains(&payload.articleship_year.as_str()) {
        return Err((
            StatusCode::UNPROCESSABLE_ENTITY,
            format!(
                "Invalid articleship_year '{}'. Valid values: {}",
                payload.articleship_year,
                VALID_YEARS.join(", ")
            ),
        ));
    }

    let conn = db.get_conn();
    let user_id = claims.sub.clone();
    let avatar_url = payload.avatar_url.unwrap_or_default();

    conn.execute(
        "INSERT INTO student_profiles
             (user_id, avatar_url, articleship_firm, articleship_year, firm_location, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, CURRENT_TIMESTAMP)
         ON CONFLICT(user_id) DO UPDATE SET
             avatar_url       = CASE WHEN ?2 != '' THEN ?2 ELSE avatar_url END,
             articleship_firm = ?3,
             articleship_year = ?4,
             firm_location    = ?5,
             updated_at       = CURRENT_TIMESTAMP",
        libsql::params![
            user_id.clone(),
            avatar_url,
            payload.articleship_firm,
            payload.articleship_year,
            payload.firm_location,
        ],
    )
    .await
    .map_err(ie)?;

    let mut stmt = conn
        .prepare(
            "SELECT user_id, avatar_url, articleship_firm, articleship_year,
                    firm_location, COALESCE(updated_at, '')
             FROM   student_profiles
             WHERE  user_id = ?1",
        )
        .await
        .map_err(ie)?;

    let mut rows = stmt
        .query(libsql::params![user_id.clone()])
        .await
        .map_err(ie)?;

    let row = rows
        .next()
        .await
        .map_err(ie)?
        .ok_or_else(|| (StatusCode::INTERNAL_SERVER_ERROR, "Profile missing after upsert".to_string()))?;

    Ok(Json(row_to_profile(&row)?))
}

// ─────────────────────────────────────────────────────────────────────────────
// Private helpers
// ─────────────────────────────────────────────────────────────────────────────

fn row_to_profile(row: &libsql::Row) -> Result<ProfileResponse, (StatusCode, String)> {
    Ok(ProfileResponse {
        user_id:          row.get(0).map_err(ie)?,
        avatar_url:       row.get::<String>(1).unwrap_or_default(),
        articleship_firm: row.get::<String>(2).unwrap_or_else(|_| "None / Other".to_string()),
        articleship_year: row.get::<String>(3).unwrap_or_else(|_| "1st Year".to_string()),
        firm_location:    row.get::<String>(4).unwrap_or_default(),
        updated_at:       row.get::<String>(5).unwrap_or_default(),
    })
}

fn ie(e: libsql::Error) -> (StatusCode, String) {
    (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
}
