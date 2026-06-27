use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{db::DbHelper, middleware::auth::RequireAdmin};

// ─────────────────────────────────────────────────────────────────────────────
// Response types
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct StagingItem {
    pub id: String,
    pub chapter_id: String,
    pub question_text: String,
    pub scoring_rubric_json: String,
    pub alternate_variants_json: String,
    pub status: String,
    pub created_at: String,
}

#[derive(Serialize)]
pub struct DataBankItem {
    pub id: String,
    pub chapter_id: String,
    pub question_text: String,
    pub scoring_rubric_json: String,
    pub alternate_variants_json: String,
    pub rating: f64,
    pub rating_deviation: f64,
    pub volatility: f64,
    pub created_at: String,
}

// ─────────────────────────────────────────────────────────────────────────────
// Request types
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct EditStagingRequest {
    pub question_text: String,
    pub scoring_rubric_json: String,
    pub alternate_variants_json: String,
}

// ─────────────────────────────────────────────────────────────────────────────
// GET /api/admin/questions/staging
// Lists every question currently in the pending_review queue.
// ─────────────────────────────────────────────────────────────────────────────

pub async fn list_staging(
    RequireAdmin(_): RequireAdmin,
    State(db): State<DbHelper>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let conn = db.get_conn();

    let mut stmt = conn
        .prepare(
            "SELECT id, chapter_id, question_text, scoring_rubric_json,
                    alternate_variants_json, status, created_at
             FROM   question_staging_queue
             WHERE  status = 'pending_review'
             ORDER  BY created_at ASC",
        )
        .await
        .map_err(libsql_err)?;

    let mut rows = stmt.query(()).await.map_err(libsql_err)?;

    let mut items: Vec<StagingItem> = Vec::new();
    while let Some(row) = rows.next().await.map_err(libsql_err)? {
        items.push(row_to_staging(&row)?);
    }

    Ok(Json(items))
}

// ─────────────────────────────────────────────────────────────────────────────
// PUT /api/admin/questions/staging/:id
// Replaces all editable fields on a pending staged question.
// ─────────────────────────────────────────────────────────────────────────────

pub async fn edit_staging(
    RequireAdmin(_): RequireAdmin,
    State(db): State<DbHelper>,
    Path(id): Path<String>,
    Json(payload): Json<EditStagingRequest>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let conn = db.get_conn();

    let affected = conn
        .execute(
            "UPDATE question_staging_queue
             SET    question_text           = ?1,
                    scoring_rubric_json     = ?2,
                    alternate_variants_json = ?3
             WHERE  id     = ?4
               AND  status = 'pending_review'",
            libsql::params![
                payload.question_text,
                payload.scoring_rubric_json,
                payload.alternate_variants_json,
                id.clone()
            ],
        )
        .await
        .map_err(libsql_err)?;

    if affected == 0 {
        return Err((
            StatusCode::NOT_FOUND,
            format!(
                "Staging item '{}' not found or is not in pending_review state",
                id
            ),
        ));
    }

    let item = fetch_staging_item(&conn, &id).await?;
    Ok(Json(item))
}

// ─────────────────────────────────────────────────────────────────────────────
// POST /api/admin/questions/staging/:id/approve
// Atomically marks the staging item 'approved' and promotes it to quiz_databank
// with Glicko-2 seed values (R=1500, RD=350, σ=0.06).
// ─────────────────────────────────────────────────────────────────────────────

pub async fn approve_staging(
    RequireAdmin(_): RequireAdmin,
    State(db): State<DbHelper>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let conn = db.get_conn();

    // Fetch staging data before the transaction to get field values for INSERT.
    let staging = fetch_staging_item(&conn, &id).await?;
    if staging.status != "pending_review" {
        return Err((
            StatusCode::CONFLICT,
            format!("Staging item '{}' is already '{}'", id, staging.status),
        ));
    }

    let databank_id = Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();

    let tx = conn
        .transaction()
        .await
        .map_err(libsql_err)?;

    // WHERE status='pending_review' is the atomic guard against concurrent
    // double-approval: if two requests race here, only one UPDATE wins.
    let affected = tx
        .execute(
            "UPDATE question_staging_queue
             SET    status = 'approved'
             WHERE  id     = ?1
               AND  status = 'pending_review'",
            libsql::params![id.clone()],
        )
        .await
        .map_err(libsql_err)?;

    if affected == 0 {
        return Err((
            StatusCode::CONFLICT,
            format!("Staging item '{}' was concurrently processed; no action taken", id),
        ));
    }

    tx.execute(
        "INSERT INTO quiz_databank
             (id, chapter_id, question_text, scoring_rubric_json,
              alternate_variants_json, rating, rating_deviation, volatility)
         VALUES (?1, ?2, ?3, ?4, ?5, 1500.0, 350.0, 0.06)",
        libsql::params![
            databank_id.clone(),
            staging.chapter_id.clone(),
            staging.question_text.clone(),
            staging.scoring_rubric_json.clone(),
            staging.alternate_variants_json.clone()
        ],
    )
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Databank insert failed: {}", e),
        )
    })?;

    tx.commit().await.map_err(libsql_err)?;

    Ok((
        StatusCode::CREATED,
        Json(DataBankItem {
            id: databank_id,
            chapter_id: staging.chapter_id,
            question_text: staging.question_text,
            scoring_rubric_json: staging.scoring_rubric_json,
            alternate_variants_json: staging.alternate_variants_json,
            rating: 1500.0,
            rating_deviation: 350.0,
            volatility: 0.06,
            created_at: now,
        }),
    ))
}

// ─────────────────────────────────────────────────────────────────────────────
// DELETE /api/admin/questions/staging/:id/reject
// Soft-deletes a staged question by setting status='rejected'.
// Preserves the audit trail while removing it from the review queue.
// ─────────────────────────────────────────────────────────────────────────────

pub async fn reject_staging(
    RequireAdmin(_): RequireAdmin,
    State(db): State<DbHelper>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let conn = db.get_conn();

    let affected = conn
        .execute(
            "UPDATE question_staging_queue
             SET    status = 'rejected'
             WHERE  id     = ?1
               AND  status = 'pending_review'",
            libsql::params![id.clone()],
        )
        .await
        .map_err(libsql_err)?;

    if affected == 0 {
        return Err((
            StatusCode::NOT_FOUND,
            format!("Staging item '{}' not found or is not in pending_review state", id),
        ));
    }

    Ok(StatusCode::NO_CONTENT)
}

// ─────────────────────────────────────────────────────────────────────────────
// DELETE /api/admin/questions/databank/:id
// Hard-deletes a question from the live quiz_databank.
// ─────────────────────────────────────────────────────────────────────────────

pub async fn delete_from_databank(
    RequireAdmin(_): RequireAdmin,
    State(db): State<DbHelper>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let conn = db.get_conn();

    let affected = conn
        .execute(
            "DELETE FROM quiz_databank WHERE id = ?1",
            libsql::params![id.clone()],
        )
        .await
        .map_err(libsql_err)?;

    if affected == 0 {
        return Err((
            StatusCode::NOT_FOUND,
            format!("Databank item '{}' not found", id),
        ));
    }

    Ok(StatusCode::NO_CONTENT)
}

// ─────────────────────────────────────────────────────────────────────────────
// Private helpers
// ─────────────────────────────────────────────────────────────────────────────

async fn fetch_staging_item(
    conn: &libsql::Connection,
    id: &str,
) -> Result<StagingItem, (StatusCode, String)> {
    let mut stmt = conn
        .prepare(
            "SELECT id, chapter_id, question_text, scoring_rubric_json,
                    alternate_variants_json, status, created_at
             FROM   question_staging_queue
             WHERE  id = ?1",
        )
        .await
        .map_err(libsql_err)?;

    let mut rows = stmt
        .query(libsql::params![id.to_string()])
        .await
        .map_err(libsql_err)?;

    rows.next()
        .await
        .map_err(libsql_err)?
        .ok_or_else(|| (StatusCode::NOT_FOUND, format!("Staging item '{}' not found", id)))
        .and_then(|row| row_to_staging(&row))
}

fn row_to_staging(row: &libsql::Row) -> Result<StagingItem, (StatusCode, String)> {
    Ok(StagingItem {
        id:                      row.get(0).map_err(libsql_err)?,
        chapter_id:              row.get::<String>(1).unwrap_or_default(),
        question_text:           row.get(2).map_err(libsql_err)?,
        scoring_rubric_json:     row.get(3).map_err(libsql_err)?,
        alternate_variants_json: row.get(4).map_err(libsql_err)?,
        status:                  row.get(5).map_err(libsql_err)?,
        created_at:              row.get(6).map_err(libsql_err)?,
    })
}

fn libsql_err(e: libsql::Error) -> (StatusCode, String) {
    (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
}
