use crate::services::database_adapter::{
    ColumnInfo, DatabaseAdapter, DatabaseType, QueryExecutionResult, QueryResult, TableInfo,
};
use anyhow::Result;
use async_trait::async_trait;
use clickhouse_rs::{Block, ClientHandle, Pool, types::Complex};
use std::time::Instant;

pub struct ClickHouseAdapter {
    pool: Option<Pool>,
    client: Option<ClientHandle>,
}

impl ClickHouseAdapter {
    pub fn new() -> Self {
        Self {
            pool: None,
            client: None,
        }
    }

    fn column_to_string(block: &Block<Complex>, row_idx: usize, col_idx: usize) -> String {
        // For now, we'll use a simple string representation
        // This is a simplified version - in production you'd handle all types properly
        match block.get::<String, _>(row_idx, col_idx) {
            Ok(val) => val,
            Err(_) => {
                // Try as i64
                match block.get::<i64, _>(row_idx, col_idx) {
                    Ok(val) => val.to_string(),
                    Err(_) => {
                        // Try as f64
                        match block.get::<f64, _>(row_idx, col_idx) {
                            Ok(val) => val.to_string(),
                            Err(_) => "NULL".to_string(),
                        }
                    }
                }
            }
        }
    }
}

#[async_trait]
impl DatabaseAdapter for ClickHouseAdapter {
    async fn connect(&mut self, connection_url: &str) -> Result<()> {
        // Parse ClickHouse URL - supports both clickhouse:// and tcp:// schemes
        let url = if connection_url.starts_with("clickhouse://") {
            connection_url.replace("clickhouse://", "tcp://")
        } else if !connection_url.starts_with("tcp://") {
            format!("tcp://{}", connection_url)
        } else {
            connection_url.to_string()
        };

        let pool = Pool::new(url);
        let client = pool.get_handle().await?;
        
        self.pool = Some(pool);
        self.client = Some(client);
        Ok(())
    }

    async fn disconnect(&mut self) -> Result<()> {
        self.client = None;
        self.pool = None;
        Ok(())
    }

    async fn is_connected(&self) -> bool {
        self.client.is_some()
    }

    async fn execute_query(&self, sql: &str) -> QueryExecutionResult {
        let start_time = Instant::now();

        let pool = match &self.pool {
            Some(pool) => pool,
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
            match pool.get_handle().await {
                Ok(mut client) => match client.query(sql).fetch_all().await {
                Ok(block) => {
                    let execution_time = start_time.elapsed().as_millis();

                    if block.row_count() == 0 {
                        return QueryExecutionResult::Select(QueryResult {
                            columns: vec![],
                            rows: vec![],
                            row_count: 0,
                            execution_time_ms: execution_time,
                        });
                    }

                    // Get column names
                    let columns: Vec<String> = block
                        .columns()
                        .iter()
                        .map(|col| col.name().to_string())
                        .collect();

                    // Convert rows to string representation
                    let mut result_rows = Vec::new();
                    for row_idx in 0..block.row_count() {
                        let mut string_row = Vec::new();
                        for col_idx in 0..block.column_count() {
                            let value = Self::column_to_string(&block, row_idx, col_idx);
                            string_row.push(value);
                        }
                        result_rows.push(string_row);
                    }

                    QueryExecutionResult::Select(QueryResult {
                        columns,
                        rows: result_rows,
                        row_count: block.row_count(),
                        execution_time_ms: execution_time,
                    })
                    },
                    Err(e) => QueryExecutionResult::Error(format!("Query failed: {}", e)),
                },
                Err(e) => QueryExecutionResult::Error(format!("Failed to get client handle: {}", e)),
            }
        } else {
            // For non-SELECT queries (INSERT, CREATE, etc.)
            match pool.get_handle().await {
                Ok(mut client) => match client.execute(sql).await {
                Ok(_) => {
                    let execution_time = start_time.elapsed().as_millis();
                    QueryExecutionResult::Modified {
                        rows_affected: 0, // ClickHouse doesn't provide rows affected for most operations
                        execution_time_ms: execution_time,
                    }
                    },
                    Err(e) => QueryExecutionResult::Error(format!("Query failed: {}", e)),
                },
                Err(e) => QueryExecutionResult::Error(format!("Failed to get client handle: {}", e)),
            }
        }
    }

    async fn get_tables(&self) -> Result<Vec<TableInfo>> {
        let pool = self
            .pool
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Database not connected"))?;

        let query = r#"
            SELECT 
                name as table_name,
                database as table_schema,
                engine as table_type
            FROM system.tables
            WHERE database NOT IN ('system', 'INFORMATION_SCHEMA', 'information_schema')
            ORDER BY database, name
        "#;

        let mut client = pool.get_handle().await?;
        let block = client.query(query).fetch_all().await?;

        let mut tables = Vec::new();
        for row_idx in 0..block.row_count() {
            let table_name = match block.get::<String, _>(row_idx, "table_name") {
                Ok(name) => name,
                Err(_) => continue,
            };
            let table_schema = match block.get::<String, _>(row_idx, "table_schema") {
                Ok(schema) => schema,
                Err(_) => "default".to_string(),
            };
            let table_type = match block.get::<String, _>(row_idx, "table_type") {
                Ok(engine) => engine,
                Err(_) => "TABLE".to_string(),
            };

            tables.push(TableInfo {
                table_name,
                table_schema,
                table_type,
            });
        }

        Ok(tables)
    }

    async fn get_table_columns(
        &self,
        table_name: &str,
        table_schema: &str,
    ) -> Result<QueryResult> {
        let pool = self
            .pool
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Database not connected"))?;

        let query = format!(r#"
            SELECT 
                name as column_name,
                type as data_type,
                default_expression as column_default,
                position as ordinal_position
            FROM system.columns
            WHERE table = '{}' AND database = '{}'
            ORDER BY position
        "#, table_name, table_schema);

        let mut client = pool.get_handle().await?;
        let block = client
            .query(&query)
            .fetch_all()
            .await?;

        let mut columns = Vec::new();
        for row_idx in 0..block.row_count() {
            let column_name = block.get::<String, _>(row_idx, "column_name").unwrap_or_default();
            let data_type = block.get::<String, _>(row_idx, "data_type").unwrap_or_default();
            let column_default = block.get::<String, _>(row_idx, "column_default").ok();
            let ordinal_position = block.get::<u64, _>(row_idx, "ordinal_position").unwrap_or(0) as i32;

            // Check if type is nullable
            let is_nullable = if data_type.starts_with("Nullable(") {
                "YES".to_string()
            } else {
                "NO".to_string()
            };

            columns.push(ColumnInfo {
                column_name,
                data_type,
                is_nullable,
                column_default,
                ordinal_position,
            });
        }

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
        let pool = self
            .pool
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Database not connected"))?;

        let mut client = pool.get_handle().await?;
        let result = client.query("SELECT 1").fetch_all().await?;
        Ok(result.row_count() > 0)
    }

    fn database_type(&self) -> DatabaseType {
        DatabaseType::ClickHouse
    }
}