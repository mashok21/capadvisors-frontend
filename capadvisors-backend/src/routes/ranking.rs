use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use libsql::Connection;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

use crate::{
    db::DbHelper,
    utils::rating::{
        calculate_tournament_performance, Glicko2Rating,
        TournamentResult as RatingTournamentResult,
    },
};

const DEFAULT_RATING: f64 = 1500.0;
const DEFAULT_RATING_DEVIATION: f64 = 350.0;
const DEFAULT_VOLATILITY: f64 = 0.06;

#[derive(Debug, Deserialize)]
pub struct ProcessTournamentRequest {
    pub standings: Vec<TournamentStanding>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct TournamentStanding {
    pub student_id: String,
    pub display_name: Option<String>,
    pub placement: u32,
}

#[derive(Debug, Serialize)]
pub struct ProcessTournamentResponse {
    pub tournament_id: String,
    pub processed_count: usize,
    pub updates: Vec<RatingUpdateResponse>,
}

#[derive(Debug, Serialize)]
pub struct RatingUpdateResponse {
    pub student_id: String,
    pub display_name: String,
    pub placement: u32,
    pub old_rating: f64,
    pub new_rating: f64,
    pub old_rating_deviation: f64,
    pub new_rating_deviation: f64,
    pub old_volatility: f64,
    pub new_volatility: f64,
    pub games_delta: i64,
}

#[derive(Debug, Serialize)]
pub struct LeaderboardEntry {
    pub student_id: String,
    pub display_name: String,
    pub rating: f64,
    pub rating_deviation: f64,
    pub volatility: f64,
    pub games_played: i64,
    pub national_rank: i64,
    pub percentile: f64,
    pub rank_tier: String,
    pub accuracy_correct: i64,
    pub accuracy_total: i64,
    pub focus_badges: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct ActivityDay {
    pub date: String,
    pub count: i64,
}

#[derive(Debug, Clone)]
struct RatingRecord {
    student_id: String,
    display_name: String,
    placement: u32,
    rating: Glicko2Rating,
    games_played: i64,
}

#[derive(Debug, Clone)]
struct RatingUpdate {
    student_id: String,
    display_name: String,
    placement: u32,
    old_rating: Glicko2Rating,
    new_rating: Glicko2Rating,
    old_games_played: i64,
    games_delta: i64,
}

pub async fn process_tournament(
    State(db): State<DbHelper>,
    Path(tournament_id): Path<String>,
    Json(payload): Json<ProcessTournamentRequest>,
) -> impl IntoResponse {
    match process_tournament_inner(db, tournament_id, payload).await {
        Ok(response) => (StatusCode::OK, Json(response)).into_response(),
        Err((status, message)) => (status, message).into_response(),
    }
}

pub async fn get_leaderboard(State(db): State<DbHelper>) -> impl IntoResponse {
    match get_leaderboard_inner(db).await {
        Ok(entries) => (StatusCode::OK, Json(entries)).into_response(),
        Err(message) => (StatusCode::INTERNAL_SERVER_ERROR, message).into_response(),
    }
}

// GET /api/users/:student_id/activity
// Returns per-day quiz counts for the past 12 weeks (84 days), used by the
// leaderboard heatmap drawer. Public endpoint — only exposes aggregate counts.
pub async fn get_user_activity(
    Path(student_id): Path<String>,
    State(db): State<DbHelper>,
) -> impl IntoResponse {
    let conn = db.get_conn();
    let mut stmt = match conn
        .prepare(
            "SELECT quiz_date, SUM(total_count) as cnt
             FROM   quiz_activity
             WHERE  student_id = ?1
               AND  quiz_date  >= date('now', '-84 days')
             GROUP  BY quiz_date
             ORDER  BY quiz_date ASC",
        )
        .await
    {
        Ok(s) => s,
        Err(e) => {
            return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
        }
    };

    let mut rows = match stmt.query(libsql::params![student_id]).await {
        Ok(r) => r,
        Err(e) => {
            return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
        }
    };

    let mut days: Vec<ActivityDay> = Vec::new();
    while let Ok(Some(row)) = rows.next().await {
        let date: String = row.get(0).unwrap_or_default();
        let count: i64 = row.get(1).unwrap_or(0);
        days.push(ActivityDay { date, count });
    }

    Json(days).into_response()
}

async fn process_tournament_inner(
    db: DbHelper,
    tournament_id: String,
    payload: ProcessTournamentRequest,
) -> Result<ProcessTournamentResponse, (StatusCode, String)> {
    if payload.standings.len() < 2 {
        return Err((
            StatusCode::BAD_REQUEST,
            "At least two tournament standings are required".to_string(),
        ));
    }

    validate_standings(&payload.standings)?;

    let conn = db.get_conn();
    let mut records = Vec::with_capacity(payload.standings.len());
    for standing in &payload.standings {
        records.push(
            fetch_or_default_rating(&conn, standing)
                .await
                .map_err(internal_error)?,
        );
    }

    let updates = calculate_updates(&records);
    write_tournament_updates(&conn, &tournament_id, &updates)
        .await
        .map_err(internal_error)?;

    Ok(ProcessTournamentResponse {
        tournament_id,
        processed_count: updates.len(),
        updates: updates
            .into_iter()
            .map(|update| RatingUpdateResponse {
                student_id: update.student_id,
                display_name: update.display_name,
                placement: update.placement,
                old_rating: update.old_rating.rating,
                new_rating: update.new_rating.rating,
                old_rating_deviation: update.old_rating.rating_deviation,
                new_rating_deviation: update.new_rating.rating_deviation,
                old_volatility: update.old_rating.volatility,
                new_volatility: update.new_rating.volatility,
                games_delta: update.games_delta,
            })
            .collect(),
    })
}

fn validate_standings(standings: &[TournamentStanding]) -> Result<(), (StatusCode, String)> {
    let mut seen_student_ids = HashSet::new();
    for standing in standings {
        if standing.student_id.trim().is_empty() {
            return Err((
                StatusCode::BAD_REQUEST,
                "student_id cannot be empty".to_string(),
            ));
        }
        if standing.placement == 0 {
            return Err((
                StatusCode::BAD_REQUEST,
                "placement must be one-based".to_string(),
            ));
        }
        if !seen_student_ids.insert(standing.student_id.as_str()) {
            return Err((
                StatusCode::BAD_REQUEST,
                format!("duplicate student_id '{}'", standing.student_id),
            ));
        }
    }
    Ok(())
}

async fn fetch_or_default_rating(
    conn: &Connection,
    standing: &TournamentStanding,
) -> Result<RatingRecord, libsql::Error> {
    let mut stmt = conn
        .prepare(
            "SELECT display_name, rating, rating_deviation, volatility, games_played
             FROM student_ratings
             WHERE student_id = ?1",
        )
        .await?;
    let mut rows = stmt
        .query(libsql::params![standing.student_id.clone()])
        .await?;

    if let Some(row) = rows.next().await? {
        let display_name: String = row.get(0)?;
        let rating: f64 = row.get(1)?;
        let rating_deviation: f64 = row.get(2)?;
        let volatility: f64 = row.get(3)?;
        let games_played: i64 = row.get(4)?;

        return Ok(RatingRecord {
            student_id: standing.student_id.clone(),
            display_name: standing.display_name.clone().unwrap_or(display_name),
            placement: standing.placement,
            rating: Glicko2Rating {
                rating,
                rating_deviation,
                volatility,
            },
            games_played,
        });
    }

    Ok(RatingRecord {
        student_id: standing.student_id.clone(),
        display_name: standing
            .display_name
            .clone()
            .unwrap_or_else(|| standing.student_id.clone()),
        placement: standing.placement,
        rating: Glicko2Rating {
            rating: DEFAULT_RATING,
            rating_deviation: DEFAULT_RATING_DEVIATION,
            volatility: DEFAULT_VOLATILITY,
        },
        games_played: 0,
    })
}

fn calculate_updates(records: &[RatingRecord]) -> Vec<RatingUpdate> {
    records
        .iter()
        .map(|target| {
            let rating_standings = records
                .iter()
                .map(|record| RatingTournamentResult {
                    rating: record.rating,
                    placement: record.placement,
                    is_target: record.student_id == target.student_id,
                })
                .collect();
            let new_rating = calculate_tournament_performance(&target.rating, rating_standings);
            let games_delta = records
                .iter()
                .filter(|record| record.student_id != target.student_id)
                .filter(|record| record.placement != target.placement)
                .count() as i64;

            RatingUpdate {
                student_id: target.student_id.clone(),
                display_name: target.display_name.clone(),
                placement: target.placement,
                old_rating: target.rating,
                new_rating,
                old_games_played: target.games_played,
                games_delta,
            }
        })
        .collect()
}

async fn write_tournament_updates(
    conn: &Connection,
    tournament_id: &str,
    updates: &[RatingUpdate],
) -> Result<(), libsql::Error> {
    let tx = conn.transaction().await?;

    for update in updates {
        tx.execute(
            "INSERT INTO student_ratings (
                student_id,
                display_name,
                rating,
                rating_deviation,
                volatility,
                games_played,
                updated_at
             )
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, CURRENT_TIMESTAMP)
             ON CONFLICT(student_id) DO UPDATE SET
                display_name = excluded.display_name,
                rating = excluded.rating,
                rating_deviation = excluded.rating_deviation,
                volatility = excluded.volatility,
                games_played = excluded.games_played,
                updated_at = CURRENT_TIMESTAMP",
            libsql::params![
                update.student_id.clone(),
                update.display_name.clone(),
                update.new_rating.rating,
                update.new_rating.rating_deviation,
                update.new_rating.volatility,
                update.old_games_played + update.games_delta,
            ],
        )
        .await?;

        tx.execute(
            "INSERT INTO rating_history (
                id,
                tournament_id,
                student_id,
                placement,
                old_rating,
                old_rating_deviation,
                old_volatility,
                new_rating,
                new_rating_deviation,
                new_volatility,
                games_delta
             )
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            libsql::params![
                Uuid::new_v4().to_string(),
                tournament_id.to_string(),
                update.student_id.clone(),
                update.placement as i64,
                update.old_rating.rating,
                update.old_rating.rating_deviation,
                update.old_rating.volatility,
                update.new_rating.rating,
                update.new_rating.rating_deviation,
                update.new_rating.volatility,
                update.games_delta,
            ],
        )
        .await?;
    }

    tx.commit().await
}

async fn get_leaderboard_inner(db: DbHelper) -> Result<Vec<LeaderboardEntry>, String> {
    let conn = db.get_conn();
    let total_students = count_rated_students(&conn).await.map_err(|e| e.to_string())?;
    if total_students == 0 {
        return Ok(Vec::new());
    }

    let mut stmt = conn
        .prepare(
            "SELECT
                student_id,
                display_name,
                rating,
                rating_deviation,
                volatility,
                games_played,
                national_rank
             FROM (
                SELECT
                    student_id,
                    display_name,
                    rating,
                    rating_deviation,
                    volatility,
                    games_played,
                    RANK() OVER (ORDER BY rating DESC) as national_rank
                FROM student_ratings
             )
             ORDER BY rating DESC, rating_deviation ASC, student_id ASC
             LIMIT 100",
        )
        .await
        .map_err(|e| e.to_string())?;
    let mut rows = stmt.query(()).await.map_err(|e| e.to_string())?;

    let mut entries = Vec::new();
    while let Some(row) = rows.next().await.map_err(|e| e.to_string())? {
        let national_rank: i64 = row.get(6).map_err(|e| e.to_string())?;
        let percentile = if total_students == 1 {
            100.0
        } else {
            ((total_students - national_rank + 1) as f64 / total_students as f64) * 100.0
        };

        let rating: f64 = row.get(2).map_err(|e| e.to_string())?;
        entries.push(LeaderboardEntry {
            student_id: row.get(0).map_err(|e| e.to_string())?,
            display_name: row.get(1).map_err(|e| e.to_string())?,
            rating,
            rating_deviation: row.get(3).map_err(|e| e.to_string())?,
            volatility: row.get(4).map_err(|e| e.to_string())?,
            games_played: row.get(5).map_err(|e| e.to_string())?,
            national_rank,
            percentile,
            rank_tier: rank_tier_from_rating(rating),
            accuracy_correct: 0,
            accuracy_total: 0,
            focus_badges: Vec::new(),
        });
    }

    // Enrich entries with accuracy stats and focus badges from quiz_activity.
    // Both queries are cheap full-table scans — activity table is sparse at this stage.
    let accuracy_map = fetch_accuracy_map(&conn).await?;
    let badges_map = fetch_badges_map(&conn).await?;

    for entry in &mut entries {
        if let Some(&(correct, total)) = accuracy_map.get(&entry.student_id) {
            entry.accuracy_correct = correct;
            entry.accuracy_total = total;
        }
        if let Some(badges) = badges_map.get(&entry.student_id) {
            entry.focus_badges = badges.clone();
        }
    }

    Ok(entries)
}

/// Aggregates correct_count / total_count per student across all quiz_activity rows.
async fn fetch_accuracy_map(conn: &Connection) -> Result<HashMap<String, (i64, i64)>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT student_id, SUM(correct_count), SUM(total_count)
             FROM   quiz_activity
             GROUP  BY student_id",
        )
        .await
        .map_err(|e| e.to_string())?;
    let mut rows = stmt.query(()).await.map_err(|e| e.to_string())?;

    let mut map = HashMap::new();
    while let Some(row) = rows.next().await.map_err(|e| e.to_string())? {
        let sid: String = row.get(0).map_err(|e| e.to_string())?;
        let correct: i64 = row.get(1).unwrap_or(0);
        let total: i64 = row.get(2).unwrap_or(0);
        map.insert(sid, (correct, total));
    }
    Ok(map)
}

/// Returns up to 3 focus badge labels per student, ranked by total correct answers.
async fn fetch_badges_map(conn: &Connection) -> Result<HashMap<String, Vec<String>>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT qa.student_id, c.chapter_name, SUM(qa.correct_count) as score
             FROM   quiz_activity qa
             JOIN   chapters c ON c.id = qa.chapter_id
             WHERE  qa.chapter_id != ''
             GROUP  BY qa.student_id, qa.chapter_id
             ORDER  BY qa.student_id ASC, score DESC",
        )
        .await
        .map_err(|e| e.to_string())?;
    let mut rows = stmt.query(()).await.map_err(|e| e.to_string())?;

    let mut map: HashMap<String, Vec<String>> = HashMap::new();
    while let Some(row) = rows.next().await.map_err(|e| e.to_string())? {
        let sid: String = row.get(0).map_err(|e| e.to_string())?;
        let chapter_name: String = row.get(1).unwrap_or_default();
        let badges = map.entry(sid).or_default();
        if badges.len() < 3 {
            badges.push(chapter_badge(&chapter_name));
        }
    }
    Ok(map)
}

fn chapter_badge(name: &str) -> String {
    if name.contains("Financial Policy") { "Corp Strategy" }
    else if name.contains("Risk Management") { "Risk Mgmt" }
    else if name.contains("Capital Budgeting") { "Cap Budget" }
    else if name.contains("Security Analysis") { "Sec Analysis" }
    else if name.contains("Security Valuation") { "Valuation" }
    else if name.contains("Portfolio") { "Portfolio" }
    else if name.contains("Securitization") { "Securitization" }
    else if name.contains("Mutual Funds") { "Mutual Funds" }
    else if name.contains("Derivatives") { "Derivatives" }
    else if name.contains("Foreign Exchange") { "Forex" }
    else if name.contains("International") { "Intl Finance" }
    else if name.contains("Interest Rate") { "Interest Rate" }
    else if name.contains("Business Valuation") { "Biz Valuation" }
    else if name.contains("Mergers") { "M&A" }
    else if name.contains("Startup") { "Startup" }
    else { "General" }
    .to_string()
}

async fn count_rated_students(conn: &Connection) -> Result<i64, libsql::Error> {
    let mut stmt = conn.prepare("SELECT COUNT(*) FROM student_ratings").await?;
    let mut rows = stmt.query(()).await?;
    let Some(row) = rows.next().await? else {
        return Ok(0);
    };
    row.get(0)
}

fn rank_tier_from_rating(rating: f64) -> String {
    match rating {
        r if r >= 2000.0 => "Strategic Master",
        r if r >= 1700.0 => "Advanced Analyst",
        r if r >= 1400.0 => "Senior Candidate",
        _ => "Novice Practitioner",
    }
    .to_string()
}

fn internal_error(error: libsql::Error) -> (StatusCode, String) {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        format!("Ranking database operation failed: {}", error),
    )
}
