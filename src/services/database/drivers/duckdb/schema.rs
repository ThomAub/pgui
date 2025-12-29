//! DuckDB schema introspection implementation.
//!
//! This module implements the `SchemaIntrospection` trait for DuckDB,
//! providing methods to query database metadata (tables, columns, indexes, etc.).

use anyhow::Result;
use async_trait::async_trait;

use super::connection::DuckDbConnection;
use crate::services::database::traits::{
    ColumnDetail, ConstraintInfo, DatabaseConnection, DatabaseInfo, DatabaseSchema,
    ForeignKeyInfo, IndexInfo, QueryExecutionResult, SchemaIntrospection, TableInfo, TableSchema,
    Value,
};

#[async_trait]
impl SchemaIntrospection for DuckDbConnection {
    async fn get_databases(&self) -> Result<Vec<DatabaseInfo>> {
        // DuckDB doesn't have multiple databases in the traditional sense
        // It has schemas within a single database
        let result = self.execute_query("PRAGMA database_list").await?;

        match result {
            QueryExecutionResult::Select(select) => {
                let databases: Vec<DatabaseInfo> = select
                    .rows
                    .iter()
                    .filter_map(|row| {
                        row.get_value(1).and_then(|v| {
                            if let Value::Text(name) = v {
                                Some(DatabaseInfo {
                                    name: name.clone(),
                                    is_current: name == "main",
                                })
                            } else {
                                None
                            }
                        })
                    })
                    .collect();
                Ok(databases)
            }
            _ => Ok(vec![DatabaseInfo {
                name: "main".to_string(),
                is_current: true,
            }]),
        }
    }

    async fn get_tables(&self) -> Result<Vec<TableInfo>> {
        let query = r#"
            SELECT
                table_name,
                table_schema,
                table_type
            FROM information_schema.tables
            WHERE table_schema NOT IN ('information_schema', 'pg_catalog')
            ORDER BY table_name
        "#;

        let result = self.execute_query(query).await?;

        match result {
            QueryExecutionResult::Select(select) => {
                let tables = select
                    .rows
                    .iter()
                    .filter_map(|row| {
                        let table_name = row.get_value(0)?.as_str()?.to_string();
                        let table_schema = row.get_value(1)?.as_str()?.to_string();
                        let table_type = row.get_value(2)?.as_str()?.to_string();

                        Some(TableInfo {
                            table_name,
                            table_schema,
                            table_type,
                            description: None,
                        })
                    })
                    .collect();
                Ok(tables)
            }
            QueryExecutionResult::Error(e) => Err(anyhow::anyhow!("{}", e.message)),
            _ => Ok(vec![]),
        }
    }

    async fn get_schema(&self, tables_filter: Option<Vec<String>>) -> Result<DatabaseSchema> {
        let tables_info = self.get_tables().await?;
        let mut tables = Vec::new();

        for table_info in tables_info {
            // Apply filter if specified
            if let Some(ref filter) = tables_filter {
                if !filter.contains(&table_info.table_name) {
                    continue;
                }
            }

            let columns = self
                .get_columns(&table_info.table_name, &table_info.table_schema)
                .await?;
            let primary_keys = self
                .get_primary_keys(&table_info.table_name, &table_info.table_schema)
                .await?;
            let foreign_keys = self
                .get_foreign_keys(&table_info.table_name, &table_info.table_schema)
                .await?;
            let indexes = self
                .get_indexes(&table_info.table_name, &table_info.table_schema)
                .await?;
            let constraints = self
                .get_constraints(&table_info.table_name, &table_info.table_schema)
                .await?;

            tables.push(TableSchema {
                table_name: table_info.table_name,
                table_schema: table_info.table_schema,
                table_type: table_info.table_type,
                columns,
                primary_keys,
                foreign_keys,
                indexes,
                constraints,
                description: table_info.description,
            });
        }

        Ok(DatabaseSchema::new(tables))
    }

    async fn get_columns(&self, table: &str, schema: &str) -> Result<Vec<ColumnDetail>> {
        let query = format!(
            r#"
            SELECT
                column_name,
                data_type,
                is_nullable,
                column_default,
                ordinal_position,
                character_maximum_length,
                numeric_precision,
                numeric_scale
            FROM information_schema.columns
            WHERE table_name = '{}'
                AND table_schema = '{}'
            ORDER BY ordinal_position
            "#,
            table.replace('\'', "''"),
            schema.replace('\'', "''")
        );

        let result = self.execute_query(&query).await?;

        match result {
            QueryExecutionResult::Select(select) => {
                let columns = select
                    .rows
                    .iter()
                    .filter_map(|row| {
                        let column_name = row.get_value(0)?.as_str()?.to_string();
                        let data_type = row.get_value(1)?.as_str()?.to_string();
                        let is_nullable = row
                            .get_value(2)
                            .and_then(|v| v.as_str())
                            .unwrap_or("NO");
                        let column_default = row.get_value(3).and_then(|v| v.as_str()).map(String::from);
                        let ordinal = row.get_value(4).and_then(|v| v.as_i64()).unwrap_or(0) as i32;
                        let char_max_len = row.get_value(5).and_then(|v| v.as_i64()).map(|v| v as i32);
                        let num_precision = row.get_value(6).and_then(|v| v.as_i64()).map(|v| v as i32);
                        let num_scale = row.get_value(7).and_then(|v| v.as_i64()).map(|v| v as i32);

                        Some(ColumnDetail {
                            column_name,
                            data_type,
                            is_nullable: is_nullable.to_uppercase() == "YES",
                            column_default,
                            ordinal_position: ordinal,
                            character_maximum_length: char_max_len,
                            numeric_precision: num_precision,
                            numeric_scale: num_scale,
                            description: None,
                        })
                    })
                    .collect();
                Ok(columns)
            }
            QueryExecutionResult::Error(e) => Err(anyhow::anyhow!("{}", e.message)),
            _ => Ok(vec![]),
        }
    }

    async fn get_primary_keys(&self, table: &str, schema: &str) -> Result<Vec<String>> {
        // DuckDB stores constraint info in duckdb_constraints()
        let query = format!(
            r#"
            SELECT constraint_column_names
            FROM duckdb_constraints()
            WHERE table_name = '{}'
                AND schema_name = '{}'
                AND constraint_type = 'PRIMARY KEY'
            "#,
            table.replace('\'', "''"),
            schema.replace('\'', "''")
        );

        let result = self.execute_query(&query).await?;

        match result {
            QueryExecutionResult::Select(select) => {
                let mut pks = Vec::new();
                for row in &select.rows {
                    if let Some(Value::Text(cols)) = row.get_value(0) {
                        // Parse the list format [col1, col2, ...]
                        let cleaned = cols.trim_matches(|c| c == '[' || c == ']');
                        for col in cleaned.split(',') {
                            let col = col.trim().trim_matches('"').trim_matches('\'');
                            if !col.is_empty() {
                                pks.push(col.to_string());
                            }
                        }
                    }
                }
                Ok(pks)
            }
            _ => Ok(vec![]),
        }
    }

    async fn get_foreign_keys(&self, table: &str, schema: &str) -> Result<Vec<ForeignKeyInfo>> {
        let query = format!(
            r#"
            SELECT
                constraint_text,
                constraint_column_names
            FROM duckdb_constraints()
            WHERE table_name = '{}'
                AND schema_name = '{}'
                AND constraint_type = 'FOREIGN KEY'
            "#,
            table.replace('\'', "''"),
            schema.replace('\'', "''")
        );

        let result = self.execute_query(&query).await?;

        match result {
            QueryExecutionResult::Select(select) => {
                let mut fks = Vec::new();
                for row in &select.rows {
                    if let (Some(Value::Text(constraint_text)), Some(Value::Text(cols))) =
                        (row.get_value(0), row.get_value(1))
                    {
                        let columns: Vec<String> = cols
                            .trim_matches(|c| c == '[' || c == ']')
                            .split(',')
                            .map(|s| s.trim().trim_matches('"').trim_matches('\'').to_string())
                            .filter(|s| !s.is_empty())
                            .collect();

                        for col in columns {
                            fks.push(ForeignKeyInfo {
                                constraint_name: constraint_text.clone(),
                                column_name: col,
                                foreign_table_schema: schema.to_string(),
                                foreign_table_name: "unknown".to_string(),
                                foreign_column_name: "unknown".to_string(),
                            });
                        }
                    }
                }
                Ok(fks)
            }
            _ => Ok(vec![]),
        }
    }

    async fn get_indexes(&self, table: &str, schema: &str) -> Result<Vec<IndexInfo>> {
        let query = format!(
            r#"
            SELECT
                index_name,
                is_unique,
                is_primary,
                sql
            FROM duckdb_indexes()
            WHERE table_name = '{}'
                AND schema_name = '{}'
            "#,
            table.replace('\'', "''"),
            schema.replace('\'', "''")
        );

        let result = self.execute_query(&query).await?;

        match result {
            QueryExecutionResult::Select(select) => {
                let indexes = select
                    .rows
                    .iter()
                    .filter_map(|row| {
                        let index_name = row.get_value(0)?.as_str()?.to_string();
                        let is_unique = row.get_value(1).and_then(|v| v.as_bool()).unwrap_or(false);
                        let is_primary = row.get_value(2).and_then(|v| v.as_bool()).unwrap_or(false);

                        Some(IndexInfo {
                            index_name,
                            columns: vec![], // Would need to parse SQL to get columns
                            is_unique,
                            is_primary,
                            index_type: "BTREE".to_string(),
                        })
                    })
                    .collect();
                Ok(indexes)
            }
            _ => Ok(vec![]),
        }
    }

    async fn get_constraints(&self, table: &str, schema: &str) -> Result<Vec<ConstraintInfo>> {
        let query = format!(
            r#"
            SELECT
                constraint_type,
                constraint_text,
                constraint_column_names
            FROM duckdb_constraints()
            WHERE table_name = '{}'
                AND schema_name = '{}'
                AND constraint_type IN ('UNIQUE', 'CHECK')
            "#,
            table.replace('\'', "''"),
            schema.replace('\'', "''")
        );

        let result = self.execute_query(&query).await?;

        match result {
            QueryExecutionResult::Select(select) => {
                let constraints = select
                    .rows
                    .iter()
                    .filter_map(|row| {
                        let constraint_type = row.get_value(0)?.as_str()?.to_string();
                        let constraint_name = row.get_value(1)?.as_str()?.to_string();
                        let columns_str = row
                            .get_value(2)
                            .and_then(|v| v.as_str())
                            .unwrap_or("");

                        let columns: Vec<String> = columns_str
                            .trim_matches(|c| c == '[' || c == ']')
                            .split(',')
                            .map(|s| s.trim().trim_matches('"').trim_matches('\'').to_string())
                            .filter(|s| !s.is_empty())
                            .collect();

                        Some(ConstraintInfo {
                            constraint_name,
                            constraint_type,
                            columns,
                            check_clause: None,
                        })
                    })
                    .collect();
                Ok(constraints)
            }
            _ => Ok(vec![]),
        }
    }
}
