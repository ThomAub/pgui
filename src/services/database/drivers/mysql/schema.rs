//! MySQL schema introspection implementation.
//!
//! This module implements the `SchemaIntrospection` trait for MySQL,
//! providing methods to query database metadata (tables, columns, indexes, etc.).

use anyhow::Result;
use async_trait::async_trait;
use sqlx::Row;

use super::connection::MySqlConnection;
use crate::services::database::traits::{
    ColumnDetail, ConstraintInfo, DatabaseConnection, DatabaseInfo, DatabaseSchema,
    ForeignKeyInfo, IndexInfo, SchemaIntrospection, TableInfo, TableSchema,
};

#[async_trait]
impl SchemaIntrospection for MySqlConnection {
    async fn get_databases(&self) -> Result<Vec<DatabaseInfo>> {
        let pool = self.get_pool_internal().await?;

        let query = "SHOW DATABASES";

        let rows = sqlx::query(query).fetch_all(&pool).await?;

        // Get the current database name from the config
        let current_db = match &self.connection_config().params {
            crate::services::database::traits::ConnectionParams::Server { database, .. } => {
                database.clone()
            }
            _ => String::new(),
        };

        let databases = rows
            .into_iter()
            .map(|row| {
                let name: String = row.get(0);
                let is_current = name == current_db;
                DatabaseInfo { name, is_current }
            })
            .collect();

        Ok(databases)
    }

    async fn get_tables(&self) -> Result<Vec<TableInfo>> {
        let pool = self.get_pool_internal().await?;

        let query = r#"
            SELECT
                TABLE_NAME as table_name,
                TABLE_SCHEMA as table_schema,
                TABLE_TYPE as table_type,
                TABLE_COMMENT as description
            FROM information_schema.TABLES
            WHERE TABLE_SCHEMA = DATABASE()
            ORDER BY TABLE_NAME
        "#;

        let rows = sqlx::query(query).fetch_all(&pool).await?;

        let tables = rows
            .into_iter()
            .map(|row| {
                let description: Option<String> = row.get("description");
                TableInfo {
                    table_name: row.get("table_name"),
                    table_schema: row.get("table_schema"),
                    table_type: row.get("table_type"),
                    description: description.filter(|s| !s.is_empty()),
                }
            })
            .collect();

        Ok(tables)
    }

    async fn get_schema(&self, tables_filter: Option<Vec<String>>) -> Result<DatabaseSchema> {
        let pool = self.get_pool_internal().await?;

        let table_query = r#"
            SELECT
                TABLE_NAME as table_name,
                TABLE_SCHEMA as table_schema,
                TABLE_TYPE as table_type,
                TABLE_COMMENT as description
            FROM information_schema.TABLES
            WHERE TABLE_SCHEMA = DATABASE()
            ORDER BY TABLE_NAME
        "#;

        let table_rows = sqlx::query(table_query).fetch_all(&pool).await?;
        let mut tables = Vec::new();

        for table_row in table_rows {
            let table_name: String = table_row.get("table_name");
            let table_schema: String = table_row.get("table_schema");
            let table_type: String = table_row.get("table_type");
            let description: Option<String> = table_row.get("description");
            let description = description.filter(|s| !s.is_empty());

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
        let pool = self.get_pool_internal().await?;

        let query = r#"
            SELECT
                COLUMN_NAME as column_name,
                DATA_TYPE as data_type,
                IS_NULLABLE as is_nullable,
                COLUMN_DEFAULT as column_default,
                ORDINAL_POSITION as ordinal_position,
                CHARACTER_MAXIMUM_LENGTH as character_maximum_length,
                NUMERIC_PRECISION as numeric_precision,
                NUMERIC_SCALE as numeric_scale,
                COLUMN_COMMENT as description
            FROM information_schema.COLUMNS
            WHERE TABLE_NAME = ? AND TABLE_SCHEMA = ?
            ORDER BY ORDINAL_POSITION
        "#;

        let rows = sqlx::query(query)
            .bind(table)
            .bind(schema)
            .fetch_all(&pool)
            .await?;

        Ok(rows
            .into_iter()
            .map(|row| {
                let is_nullable: String = row.get("is_nullable");
                let description: Option<String> = row.get("description");
                let ordinal: u32 = row.get("ordinal_position");
                let char_max_len: Option<i64> = row.get("character_maximum_length");
                let num_precision: Option<u32> = row.get("numeric_precision");
                let num_scale: Option<u32> = row.get("numeric_scale");

                ColumnDetail {
                    column_name: row.get("column_name"),
                    data_type: row.get("data_type"),
                    is_nullable: is_nullable == "YES",
                    column_default: row.get("column_default"),
                    ordinal_position: ordinal as i32,
                    character_maximum_length: char_max_len.map(|v| v as i32),
                    numeric_precision: num_precision.map(|v| v as i32),
                    numeric_scale: num_scale.map(|v| v as i32),
                    description: description.filter(|s| !s.is_empty()),
                }
            })
            .collect())
    }

    async fn get_primary_keys(&self, table: &str, schema: &str) -> Result<Vec<String>> {
        let pool = self.get_pool_internal().await?;

        let query = r#"
            SELECT COLUMN_NAME as column_name
            FROM information_schema.KEY_COLUMN_USAGE
            WHERE TABLE_NAME = ?
                AND TABLE_SCHEMA = ?
                AND CONSTRAINT_NAME = 'PRIMARY'
            ORDER BY ORDINAL_POSITION
        "#;

        let rows = sqlx::query(query)
            .bind(table)
            .bind(schema)
            .fetch_all(&pool)
            .await?;

        Ok(rows.into_iter().map(|row| row.get("column_name")).collect())
    }

    async fn get_foreign_keys(&self, table: &str, schema: &str) -> Result<Vec<ForeignKeyInfo>> {
        let pool = self.get_pool_internal().await?;

        let query = r#"
            SELECT
                kcu.CONSTRAINT_NAME as constraint_name,
                kcu.COLUMN_NAME as column_name,
                kcu.REFERENCED_TABLE_SCHEMA as foreign_table_schema,
                kcu.REFERENCED_TABLE_NAME as foreign_table_name,
                kcu.REFERENCED_COLUMN_NAME as foreign_column_name
            FROM information_schema.KEY_COLUMN_USAGE kcu
            WHERE kcu.TABLE_NAME = ?
                AND kcu.TABLE_SCHEMA = ?
                AND kcu.REFERENCED_TABLE_NAME IS NOT NULL
        "#;

        let rows = sqlx::query(query)
            .bind(table)
            .bind(schema)
            .fetch_all(&pool)
            .await?;

        Ok(rows
            .into_iter()
            .map(|row| ForeignKeyInfo {
                constraint_name: row.get("constraint_name"),
                column_name: row.get("column_name"),
                foreign_table_schema: row.get("foreign_table_schema"),
                foreign_table_name: row.get("foreign_table_name"),
                foreign_column_name: row.get("foreign_column_name"),
            })
            .collect())
    }

    async fn get_indexes(&self, table: &str, schema: &str) -> Result<Vec<IndexInfo>> {
        let pool = self.get_pool_internal().await?;

        let query = r#"
            SELECT
                INDEX_NAME as index_name,
                GROUP_CONCAT(COLUMN_NAME ORDER BY SEQ_IN_INDEX) as columns,
                NOT NON_UNIQUE as is_unique,
                INDEX_NAME = 'PRIMARY' as is_primary,
                INDEX_TYPE as index_type
            FROM information_schema.STATISTICS
            WHERE TABLE_NAME = ?
                AND TABLE_SCHEMA = ?
            GROUP BY INDEX_NAME, NON_UNIQUE, INDEX_TYPE
        "#;

        let rows = sqlx::query(query)
            .bind(table)
            .bind(schema)
            .fetch_all(&pool)
            .await?;

        Ok(rows
            .into_iter()
            .map(|row| {
                let columns_str: String = row.get("columns");
                let columns: Vec<String> = columns_str.split(',').map(|s| s.to_string()).collect();
                let is_unique: i32 = row.get("is_unique");
                let is_primary: i32 = row.get("is_primary");

                IndexInfo {
                    index_name: row.get("index_name"),
                    columns,
                    is_unique: is_unique != 0,
                    is_primary: is_primary != 0,
                    index_type: row.get("index_type"),
                }
            })
            .collect())
    }

    async fn get_constraints(&self, table: &str, schema: &str) -> Result<Vec<ConstraintInfo>> {
        let pool = self.get_pool_internal().await?;

        // MySQL stores constraints in TABLE_CONSTRAINTS and CHECK_CONSTRAINTS
        let query = r#"
            SELECT
                tc.CONSTRAINT_NAME as constraint_name,
                tc.CONSTRAINT_TYPE as constraint_type,
                GROUP_CONCAT(kcu.COLUMN_NAME) as columns,
                cc.CHECK_CLAUSE as check_clause
            FROM information_schema.TABLE_CONSTRAINTS tc
            LEFT JOIN information_schema.KEY_COLUMN_USAGE kcu
                ON tc.CONSTRAINT_NAME = kcu.CONSTRAINT_NAME
                AND tc.TABLE_SCHEMA = kcu.TABLE_SCHEMA
                AND tc.TABLE_NAME = kcu.TABLE_NAME
            LEFT JOIN information_schema.CHECK_CONSTRAINTS cc
                ON tc.CONSTRAINT_NAME = cc.CONSTRAINT_NAME
                AND tc.CONSTRAINT_SCHEMA = cc.CONSTRAINT_SCHEMA
            WHERE tc.TABLE_NAME = ?
                AND tc.TABLE_SCHEMA = ?
                AND tc.CONSTRAINT_TYPE IN ('UNIQUE', 'CHECK')
            GROUP BY tc.CONSTRAINT_NAME, tc.CONSTRAINT_TYPE, cc.CHECK_CLAUSE
        "#;

        let rows = sqlx::query(query)
            .bind(table)
            .bind(schema)
            .fetch_all(&pool)
            .await?;

        Ok(rows
            .into_iter()
            .map(|row| {
                let columns_str: Option<String> = row.get("columns");
                let columns: Vec<String> = columns_str
                    .map(|s| s.split(',').map(|c| c.to_string()).collect())
                    .unwrap_or_default();

                ConstraintInfo {
                    constraint_name: row.get("constraint_name"),
                    constraint_type: row.get("constraint_type"),
                    columns,
                    check_clause: row.get("check_clause"),
                }
            })
            .collect())
    }
}
