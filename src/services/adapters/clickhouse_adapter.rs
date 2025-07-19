use crate::services::database_adapter::{
    ColumnInfo, DatabaseAdapter, DatabaseType, QueryExecutionResult, QueryResult, TableInfo,
};
use anyhow::Result;
use async_trait::async_trait;
use clickhouse::{Client, Row};
use serde::Deserialize;
use std::time::Instant;

pub struct ClickHouseAdapter {
    client: Option<Client>,
}

impl ClickHouseAdapter {
    pub fn new() -> Self {
        Self { client: None }
    }

    fn parse_connection_url(url: &str) -> Result<(String, String, String, String)> {
        // Parse URL to extract components
        // Expected format: http://username:password@localhost:8123/database
        let url_without_scheme = if url.starts_with("http://") {
            url.strip_prefix("http://").unwrap()
        } else if url.starts_with("https://") {
            url.strip_prefix("https://").unwrap()
        } else {
            return Err(anyhow::anyhow!("ClickHouse requires HTTP or HTTPS URL"));
        };

        // Extract user:pass@host:port/database
        let parts: Vec<&str> = url_without_scheme.split('@').collect();
        if parts.len() != 2 {
            return Err(anyhow::anyhow!("Invalid URL format"));
        }

        let (user_pass, host_db) = (parts[0], parts[1]);
        
        // Parse user:pass
        let creds: Vec<&str> = user_pass.split(':').collect();
        let (user, password) = if creds.len() == 2 {
            (creds[0].to_string(), creds[1].to_string())
        } else {
            ("default".to_string(), "".to_string())
        };

        // Parse host:port/database
        let host_parts: Vec<&str> = host_db.split('/').collect();
        if host_parts.is_empty() {
            return Err(anyhow::anyhow!("Invalid URL format"));
        }

        let host_port = host_parts[0];
        let database = if host_parts.len() > 1 {
            host_parts[1].to_string()
        } else {
            "default".to_string()
        };

        let full_url = if url.starts_with("https://") {
            format!("https://{}", host_port)
        } else {
            format!("http://{}", host_port)
        };

        Ok((full_url, user, password, database))
    }
}

// Helper structs for deserializing ClickHouse system tables
#[derive(Debug, Deserialize, Row)]
struct TableRow {
    name: String,
    database: String,
    engine: String,
}

#[derive(Debug, Deserialize, Row)]
struct ColumnRow {
    name: String,
    #[serde(rename = "type")]
    data_type: String,
    default_expression: String,
    position: u64,
}

#[async_trait]
impl DatabaseAdapter for ClickHouseAdapter {
    async fn connect(&mut self, connection_url: &str) -> Result<()> {
        let (url, user, password, database) = Self::parse_connection_url(connection_url)?;
        
        let client = Client::default()
            .with_url(&url)
            .with_user(&user)
            .with_password(&password)
            .with_database(&database);
        
        self.client = Some(client);
        Ok(())
    }

    async fn disconnect(&mut self) -> Result<()> {
        self.client = None;
        Ok(())
    }

    async fn is_connected(&self) -> bool {
        self.client.is_some()
    }

    async fn execute_query(&self, sql: &str) -> QueryExecutionResult {
        let start_time = Instant::now();

        let client = match &self.client {
            Some(client) => client,
            None => return QueryExecutionResult::Error("Database not connected".to_string()),
        };

        let sql = sql.trim();
        if sql.is_empty() {
            return QueryExecutionResult::Error("Empty query".to_string());
        }

        let is_select = sql.to_lowercase().trim_start().starts_with("select")
            || sql.to_lowercase().trim_start().starts_with("with")
            || sql.to_lowercase().trim_start().starts_with("show")
            || sql.to_lowercase().trim_start().starts_with("describe");

        if is_select {
            // For SELECT queries, we need to fetch the raw data as string
            match client.query(sql).fetch_all::<String>().await {
                Ok(rows) => {
                    let execution_time = start_time.elapsed().as_millis();

                    if rows.is_empty() {
                        return QueryExecutionResult::Select(QueryResult {
                            columns: vec![],
                            rows: vec![],
                            row_count: 0,
                            execution_time_ms: execution_time,
                        });
                    }

                    // Parse the first row to get column names
                    // ClickHouse returns data in TSV format by default
                    let first_row = &rows[0];
                    let column_count = first_row.split('\t').count();
                    
                    // For now, generate generic column names
                    // In a real implementation, we'd parse the query or use DESCRIBE
                    let columns: Vec<String> = (0..column_count)
                        .map(|i| format!("column_{}", i + 1))
                        .collect();

                    // Convert rows to Vec<Vec<String>>
                    let result_rows: Vec<Vec<String>> = rows
                        .iter()
                        .map(|row| row.split('\t').map(|s| s.to_string()).collect())
                        .collect();

                    QueryExecutionResult::Select(QueryResult {
                        columns,
                        rows: result_rows,
                        row_count: rows.len(),
                        execution_time_ms: execution_time,
                    })
                }
                Err(e) => QueryExecutionResult::Error(format!("Query failed: {}", e)),
            }
        } else {
            // For non-SELECT queries (INSERT, CREATE, etc.)
            match client.query(sql).execute().await {
                Ok(_) => {
                    let execution_time = start_time.elapsed().as_millis();
                    QueryExecutionResult::Modified {
                        rows_affected: 0, // ClickHouse doesn't provide rows affected
                        execution_time_ms: execution_time,
                    }
                }
                Err(e) => QueryExecutionResult::Error(format!("Query failed: {}", e)),
            }
        }
    }

    async fn get_tables(&self) -> Result<Vec<TableInfo>> {
        let client = self
            .client
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Database not connected"))?;

        let query = r#"
            SELECT 
                name,
                database,
                engine
            FROM system.tables
            WHERE database NOT IN ('system', 'INFORMATION_SCHEMA', 'information_schema')
            ORDER BY database, name
        "#;

        let rows = client.query(query).fetch_all::<TableRow>().await?;

        let tables = rows
            .into_iter()
            .map(|row| TableInfo {
                table_name: row.name,
                table_schema: row.database,
                table_type: row.engine,
            })
            .collect();

        Ok(tables)
    }

    async fn get_table_columns(
        &self,
        table_name: &str,
        table_schema: &str,
    ) -> Result<QueryResult> {
        let client = self
            .client
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Database not connected"))?;

        let query = r#"
            SELECT 
                name,
                type,
                default_expression,
                position
            FROM system.columns
            WHERE table = ? AND database = ?
            ORDER BY position
        "#;

        let rows = client
            .query(query)
            .bind(table_name)
            .bind(table_schema)
            .fetch_all::<ColumnRow>()
            .await?;

        let columns: Vec<ColumnInfo> = rows
            .into_iter()
            .map(|row| {
                let is_nullable = if row.data_type.starts_with("Nullable(") {
                    "YES".to_string()
                } else {
                    "NO".to_string()
                };

                ColumnInfo {
                    column_name: row.name,
                    data_type: row.data_type,
                    is_nullable,
                    column_default: if row.default_expression.is_empty() {
                        None
                    } else {
                        Some(row.default_expression)
                    },
                    ordinal_position: row.position as i32,
                }
            })
            .collect();

        let column_names = vec![
            "Column Name".to_string(),
            "Data Type".to_string(),
            "Nullable".to_string(),
            "Default".to_string(),
        ];
        let column_rows: Vec<Vec<String>> = columns
            .into_iter()
            .map(|col| {
                vec![
                    col.column_name,
                    col.data_type,
                    col.is_nullable,
                    col.column_default.unwrap_or_else(|| "NULL".to_string()),
                ]
            })
            .collect();

        let query_result = QueryResult {
            columns: column_names,
            rows: column_rows.clone(),
            row_count: column_rows.len(),
            execution_time_ms: 0,
        };

        Ok(query_result)
    }

    async fn test_connection(&self) -> Result<bool> {
        let client = self
            .client
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Database not connected"))?;

        client.query("SELECT 1").execute().await?;
        Ok(true)
    }

    fn database_type(&self) -> DatabaseType {
        DatabaseType::ClickHouse
    }
}