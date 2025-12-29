//! MySQL connection implementation.
//!
//! This module implements the `DatabaseConnection` trait for MySQL
//! using SQLx's MySqlPool.

use anyhow::{anyhow, Result};
use async_lock::RwLock;
use async_trait::async_trait;
use futures::stream::BoxStream;
use futures::StreamExt;
use sqlx::mysql::{MySqlConnectOptions, MySqlPoolOptions, MySqlSslMode};
use sqlx::MySqlPool;
use std::time::Duration;

use super::types::MySqlValueConverter;
use crate::services::database::traits::{
    BoxedConnection, ConnectionConfig, ConnectionParams, DatabaseConnection,
    DatabaseType, ErrorResult, ModifiedResult, QueryExecutionResult, Row, SelectResult,
};

/// MySQL database connection.
///
/// This struct wraps a SQLx MySqlPool and implements the `DatabaseConnection` trait.
pub struct MySqlConnection {
    config: ConnectionConfig,
    pool: RwLock<Option<MySqlPool>>,
}

impl std::fmt::Debug for MySqlConnection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MySqlConnection")
            .field("config", &self.config)
            .field("pool", &"<MySqlPool>")
            .finish()
    }
}

impl MySqlConnection {
    /// Create a new MySQL connection from configuration.
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

    /// Build MySqlConnectOptions from the configuration.
    fn build_connect_options(&self) -> Result<MySqlConnectOptions> {
        match &self.config.params {
            ConnectionParams::Server {
                hostname,
                port,
                username,
                password,
                database,
                ssl_mode,
                ..
            } => {
                let (require_ssl, _verify) = MySqlValueConverter::map_ssl_mode(ssl_mode);

                let mysql_ssl_mode = if require_ssl {
                    MySqlSslMode::Required
                } else {
                    MySqlSslMode::Preferred
                };

                Ok(MySqlConnectOptions::new()
                    .host(hostname)
                    .port(*port)
                    .username(username)
                    .password(password)
                    .database(database)
                    .ssl_mode(mysql_ssl_mode))
            }
            ConnectionParams::File { .. } | ConnectionParams::InMemory { .. } => Err(anyhow!(
                "MySQL does not support file-based or in-memory connections"
            )),
        }
    }

    /// Get a reference to the connection pool.
    ///
    /// Returns an error if not connected.
    async fn get_pool(&self) -> Result<MySqlPool> {
        let guard = self.pool.read().await;
        guard
            .as_ref()
            .cloned()
            .ok_or_else(|| anyhow!("Database not connected"))
    }

    /// Get a reference to the pool (internal helper for schema module).
    pub(crate) async fn get_pool_internal(&self) -> Result<MySqlPool> {
        self.get_pool().await
    }

    /// Check if the query is a SELECT-type query.
    fn is_select_query(sql: &str) -> bool {
        let lower = sql.to_lowercase();
        let trimmed = lower.trim_start();
        trimmed.starts_with("select")
            || trimmed.starts_with("with")
            || trimmed.starts_with("show")
            || trimmed.starts_with("describe")
            || trimmed.starts_with("desc")
            || trimmed.starts_with("explain")
    }

    /// Execute a SELECT query.
    async fn execute_select(&self, sql: &str, pool: &MySqlPool) -> QueryExecutionResult {
        let start_time = std::time::Instant::now();
        let original_query = sql.to_string();

        // Add LIMIT if not present to prevent massive result sets
        let limited_sql = if !sql.to_lowercase().contains(" limit ")
            && !sql.to_lowercase().starts_with("show")
            && !sql.to_lowercase().starts_with("describe")
            && !sql.to_lowercase().starts_with("desc")
            && !sql.to_lowercase().starts_with("explain")
        {
            format!("{} LIMIT {}", sql.trim_end_matches(';'), 1_000)
        } else {
            sql.to_string()
        };

        match sqlx::query(&limited_sql).fetch_all(pool).await {
            Ok(mysql_rows) => {
                let execution_time_ms = start_time.elapsed().as_millis();

                if mysql_rows.is_empty() {
                    return QueryExecutionResult::Select(SelectResult::new(
                        vec![],
                        vec![],
                        execution_time_ms,
                        original_query,
                    ));
                }

                // Convert to trait types
                let columns = MySqlValueConverter::build_column_info(&mysql_rows[0]);
                let rows: Vec<Row> = mysql_rows
                    .iter()
                    .map(MySqlValueConverter::convert_row)
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
    async fn execute_modification(&self, sql: &str, pool: &MySqlPool) -> QueryExecutionResult {
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
impl DatabaseConnection for MySqlConnection {
    fn database_type(&self) -> DatabaseType {
        DatabaseType::MySQL
    }

    fn connection_config(&self) -> &ConnectionConfig {
        &self.config
    }

    async fn connect(&mut self) -> Result<()> {
        let options = self.build_connect_options()?;

        let pool = MySqlPoolOptions::new()
            .max_connections(5)
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
            .map(|result| result.map(|mysql_row| MySqlValueConverter::convert_row(&mysql_row)).map_err(|e| anyhow!(e)));

        Ok(Box::pin(stream))
    }

    async fn test_connection(config: &ConnectionConfig) -> Result<()> {
        // Build options from config
        let options = match &config.params {
            ConnectionParams::Server {
                hostname,
                port,
                username,
                password,
                database,
                ssl_mode,
                ..
            } => {
                let (require_ssl, _verify) = MySqlValueConverter::map_ssl_mode(ssl_mode);

                let mysql_ssl_mode = if require_ssl {
                    MySqlSslMode::Required
                } else {
                    MySqlSslMode::Preferred
                };

                MySqlConnectOptions::new()
                    .host(hostname)
                    .port(*port)
                    .username(username)
                    .password(password)
                    .database(database)
                    .ssl_mode(mysql_ssl_mode)
            }
            _ => return Err(anyhow!("MySQL requires server connection parameters")),
        };

        // Create a minimal pool for testing
        let pool = MySqlPoolOptions::new()
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

// Ensure MySqlConnection can be sent between threads
unsafe impl Send for MySqlConnection {}
unsafe impl Sync for MySqlConnection {}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_config() -> ConnectionConfig {
        ConnectionConfig::new(
            "test".to_string(),
            DatabaseType::MySQL,
            ConnectionParams::server(
                "localhost".to_string(),
                3306,
                "root".to_string(),
                "password".to_string(),
                "mysql".to_string(),
            ),
        )
    }

    #[test]
    fn test_mysql_connection_new() {
        let config = create_test_config();
        let conn = MySqlConnection::new(config.clone());

        assert_eq!(conn.database_type(), DatabaseType::MySQL);
        assert_eq!(conn.connection_config().name, "test");
    }

    #[test]
    fn test_is_select_query() {
        assert!(MySqlConnection::is_select_query("SELECT * FROM users"));
        assert!(MySqlConnection::is_select_query("select * from users"));
        assert!(MySqlConnection::is_select_query("  SELECT * FROM users"));
        assert!(MySqlConnection::is_select_query(
            "WITH cte AS (SELECT 1) SELECT * FROM cte"
        ));
        assert!(MySqlConnection::is_select_query("SHOW TABLES"));
        assert!(MySqlConnection::is_select_query("DESCRIBE users"));
        assert!(MySqlConnection::is_select_query("EXPLAIN SELECT * FROM users"));

        assert!(!MySqlConnection::is_select_query("INSERT INTO users VALUES (1)"));
        assert!(!MySqlConnection::is_select_query("UPDATE users SET name = 'test'"));
        assert!(!MySqlConnection::is_select_query("DELETE FROM users"));
    }

    #[test]
    fn test_build_connect_options() {
        let config = create_test_config();
        let conn = MySqlConnection::new(config);

        let result = conn.build_connect_options();
        assert!(result.is_ok());
    }

    #[test]
    fn test_file_params_rejected() {
        let config = ConnectionConfig::new(
            "test".to_string(),
            DatabaseType::MySQL,
            ConnectionParams::file(std::path::PathBuf::from("/tmp/test.db"), false),
        );
        let conn = MySqlConnection::new(config);

        let result = conn.build_connect_options();
        assert!(result.is_err());
    }
}
