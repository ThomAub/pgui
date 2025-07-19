use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableInfo {
    pub table_name: String,
    pub table_schema: String,
    pub table_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryResult {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
    pub row_count: usize,
    pub execution_time_ms: u128,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnInfo {
    pub column_name: String,
    pub data_type: String,
    pub is_nullable: String,
    pub column_default: Option<String>,
    pub ordinal_position: i32,
}

#[derive(Debug, Clone)]
pub enum QueryExecutionResult {
    Select(QueryResult),
    Modified {
        rows_affected: u64,
        execution_time_ms: u128,
    },
    Error(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum DatabaseType {
    PostgreSQL,
    SQLite,
    ClickHouse,
}

impl DatabaseType {
    pub fn from_url(url: &str) -> Result<Self> {
        if url.starts_with("postgres://") || url.starts_with("postgresql://") {
            Ok(DatabaseType::PostgreSQL)
        } else if url.starts_with("sqlite://") || url.ends_with(".db") || url.ends_with(".sqlite") {
            Ok(DatabaseType::SQLite)
        } else if url.starts_with("clickhouse://") 
            || url.starts_with("tcp://") 
            || (url.starts_with("http://") && (url.contains(":8123") || url.contains("clickhouse")))
            || (url.starts_with("https://") && (url.contains(":8123") || url.contains("clickhouse"))) {
            Ok(DatabaseType::ClickHouse)
        } else {
            Err(anyhow::anyhow!("Unsupported database URL scheme"))
        }
    }
}

#[async_trait]
pub trait DatabaseAdapter: Send + Sync {
    async fn connect(&mut self, connection_url: &str) -> Result<()>;
    
    async fn disconnect(&mut self) -> Result<()>;
    
    async fn is_connected(&self) -> bool;
    
    async fn execute_query(&self, query: &str) -> QueryExecutionResult;
    
    async fn get_tables(&self) -> Result<Vec<TableInfo>>;
    
    async fn get_table_columns(&self, table_name: &str, table_schema: &str) -> Result<QueryResult>;
    
    async fn test_connection(&self) -> Result<bool>;
    
    fn database_type(&self) -> DatabaseType;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_database_type_from_url_postgres() {
        // Standard PostgreSQL URLs
        assert_eq!(
            DatabaseType::from_url("postgres://user:pass@localhost:5432/db").unwrap(),
            DatabaseType::PostgreSQL
        );
        assert_eq!(
            DatabaseType::from_url("postgresql://user:pass@localhost:5432/db").unwrap(),
            DatabaseType::PostgreSQL
        );
        assert_eq!(
            DatabaseType::from_url("postgres://localhost/db").unwrap(),
            DatabaseType::PostgreSQL
        );
    }

    #[test]
    fn test_database_type_from_url_sqlite() {
        // SQLite URLs
        assert_eq!(
            DatabaseType::from_url("sqlite://path/to/database.db").unwrap(),
            DatabaseType::SQLite
        );
        assert_eq!(
            DatabaseType::from_url("sqlite:///absolute/path/database.db").unwrap(),
            DatabaseType::SQLite
        );
        // File paths
        assert_eq!(
            DatabaseType::from_url("/path/to/database.db").unwrap(),
            DatabaseType::SQLite
        );
        assert_eq!(
            DatabaseType::from_url("relative/path/database.sqlite").unwrap(),
            DatabaseType::SQLite
        );
        assert_eq!(
            DatabaseType::from_url("test.db").unwrap(),
            DatabaseType::SQLite
        );
    }

    #[test]
    fn test_database_type_from_url_clickhouse() {
        // ClickHouse URLs
        assert_eq!(
            DatabaseType::from_url("clickhouse://user:pass@localhost:9000/db").unwrap(),
            DatabaseType::ClickHouse
        );
        assert_eq!(
            DatabaseType::from_url("tcp://user:pass@localhost:9000/db").unwrap(),
            DatabaseType::ClickHouse
        );
        // HTTP URLs with port 8123
        assert_eq!(
            DatabaseType::from_url("http://user:pass@localhost:8123/db").unwrap(),
            DatabaseType::ClickHouse
        );
        assert_eq!(
            DatabaseType::from_url("https://user:pass@localhost:8123/db").unwrap(),
            DatabaseType::ClickHouse
        );
        // HTTP URLs with "clickhouse" in hostname
        assert_eq!(
            DatabaseType::from_url("http://clickhouse.example.com/db").unwrap(),
            DatabaseType::ClickHouse
        );
        assert_eq!(
            DatabaseType::from_url("https://my-clickhouse-server.com/db").unwrap(),
            DatabaseType::ClickHouse
        );
    }

    #[test]
    fn test_database_type_from_url_unsupported() {
        // Unsupported URLs
        assert!(DatabaseType::from_url("mysql://localhost/db").is_err());
        assert!(DatabaseType::from_url("mongodb://localhost/db").is_err());
        assert!(DatabaseType::from_url("redis://localhost:6379").is_err());
        assert!(DatabaseType::from_url("http://localhost:3000").is_err()); // HTTP without ClickHouse indicators
        assert!(DatabaseType::from_url("ftp://localhost/file").is_err());
        assert!(DatabaseType::from_url("invalid-url").is_err());
    }

    #[test]
    fn test_database_type_edge_cases() {
        // Empty string
        assert!(DatabaseType::from_url("").is_err());
        
        // URLs with special characters
        assert_eq!(
            DatabaseType::from_url("postgres://user%40:p%40ss@localhost/db").unwrap(),
            DatabaseType::PostgreSQL
        );
        
        // Case sensitivity (URL schemes are case sensitive)
        assert!(DatabaseType::from_url("POSTGRES://localhost/db").is_err());
        
        // SQLite with spaces (should fail as it's not .db or .sqlite)
        assert!(DatabaseType::from_url("my database file").is_err());
    }
}