//! SQLite connection implementation.
//!
//! This module implements the `DatabaseConnection` trait for SQLite
//! using SQLx's SqlitePool.

use anyhow::{anyhow, Result};
use async_lock::RwLock;
use async_trait::async_trait;
use futures::stream::BoxStream;
use futures::StreamExt;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;
use std::str::FromStr;
use std::time::Duration;

use super::types::SqliteValueConverter;
use crate::services::database::traits::{
    BoxedConnection, ColumnInfo, ConnectionConfig, ConnectionParams, DatabaseConnection,
    DatabaseType, ErrorResult, ModifiedResult, QueryExecutionResult, Row, SelectResult,
};

/// SQLite database connection.
///
/// This struct wraps a SQLx SqlitePool and implements the `DatabaseConnection` trait.
/// SQLite supports both file-based and in-memory databases.
pub struct SqliteConnection {
    config: ConnectionConfig,
    pool: RwLock<Option<SqlitePool>>,
}

impl std::fmt::Debug for SqliteConnection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SqliteConnection")
            .field("config", &self.config)
            .field("pool", &"<SqlitePool>")
            .finish()
    }
}

impl SqliteConnection {
    /// Create a new SQLite connection from configuration.
    ///
    /// This does not connect immediately - call `connect()` to establish the connection.
    pub fn new(config: ConnectionConfig) -> Self {
        Self {
            config,
            pool: RwLock::new(None),
        }
    }

    /// Create a boxed connection (for factory use).
    pub fn boxed(config: ConnectionConfig) -> BoxedConnection {
        Box::new(Self::new(config))
    }

    /// Build SqliteConnectOptions from the configuration.
    fn build_connect_options(&self) -> Result<SqliteConnectOptions> {
        match &self.config.params {
            ConnectionParams::File {
                path,
                read_only,
                ..
            } => {
                let mut options = SqliteConnectOptions::new()
                    .filename(path)
                    .create_if_missing(!read_only);

                if *read_only {
                    options = options.read_only(true);
                }

                // Enable foreign keys by default
                options = options.foreign_keys(true);

                // Enable WAL mode for better concurrency (unless read-only)
                if !read_only {
                    options = options.journal_mode(sqlx::sqlite::SqliteJournalMode::Wal);
                }

                Ok(options)
            }
            ConnectionParams::InMemory { .. } => {
                // In-memory database with shared cache for connection pooling
                let options = SqliteConnectOptions::from_str(":memory:")?
                    .foreign_keys(true)
                    .shared_cache(true);

                Ok(options)
            }
            ConnectionParams::Server { .. } => Err(anyhow!(
                "SQLite does not support server-based connections. Use File or InMemory params."
            )),
        }
    }

    /// Get a reference to the connection pool.
    ///
    /// Returns an error if not connected.
    async fn get_pool(&self) -> Result<SqlitePool> {
        let guard = self.pool.read().await;
        guard
            .as_ref()
            .cloned()
            .ok_or_else(|| anyhow!("Database not connected"))
    }

    /// Get a reference to the pool (internal helper for schema module).
    pub(crate) async fn get_pool_internal(&self) -> Result<SqlitePool> {
        self.get_pool().await
    }

    /// Check if the query is a SELECT-type query.
    fn is_select_query(sql: &str) -> bool {
        let lower = sql.to_lowercase();
        let trimmed = lower.trim_start();
        trimmed.starts_with("select")
            || trimmed.starts_with("with")
            || trimmed.starts_with("pragma")
    }

    /// Execute a SELECT query.
    async fn execute_select(&self, sql: &str, pool: &SqlitePool) -> QueryExecutionResult {
        let start_time = std::time::Instant::now();
        let original_query = sql.to_string();

        // Add LIMIT if not present to prevent massive result sets
        // (unless it's a PRAGMA query)
        let limited_sql = if !sql.to_lowercase().contains(" limit ")
            && !sql.to_lowercase().trim_start().starts_with("pragma")
        {
            format!("{} LIMIT {}", sql.trim_end_matches(';'), 1_000)
        } else {
            sql.to_string()
        };

        match sqlx::query(&limited_sql).fetch_all(pool).await {
            Ok(sqlite_rows) => {
                let execution_time_ms = start_time.elapsed().as_millis();

                if sqlite_rows.is_empty() {
                    return QueryExecutionResult::Select(SelectResult::new(
                        vec![],
                        vec![],
                        execution_time_ms,
                        original_query,
                    ));
                }

                // Convert to trait types
                let columns = SqliteValueConverter::build_column_info(&sqlite_rows[0]);
                let rows: Vec<Row> = sqlite_rows
                    .iter()
                    .map(SqliteValueConverter::convert_row)
                    .collect();

                QueryExecutionResult::Select(SelectResult::new(
                    columns,
                    rows,
                    execution_time_ms,
                    original_query,
                ))
            }
            Err(e) => {
                let execution_time_ms = start_time.elapsed().as_millis();
                QueryExecutionResult::Error(ErrorResult::new(
                    format!("Query failed: {}", e),
                    execution_time_ms,
                ))
            }
        }
    }

    /// Execute a modification query (INSERT, UPDATE, DELETE).
    async fn execute_modification(&self, sql: &str, pool: &SqlitePool) -> QueryExecutionResult {
        let start_time = std::time::Instant::now();

        match sqlx::query(sql).execute(pool).await {
            Ok(result) => {
                let execution_time_ms = start_time.elapsed().as_millis();
                QueryExecutionResult::Modified(ModifiedResult::new(
                    result.rows_affected(),
                    execution_time_ms,
                ))
            }
            Err(e) => {
                let execution_time_ms = start_time.elapsed().as_millis();
                QueryExecutionResult::Error(ErrorResult::new(
                    format!("Query failed: {}", e),
                    execution_time_ms,
                ))
            }
        }
    }
}

#[async_trait]
impl DatabaseConnection for SqliteConnection {
    fn database_type(&self) -> DatabaseType {
        DatabaseType::SQLite
    }

    fn connection_config(&self) -> &ConnectionConfig {
        &self.config
    }

    async fn connect(&mut self) -> Result<()> {
        let options = self.build_connect_options()?;

        // SQLite pools should be smaller due to single-writer limitation
        let pool = SqlitePoolOptions::new()
            .max_connections(3)
            .acquire_timeout(Duration::from_secs(5))
            .connect_with(options)
            .await?;

        let mut guard = self.pool.write().await;
        *guard = Some(pool);

        Ok(())
    }

    async fn disconnect(&mut self) -> Result<()> {
        let mut guard = self.pool.write().await;
        if let Some(pool) = guard.take() {
            pool.close().await;
            Ok(())
        } else {
            Err(anyhow!("No active database connection to disconnect"))
        }
    }

    async fn is_connected(&self) -> bool {
        let guard = self.pool.read().await;
        if let Some(pool) = guard.as_ref() {
            // Perform a simple query to check connection health
            sqlx::query("SELECT 1").fetch_one(pool).await.is_ok()
        } else {
            false
        }
    }

    async fn execute_query(&self, sql: &str) -> Result<QueryExecutionResult> {
        let pool = self.get_pool().await?;

        let sql = sql.trim();
        if sql.is_empty() {
            return Ok(QueryExecutionResult::Error(ErrorResult::new(
                "Empty query".to_string(),
                0,
            )));
        }

        if Self::is_select_query(sql) {
            Ok(self.execute_select(sql, &pool).await)
        } else {
            Ok(self.execute_modification(sql, &pool).await)
        }
    }

    async fn stream_query<'a>(&'a self, sql: &'a str) -> Result<BoxStream<'a, Result<Row>>> {
        let pool = self.get_pool().await?;

        let stream = sqlx::query(sql)
            .fetch(&pool)
            .map(|result| {
                result
                    .map(|sqlite_row| SqliteValueConverter::convert_row(&sqlite_row))
                    .map_err(|e| anyhow!(e))
            });

        Ok(Box::pin(stream))
    }

    async fn test_connection(config: &ConnectionConfig) -> Result<()> {
        // Build options from config
        let options = match &config.params {
            ConnectionParams::File {
                path,
                read_only,
                ..
            } => SqliteConnectOptions::new()
                .filename(path)
                .create_if_missing(!read_only)
                .read_only(*read_only),
            ConnectionParams::InMemory { .. } => SqliteConnectOptions::from_str(":memory:")?,
            _ => return Err(anyhow!("SQLite requires file or in-memory connection parameters")),
        };

        // Create a minimal pool for testing
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .acquire_timeout(Duration::from_secs(5))
            .connect_with(options)
            .await?;

        // Run a simple query
        sqlx::query("SELECT 1").fetch_one(&pool).await?;

        // Close the pool
        pool.close().await;

        Ok(())
    }
}

// Ensure SqliteConnection can be sent between threads
unsafe impl Send for SqliteConnection {}
unsafe impl Sync for SqliteConnection {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn create_file_config() -> ConnectionConfig {
        ConnectionConfig::new(
            "test".to_string(),
            DatabaseType::SQLite,
            ConnectionParams::file(PathBuf::from("/tmp/test.db"), false),
        )
    }

    fn create_memory_config() -> ConnectionConfig {
        ConnectionConfig::new(
            "test-memory".to_string(),
            DatabaseType::SQLite,
            ConnectionParams::in_memory(),
        )
    }

    #[test]
    fn test_sqlite_connection_new() {
        let config = create_file_config();
        let conn = SqliteConnection::new(config.clone());

        assert_eq!(conn.database_type(), DatabaseType::SQLite);
        assert_eq!(conn.connection_config().name, "test");
    }

    #[test]
    fn test_sqlite_memory_connection_new() {
        let config = create_memory_config();
        let conn = SqliteConnection::new(config.clone());

        assert_eq!(conn.database_type(), DatabaseType::SQLite);
        assert_eq!(conn.connection_config().name, "test-memory");
    }

    #[test]
    fn test_is_select_query() {
        assert!(SqliteConnection::is_select_query("SELECT * FROM users"));
        assert!(SqliteConnection::is_select_query("select * from users"));
        assert!(SqliteConnection::is_select_query("  SELECT * FROM users"));
        assert!(SqliteConnection::is_select_query(
            "WITH cte AS (SELECT 1) SELECT * FROM cte"
        ));
        assert!(SqliteConnection::is_select_query("PRAGMA table_info(users)"));
        assert!(SqliteConnection::is_select_query("pragma foreign_keys"));

        assert!(!SqliteConnection::is_select_query("INSERT INTO users VALUES (1)"));
        assert!(!SqliteConnection::is_select_query("UPDATE users SET name = 'test'"));
        assert!(!SqliteConnection::is_select_query("DELETE FROM users"));
        assert!(!SqliteConnection::is_select_query("CREATE TABLE foo (id INT)"));
    }

    #[test]
    fn test_build_connect_options_file() {
        let config = create_file_config();
        let conn = SqliteConnection::new(config);

        let result = conn.build_connect_options();
        assert!(result.is_ok());
    }

    #[test]
    fn test_build_connect_options_memory() {
        let config = create_memory_config();
        let conn = SqliteConnection::new(config);

        let result = conn.build_connect_options();
        assert!(result.is_ok());
    }

    #[test]
    fn test_server_params_rejected() {
        use crate::services::database::traits::SslMode;

        let config = ConnectionConfig::new(
            "test".to_string(),
            DatabaseType::SQLite,
            ConnectionParams::server(
                "localhost".to_string(),
                5432,
                "user".to_string(),
                "pass".to_string(),
                "db".to_string(),
                SslMode::Prefer,
            ),
        );
        let conn = SqliteConnection::new(config);

        let result = conn.build_connect_options();
        assert!(result.is_err());
    }
}
