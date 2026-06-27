use axum::{
    extract::{Multipart, State},
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
// API response
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct MapAnalysisResponse {
    pub analysis_id: String,
    pub document_id: String,
    pub chapter_id: String,
    pub chapter_name: String,
    pub chapter_code: String,
    pub total_words_analysed: usize,
    /// 0–100 integer compliance depth.
    pub coverage_metric: i64,
    /// Ordered list of verified mathematical derivations.
    pub computational_checks: Vec<String>,
    /// High-stakes ICAI-pattern case scenario derived from the document.
    pub complex_exam_question: String,
    /// Step-by-step fractional marks breakdown.
    pub scoring_rubric: ScoringRubric,
    /// Exactly 3 diagnostic permutations of the case scenario.
    pub alternate_diagnostic_variants: Vec<DiagnosticVariant>,
}

// ─────────────────────────────────────────────────────────────────────────────
// Handler: POST /api/map-document
// ─────────────────────────────────────────────────────────────────────────────

pub async fn map_document(
    State(db): State<DbHelper>,
    mut multipart: Multipart,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    // ── 1. Parse multipart fields ─────────────────────────────────────────────
    let mut chapter_id = String::new();
    let mut file_name = String::new();
    let mut file_data: Vec<u8> = Vec::new();

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| (StatusCode::BAD_REQUEST, format!("Multipart parse error: {}", e)))?
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

    // ── 2. Extract raw text as a unified, unsplit string ──────────────────────
    // The entire document is forwarded to Gemini 2.5 Pro in one request.
    // Splitting would sever multi-step formula chains (Black-Scholes,
    // Forex hedging grids, APV derivations) across chunk boundaries.
    let raw_text = extract_pdf_text(&file_name, &file_data)?;
    let total_words = word_count(&raw_text);

    if total_words < 50 {
        return Err((
            StatusCode::UNPROCESSABLE_ENTITY,
            format!(
                "Document contains only {} words — minimum 50 required for analysis",
                total_words
            ),
        ));
    }

    println!(
        "[map] Ingested '{}' — {} words forwarded as a unified context to Gemini 2.5 Pro",
        file_name, total_words
    );

    // ── 3. Resolve chapter metadata ───────────────────────────────────────────
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

    // ── 5. Full-context Gemini 2.5 Pro analysis ────────────────────────────────
    // A new reqwest::Client is created per request. The 300-second timeout is
    // set at the request level inside call_gemini_map, making the client
    // configuration here intentionally lightweight.
    let http = reqwest::Client::new();

    let result = call_gemini_map(&http, &chapter_name, &chapter_code, &raw_text)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Gemini analysis: {}", e)))?;

    // Soft validation: Gemini is instructed to return exactly 3 variants.
    // Log a telemetry trace but do not abort — the question and rubric are
    // still valid even if variant count deviates.
    if result.alternate_diagnostic_variants.len() != 3 {
        println!(
            "[map] Telemetry: Gemini returned {} diagnostic variants for '{}' (expected 3)",
            result.alternate_diagnostic_variants.len(),
            chapter_code
        );
    }

    // ── 6. Persist structured analysis result to chapter_gap_analysis ─────────
    let analysis_id = Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    // Normalise the 0–100 integer into a 0.0–1.0 REAL for the legacy column.
    let coverage_score = (result.coverage_metric as f64) / 100.0;

    conn.execute(
        "INSERT INTO chapter_gap_analysis
             (id, chapter_id, document_id, coverage_score, coverage_metric,
              gap_topics_json, compliant_topics_json, recommendations_json,
              computational_checks_json, complex_exam_question,
              scoring_rubric_json, diagnostic_variants_json, analyzed_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
        libsql::params![
            analysis_id.clone(),                                                        // ?1
            chapter_id.clone(),                                                         // ?2
            doc_id.clone(),                                                             // ?3
            coverage_score,                                                             // ?4
            result.coverage_metric,                                                     // ?5
            "[]",                                                                       // ?6  gap_topics_json
            "[]",                                                                       // ?7  compliant_topics_json
            "[]",                                                                       // ?8  recommendations_json
            serde_json::to_string(&result.computational_checks)                         // ?9
                .unwrap_or_else(|_| "[]".to_string()),
            result.complex_exam_question.clone(),                                       // ?10
            serde_json::to_string(&result.scoring_rubric_json)                          // ?11
                .unwrap_or_else(|_| "{}".to_string()),
            serde_json::to_string(&result.alternate_diagnostic_variants)                // ?12
                .unwrap_or_else(|_| "[]".to_string()),
            now                                                                         // ?13
        ],
    )
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Analysis persist: {}", e)))?;

    println!(
        "[map] analysis_id={} persisted — coverage={}% checks={} variants={}",
        analysis_id,
        result.coverage_metric,
        result.computational_checks.len(),
        result.alternate_diagnostic_variants.len()
    );

    Ok(Json(MapAnalysisResponse {
        analysis_id,
        document_id: doc_id,
        chapter_id,
        chapter_name,
        chapter_code,
        total_words_analysed: total_words,
        coverage_metric: result.coverage_metric,
        computational_checks: result.computational_checks,
        complex_exam_question: result.complex_exam_question,
        scoring_rubric: result.scoring_rubric_json,
        alternate_diagnostic_variants: result.alternate_diagnostic_variants,
    }))
}

// ─────────────────────────────────────────────────────────────────────────────
// Private helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Writes the PDF bytes to a temp file, extracts text via `pdf_extract`, then
/// removes the temp file. Non-PDF uploads are decoded as UTF-8.
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

/// Queries the `chapters` table and returns `(chapter_name, chapter_code)`.
async fn resolve_chapter(
    conn: &libsql::Connection,
    chapter_id: &str,
) -> Result<(String, String), (StatusCode, String)> {
    let mut stmt = conn
        .prepare("SELECT chapter_name, chapter_code FROM chapters WHERE id = ?1")
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let mut rows = stmt
        .query(libsql::params![chapter_id])
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    match rows
        .next()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    {
        Some(row) => {
            let name: String =
                row.get(0).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
            let code: String =
                row.get(1).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
            Ok((name, code))
        }
        None => Err((
            StatusCode::NOT_FOUND,
            format!("Chapter '{}' not found in the database", chapter_id),
        )),
    }
}
