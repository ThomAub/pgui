//! Unified SQLite storage for the application.

mod connections;
mod history;
mod storage_connections;
mod types;

pub use connections::ConnectionsRepository;
pub use history::QueryHistoryRepository;
pub use storage_connections::StorageConnectionsRepository;
pub use types::*;

use anyhow::Result;
use async_lock::OnceCell;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePool, SqlitePoolOptions};
use std::path::PathBuf;
use std::str::FromStr;

/// Shared application storage backed by SQLite.
#[derive(Debug, Clone)]
pub struct AppStore {
    pool: SqlitePool,
}

/// Global singleton instance
static STORE: OnceCell<AppStore> = OnceCell::new();

impl AppStore {
    /// Get or initialize the global AppStore singleton.
    /// Schema initialization and migration only run once.
    pub async fn singleton() -> Result<&'static Self> {
        STORE.get_or_try_init(|| Self::init()).await
    }

    pub async fn init() -> Result<Self> {
        let db_path = Self::get_db_path()?;
        Self::from_path(db_path).await
    }

    async fn from_path(db_path: PathBuf) -> Result<Self> {
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let options = SqliteConnectOptions::from_str(&format!("sqlite://{}", db_path.display()))?
            .create_if_missing(true);

        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect_with(options)
            .await?;

        let store = Self { pool };
        store.initialize_schema().await?;
        store.migrate_schema().await?;
        Ok(store)
    }

    fn get_db_path() -> Result<PathBuf> {
        let home =
            dirs::home_dir().ok_or_else(|| anyhow::anyhow!("Could not find home directory"))?;
        Ok(home.join(".pgui").join("pgui.db")) // Renamed to be more generic
    }

    /// Get a connections repository
    pub fn connections(&self) -> ConnectionsRepository {
        ConnectionsRepository::new(self.pool.clone())
    }

    /// Get a query history repository
    #[allow(dead_code)]
    pub fn history(&self) -> QueryHistoryRepository {
        QueryHistoryRepository::new(self.pool.clone())
    }

    /// Get a storage connections repository
    pub fn storage_connections(&self) -> StorageConnectionsRepository {
        StorageConnectionsRepository::new(self.pool.clone())
    }

    /// Initialize the database schema
    async fn initialize_schema(&self) -> Result<()> {
        sqlx::query(
            r#"
                CREATE TABLE IF NOT EXISTS connections (
                    id TEXT PRIMARY KEY,
                    name TEXT NOT NULL UNIQUE,
                    database_type TEXT NOT NULL DEFAULT 'postgresql',
                    hostname TEXT NOT NULL DEFAULT '',
                    username TEXT NOT NULL DEFAULT '',
                    database TEXT NOT NULL DEFAULT '',
                    port INTEGER NOT NULL DEFAULT 5432,
                    ssl_mode TEXT NOT NULL DEFAULT 'prefer',
                    file_path TEXT,
                    read_only INTEGER NOT NULL DEFAULT 0,
                    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
                )
                "#,
        )
        .execute(&self.pool)
        .await?;

        // Create index on name for faster lookups
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_connections_name ON connections(name)")
            .execute(&self.pool)
            .await?;

        // Query history table
        sqlx::query(
            r#"
                CREATE TABLE IF NOT EXISTS query_history (
                    id TEXT PRIMARY KEY,
                    connection_id TEXT NOT NULL,
                    sql TEXT NOT NULL,
                    execution_time_ms INTEGER NOT NULL,
                    rows_affected INTEGER,
                    success INTEGER NOT NULL,
                    error_message TEXT,
                    executed_at TIMESTAMP NOT NULL,
                    FOREIGN KEY (connection_id) REFERENCES connections(id) ON DELETE CASCADE
                )
                "#,
        )
        .execute(&self.pool)
        .await?;

        // Index for fast lookups by connection
        sqlx::query(
                "CREATE INDEX IF NOT EXISTS idx_history_connection ON query_history(connection_id, executed_at DESC)"
            )
            .execute(&self.pool)
            .await?;

        // Storage connections table (for S3, GCS, Azure, etc.)
        sqlx::query(
            r#"
                CREATE TABLE IF NOT EXISTS storage_connections (
                    id TEXT PRIMARY KEY,
                    name TEXT NOT NULL UNIQUE,
                    storage_type TEXT NOT NULL DEFAULT 's3',
                    endpoint TEXT,
                    region TEXT NOT NULL DEFAULT '',
                    bucket TEXT NOT NULL DEFAULT '',
                    access_key_id TEXT,
                    path_style INTEGER NOT NULL DEFAULT 0,
                    allow_anonymous INTEGER NOT NULL DEFAULT 0,
                    root_path TEXT,
                    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
                )
                "#,
        )
        .execute(&self.pool)
        .await?;

        // Index for storage connections by name
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_storage_connections_name ON storage_connections(name)")
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    /// Migrate schema for existing databases
    async fn migrate_schema(&self) -> Result<()> {
        // Migration: Add ssl_mode column (v0.1.x -> v0.1.y)
        self.migrate_add_column("ssl_mode", "TEXT NOT NULL DEFAULT 'prefer'")
            .await;

        // Migration: Add database_type column (for multi-database support)
        self.migrate_add_column("database_type", "TEXT NOT NULL DEFAULT 'postgresql'")
            .await;

        // Migration: Add file_path column (for file-based databases)
        self.migrate_add_column("file_path", "TEXT").await;

        // Migration: Add read_only column (for file-based databases)
        self.migrate_add_column("read_only", "INTEGER NOT NULL DEFAULT 0")
            .await;

        Ok(())
    }

    /// Helper to add a column if it doesn't exist
    async fn migrate_add_column(&self, column_name: &str, column_def: &str) {
        // Check if column exists
        let check_query = format!("SELECT {} FROM connections LIMIT 1", column_name);
        let column_exists = sqlx::query(&check_query)
            .fetch_optional(&self.pool)
            .await
            .is_ok();

        if !column_exists {
            tracing::debug!("Migration: {} column not found, adding it...", column_name);

            let alter_query = format!(
                "ALTER TABLE connections ADD COLUMN {} {}",
                column_name, column_def
            );

            match sqlx::query(&alter_query).execute(&self.pool).await {
                Ok(_) => {
                    tracing::debug!("Migration: Successfully added {} column", column_name);
                }
                Err(e) => {
                    // If column already exists, SQLite will error - that's okay
                    tracing::warn!(
                        "Migration: Column {} may already exist: {}",
                        column_name,
                        e
                    );
                }
            }
        } else {
            tracing::debug!("Migration: {} column already exists", column_name);
        }
    }
}
