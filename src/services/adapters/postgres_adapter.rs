use crate::services::database_adapter::{
    ColumnInfo, DatabaseAdapter, DatabaseType, QueryExecutionResult, QueryResult, TableInfo,
};
use anyhow::Result;
use async_trait::async_trait;
use sqlx::{postgres::PgPoolOptions, Column, PgPool, Row, TypeInfo, ValueRef};

pub struct PostgresAdapter {
    pool: Option<PgPool>,
}

impl PostgresAdapter {
    pub fn new() -> Self {
        Self { pool: None }
    }
}

#[async_trait]
impl DatabaseAdapter for PostgresAdapter {
    async fn connect(&mut self, connection_url: &str) -> Result<()> {
        let pool = PgPoolOptions::new()
            .max_connections(10)
            .connect(connection_url)
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
            || sql.to_lowercase().trim_start().starts_with("with");

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
                                        match column.type_info().name() {
                                            "BOOL" => row
                                                .try_get::<bool, _>(i)
                                                .map(|v| v.to_string())
                                                .unwrap_or_else(|_| "NULL".to_string()),
                                            "INT2" | "INT4" => row
                                                .try_get::<i32, _>(i)
                                                .map(|v| v.to_string())
                                                .unwrap_or_else(|_| "NULL".to_string()),
                                            "INT8" => row
                                                .try_get::<i64, _>(i)
                                                .map(|v| v.to_string())
                                                .unwrap_or_else(|_| "NULL".to_string()),
                                            "FLOAT4" => row
                                                .try_get::<f32, _>(i)
                                                .map(|v| v.to_string())
                                                .unwrap_or_else(|_| "NULL".to_string()),
                                            "FLOAT8" => row
                                                .try_get::<f64, _>(i)
                                                .map(|v| v.to_string())
                                                .unwrap_or_else(|_| "NULL".to_string()),
                                            "NUMERIC" => row
                                                .try_get::<rust_decimal::Decimal, _>(i)
                                                .map(|v| v.to_string())
                                                .unwrap_or_else(|_| {
                                                    row.try_get::<String, _>(i)
                                                        .unwrap_or_else(|_| "NULL".to_string())
                                                }),
                                            "MONEY" => row
                                                .try_get::<String, _>(i)
                                                .unwrap_or_else(|_| "NULL".to_string()),
                                            "DATE" | "TIME" | "TIMESTAMP" | "TIMESTAMPTZ"
                                            | "TIMETZ" => row
                                                .try_get::<String, _>(i)
                                                .unwrap_or_else(|_| "NULL".to_string()),
                                            "UUID" => row
                                                .try_get::<String, _>(i)
                                                .unwrap_or_else(|_| "NULL".to_string()),
                                            "JSON" | "JSONB" => row
                                                .try_get::<String, _>(i)
                                                .unwrap_or_else(|_| "NULL".to_string()),
                                            "BYTEA" => match row.try_get::<Vec<u8>, _>(i) {
                                                Ok(bytes) => format!(
                                                    "\\x{}",
                                                    hex::encode(&bytes[..bytes.len().min(16)])
                                                ),
                                                Err(_) => "BINARY".to_string(),
                                            },
                                            _ => row
                                                .try_get::<String, _>(i)
                                                .unwrap_or_else(|_| "NULL".to_string()),
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

        let query = r#"
            SELECT
                table_name,
                table_schema,
                table_type
            FROM information_schema.tables
            WHERE table_schema NOT IN ('information_schema', 'pg_catalog')
            ORDER BY table_schema, table_name
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
        table_schema: &str,
    ) -> Result<QueryResult> {
        let pool = self
            .pool
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Database not connected"))?;

        let query = r#"
            SELECT
                column_name,
                data_type,
                is_nullable,
                column_default,
                ordinal_position
            FROM information_schema.columns
            WHERE table_name = $1 AND table_schema = $2
            ORDER BY ordinal_position
        "#;

        let rows = sqlx::query(query)
            .bind(table_name)
            .bind(table_schema)
            .fetch_all(pool)
            .await?;

        let columns: Vec<ColumnInfo> = rows
            .into_iter()
            .map(|row| ColumnInfo {
                column_name: row.get("column_name"),
                data_type: row.get("data_type"),
                is_nullable: row.get("is_nullable"),
                column_default: row.get("column_default"),
                ordinal_position: row.get("ordinal_position"),
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
        DatabaseType::PostgreSQL
    }
}