use libsql::{Builder, Connection, Database};
use std::env;
use std::sync::Arc;

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
                email TEXT UNIQUE NOT NULL,
                password_hash TEXT NOT NULL,
                role TEXT NOT NULL DEFAULT 'student',
                created_at TEXT NOT NULL
            );",
            (),
        )
        .await?;

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

        // Schema evolution: add Gemini map-analysis columns to chapter_gap_analysis.
        // ALTER TABLE ADD COLUMN errors only when the column already exists, so
        // `.ok()` makes each migration idempotent across server restarts.
        for col_sql in [
            "ALTER TABLE chapter_gap_analysis ADD COLUMN coverage_metric            INTEGER NOT NULL DEFAULT 0",
            "ALTER TABLE chapter_gap_analysis ADD COLUMN computational_checks_json  TEXT    NOT NULL DEFAULT '[]'",
            "ALTER TABLE chapter_gap_analysis ADD COLUMN complex_exam_question      TEXT    NOT NULL DEFAULT ''",
            "ALTER TABLE chapter_gap_analysis ADD COLUMN scoring_rubric_json        TEXT    NOT NULL DEFAULT '{}'",
            "ALTER TABLE chapter_gap_analysis ADD COLUMN diagnostic_variants_json   TEXT    NOT NULL DEFAULT '[]'",
        ] {
            conn.execute(col_sql, ()).await.ok();
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
