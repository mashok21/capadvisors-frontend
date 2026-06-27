use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

use crate::{
    db::DbHelper,
    middleware::auth::AuthUser,
    utils::glicko2::compute_glicko2_update,
};

const DEFAULT_RATING: f64 = 1500.0;
const DEFAULT_RD: f64 = 350.0;
const DEFAULT_VOL: f64 = 0.06;

// ─────────────────────────────────────────────────────────────────────────────
// Request / response shapes
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct Answer {
    pub question_id: String,
    pub selected_option: String,
}

#[derive(Deserialize)]
pub struct QuizSubmitRequest {
    pub quiz_id: Option<String>,
    pub answers: Vec<Answer>,
}

#[derive(Serialize)]
pub struct QuizSubmitResponse {
    pub quiz_id: String,
    pub old_rating: f64,
    pub new_rating: f64,
    pub rating_change: f64,
    pub old_rating_deviation: f64,
    pub new_rating_deviation: f64,
    pub volatility: f64,
    pub rank_tier: String,
    pub questions_evaluated: usize,
    pub questions_skipped: usize,
}

// ─────────────────────────────────────────────────────────────────────────────
// Internal evaluation record — not part of the public API
// ─────────────────────────────────────────────────────────────────────────────

struct EvaluatedAnswer {
    question_id: String,
    chapter_id: String,
    q_rating: f64,
    q_rd: f64,
    q_vol: f64,
    score: f64, // 1.0 = student correct, 0.0 = student wrong
}

// ─────────────────────────────────────────────────────────────────────────────
// POST /api/quizzes/submit
//
// Runs the Glicko-2 match engine over each answered question:
//   - Student faces each question as an individual opponent.
//   - Score: 1.0 if correct (student wins), 0.0 if wrong (question wins).
//   - Student rating updated against all questions in one period.
//   - Each question's rating updated against the student in its own period
//     with the reversed score (question wins ↔ student fails).
// ─────────────────────────────────────────────────────────────────────────────

pub async fn submit_quiz(
    AuthUser(claims): AuthUser,
    State(db): State<DbHelper>,
    Json(payload): Json<QuizSubmitRequest>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    if payload.answers.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            "answers array cannot be empty".to_string(),
        ));
    }

    let conn = db.get_conn();
    let student_id = claims.sub.clone();
    let quiz_id = payload
        .quiz_id
        .unwrap_or_else(|| Uuid::new_v4().to_string());

    // 1. Fetch student's current rating (defaults for unrated players)
    let (old_rating, old_rd, old_vol) = fetch_student_rating(&conn, &student_id).await?;

    // 2. Evaluate each answer against the question's stored correct_answer
    let mut evaluated: Vec<EvaluatedAnswer> = Vec::new();
    let mut skipped: usize = 0;

    for answer in &payload.answers {
        let Some((q_rating, q_rd, q_vol, correct_answer, chapter_id)) =
            fetch_question(&conn, &answer.question_id).await?
        else {
            skipped += 1;
            continue;
        };

        if correct_answer.trim().is_empty() {
            // Admin hasn't set an answer key for this question yet
            skipped += 1;
            continue;
        }

        let score = if answer.selected_option.trim() == correct_answer.trim() {
            1.0
        } else {
            0.0
        };

        evaluated.push(EvaluatedAnswer {
            question_id: answer.question_id.clone(),
            chapter_id,
            q_rating,
            q_rd,
            q_vol,
            score,
        });
    }

    // If nothing was evaluatable, return current stats unchanged
    if evaluated.is_empty() {
        return Ok(Json(QuizSubmitResponse {
            quiz_id,
            old_rating,
            new_rating: old_rating,
            rating_change: 0.0,
            old_rating_deviation: old_rd,
            new_rating_deviation: old_rd,
            volatility: old_vol,
            rank_tier: rank_tier(old_rating),
            questions_evaluated: 0,
            questions_skipped: skipped,
        }));
    }

    // 3. Compute student's updated rating — all questions as opponents
    let student_matches: Vec<(f64, f64, f64)> = evaluated
        .iter()
        .map(|e| (e.q_rating, e.q_rd, e.score))
        .collect();
    let (new_rating, new_rd, new_vol) =
        compute_glicko2_update(old_rating, old_rd, old_vol, student_matches);

    // 4. Compute each question's updated rating — student is the single opponent,
    //    score reversed (question wins when student answers incorrectly)
    let question_updates: Vec<(String, f64, f64, f64)> = evaluated
        .iter()
        .map(|e| {
            let q_score = 1.0 - e.score;
            let (nr, nrd, nvol) = compute_glicko2_update(
                e.q_rating,
                e.q_rd,
                e.q_vol,
                vec![(old_rating, old_rd, q_score)],
            );
            (e.question_id.clone(), nr, nrd, nvol)
        })
        .collect();

    // 5. Persist all changes atomically
    let tx = conn
        .transaction()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    tx.execute(
        "INSERT INTO student_ratings
             (student_id, display_name, rating, rating_deviation, volatility,
              games_played, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, 1, CURRENT_TIMESTAMP)
         ON CONFLICT(student_id) DO UPDATE SET
             rating           = excluded.rating,
             rating_deviation = excluded.rating_deviation,
             volatility       = excluded.volatility,
             games_played     = games_played + 1,
             updated_at       = CURRENT_TIMESTAMP",
        libsql::params![
            student_id.clone(),
            claims.email.clone(), // display_name fallback; preserved by ON CONFLICT path
            new_rating,
            new_rd,
            new_vol,
        ],
    )
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Student rating update failed: {}", e),
        )
    })?;

    for (q_id, nr, nrd, nvol) in &question_updates {
        tx.execute(
            "UPDATE quiz_databank
             SET    rating           = ?1,
                    rating_deviation = ?2,
                    volatility       = ?3
             WHERE  id = ?4",
            libsql::params![*nr, *nrd, *nvol, q_id.clone()],
        )
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Question rating update failed for '{}': {}", q_id, e),
            )
        })?;
    }

    // Record per-chapter activity for heatmap and focus badges.
    // Aggregate correct/total counts per chapter within this quiz session.
    let mut chapter_stats: HashMap<String, (i64, i64)> = HashMap::new();
    for e in &evaluated {
        let entry = chapter_stats.entry(e.chapter_id.clone()).or_insert((0, 0));
        entry.0 += if e.score > 0.5 { 1 } else { 0 };
        entry.1 += 1;
    }
    for (chapter_id, (correct, total)) in &chapter_stats {
        let act_id = Uuid::new_v4().to_string();
        tx.execute(
            "INSERT INTO quiz_activity
                 (id, student_id, chapter_id, quiz_date, correct_count, total_count)
             VALUES (?1, ?2, ?3, date('now'), ?4, ?5)
             ON CONFLICT(student_id, chapter_id, quiz_date) DO UPDATE SET
                 correct_count = correct_count + excluded.correct_count,
                 total_count   = total_count   + excluded.total_count",
            libsql::params![
                act_id,
                student_id.clone(),
                chapter_id.clone(),
                *correct,
                *total,
            ],
        )
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Activity record failed: {}", e),
            )
        })?;
    }

    tx.commit()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(QuizSubmitResponse {
        quiz_id,
        old_rating,
        new_rating: (new_rating * 100.0).round() / 100.0,
        rating_change: ((new_rating - old_rating) * 100.0).round() / 100.0,
        old_rating_deviation: old_rd,
        new_rating_deviation: (new_rd * 100.0).round() / 100.0,
        volatility: (new_vol * 1_000_000.0).round() / 1_000_000.0,
        rank_tier: rank_tier(new_rating),
        questions_evaluated: evaluated.len(),
        questions_skipped: skipped,
    }))
}

// ─────────────────────────────────────────────────────────────────────────────
// Private helpers
// ─────────────────────────────────────────────────────────────────────────────

async fn fetch_student_rating(
    conn: &libsql::Connection,
    student_id: &str,
) -> Result<(f64, f64, f64), (StatusCode, String)> {
    let mut stmt = conn
        .prepare(
            "SELECT rating, rating_deviation, volatility
             FROM   student_ratings
             WHERE  student_id = ?1",
        )
        .await
        .map_err(ie)?;

    let mut rows = stmt
        .query(libsql::params![student_id.to_string()])
        .await
        .map_err(ie)?;

    if let Some(row) = rows.next().await.map_err(ie)? {
        Ok((
            row.get(0).map_err(ie)?,
            row.get(1).map_err(ie)?,
            row.get(2).map_err(ie)?,
        ))
    } else {
        Ok((DEFAULT_RATING, DEFAULT_RD, DEFAULT_VOL))
    }
}

/// Returns `Some((rating, rd, volatility, correct_answer, chapter_id))` or `None`
/// if the question ID doesn't exist in quiz_databank.
async fn fetch_question(
    conn: &libsql::Connection,
    question_id: &str,
) -> Result<Option<(f64, f64, f64, String, String)>, (StatusCode, String)> {
    let mut stmt = conn
        .prepare(
            "SELECT rating, rating_deviation, volatility, correct_answer,
                    COALESCE(chapter_id, '')
             FROM   quiz_databank
             WHERE  id = ?1",
        )
        .await
        .map_err(ie)?;

    let mut rows = stmt
        .query(libsql::params![question_id.to_string()])
        .await
        .map_err(ie)?;

    if let Some(row) = rows.next().await.map_err(ie)? {
        Ok(Some((
            row.get(0).map_err(ie)?,
            row.get(1).map_err(ie)?,
            row.get(2).map_err(ie)?,
            row.get::<String>(3).unwrap_or_default(),
            row.get::<String>(4).unwrap_or_default(),
        )))
    } else {
        Ok(None)
    }
}

fn rank_tier(rating: f64) -> String {
    match rating {
        r if r >= 2000.0 => "Strategic Master",
        r if r >= 1700.0 => "Advanced Analyst",
        r if r >= 1400.0 => "Senior Candidate",
        _ => "Novice Practitioner",
    }
    .to_string()
}

fn ie(e: libsql::Error) -> (StatusCode, String) {
    (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
}
