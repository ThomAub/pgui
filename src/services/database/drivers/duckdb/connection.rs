//! DuckDB connection implementation.
//!
//! This module implements the `DatabaseConnection` trait for DuckDB.
//! DuckDB uses a synchronous API, so we wrap operations with smol::unblock.

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use duckdb::{Config, Connection};
use futures::stream::BoxStream;
use std::sync::Mutex;
use std::time::Instant;

use super::types::DuckDbValueConverter;
use crate::services::database::traits::{
    BoxedConnection, ConnectionConfig, ConnectionParams, DatabaseConnection, DatabaseType,
    ErrorResult, ModifiedResult, QueryExecutionResult, Row, SelectResult,
};

/// DuckDB database connection.
///
/// This struct wraps a DuckDB Connection and implements the `DatabaseConnection` trait.
/// DuckDB is an in-process analytical database, similar to SQLite but optimized for OLAP.
pub struct DuckDbConnection {
    config: ConnectionConfig,
    connection: Mutex<Option<Connection>>,
}

impl DuckDbConnection {
    /// Create a new DuckDB connection with the given configuration.
    pub fn new(config: ConnectionConfig) -> Self {
        Self {
            config,
            connection: Mutex::new(None),
        }
    }

    /// Create a boxed DuckDB connection.
    pub fn boxed(config: ConnectionConfig) -> BoxedConnection {
        Box::new(Self::new(config))
    }

    /// Build a DuckDB connection from the configuration.
    fn build_connection(config: &ConnectionConfig) -> Result<Connection> {
        match &config.params {
            ConnectionParams::File { path, read_only, .. } => {
                let cfg = if *read_only {
                    Config::default().access_mode(duckdb::AccessMode::ReadOnly)?
                } else {
                    Config::default()
                };

                Connection::open_with_flags(path, cfg)
                    .map_err(|e| anyhow!("Failed to open DuckDB file: {}", e))
            }
            ConnectionParams::InMemory { .. } => Connection::open_in_memory()
                .map_err(|e| anyhow!("Failed to create in-memory DuckDB: {}", e)),
            ConnectionParams::Server { .. } => {
                Err(anyhow!("DuckDB does not support server connections"))
            }
        }
    }

    /// Check if this is a SELECT-like query.
    fn is_select_query(sql: &str) -> bool {
        let trimmed = sql.trim().to_uppercase();
        trimmed.starts_with("SELECT")
            || trimmed.starts_with("WITH")
            || trimmed.starts_with("SHOW")
            || trimmed.starts_with("DESCRIBE")
            || trimmed.starts_with("EXPLAIN")
            || trimmed.starts_with("PRAGMA")
            || trimmed.starts_with("FROM") // DuckDB supports FROM-first queries
    }

    /// Execute a SELECT query synchronously.
    fn execute_select_sync(
        conn: &Connection,
        sql: &str,
        original_query: &str,
    ) -> QueryExecutionResult {
        let start = Instant::now();

        let result = (|| -> Result<(Vec<_>, Vec<_>), duckdb::Error> {
            let mut stmt = conn.prepare(sql)?;
            let columns = DuckDbValueConverter::build_column_info(&stmt);
            let column_count = columns.len();

            let mut rows = Vec::new();
            let mut row_iter = stmt.query([])?;

            while let Some(row) = row_iter.next()? {
                rows.push(DuckDbValueConverter::convert_row(row, column_count));
            }

            Ok((columns, rows))
        })();

        let execution_time_ms = start.elapsed().as_millis();

        match result {
            Ok((columns, rows)) => QueryExecutionResult::Select(SelectResult::new(
                columns,
                rows,
                execution_time_ms,
                original_query.to_string(),
            )),
            Err(e) => QueryExecutionResult::Error(ErrorResult::new(e.to_string(), execution_time_ms)),
        }
    }

    /// Execute a modification query synchronously.
    fn execute_modification_sync(
        conn: &Connection,
        sql: &str,
    ) -> QueryExecutionResult {
        let start = Instant::now();
        let result = conn.execute(sql, []);
        let execution_time_ms = start.elapsed().as_millis();

        match result {
            Ok(affected) => {
                QueryExecutionResult::Modified(ModifiedResult::new(affected as u64, execution_time_ms))
            }
            Err(e) => QueryExecutionResult::Error(ErrorResult::new(e.to_string(), execution_time_ms)),
        }
    }
}

#[async_trait]
impl DatabaseConnection for DuckDbConnection {
    fn database_type(&self) -> DatabaseType {
        DatabaseType::DuckDB
    }

    fn connection_config(&self) -> &ConnectionConfig {
        &self.config
    }

    async fn connect(&mut self) -> Result<()> {
        let config = self.config.clone();

        let conn = smol::unblock(move || Self::build_connection(&config)).await?;

        let mut guard = self.connection.lock().map_err(|_| anyhow!("Lock poisoned"))?;
        *guard = Some(conn);
        Ok(())
    }

    async fn disconnect(&mut self) -> Result<()> {
        let mut guard = self.connection.lock().map_err(|_| anyhow!("Lock poisoned"))?;
        *guard = None;
        Ok(())
    }

    async fn is_connected(&self) -> bool {
        let guard = self.connection.lock();
        guard.map(|g| g.is_some()).unwrap_or(false)
    }

    async fn execute_query(&self, sql: &str) -> Result<QueryExecutionResult> {
        let guard = self.connection.lock().map_err(|_| anyhow!("Lock poisoned"))?;
        let conn = guard.as_ref().ok_or_else(|| anyhow!("Not connected"))?;

        // DuckDB is synchronous - we could use unblock but for simplicity
        // we'll just run it inline since we're holding the lock
        let result = if Self::is_select_query(sql) {
            Self::execute_select_sync(conn, sql, sql)
        } else {
            Self::execute_modification_sync(conn, sql)
        };

        Ok(result)
    }

    async fn stream_query<'a>(
        &'a self,
        sql: &'a str,
    ) -> Result<BoxStream<'a, Result<Row>>> {
        // Execute and convert to stream
        let result = self.execute_query(sql).await?;

        match result {
            QueryExecutionResult::Select(select) => {
                let rows = select.rows;
                Ok(Box::pin(futures::stream::iter(rows.into_iter().map(Ok))))
            }
            QueryExecutionResult::Error(e) => {
                Err(anyhow!("{}", e.message))
            }
            _ => Ok(Box::pin(futures::stream::empty())),
        }
    }

    async fn test_connection(config: &ConnectionConfig) -> Result<()>
    where
        Self: Sized,
    {
        let config = config.clone();

        smol::unblock(move || {
            let conn = Self::build_connection(&config)?;
            let mut stmt = conn.prepare("SELECT 1")?;
            let mut rows = stmt.query([])?;
            let _ = rows.next()?;
            Ok(())
        })
        .await
    }
}

// DuckDB Connection is not Sync but we wrap it in Mutex
unsafe impl Send for DuckDbConnection {}
unsafe impl Sync for DuckDbConnection {}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_memory_config() -> ConnectionConfig {
        ConnectionConfig::new(
            "test".to_string(),
            DatabaseType::DuckDB,
            ConnectionParams::in_memory(),
        )
    }

    #[test]
    fn test_duckdb_connection_new() {
        let config = create_memory_config();
        let conn = DuckDbConnection::new(config.clone());

        assert_eq!(conn.database_type(), DatabaseType::DuckDB);
        assert_eq!(conn.connection_config().name, "test");
    }

    #[test]
    fn test_is_select_query() {
        assert!(DuckDbConnection::is_select_query("SELECT * FROM users"));
        assert!(DuckDbConnection::is_select_query("select * from users"));
        assert!(DuckDbConnection::is_select_query("  SELECT * FROM users"));
        assert!(DuckDbConnection::is_select_query(
            "WITH cte AS (SELECT 1) SELECT * FROM cte"
        ));
        assert!(DuckDbConnection::is_select_query("SHOW TABLES"));
        assert!(DuckDbConnection::is_select_query("DESCRIBE users"));
        assert!(DuckDbConnection::is_select_query("EXPLAIN SELECT * FROM users"));
        assert!(DuckDbConnection::is_select_query("PRAGMA database_list"));
        assert!(DuckDbConnection::is_select_query("FROM users SELECT *"));

        assert!(!DuckDbConnection::is_select_query(
            "INSERT INTO users VALUES (1)"
        ));
        assert!(!DuckDbConnection::is_select_query(
            "UPDATE users SET name = 'test'"
        ));
        assert!(!DuckDbConnection::is_select_query("DELETE FROM users"));
        assert!(!DuckDbConnection::is_select_query(
            "CREATE TABLE users (id INT)"
        ));
    }

    #[test]
    fn test_server_params_rejected() {
        let config = ConnectionConfig::new(
            "test".to_string(),
            DatabaseType::DuckDB,
            ConnectionParams::server(
                "localhost".to_string(),
                5432,
                "user".to_string(),
                "pass".to_string(),
                "db".to_string(),
            ),
        );

        let result = DuckDbConnection::build_connection(&config);
        assert!(result.is_err());
    }
}
