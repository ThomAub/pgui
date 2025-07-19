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