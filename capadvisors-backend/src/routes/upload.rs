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

        // Magic-byte verification: reject files where the Content-Type header
        // does not match the actual binary signature.  This closes the attack
        // vector where a client sends e.g. a PHP script with Content-Type:
        // image/jpeg and the MIME check alone would pass.
        if !validate_image_bytes(&content_type, &data) {
            return Err((
                StatusCode::BAD_REQUEST,
                format!(
                    "File binary signature does not match declared Content-Type '{}'.",
                    content_type
                ),
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

// ─────────────────────────────────────────────────────────────────────────────
// Private helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Sniffs the first bytes of `bytes` against the canonical magic-byte
/// signature for `ct`.  Returns `false` if the binary content does not match
/// the declared MIME type — the caller should reject the upload.
fn validate_image_bytes(ct: &str, bytes: &[u8]) -> bool {
    match ct {
        "image/png" => bytes.starts_with(b"\x89PNG\r\n\x1a\n"),
        "image/jpeg" => bytes.starts_with(&[0xFF, 0xD8, 0xFF]),
        "image/gif" => bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a"),
        "image/webp" => {
            bytes.len() > 12
                && bytes[0..4] == *b"RIFF"
                && bytes[8..12] == *b"WEBP"
        }
        // Any MIME type not in ALLOWED_CONTENT_TYPES is already rejected before
        // this function is reached, but fail-closed for any unknown arm.
        _ => false,
    }
}
