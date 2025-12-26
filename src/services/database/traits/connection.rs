//! Core database connection traits.
//!
//! This module defines the `DatabaseConnection` trait that all database drivers must implement,
//! as well as the `Transactional` trait for databases that support transactions.

use anyhow::Result;
use async_trait::async_trait;
use futures::stream::BoxStream;

use super::row::{ColumnInfo, Row};
use super::types::{ConnectionConfig, DatabaseType};

/// Result of executing a query
#[derive(Debug, Clone)]
pub enum QueryExecutionResult {
    /// SELECT query result with rows
    Select(SelectResult),
    /// Modification query result (INSERT, UPDATE, DELETE)
    Modified(ModifiedResult),
    /// Query execution error
    Error(ErrorResult),
}

/// Result of a SELECT query
#[derive(Debug, Clone)]
pub struct SelectResult {
    /// Column metadata
    pub columns: Vec<ColumnInfo>,
    /// Result rows
    pub rows: Vec<Row>,
    /// Total row count
    pub row_count: usize,
    /// Execution time in milliseconds
    pub execution_time_ms: u128,
    /// The original query that was executed
    pub original_query: String,
}

impl SelectResult {
    /// Create a new select result
    pub fn new(
        columns: Vec<ColumnInfo>,
        rows: Vec<Row>,
        execution_time_ms: u128,
        original_query: String,
    ) -> Self {
        let row_count = rows.len();
        Self {
            columns,
            rows,
            row_count,
            execution_time_ms,
            original_query,
        }
    }
}

/// Result of a modification query (INSERT, UPDATE, DELETE)
#[derive(Debug, Clone)]
pub struct ModifiedResult {
    /// Number of rows affected
    pub rows_affected: u64,
    /// Execution time in milliseconds
    pub execution_time_ms: u128,
}

impl ModifiedResult {
    /// Create a new modified result
    pub fn new(rows_affected: u64, execution_time_ms: u128) -> Self {
        Self {
            rows_affected,
            execution_time_ms,
        }
    }
}

/// Result when a query fails
#[derive(Debug, Clone)]
pub struct ErrorResult {
    /// Error message
    pub message: String,
    /// Execution time in milliseconds
    pub execution_time_ms: u128,
}

impl ErrorResult {
    /// Create a new error result
    pub fn new(message: String, execution_time_ms: u128) -> Self {
        Self {
            message,
            execution_time_ms,
        }
    }
}

/// Core trait for all database connections.
///
/// This trait defines the interface that all database drivers must implement,
/// providing a unified API for connecting, querying, and managing database connections.
///
/// # Example
///
/// ```ignore
/// use pgui::services::database::traits::{DatabaseConnection, ConnectionConfig};
///
/// async fn example(conn: &dyn DatabaseConnection) -> Result<()> {
///     if conn.is_connected().await {
///         let result = conn.execute_query("SELECT 1").await?;
///         // Process result...
///     }
///     Ok(())
/// }
/// ```
#[async_trait]
pub trait DatabaseConnection: Send + Sync {
    /// Get the database type for this connection
    fn database_type(&self) -> DatabaseType;

    /// Get the connection configuration
    fn connection_config(&self) -> &ConnectionConfig;

    /// Establish a connection to the database.
    ///
    /// # Errors
    ///
    /// Returns an error if the connection cannot be established, such as:
    /// - Invalid credentials
    /// - Network errors
    /// - Server not available
    async fn connect(&mut self) -> Result<()>;

    /// Disconnect from the database.
    ///
    /// This should gracefully close the connection and release any resources.
    async fn disconnect(&mut self) -> Result<()>;

    /// Check if the connection is currently active.
    ///
    /// This may perform a lightweight ping to verify the connection is alive.
    async fn is_connected(&self) -> bool;

    /// Execute a query and return the result.
    ///
    /// This method handles both SELECT queries (returning rows) and
    /// modification queries (INSERT, UPDATE, DELETE).
    ///
    /// # Arguments
    ///
    /// * `sql` - The SQL query to execute
    ///
    /// # Returns
    ///
    /// Returns a `QueryExecutionResult` which can be:
    /// - `Select` - For SELECT queries, contains rows and column metadata
    /// - `Modified` - For INSERT/UPDATE/DELETE, contains rows affected count
    /// - `Error` - If the query fails
    async fn execute_query(&self, sql: &str) -> Result<QueryExecutionResult>;

    /// Execute a query and return results as a stream.
    ///
    /// This is useful for large result sets that shouldn't be loaded entirely into memory.
    ///
    /// # Arguments
    ///
    /// * `sql` - The SQL query to execute
    ///
    /// # Returns
    ///
    /// Returns a stream of `Row` results
    async fn stream_query<'a>(
        &'a self,
        sql: &'a str,
    ) -> Result<BoxStream<'a, Result<Row>>>;

    /// Test the connection without fully connecting.
    ///
    /// This creates a temporary connection, verifies it works, and closes it.
    /// Useful for testing connection parameters before saving them.
    async fn test_connection(config: &ConnectionConfig) -> Result<()>
    where
        Self: Sized;

    /// Get a display name for the current connection.
    ///
    /// Returns a human-readable string describing the connection,
    /// typically in the format "user@host:port/database" for server connections
    /// or the file path for file-based connections.
    fn display_name(&self) -> String {
        let config = self.connection_config();
        match &config.params {
            super::types::ConnectionParams::Server {
                hostname,
                port,
                username,
                database,
                ..
            } => {
                format!("{}@{}:{}/{}", username, hostname, port, database)
            }
            super::types::ConnectionParams::File { path, .. } => {
                path.display().to_string()
            }
            super::types::ConnectionParams::InMemory { .. } => {
                ":memory:".to_string()
            }
        }
    }
}

/// Transaction handle for databases that support transactions.
///
/// This is a placeholder type that individual drivers can extend
/// with their specific transaction implementation.
pub struct Transaction {
    /// Transaction ID (for tracking)
    pub id: u64,
    /// Whether the transaction is still active
    pub active: bool,
}

/// Trait for databases that support transactions.
///
/// Not all databases support transactions (or support them in the same way),
/// so this is a separate trait that drivers can optionally implement.
#[async_trait]
pub trait Transactional: DatabaseConnection {
    /// Begin a new transaction.
    ///
    /// # Returns
    ///
    /// Returns a transaction handle that can be used with commit/rollback.
    async fn begin_transaction(&self) -> Result<Transaction>;

    /// Commit a transaction.
    ///
    /// # Arguments
    ///
    /// * `tx` - The transaction to commit
    async fn commit(&self, tx: Transaction) -> Result<()>;

    /// Rollback a transaction.
    ///
    /// # Arguments
    ///
    /// * `tx` - The transaction to rollback
    async fn rollback(&self, tx: Transaction) -> Result<()>;

    /// Execute a query within a transaction.
    ///
    /// # Arguments
    ///
    /// * `tx` - The transaction to execute within
    /// * `sql` - The SQL query to execute
    async fn execute_in_transaction(
        &self,
        tx: &Transaction,
        sql: &str,
    ) -> Result<QueryExecutionResult>;
}

/// A boxed database connection trait object.
///
/// This type alias makes it easier to work with trait objects.
pub type BoxedConnection = Box<dyn DatabaseConnection>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_select_result_creation() {
        let result = SelectResult::new(
            vec![ColumnInfo::new("id".to_string(), "int4".to_string(), 0)],
            vec![],
            100,
            "SELECT 1".to_string(),
        );

        assert_eq!(result.row_count, 0);
        assert_eq!(result.execution_time_ms, 100);
        assert_eq!(result.columns.len(), 1);
    }

    #[test]
    fn test_modified_result_creation() {
        let result = ModifiedResult::new(5, 50);
        assert_eq!(result.rows_affected, 5);
        assert_eq!(result.execution_time_ms, 50);
    }

    #[test]
    fn test_error_result_creation() {
        let result = ErrorResult::new("Connection failed".to_string(), 10);
        assert_eq!(result.message, "Connection failed");
        assert_eq!(result.execution_time_ms, 10);
    }
}
