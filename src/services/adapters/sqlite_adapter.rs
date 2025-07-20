use crate::services::database_adapter::{
    ColumnInfo, DatabaseAdapter, DatabaseType, QueryExecutionResult, QueryResult, TableInfo,
};
use anyhow::Result;
use async_trait::async_trait;
use sqlx::{sqlite::SqlitePoolOptions, Column, Row, SqlitePool, TypeInfo, ValueRef};

pub struct SqliteAdapter {
    pool: Option<SqlitePool>,
}

impl SqliteAdapter {
    pub fn new() -> Self {
        Self { pool: None }
    }
}

#[async_trait]
impl DatabaseAdapter for SqliteAdapter {
    async fn connect(&mut self, connection_url: &str) -> Result<()> {
        // Handle both sqlite:// URLs and direct file paths
        let db_path = if connection_url.starts_with("sqlite://") {
            connection_url.to_string()
        } else {
            format!("sqlite://{}", connection_url)
        };

        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect(&db_path)
            .await?;
        self.pool = Some(pool);
        Ok(())
    }

    async fn disconnect(&mut self) -> Result<()> {
        if let Some(pool) = self.pool.take() {
            pool.close().await;
        }
        Ok(())
    }

    async fn is_connected(&self) -> bool {
        self.pool.is_some()
    }

    async fn execute_query(&self, sql: &str) -> QueryExecutionResult {
        let start_time = std::time::Instant::now();

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
            || sql.to_lowercase().trim_start().starts_with("pragma");

        if is_select {
            match sqlx::query(sql).fetch_all(pool).await {
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

                    let columns: Vec<String> = rows[0]
                        .columns()
                        .iter()
                        .map(|col| col.name().to_string())
                        .collect();

                    let mut result_rows = Vec::new();
                    for row in &rows {
                        let mut string_row = Vec::new();
                        for (i, column) in row.columns().iter().enumerate() {
                            let value = match row.try_get_raw(i) {
                                Ok(raw_value) => {
                                    if raw_value.is_null() {
                                        "NULL".to_string()
                                    } else {
                                        // SQLite has simpler type system
                                        match column.type_info().name() {
                                            "INTEGER" => row
                                                .try_get::<i64, _>(i)
                                                .map(|v| v.to_string())
                                                .unwrap_or_else(|_| {
                                                    row.try_get::<i32, _>(i)
                                                        .map(|v| v.to_string())
                                                        .unwrap_or_else(|_| "NULL".to_string())
                                                }),
                                            "REAL" => row
                                                .try_get::<f64, _>(i)
                                                .map(|v| v.to_string())
                                                .unwrap_or_else(|_| "NULL".to_string()),
                                            "TEXT" => row
                                                .try_get::<String, _>(i)
                                                .unwrap_or_else(|_| "NULL".to_string()),
                                            "BLOB" => match row.try_get::<Vec<u8>, _>(i) {
                                                Ok(bytes) => format!(
                                                    "\\x{}",
                                                    hex::encode(&bytes[..bytes.len().min(16)])
                                                ),
                                                Err(_) => "BINARY".to_string(),
                                            },
                                            _ => row
                                                .try_get::<String, _>(i)
                                                .unwrap_or_else(|_| {
                                                    row.try_get::<i64, _>(i)
                                                        .map(|v| v.to_string())
                                                        .unwrap_or_else(|_| {
                                                            row.try_get::<f64, _>(i)
                                                                .map(|v| v.to_string())
                                                                .unwrap_or_else(|_| "NULL".to_string())
                                                        })
                                                }),
                                        }
                                    }
                                }
                                Err(_) => "ERROR".to_string(),
                            };
                            string_row.push(value);
                        }
                        result_rows.push(string_row);
                    }

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
            match sqlx::query(sql).execute(pool).await {
                Ok(result) => {
                    let execution_time = start_time.elapsed().as_millis();
                    QueryExecutionResult::Modified {
                        rows_affected: result.rows_affected(),
                        execution_time_ms: execution_time,
                    }
                }
                Err(e) => QueryExecutionResult::Error(format!("Query failed: {}", e)),
            }
        }
    }

    async fn get_tables(&self) -> Result<Vec<TableInfo>> {
        let pool = self
            .pool
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Database not connected"))?;

        // SQLite doesn't have schemas, so we use "main" as the default schema
        let query = r#"
            SELECT 
                name as table_name,
                'main' as table_schema,
                type as table_type
            FROM sqlite_master
            WHERE type IN ('table', 'view')
            AND name NOT LIKE 'sqlite_%'
            ORDER BY name
        "#;

        let rows = sqlx::query(query).fetch_all(pool).await?;

        let tables = rows
            .into_iter()
            .map(|row| TableInfo {
                table_name: row.get("table_name"),
                table_schema: row.get("table_schema"),
                table_type: row.get("table_type"),
            })
            .collect();

        Ok(tables)
    }

    async fn get_table_columns(
        &self,
        table_name: &str,
        _table_schema: &str, // SQLite doesn't use schemas
    ) -> Result<QueryResult> {
        let pool = self
            .pool
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Database not connected"))?;

        // Use PRAGMA table_info to get column information
        let query = format!("PRAGMA table_info('{}')", table_name);
        let rows = sqlx::query(&query).fetch_all(pool).await?;

        let columns: Vec<ColumnInfo> = rows
            .into_iter()
            .enumerate()
            .map(|(idx, row)| {
                let col_name: String = row.get("name");
                let col_type: String = row.get("type");
                let not_null: i32 = row.get("notnull");
                let default_value: Option<String> = row.try_get("dflt_value").ok();
                let pk: i32 = row.get("pk");

                ColumnInfo {
                    column_name: col_name,
                    data_type: col_type,
                    is_nullable: if not_null == 0 { "YES" } else { "NO" }.to_string(),
                    column_default: default_value.or_else(|| {
                        if pk > 0 {
                            Some("PRIMARY KEY".to_string())
                        } else {
                            None
                        }
                    }),
                    ordinal_position: (idx + 1) as i32,
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
        let pool = self
            .pool
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Database not connected"))?;

        let _: (i32,) = sqlx::query_as("SELECT 1").fetch_one(pool).await?;
        Ok(true)
    }

    fn database_type(&self) -> DatabaseType {
        DatabaseType::SQLite
    }
}