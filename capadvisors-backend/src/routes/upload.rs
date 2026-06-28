use axum::{
    extract::{Multipart, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use serde::Serialize;

use crate::{db::DbHelper, middleware::auth::AuthUser};

const MAX_AVATAR_BYTES: usize = 2 * 1024 * 1024; // 2 MB
const ALLOWED_CONTENT_TYPES: &[&str] = &["image/jpeg", "image/png", "image/webp", "image/gif"];

// ─────────────────────────────────────────────────────────────────────────────
// POST /api/user/avatar/upload
//
// Accepts a multipart field named "avatar", validates its MIME type and size,
// base64-encodes the bytes into a data URI, and upserts it into student_profiles.
// Storing as a data URI keeps the system self-contained without requiring
// external blob storage on Railway's ephemeral filesystem.
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct AvatarUploadResponse {
    pub avatar_url: String,
}

pub async fn upload_avatar(
    AuthUser(claims): AuthUser,
    State(db): State<DbHelper>,
    mut multipart: Multipart,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let mut file_bytes: Option<Vec<u8>> = None;
    let mut content_type = "image/jpeg".to_string();

    while let Some(field) = multipart.next_field().await.map_err(|e| {
        (StatusCode::BAD_REQUEST, format!("Multipart read error: {}", e))
    })? {
        if field.name().unwrap_or("") != "avatar" {
            continue;
        }

        let ct = field
            .content_type()
            .unwrap_or("image/jpeg")
            .to_string();

        if !ALLOWED_CONTENT_TYPES.contains(&ct.as_str()) {
            return Err((
                StatusCode::UNPROCESSABLE_ENTITY,
                format!(
                    "Unsupported image type '{}'. Accepted: jpeg, png, webp, gif.",
                    ct
                ),
            ));
        }
        content_type = ct;

        let data = field.bytes().await.map_err(|e| {
            (StatusCode::BAD_REQUEST, format!("Failed to read avatar bytes: {}", e))
        })?;

        if data.len() > MAX_AVATAR_BYTES {
            return Err((
                StatusCode::PAYLOAD_TOO_LARGE,
                format!("Avatar must be ≤ 2 MB. Received {} bytes.", data.len()),
            ));
        }

        file_bytes = Some(data.to_vec());
    }

    let bytes = file_bytes.ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            "No 'avatar' field found in multipart body.".to_string(),
        )
    })?;

    let avatar_url = format!("data:{};base64,{}", content_type, BASE64.encode(&bytes));

    let conn = db.get_conn();
    conn.execute(
        "INSERT INTO student_profiles (user_id, avatar_url, updated_at)
         VALUES (?1, ?2, CURRENT_TIMESTAMP)
         ON CONFLICT(user_id) DO UPDATE SET
             avatar_url = excluded.avatar_url,
             updated_at = CURRENT_TIMESTAMP",
        libsql::params![claims.sub.clone(), avatar_url.clone()],
    )
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(AvatarUploadResponse { avatar_url }))
}
