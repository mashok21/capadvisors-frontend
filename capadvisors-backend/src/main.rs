use axum::{routing::{get, post}, Router};
use std::net::SocketAddr;
use tower_http::cors::{Any, CorsLayer};

mod db;
mod routes {
    pub mod nexus;
    pub mod ranking;
}
pub mod utils;

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    // Initialize database
    let db_helper = db::DbHelper::init().await;

    let app = Router::new()
        .route("/api/hello", get(|| async { "Hello from Rust Backend!" }))
        .route("/api/db-check", get(|axum::extract::State(db_helper): axum::extract::State<db::DbHelper>| async move {
            (axum::http::StatusCode::OK, db_helper.connection_status.clone())
        }))
        .route("/api/nexus/coverage", get(routes::nexus::get_coverage))
        .route("/api/nexus/upload", axum::routing::post(routes::nexus::upload_document))
        .route("/api/nexus/chapters/{chapter_id}/questions", get(routes::nexus::get_chapter_questions))
        .route("/api/ranking/tournaments/{tournament_id}/process", post(routes::ranking::process_tournament))
        .route("/api/ranking/leaderboard", get(routes::ranking::get_leaderboard))
        .with_state(db_helper)
        .layer(cors);

    // Railway sets the PORT environment variable automatically
    let port = std::env::var("PORT")
        .unwrap_or_else(|_| "3000".to_string())
        .parse::<u16>()
        .unwrap();
    
    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    println!("Listening on {}", addr);
    axum::serve(listener, app).await.unwrap();
}
