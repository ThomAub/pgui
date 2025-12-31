//! SQLite schema introspection implementation.
//!
//! This module implements the `SchemaIntrospection` trait for SQLite,
//! providing methods to query database metadata using SQLite's PRAGMA statements
//! and sqlite_master table.

use anyhow::Result;
use async_trait::async_trait;
use sqlx::Row;

use super::connection::SqliteConnection;
use crate::services::database::traits::{
    ColumnDetail, ConstraintInfo, DatabaseInfo, DatabaseSchema, ForeignKeyInfo, IndexInfo,
    SchemaIntrospection, TableInfo, TableSchema,
};

#[async_trait]
impl SchemaIntrospection for SqliteConnection {
    async fn get_databases(&self) -> Result<Vec<DatabaseInfo>> {
        // SQLite typically has one "main" database plus any attached databases
        let pool = self.get_pool_internal().await?;

        let query = "PRAGMA database_list";
        let rows = sqlx::query(query).fetch_all(&pool).await?;

        let databases = rows
            .into_iter()
            .map(|row| {
                let name: String = row.get("name");
                let is_current = name == "main";
                DatabaseInfo { name, is_current }
            })
            .collect();

        Ok(databases)
    }

    async fn get_tables(&self) -> Result<Vec<TableInfo>> {
        let pool = self.get_pool_internal().await?;

        // Query sqlite_master for tables and views
        let query = r#"
            SELECT
                name as table_name,
                type as table_type
            FROM sqlite_master
            WHERE type IN ('table', 'view')
                AND name NOT LIKE 'sqlite_%'
            ORDER BY name
        "#;

        let rows = sqlx::query(query).fetch_all(&pool).await?;

        let tables = rows
            .into_iter()
            .map(|row| {
                let table_name: String = row.get("table_name");
                let table_type: String = row.get("table_type");

                TableInfo {
                    table_name,
                    table_schema: "main".to_string(), // SQLite uses "main" as default schema
                    table_type: table_type.to_uppercase(),
                    description: None,
                }
            })
            .collect();

        Ok(tables)
    }

    async fn get_schema(&self, tables_filter: Option<Vec<String>>) -> Result<DatabaseSchema> {
        let pool = self.get_pool_internal().await?;

        // Get all tables
        let table_query = r#"
            SELECT
                name as table_name,
                type as table_type,
                sql
            FROM sqlite_master
            WHERE type IN ('table', 'view')
                AND name NOT LIKE 'sqlite_%'
            ORDER BY name
        "#;

        let table_rows = sqlx::query(table_query).fetch_all(&pool).await?;
        let mut tables = Vec::new();

        for table_row in table_rows {
            let table_name: String = table_row.get("table_name");
            let table_type: String = table_row.get("table_type");

            // Apply filter if specified
            if let Some(ref filter) = tables_filter {
                if !filter.contains(&table_name) {
                    continue;
                }
            }

            // Get detailed schema info
            let columns = self.get_columns(&table_name, "main").await?;
            let primary_keys = self.get_primary_keys(&table_name, "main").await?;
            let foreign_keys = self.get_foreign_keys(&table_name, "main").await?;
            let indexes = self.get_indexes(&table_name, "main").await?;
            let constraints = self.get_constraints(&table_name, "main").await?;

            tables.push(TableSchema {
                table_name,
                table_schema: "main".to_string(),
                table_type: table_type.to_uppercase(),
                columns,
                primary_keys,
                foreign_keys,
                indexes,
                constraints,
                description: None,
            });
        }

        Ok(DatabaseSchema::new(tables))
    }

    async fn get_columns(&self, table: &str, _schema: &str) -> Result<Vec<ColumnDetail>> {
        let pool = self.get_pool_internal().await?;

        // Use PRAGMA table_info to get column information
        let query = format!("PRAGMA table_info('{}')", table.replace('\'', "''"));
        let rows = sqlx::query(&query).fetch_all(&pool).await?;

        Ok(rows
            .into_iter()
            .map(|row| {
                let cid: i32 = row.get("cid");
                let name: String = row.get("name");
                let data_type: String = row.get("type");
                let notnull: i32 = row.get("notnull");
                let dflt_value: Option<String> = row.get("dflt_value");
                let pk: i32 = row.get("pk");

                ColumnDetail {
                    column_name: name,
                    data_type,
                    is_nullable: notnull == 0,
                    column_default: dflt_value,
                    ordinal_position: cid + 1, // SQLite cid is 0-indexed
                    character_maximum_length: None,
                    numeric_precision: None,
                    numeric_scale: None,
                    description: if pk > 0 {
                        Some("Primary Key".to_string())
                    } else {
                        None
                    },
                }
            })
            .collect())
    }

    async fn get_primary_keys(&self, table: &str, _schema: &str) -> Result<Vec<String>> {
        let pool = self.get_pool_internal().await?;

        // PRAGMA table_info includes pk column (1+ for primary key columns)
        let query = format!("PRAGMA table_info('{}')", table.replace('\'', "''"));
        let rows = sqlx::query(&query).fetch_all(&pool).await?;

        let mut pk_columns: Vec<(i32, String)> = rows
            .into_iter()
            .filter_map(|row| {
                let pk: i32 = row.get("pk");
                if pk > 0 {
                    let name: String = row.get("name");
                    Some((pk, name))
                } else {
                    None
                }
            })
            .collect();

        // Sort by pk order (for composite primary keys)
        pk_columns.sort_by_key(|(pk, _)| *pk);

        Ok(pk_columns.into_iter().map(|(_, name)| name).collect())
    }

    async fn get_foreign_keys(&self, table: &str, _schema: &str) -> Result<Vec<ForeignKeyInfo>> {
        let pool = self.get_pool_internal().await?;

        // Use PRAGMA foreign_key_list
        let query = format!("PRAGMA foreign_key_list('{}')", table.replace('\'', "''"));
        let rows = sqlx::query(&query).fetch_all(&pool).await?;

        Ok(rows
            .into_iter()
            .map(|row| {
                let id: i32 = row.get("id");
                let from_col: String = row.get("from");
                let to_table: String = row.get("table");
                let to_col: String = row.get("to");

                ForeignKeyInfo {
                    constraint_name: format!("fk_{}_{}", table, id),
                    column_name: from_col,
                    foreign_table_schema: "main".to_string(),
                    foreign_table_name: to_table,
                    foreign_column_name: to_col,
                }
            })
            .collect())
    }

    async fn get_indexes(&self, table: &str, _schema: &str) -> Result<Vec<IndexInfo>> {
        let pool = self.get_pool_internal().await?;

        // Use PRAGMA index_list to get indexes
        let query = format!("PRAGMA index_list('{}')", table.replace('\'', "''"));
        let index_rows = sqlx::query(&query).fetch_all(&pool).await?;

        let mut indexes = Vec::new();

        for index_row in index_rows {
            let index_name: String = index_row.get("name");
            let unique: i32 = index_row.get("unique");
            let origin: String = index_row.get("origin");

            // Get columns for this index
            let col_query = format!("PRAGMA index_info('{}')", index_name.replace('\'', "''"));
            let col_rows = sqlx::query(&col_query).fetch_all(&pool).await?;

            let columns: Vec<String> = col_rows
                .into_iter()
                .map(|row| row.get("name"))
                .collect();

            indexes.push(IndexInfo {
                index_name: index_name.clone(),
                columns,
                is_unique: unique != 0,
                is_primary: origin == "pk",
                index_type: if origin == "pk" {
                    "PRIMARY".to_string()
                } else {
                    "BTREE".to_string() // SQLite uses B-tree for all indexes
                },
            });
        }

        Ok(indexes)
    }

    async fn get_constraints(&self, table: &str, _schema: &str) -> Result<Vec<ConstraintInfo>> {
        let pool = self.get_pool_internal().await?;

        let mut constraints = Vec::new();

        // Get unique constraints from index_list (origin = 'u')
        let query = format!("PRAGMA index_list('{}')", table.replace('\'', "''"));
        let index_rows = sqlx::query(&query).fetch_all(&pool).await?;

        for index_row in index_rows {
            let index_name: String = index_row.get("name");
            let unique: i32 = index_row.get("unique");
            let origin: String = index_row.get("origin");

            // Only include unique constraints (not primary keys which are handled separately)
            if unique != 0 && origin == "u" {
                let col_query = format!("PRAGMA index_info('{}')", index_name.replace('\'', "''"));
                let col_rows = sqlx::query(&col_query).fetch_all(&pool).await?;

                let columns: Vec<String> = col_rows
                    .into_iter()
                    .map(|row| row.get("name"))
                    .collect();

                constraints.push(ConstraintInfo {
                    constraint_name: index_name,
                    constraint_type: "UNIQUE".to_string(),
                    columns,
                    check_clause: None,
                });
            }
        }

        // Note: SQLite's CHECK constraints would require parsing the CREATE TABLE SQL
        // which is more complex. For now, we only return UNIQUE constraints.

        Ok(constraints)
    }
}
