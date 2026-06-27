use axum::{
    extract::{DefaultBodyLimit, FromRef},
    routing::{delete, get, post, put},
    Router,
};
use std::{net::SocketAddr, sync::Arc};
use tokio::sync::Semaphore;
use tower_http::cors::{Any, CorsLayer};

mod db;
mod middleware {
    pub mod auth;
}
mod routes {
    pub mod auth;
    pub mod map;
    pub mod nexus;
    pub mod quiz;
    pub mod ranking;
}
pub mod utils;

/// Shared application state threaded through every Axum handler.
/// All routes that only need the database still declare `State<DbHelper>` —
/// Axum resolves that via the `FromRef` impl below without any code changes
/// to auth / nexus / ranking routes.
#[derive(Clone)]
pub struct AppState {
    pub db: db::DbHelper,
    /// Limits concurrent Gemini mapping jobs to prevent Railway 60-s timeouts
    /// from cascading into an unbounded queue of long-running tasks.
    pub semaphore: Arc<Semaphore>,
}

impl FromRef<AppState> for db::DbHelper {
    fn from_ref(state: &AppState) -> db::DbHelper {
        state.db.clone()
    }
}

impl FromRef<AppState> for Arc<Semaphore> {
    fn from_ref(state: &AppState) -> Arc<Semaphore> {
        state.semaphore.clone()
    }
}

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app_state = AppState {
        db: db::DbHelper::init().await,
        semaphore: Arc::new(Semaphore::new(3)),
    };

    let app = Router::new()
        .route("/api/hello", get(|| async { "Hello from Rust Backend!" }))
        .route(
            "/api/db-check",
            get(
                |axum::extract::State(db): axum::extract::State<db::DbHelper>| async move {
                    (axum::http::StatusCode::OK, db.connection_status.clone())
                },
            ),
        )
        .route("/api/nexus/coverage", get(routes::nexus::get_coverage))
        .route(
            "/api/nexus/upload",
            axum::routing::post(routes::nexus::upload_document),
        )
        .route(
            "/api/nexus/chapters/{chapter_id}/questions",
            get(routes::nexus::get_chapter_questions),
        )
        .route(
            "/api/ranking/tournaments/{tournament_id}/process",
            post(routes::ranking::process_tournament),
        )
        .route("/api/ranking/leaderboard", get(routes::ranking::get_leaderboard))
        .route("/api/leaderboard", get(routes::ranking::get_leaderboard))
        .route("/api/auth/register", post(routes::auth::register))
        .route("/api/auth/login", post(routes::auth::login))
        .route("/api/auth/admin/create", post(routes::auth::create_admin))
        .route("/api/admin/questions/staging", get(routes::quiz::list_staging))
        .route("/api/admin/questions/staging/{id}", put(routes::quiz::edit_staging))
        .route(
            "/api/admin/questions/staging/{id}/approve",
            post(routes::quiz::approve_staging),
        )
        .route(
            "/api/admin/questions/staging/{id}/improvise",
            post(routes::quiz::improvise_staging),
        )
        .route(
            "/api/admin/questions/staging/{id}/reject",
            delete(routes::quiz::reject_staging),
        )
        .route(
            "/api/admin/questions/databank/{id}",
            delete(routes::quiz::delete_from_databank),
        )
        .route("/api/map-document", post(routes::map::map_document))
        .route(
            "/api/map-document/jobs/{job_id}",
            get(routes::map::get_job_status),
        )
        .with_state(app_state)
        .layer(DefaultBodyLimit::max(35 * 1024 * 1024))
        .layer(cors);

    let port = std::env::var("PORT")
        .unwrap_or_else(|_| "3000".to_string())
        .parse::<u16>()
        .unwrap();

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    println!("Listening on {}", addr);
    axum::serve(listener, app).await.unwrap();
}
