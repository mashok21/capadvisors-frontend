use axum::{
    extract::{Multipart, Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::Serialize;
use uuid::Uuid;

use crate::{
    db::DbHelper,
    utils::{
        gemini_map::{call_gemini_map, DiagnosticVariant, ScoringRubric},
        text::word_count,
    },
};

// ─────────────────────────────────────────────────────────────────────────────
// Response types
// ─────────────────────────────────────────────────────────────────────────────

/// Returned immediately (202) so the HTTP thread is freed within milliseconds.
#[derive(Serialize)]
pub struct JobSubmitResponse {
    pub job_id: String,
    pub document_id: String,
    /// Always "pending" on first response.
    pub status: &'static str,
    /// URL the client should poll to check completion.
    pub poll_url: String,
}

#[derive(Serialize)]
pub struct JobStatusResponse {
    pub job_id: String,
    /// "pending" | "completed" | "failed"
    pub status: String,
    pub error_message: Option<String>,
    /// Populated only when status == "completed".
    pub result: Option<MapAnalysisResult>,
}

#[derive(Serialize)]
pub struct MapAnalysisResult {
    pub analysis_id: String,
    pub document_id: String,
    pub chapter_id: String,
    pub coverage_metric: i64,
    pub computational_checks: Vec<String>,
    pub complex_exam_question: String,
    pub scoring_rubric: ScoringRubric,
    pub alternate_diagnostic_variants: Vec<DiagnosticVariant>,
}

// ─────────────────────────────────────────────────────────────────────────────
// Handler: POST /api/map-document  →  202 Accepted
// ─────────────────────────────────────────────────────────────────────────────
// Gemini 2.5 Pro on a full AFM chapter can take 60–180 seconds.
// Railway terminates connections at 60 s, so we dispatch immediately and let
// the client poll GET /api/map-document/jobs/{id} for the result.

pub async fn map_document(
    State(db): State<DbHelper>,
    mut multipart: Multipart,
) -> Result<(StatusCode, Json<JobSubmitResponse>), (StatusCode, String)> {
    // ── 1. Parse multipart ────────────────────────────────────────────────────
    let mut chapter_id = String::new();
    let mut file_name = String::new();
    let mut file_data: Vec<u8> = Vec::new();

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| (StatusCode::BAD_REQUEST, format!("Multipart error: {}", e)))?
    {
        match field.name().unwrap_or("") {
            "chapter_id" => {
                chapter_id = field
                    .text()
                    .await
                    .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
            }
            "file" => {
                file_name = field.file_name().unwrap_or("document.pdf").to_string();
                file_data = field
                    .bytes()
                    .await
                    .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?
                    .to_vec();
            }
            _ => {}
        }
    }

    if chapter_id.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "chapter_id field is required".to_string()));
    }
    if file_data.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "file field is missing or empty".to_string()));
    }

    // ── 2. Extract raw text (synchronous; fast — done inline before dispatch) ─
    let raw_text = extract_pdf_text(&file_name, &file_data)?;
    let total_words = word_count(&raw_text);

    if total_words < 50 {
        return Err((
            StatusCode::UNPROCESSABLE_ENTITY,
            format!("Document has only {} words — 50 minimum required", total_words),
        ));
    }

    // ── 3. Validate chapter exists before queuing the job ─────────────────────
    let conn = db.get_conn();
    let (chapter_name, chapter_code) = resolve_chapter(&conn, &chapter_id).await?;

    // ── 4. Persist source document record ─────────────────────────────────────
    let doc_id = Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO source_documents (id, file_name, upload_type, total_word_count)
         VALUES (?1, ?2, 'TARGETED', ?3)",
        libsql::params![doc_id.clone(), file_name.clone(), total_words as i64],
    )
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Source document insert: {}", e)))?;

    // ── 5. Create mapping_jobs record with status = 'pending' ─────────────────
    let job_id = Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO mapping_jobs (id, chapter_id, document_id, status, created_at)
         VALUES (?1, ?2, ?3, 'pending', ?4)",
        libsql::params![
            job_id.clone(),
            chapter_id.clone(),
            doc_id.clone(),
            now
        ],
    )
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Job record insert: {}", e)))?;

    println!(
        "[map] job_id={} dispatched — {} words, chapter '{}' ({} words)",
        job_id, total_words, chapter_code, total_words
    );

    // ── 6. Spawn background worker — HTTP thread returns in microseconds ───────
    let db_bg       = db.clone();
    let job_id_bg   = job_id.clone();
    let doc_id_bg   = doc_id.clone();

    tokio::spawn(async move {
        run_mapping_job(
            db_bg,
            job_id_bg,
            chapter_id,
            chapter_name,
            chapter_code,
            raw_text,
            doc_id_bg,
        )
        .await;
    });

    // ── 7. 202 Accepted — client polls the job endpoint for completion ─────────
    Ok((
        StatusCode::ACCEPTED,
        Json(JobSubmitResponse {
            poll_url: format!("/api/map-document/jobs/{}", job_id),
            job_id,
            document_id: doc_id,
            status: "pending",
        }),
    ))
}

// ─────────────────────────────────────────────────────────────────────────────
// Handler: GET /api/map-document/jobs/{job_id}
// ─────────────────────────────────────────────────────────────────────────────

pub async fn get_job_status(
    State(db): State<DbHelper>,
    Path(job_id): Path<String>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let conn = db.get_conn();

    let mut stmt = conn
        .prepare(
            "SELECT status, error_message, analysis_id
             FROM   mapping_jobs
             WHERE  id = ?1",
        )
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let mut rows = stmt
        .query(libsql::params![job_id.clone()])
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let row = rows
        .next()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or_else(|| (StatusCode::NOT_FOUND, format!("Job '{}' not found", job_id)))?;

    let status: String = row
        .get(0)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // NULL columns return Err from get::<String>; treat that as None.
    let error_message: Option<String> = row.get::<String>(1).ok();
    let analysis_id: Option<String>   = row.get::<String>(2).ok();

    let result = if status == "completed" {
        match analysis_id {
            Some(aid) => fetch_analysis_result(&conn, &aid)
                .await
                .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?
                .into(),
            None => None,
        }
    } else {
        None
    };

    Ok(Json(JobStatusResponse {
        job_id,
        status,
        error_message,
        result,
    }))
}

// ─────────────────────────────────────────────────────────────────────────────
// Background worker
// ─────────────────────────────────────────────────────────────────────────────

async fn run_mapping_job(
    db: DbHelper,
    job_id: String,
    chapter_id: String,
    chapter_name: String,
    chapter_code: String,
    raw_text: String,
    doc_id: String,
) {
    let conn = db.get_conn();
    let http = reqwest::Client::new();

    match call_gemini_map(&http, &chapter_name, &chapter_code, &raw_text).await {
        Ok(result) => {
            let analysis_id   = Uuid::new_v4().to_string();
            let now           = chrono::Utc::now().to_rfc3339();
            let coverage_score = (result.coverage_metric as f64) / 100.0;

            let persist = conn
                .execute(
                    "INSERT INTO chapter_gap_analysis
                         (id, chapter_id, document_id, coverage_score, coverage_metric,
                          gap_topics_json, compliant_topics_json, recommendations_json,
                          computational_checks_json, complex_exam_question,
                          scoring_rubric_json, diagnostic_variants_json, analyzed_at)
                     VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13)",
                    libsql::params![
                        analysis_id.clone(),
                        chapter_id.clone(),
                        doc_id.clone(),
                        coverage_score,
                        result.coverage_metric,
                        "[]",
                        "[]",
                        "[]",
                        serde_json::to_string(&result.computational_checks)
                            .unwrap_or_default(),
                        result.complex_exam_question.clone(),
                        serde_json::to_string(&result.scoring_rubric_json)
                            .unwrap_or_default(),
                        serde_json::to_string(&result.alternate_diagnostic_variants)
                            .unwrap_or_default(),
                        now
                    ],
                )
                .await;

            match persist {
                Ok(_) => {
                    mark_job_completed(&conn, &job_id, &analysis_id).await;
                    println!(
                        "[map] job_id={} completed — analysis_id={} coverage={}%",
                        job_id, analysis_id, result.coverage_metric
                    );
                }
                Err(e) => {
                    let msg = format!("Analysis persist failed: {}", e);
                    eprintln!("[map] job_id={} — {}", job_id, msg);
                    mark_job_failed(&conn, &job_id, &msg).await;
                }
            }
        }
        Err(e) => {
            eprintln!("[map] job_id={} — Gemini analysis failed: {}", job_id, e);
            mark_job_failed(&conn, &job_id, &e).await;
        }
    }
}

async fn mark_job_completed(conn: &libsql::Connection, job_id: &str, analysis_id: &str) {
    if let Err(e) = conn
        .execute(
            "UPDATE mapping_jobs SET status = 'completed', analysis_id = ?1 WHERE id = ?2",
            libsql::params![analysis_id.to_string(), job_id.to_string()],
        )
        .await
    {
        eprintln!("[map] Failed to mark job {} completed: {}", job_id, e);
    }
}

async fn mark_job_failed(conn: &libsql::Connection, job_id: &str, error: &str) {
    if let Err(e) = conn
        .execute(
            "UPDATE mapping_jobs SET status = 'failed', error_message = ?1 WHERE id = ?2",
            libsql::params![error.to_string(), job_id.to_string()],
        )
        .await
    {
        eprintln!("[map] Failed to mark job {} failed: {}", job_id, e);
    }
}

async fn fetch_analysis_result(
    conn: &libsql::Connection,
    analysis_id: &str,
) -> Result<Option<MapAnalysisResult>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, chapter_id, document_id, coverage_metric,
                    computational_checks_json, complex_exam_question,
                    scoring_rubric_json, diagnostic_variants_json
             FROM   chapter_gap_analysis
             WHERE  id = ?1",
        )
        .await
        .map_err(|e| e.to_string())?;

    let mut rows = stmt
        .query(libsql::params![analysis_id.to_string()])
        .await
        .map_err(|e| e.to_string())?;

    let row = match rows.next().await.map_err(|e| e.to_string())? {
        Some(r) => r,
        None => return Ok(None),
    };

    let id: String            = row.get(0).map_err(|e| e.to_string())?;
    let chapter_id: String    = row.get(1).map_err(|e| e.to_string())?;
    let document_id: String   = row.get(2).map_err(|e| e.to_string())?;
    let coverage_metric: i64  = row.get(3).map_err(|e| e.to_string())?;
    let checks_json: String   = row.get(4).map_err(|e| e.to_string())?;
    let question: String      = row.get(5).map_err(|e| e.to_string())?;
    let rubric_json: String   = row.get(6).map_err(|e| e.to_string())?;
    let variants_json: String = row.get(7).map_err(|e| e.to_string())?;

    let computational_checks: Vec<String> =
        serde_json::from_str(&checks_json).map_err(|e| format!("checks JSON: {}", e))?;
    let scoring_rubric: ScoringRubric =
        serde_json::from_str(&rubric_json).map_err(|e| format!("rubric JSON: {}", e))?;
    let alternate_diagnostic_variants: Vec<DiagnosticVariant> =
        serde_json::from_str(&variants_json).map_err(|e| format!("variants JSON: {}", e))?;

    Ok(Some(MapAnalysisResult {
        analysis_id: id,
        document_id,
        chapter_id,
        coverage_metric,
        computational_checks,
        complex_exam_question: question,
        scoring_rubric,
        alternate_diagnostic_variants,
    }))
}

// ─────────────────────────────────────────────────────────────────────────────
// Private helpers
// ─────────────────────────────────────────────────────────────────────────────

fn extract_pdf_text(file_name: &str, data: &[u8]) -> Result<String, (StatusCode, String)> {
    if !file_name.to_lowercase().ends_with(".pdf") {
        return Ok(String::from_utf8_lossy(data).into_owned());
    }
    let temp_path = format!("map_tmp_{}.pdf", Uuid::new_v4());
    std::fs::write(&temp_path, data)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Temp file write: {}", e)))?;
    let text = pdf_extract::extract_text(&temp_path).map_err(|e| {
        std::fs::remove_file(&temp_path).ok();
        (StatusCode::UNPROCESSABLE_ENTITY, format!("PDF parsing failed: {}", e))
    })?;
    std::fs::remove_file(&temp_path).ok();
    Ok(text)
}

async fn resolve_chapter(
    conn: &libsql::Connection,
    chapter_id: &str,
) -> Result<(String, String), (StatusCode, String)> {
    let mut stmt = conn
        .prepare("SELECT chapter_name, chapter_code FROM chapters WHERE id = ?1")
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let mut rows = stmt
        .query(libsql::params![chapter_id.to_string()])
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    match rows
        .next()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    {
        Some(row) => {
            let name: String = row.get(0).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
            let code: String = row.get(1).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
            Ok((name, code))
        }
        None => Err((
            StatusCode::NOT_FOUND,
            format!("Chapter '{}' not found", chapter_id),
        )),
    }
}
