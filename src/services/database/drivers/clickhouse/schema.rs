//! ClickHouse schema introspection implementation.
//!
//! This module implements the `SchemaIntrospection` trait for ClickHouse,
//! providing methods to query database metadata (tables, columns, etc.).
//!
//! ClickHouse stores metadata in system tables:
//! - system.databases - list of databases
//! - system.tables - tables and views
//! - system.columns - column information
//! - system.data_skipping_indices - skip indexes
//!
//! Note: ClickHouse uses a different indexing model than traditional RDBMS.
//! Primary keys define data ordering (sorting keys), not unique constraints.

use anyhow::Result;
use async_trait::async_trait;

use super::connection::ClickHouseConnection;
use crate::services::database::traits::{
    ColumnDetail, ConstraintInfo, DatabaseConnection, DatabaseInfo, DatabaseSchema,
    ForeignKeyInfo, IndexInfo, QueryExecutionResult, SchemaIntrospection, TableInfo, TableSchema,
    Value,
};

#[async_trait]
impl SchemaIntrospection for ClickHouseConnection {
    async fn get_databases(&self) -> Result<Vec<DatabaseInfo>> {
        let query = "SELECT name FROM system.databases ORDER BY name";

        let result = self.execute_query(query).await?;

        // Get the current database from config
        let current_db = match &self.connection_config().params {
            crate::services::database::traits::ConnectionParams::Server { database, .. } => {
                database.clone()
            }
            _ => String::new(),
        };

        match result {
            QueryExecutionResult::Select(select) => {
                let databases = select
                    .rows
                    .iter()
                    .filter_map(|row| {
                        let name = row.get_value(0)?.as_str()?.to_string();
                        let is_current = name == current_db;
                        Some(DatabaseInfo { name, is_current })
                    })
                    .collect();
                Ok(databases)
            }
            QueryExecutionResult::Error(e) => Err(anyhow::anyhow!("Query failed: {}", e.message)),
            _ => Ok(vec![]),
        }
    }

    async fn get_tables(&self) -> Result<Vec<TableInfo>> {
        let query = r#"
            SELECT
                name AS table_name,
                database AS table_schema,
                if(engine LIKE '%View%', 'VIEW', 'BASE TABLE') AS table_type,
                comment AS description
            FROM system.tables
            WHERE database = currentDatabase()
            ORDER BY name
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
                        let description = match row.get_value(3) {
                            Some(Value::Text(s)) if !s.is_empty() => Some(s.clone()),
                            _ => None,
                        };
                        Some(TableInfo {
                            table_name,
                            table_schema,
                            table_type,
                            description,
                        })
                    })
                    .collect();
                Ok(tables)
            }
            QueryExecutionResult::Error(e) => Err(anyhow::anyhow!("Query failed: {}", e.message)),
            _ => Ok(vec![]),
        }
    }

    async fn get_schema(&self, tables_filter: Option<Vec<String>>) -> Result<DatabaseSchema> {
        let table_query = r#"
            SELECT
                name AS table_name,
                database AS table_schema,
                if(engine LIKE '%View%', 'VIEW', 'BASE TABLE') AS table_type,
                comment AS description
            FROM system.tables
            WHERE database = currentDatabase()
            ORDER BY name
        "#;

        let result = self.execute_query(table_query).await?;

        let table_infos: Vec<(String, String, String, Option<String>)> = match result {
            QueryExecutionResult::Select(select) => select
                .rows
                .iter()
                .filter_map(|row| {
                    let table_name = row.get_value(0)?.as_str()?.to_string();
                    let table_schema = row.get_value(1)?.as_str()?.to_string();
                    let table_type = row.get_value(2)?.as_str()?.to_string();
                    let description = match row.get_value(3) {
                        Some(Value::Text(s)) if !s.is_empty() => Some(s.clone()),
                        _ => None,
                    };
                    Some((table_name, table_schema, table_type, description))
                })
                .collect(),
            _ => vec![],
        };

        let mut tables = Vec::new();

        for (table_name, table_schema, table_type, description) in table_infos {
            // Apply filter if specified
            if let Some(ref filter) = tables_filter {
                if !filter.contains(&table_name) {
                    continue;
                }
            }

            // Get detailed schema info
            let columns = self.get_columns(&table_name, &table_schema).await?;
            let primary_keys = self.get_primary_keys(&table_name, &table_schema).await?;
            let foreign_keys = self.get_foreign_keys(&table_name, &table_schema).await?;
            let indexes = self.get_indexes(&table_name, &table_schema).await?;
            let constraints = self.get_constraints(&table_name, &table_schema).await?;

            tables.push(TableSchema {
                table_name,
                table_schema,
                table_type,
                columns,
                primary_keys,
                foreign_keys,
                indexes,
                constraints,
                description,
            });
        }

        Ok(DatabaseSchema::new(tables))
    }

    async fn get_columns(&self, table: &str, schema: &str) -> Result<Vec<ColumnDetail>> {
        let query = format!(
            r#"
            SELECT
                name AS column_name,
                type AS data_type,
                if(position(type, 'Nullable') > 0, 'YES', 'NO') AS is_nullable,
                default_expression AS column_default,
                position AS ordinal_position,
                0 AS character_maximum_length,
                0 AS numeric_precision,
                0 AS numeric_scale,
                comment AS description
            FROM system.columns
            WHERE table = '{}' AND database = '{}'
            ORDER BY position
            "#,
            escape_string(table),
            escape_string(schema)
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
                        let is_nullable_str = row.get_value(2)?.as_str()?;
                        let is_nullable = is_nullable_str == "YES";
                        let column_default = match row.get_value(3) {
                            Some(Value::Text(s)) if !s.is_empty() => Some(s.clone()),
                            _ => None,
                        };
                        let ordinal_position = row.get_value(4)?.as_i64()? as i32;
                        let description = match row.get_value(8) {
                            Some(Value::Text(s)) if !s.is_empty() => Some(s.clone()),
                            _ => None,
                        };

                        Some(ColumnDetail {
                            column_name,
                            data_type,
                            is_nullable,
                            column_default,
                            ordinal_position,
                            character_maximum_length: None,
                            numeric_precision: None,
                            numeric_scale: None,
                            description,
                        })
                    })
                    .collect();
                Ok(columns)
            }
            QueryExecutionResult::Error(e) => Err(anyhow::anyhow!("Query failed: {}", e.message)),
            _ => Ok(vec![]),
        }
    }

    async fn get_primary_keys(&self, table: &str, schema: &str) -> Result<Vec<String>> {
        // ClickHouse uses sorting_key instead of traditional primary keys
        // The primary key is typically the same as or prefix of sorting key
        let query = format!(
            r#"
            SELECT sorting_key
            FROM system.tables
            WHERE name = '{}' AND database = '{}'
            "#,
            escape_string(table),
            escape_string(schema)
        );

        let result = self.execute_query(&query).await?;

        match result {
            QueryExecutionResult::Select(select) => {
                if let Some(row) = select.rows.first() {
                    if let Some(Value::Text(sorting_key)) = row.get_value(0) {
                        // Parse sorting key (format: "col1, col2, ...")
                        let keys: Vec<String> = sorting_key
                            .split(',')
                            .map(|s| s.trim().to_string())
                            .filter(|s| !s.is_empty())
                            .collect();
                        return Ok(keys);
                    }
                }
                Ok(vec![])
            }
            QueryExecutionResult::Error(e) => Err(anyhow::anyhow!("Query failed: {}", e.message)),
            _ => Ok(vec![]),
        }
    }

    async fn get_foreign_keys(&self, _table: &str, _schema: &str) -> Result<Vec<ForeignKeyInfo>> {
        // ClickHouse doesn't have foreign key constraints
        // It's an OLAP database optimized for analytical queries, not referential integrity
        Ok(vec![])
    }

    async fn get_indexes(&self, table: &str, schema: &str) -> Result<Vec<IndexInfo>> {
        // ClickHouse has data skipping indexes (for MergeTree family)
        let query = format!(
            r#"
            SELECT
                name AS index_name,
                expr AS columns,
                type AS index_type
            FROM system.data_skipping_indices
            WHERE table = '{}' AND database = '{}'
            "#,
            escape_string(table),
            escape_string(schema)
        );

        let result = self.execute_query(&query).await?;

        match result {
            QueryExecutionResult::Select(select) => {
                let indexes = select
                    .rows
                    .iter()
                    .filter_map(|row| {
                        let index_name = row.get_value(0)?.as_str()?.to_string();
                        let columns_expr = row.get_value(1)?.as_str()?.to_string();
                        let index_type = row.get_value(2)?.as_str()?.to_string();

                        // Parse column expression
                        let columns: Vec<String> = columns_expr
                            .split(',')
                            .map(|s| s.trim().to_string())
                            .filter(|s| !s.is_empty())
                            .collect();

                        Some(IndexInfo {
                            index_name,
                            columns,
                            is_unique: false, // Skip indexes are not for uniqueness
                            is_primary: false,
                            index_type,
                        })
                    })
                    .collect();
                Ok(indexes)
            }
            QueryExecutionResult::Error(e) => Err(anyhow::anyhow!("Query failed: {}", e.message)),
            _ => Ok(vec![]),
        }
    }

    async fn get_constraints(&self, _table: &str, _schema: &str) -> Result<Vec<ConstraintInfo>> {
        // ClickHouse has limited constraint support
        // It supports ASSUME constraints but they're not enforced
        // For now, return empty as there's no standard constraint mechanism
        Ok(vec![])
    }
}

/// Escape a string for use in ClickHouse queries.
fn escape_string(s: &str) -> String {
    s.replace('\\', "\\\\").replace('\'', "\\'")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_escape_string() {
        assert_eq!(escape_string("test"), "test");
        assert_eq!(escape_string("test's"), "test\\'s");
        assert_eq!(escape_string("path\\to\\file"), "path\\\\to\\\\file");
    }
}
