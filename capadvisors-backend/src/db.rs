use libsql::{Builder, Connection, Database};
use std::env;
use std::sync::Arc;

/// Returns `true` when `column` is already present in `table`, using
/// `PRAGMA table_info` rather than brittle error-string matching.
async fn column_exists(
    conn: &Connection,
    table: &str,
    column: &str,
) -> Result<bool, libsql::Error> {
    let mut stmt = conn
        .prepare(&format!("PRAGMA table_info({})", table))
        .await?;
    let mut rows = stmt.query(()).await?;
    // PRAGMA table_info columns: cid(0) | name(1) | type(2) | notnull(3) | dflt_value(4) | pk(5)
    while let Some(row) = rows.next().await? {
        let col_name: String = row.get(1)?;
        if col_name == column {
            return Ok(true);
        }
    }
    Ok(false)
}

#[derive(Clone)]
pub struct DbHelper {
    db: Arc<Database>,
    pub connection_status: String,
}

impl DbHelper {
    pub async fn init() -> Self {
        let mut connection_status = "Initialized".to_string();
        let turso_url = env::var("TURSO_DATABASE_URL").ok();
        let turso_token = env::var("TURSO_AUTH_TOKEN").ok();

        let db = if let (Some(url), Some(token)) = (turso_url, turso_token) {
            if !url.to_lowercase().starts_with("libsql://") {
                connection_status = "TURSO_DATABASE_URL must start with libsql://. Falling back to local SQLite.".to_string();
                println!("{}", connection_status);
                Builder::new_local("capadvisors.db").build().await.expect("Failed to connect to local SQLite db")
            } else {
                println!("Connecting to remote Turso database...");
                match Builder::new_remote(url, token).build().await {
                    Ok(database) => {
                        match database.connect() {
                            Ok(conn) => {
                                match conn.execute("SELECT 1", ()).await {
                                    Ok(_) => {
                                        connection_status = "Connected to remote Turso database successfully.".to_string();
                                        database
                                    }
                                    Err(e) => {
                                        connection_status = format!("Failed to query remote Turso db: {}. Falling back to local SQLite.", e);
                                        println!("{}", connection_status);
                                        Builder::new_local("capadvisors.db").build().await.expect("Failed to connect to local SQLite db")
                                    }
                                }
                            }
                            Err(e) => {
                                connection_status = format!("Failed to connect to remote Turso db: {}. Falling back to local SQLite.", e);
                                println!("{}", connection_status);
                                Builder::new_local("capadvisors.db").build().await.expect("Failed to connect to local SQLite db")
                            }
                        }
                    }
                    Err(e) => {
                        connection_status = format!("Failed to build remote Turso db client: {}. Falling back to local SQLite.", e);
                        println!("{}", connection_status);
                        Builder::new_local("capadvisors.db").build().await.expect("Failed to connect to local SQLite db")
                    }
                }
            }
        } else {
            connection_status = "Connecting to local SQLite database (capadvisors.db) because TURSO_DATABASE_URL and/or TURSO_AUTH_TOKEN are not set...".to_string();
            println!("{}", connection_status);
            Builder::new_local("capadvisors.db").build().await.expect("Failed to connect to local SQLite db")
        };
        
        let mut helper = DbHelper {
            db: Arc::new(db),
            connection_status,
        };
        if let Err(e) = helper.setup_schema().await {
            helper.connection_status = format!(
                "{} Schema initialization failed: {}",
                helper.connection_status, e
            );
            println!("{}", helper.connection_status);
        }
        helper
    }

    pub fn get_conn(&self) -> Connection {
        self.db.connect().expect("Failed to establish a database connection")
    }

    async fn setup_schema(&self) -> Result<(), libsql::Error> {
        let conn = self.get_conn();

        // Create Chapters Table
        conn.execute(
            "CREATE TABLE IF NOT EXISTS chapters (
                id TEXT PRIMARY KEY,
                subject_name TEXT NOT NULL,
                chapter_code TEXT NOT NULL UNIQUE,
                chapter_name TEXT NOT NULL
            );",
            (),
        )
        .await?;

        // Create Source Documents Table
        conn.execute(
            "CREATE TABLE IF NOT EXISTS source_documents (
                id TEXT PRIMARY KEY,
                file_name TEXT NOT NULL,
                upload_type TEXT CHECK(upload_type IN ('TARGETED', 'BULK')) NOT NULL,
                total_word_count INTEGER NOT NULL,
                uploaded_at TEXT DEFAULT CURRENT_TIMESTAMP
            );",
            (),
        )
        .await?;

        // Create Document Chunks Table
        conn.execute(
            "CREATE TABLE IF NOT EXISTS document_chunks (
                id TEXT PRIMARY KEY,
                document_id TEXT,
                chunk_text TEXT NOT NULL,
                word_count INTEGER NOT NULL,
                FOREIGN KEY(document_id) REFERENCES source_documents(id) ON DELETE CASCADE
            );",
            (),
        )
        .await?;

        // Create Chapter Chunk Mapping Table
        conn.execute(
            "CREATE TABLE IF NOT EXISTS chapter_chunk_mapping (
                chapter_id TEXT,
                chunk_id TEXT,
                confidence_score REAL DEFAULT 1.0,
                PRIMARY KEY (chapter_id, chunk_id),
                FOREIGN KEY(chapter_id) REFERENCES chapters(id) ON DELETE CASCADE,
                FOREIGN KEY(chunk_id) REFERENCES document_chunks(id) ON DELETE CASCADE
            );",
            (),
        )
        .await?;

        // Create Questions Table
        conn.execute(
            "CREATE TABLE IF NOT EXISTS questions (
                id TEXT PRIMARY KEY,
                chapter_id TEXT,
                difficulty TEXT,
                scenario TEXT NOT NULL,
                options_json TEXT NOT NULL,
                correct_option TEXT NOT NULL,
                explanation TEXT NOT NULL,
                FOREIGN KEY(chapter_id) REFERENCES chapters(id)
            );",
            (),
        )
        .await?;

        // Create Users Table
        conn.execute(
            "CREATE TABLE IF NOT EXISTS users (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL DEFAULT '',
                email TEXT NOT NULL UNIQUE,
                password_hash TEXT NOT NULL,
                role TEXT NOT NULL
                    CHECK(role IN ('super_admin', 'admin', 'quiz_taker'))
                    DEFAULT 'quiz_taker',
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP
            );",
            (),
        )
        .await?;

        // Migration: add name column to existing users tables created before this schema.
        if !column_exists(&conn, "users", "name").await? {
            conn.execute(
                "ALTER TABLE users ADD COLUMN name TEXT NOT NULL DEFAULT '';",
                (),
            )
            .await?;
            println!("[schema] Migration: added column 'name' to users");
        }

        // Create Student Ratings Table
        conn.execute(
            "CREATE TABLE IF NOT EXISTS student_ratings (
                student_id TEXT PRIMARY KEY,
                display_name TEXT NOT NULL,
                rating REAL NOT NULL DEFAULT 1500.0,
                rating_deviation REAL NOT NULL DEFAULT 350.0,
                volatility REAL NOT NULL DEFAULT 0.06,
                games_played INTEGER NOT NULL DEFAULT 0,
                created_at TEXT DEFAULT CURRENT_TIMESTAMP,
                updated_at TEXT DEFAULT CURRENT_TIMESTAMP
            );",
            (),
        )
        .await?;

        // ── Vector store for the Map & Analyse pipeline ──────────────────────
        // Extends document_chunks with a 768-dim F32_BLOB column.
        // Dimension MUST match `EMBEDDING_DIM` in utils/embedding.rs.
        // This table is only meaningful against Turso cloud; local SQLite
        // does not ship the libsql vector extension.
        conn.execute(
            "CREATE TABLE IF NOT EXISTS chunk_embeddings (
                chunk_id   TEXT PRIMARY KEY,
                chapter_id TEXT,
                embedding  F32_BLOB(768),
                FOREIGN KEY(chunk_id) REFERENCES document_chunks(id) ON DELETE CASCADE
            );",
            (),
        )
        .await?;

        // DiskANN vector index — enables sub-linear approximate nearest-neighbour
        // search via `vector_top_k('chunk_embeddings_vec_idx', query, k)`.
        conn.execute(
            "CREATE INDEX IF NOT EXISTS chunk_embeddings_vec_idx
             ON chunk_embeddings (libsql_vector_idx(embedding));",
            (),
        )
        .await?;

        // Persists the structured JSON output from each LLM gap-analysis run.
        conn.execute(
            "CREATE TABLE IF NOT EXISTS chapter_gap_analysis (
                id                    TEXT PRIMARY KEY,
                chapter_id            TEXT NOT NULL,
                document_id           TEXT NOT NULL,
                coverage_score        REAL NOT NULL,
                gap_topics_json       TEXT NOT NULL DEFAULT '[]',
                compliant_topics_json TEXT NOT NULL DEFAULT '[]',
                recommendations_json  TEXT NOT NULL DEFAULT '[]',
                analyzed_at           TEXT NOT NULL,
                FOREIGN KEY(chapter_id)  REFERENCES chapters(id),
                FOREIGN KEY(document_id) REFERENCES source_documents(id)
            );",
            (),
        )
        .await?;

        // Job queue for the async 202 map-document pipeline.
        conn.execute(
            "CREATE TABLE IF NOT EXISTS mapping_jobs (
                id            TEXT PRIMARY KEY,
                chapter_id    TEXT NOT NULL,
                document_id   TEXT NOT NULL,
                status        TEXT NOT NULL DEFAULT 'pending'
                              CHECK(status IN ('pending', 'completed', 'failed')),
                analysis_id   TEXT,
                error_message TEXT,
                created_at    TEXT NOT NULL
            );",
            (),
        )
        .await?;

        // Schema evolution: add Gemini map-analysis columns to chapter_gap_analysis.
        // Uses PRAGMA table_info to check existence before issuing ALTER TABLE so that
        // restarts are idempotent without relying on fragile error-message strings.
        for (col_name, col_def) in [
            ("coverage_metric",           "INTEGER NOT NULL DEFAULT 0"),
            ("computational_checks_json", "TEXT    NOT NULL DEFAULT '[]'"),
            ("complex_exam_question",     "TEXT    NOT NULL DEFAULT ''"),
            ("scoring_rubric_json",       "TEXT    NOT NULL DEFAULT '{}'"),
            ("diagnostic_variants_json",  "TEXT    NOT NULL DEFAULT '[]'"),
        ] {
            if column_exists(&conn, "chapter_gap_analysis", col_name).await? {
                continue; // Already present — restart-safe skip.
            }
            let sql = format!(
                "ALTER TABLE chapter_gap_analysis ADD COLUMN {} {};",
                col_name, col_def
            );
            conn.execute(&sql, ()).await?;
            println!("[schema] Migration: added column '{}'", col_name);
        }

        // Create Rating History Table
        conn.execute(
            "CREATE TABLE IF NOT EXISTS rating_history (
                id TEXT PRIMARY KEY,
                tournament_id TEXT NOT NULL,
                student_id TEXT NOT NULL,
                placement INTEGER NOT NULL,
                old_rating REAL NOT NULL,
                old_rating_deviation REAL NOT NULL,
                old_volatility REAL NOT NULL,
                new_rating REAL NOT NULL,
                new_rating_deviation REAL NOT NULL,
                new_volatility REAL NOT NULL,
                games_delta INTEGER NOT NULL,
                created_at TEXT DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY(student_id) REFERENCES student_ratings(student_id)
            );",
            (),
        )
        .await?;

        // Live question pool — questions graduate here after admin approval.
        // Glicko-2 fields track per-question difficulty via student interaction.
        conn.execute(
            "CREATE TABLE IF NOT EXISTS quiz_databank (
                id                      TEXT PRIMARY KEY,
                chapter_id              TEXT,
                question_text           TEXT NOT NULL,
                scoring_rubric_json     TEXT NOT NULL DEFAULT '{}',
                alternate_variants_json TEXT NOT NULL DEFAULT '[]',
                rating                  REAL NOT NULL DEFAULT 1500.0,
                rating_deviation        REAL NOT NULL DEFAULT 350.0,
                volatility              REAL NOT NULL DEFAULT 0.06,
                created_at              DATETIME DEFAULT CURRENT_TIMESTAMP
            );",
            (),
        )
        .await?;

        // Staging queue — raw questions from the mapping pipeline await admin
        // review before promotion into quiz_databank.
        conn.execute(
            "CREATE TABLE IF NOT EXISTS question_staging_queue (
                id                      TEXT PRIMARY KEY,
                chapter_id              TEXT,
                question_text           TEXT NOT NULL,
                scoring_rubric_json     TEXT NOT NULL DEFAULT '{}',
                alternate_variants_json TEXT NOT NULL DEFAULT '[]',
                status                  TEXT NOT NULL DEFAULT 'pending_review'
                                        CHECK(status IN ('pending_review', 'approved', 'rejected')),
                created_at              DATETIME DEFAULT CURRENT_TIMESTAMP
            );",
            (),
        )
        .await?;

        // Add correct_answer to staging and databank so quiz submission can
        // evaluate answers without re-running the mapping pipeline.
        for (table, col, def) in [
            ("question_staging_queue", "correct_answer", "TEXT NOT NULL DEFAULT ''"),
            ("quiz_databank",          "correct_answer", "TEXT NOT NULL DEFAULT ''"),
        ] {
            if !column_exists(&conn, table, col).await? {
                conn.execute(
                    &format!("ALTER TABLE {} ADD COLUMN {} {};", table, col, def),
                    (),
                )
                .await?;
                println!("[schema] Migration: added column '{}' to {}", col, table);
            }
        }

        // Per-day, per-chapter activity log used by the leaderboard heatmap and
        // focus badge computation. UNIQUE on (student_id, chapter_id, quiz_date)
        // lets the quiz submission UPSERT accumulate counts idempotently.
        conn.execute(
            "CREATE TABLE IF NOT EXISTS quiz_activity (
                id            TEXT PRIMARY KEY,
                student_id    TEXT NOT NULL,
                chapter_id    TEXT NOT NULL DEFAULT '',
                quiz_date     TEXT NOT NULL,
                correct_count INTEGER NOT NULL DEFAULT 0,
                total_count   INTEGER NOT NULL DEFAULT 0,
                UNIQUE(student_id, chapter_id, quiz_date)
            );",
            (),
        )
        .await?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS quiz_activity_student_idx
             ON quiz_activity (student_id, quiz_date);",
            (),
        )
        .await?;

        // Seed Chapters
        self.seed_chapters(&conn).await?;
        Ok(())
    }

    async fn seed_chapters(&self, conn: &Connection) -> Result<(), libsql::Error> {
        let chapters = vec![
            ("AFM-CH01", "Financial Policy and Corporate Strategy"),
            ("AFM-CH02", "Risk Management"),
            ("AFM-CH03", "Advanced Capital Budgeting Decisions"),
            ("AFM-CH04", "Security Analysis"),
            ("AFM-CH05", "Security Valuation"),
            ("AFM-CH06", "Portfolio Management"),
            ("AFM-CH07", "Securitization"),
            ("AFM-CH08", "Mutual Funds"),
            ("AFM-CH09", "Derivatives Analysis and Valuation"),
            ("AFM-CH10", "Foreign Exchange Exposure and Risk Management"),
            ("AFM-CH11", "International Financial Management"),
            ("AFM-CH12", "Interest Rate Risk Management"),
            ("AFM-CH13", "Business Valuation"),
            ("AFM-CH14", "Mergers, Acquisitions and Corporate Restructuring"),
            ("AFM-CH15", "Startup Finance"),
        ];

        for (code, name) in chapters {
            // Check if chapter code already exists
            let mut stmt = conn
                .prepare("SELECT 1 FROM chapters WHERE chapter_code = ?1")
                .await?;
            
            let mut rows = stmt.query(libsql::params![code]).await?;
            
            if rows.next().await?.is_none() {
                let id = uuid::Uuid::new_v4().to_string();
                conn.execute(
                    "INSERT INTO chapters (id, subject_name, chapter_code, chapter_name) 
                     VALUES (?1, 'Advanced Financial Management', ?2, ?3)",
                    libsql::params![id, code, name],
                )
                .await?;
            }
        }
        println!("Chapters seeding complete.");
        Ok(())
    }
}
