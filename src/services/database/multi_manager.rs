//! Multi-database manager using the ConnectionFactory.
//!
//! This module provides a unified database manager that works with all
//! supported database types (PostgreSQL, MySQL, SQLite, ClickHouse, DuckDB).

use anyhow::Result;
use async_lock::RwLock;
use std::sync::Arc;

use super::drivers::ConnectionFactory;
use super::traits::{ConnectionConfig, DatabaseType, SchemaIntrospection};
use super::types::{
    ColumnDetail, ConstraintInfo, DatabaseInfo, DatabaseSchema, ErrorResult, ForeignKeyInfo,
    IndexInfo, ModifiedResult, QueryExecutionResult, QueryResult, ResultCell, ResultColumnMetadata,
    ResultRow, TableInfo, TableSchema,
};

use crate::services::database::traits::{
    ColumnDetail as TraitColumnDetail, ConstraintInfo as TraitConstraintInfo,
    DatabaseInfo as TraitDatabaseInfo, DatabaseSchema as TraitDatabaseSchema,
    ForeignKeyInfo as TraitForeignKeyInfo, IndexInfo as TraitIndexInfo, TableInfo as TraitTableInfo,
    TableSchema as TraitTableSchema,
};

use crate::services::database::traits::connection::{
    QueryExecutionResult as TraitQueryResult, SelectResult as TraitSelectResult,
};

/// Multi-database manager that works with all supported database types.
///
/// This manager uses the `ConnectionFactory` to create the appropriate
/// driver for the given database type, and provides a unified interface
/// for all database operations.
#[derive(Clone)]
pub struct MultiDatabaseManager {
    /// The active database connection (if any)
    connection: Arc<RwLock<Option<Box<dyn SchemaIntrospection>>>>,
    /// The current connection configuration
    config: Arc<RwLock<Option<ConnectionConfig>>>,
}

impl std::fmt::Debug for MultiDatabaseManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MultiDatabaseManager").finish()
    }
}

impl MultiDatabaseManager {
    /// Create a new multi-database manager.
    pub fn new() -> Self {
        Self {
            connection: Arc::new(RwLock::new(None)),
            config: Arc::new(RwLock::new(None)),
        }
    }

    /// Connect to a database using the given configuration.
    pub async fn connect(&self, config: ConnectionConfig) -> Result<()> {
        // Create the connection using the factory
        let mut conn = ConnectionFactory::create_with_schema(config.clone())?;

        // Connect
        conn.connect().await?;

        // Store the connection
        let mut conn_guard = self.connection.write().await;
        *conn_guard = Some(conn);

        let mut config_guard = self.config.write().await;
        *config_guard = Some(config);

        Ok(())
    }

    /// Disconnect from the database.
    pub async fn disconnect(&self) -> Result<()> {
        let mut conn_guard = self.connection.write().await;
        if let Some(mut conn) = conn_guard.take() {
            conn.disconnect().await?;
        }

        let mut config_guard = self.config.write().await;
        *config_guard = None;

        Ok(())
    }

    /// Check if connected to a database.
    pub async fn is_connected(&self) -> bool {
        let conn_guard = self.connection.read().await;
        if let Some(ref conn) = *conn_guard {
            conn.is_connected().await
        } else {
            false
        }
    }

    /// Test a connection without permanently connecting.
    ///
    /// This creates a temporary connection, verifies it works, and closes it.
    /// Useful for testing connection parameters before saving them.
    pub async fn test_connection(config: ConnectionConfig) -> Result<()> {
        // Create a temporary connection using the factory
        let mut conn = ConnectionFactory::create_with_schema(config)?;

        // Try to connect
        conn.connect().await?;

        // Verify connection is working
        if !conn.is_connected().await {
            return Err(anyhow::anyhow!("Connection test failed: not connected"));
        }

        // Disconnect
        conn.disconnect().await?;

        Ok(())
    }

    /// Get the current database type.
    pub async fn database_type(&self) -> Option<DatabaseType> {
        let config_guard = self.config.read().await;
        config_guard.as_ref().map(|c| c.database_type)
    }

    /// Stream query results.
    ///
    /// Note: Streaming export is not yet supported for multi-database connections.
    /// This method is a placeholder that returns an error.
    pub async fn stream_query<'a>(
        &'a self,
        _sql: &'a str,
    ) -> Result<futures::stream::BoxStream<'a, Result<sqlx::postgres::PgRow, sqlx::Error>>, String>
    {
        Err("Streaming export not yet supported for multi-database connections".to_string())
    }

    /// Execute a query and return results.
    pub async fn execute_query_enhanced(&self, sql: &str) -> QueryExecutionResult {
        let conn_guard = self.connection.read().await;

        let conn = match conn_guard.as_ref() {
            Some(c) => c,
            None => {
                return QueryExecutionResult::Error(ErrorResult {
                    message: "Database not connected".to_string(),
                    execution_time_ms: 0,
                });
            }
        };

        let sql = sql.trim();
        if sql.is_empty() {
            return QueryExecutionResult::Error(ErrorResult {
                message: "Empty query".to_string(),
                execution_time_ms: 0,
            });
        }

        // Execute the query using the trait method
        match conn.execute_query(sql).await {
            Ok(result) => self.convert_query_result(result, sql.to_string()),
            Err(e) => QueryExecutionResult::Error(ErrorResult {
                message: format!("Query failed: {}", e),
                execution_time_ms: 0,
            }),
        }
    }

    /// Get a list of databases.
    pub async fn get_databases(&self) -> Result<Vec<DatabaseInfo>> {
        let conn_guard = self.connection.read().await;
        let conn = conn_guard
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Database not connected"))?;

        let databases = conn.get_databases().await?;
        Ok(databases.into_iter().map(Self::convert_database_info).collect())
    }

    /// Get a list of tables.
    pub async fn get_tables(&self) -> Result<Vec<TableInfo>> {
        let conn_guard = self.connection.read().await;
        let conn = conn_guard
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Database not connected"))?;

        let tables = conn.get_tables().await?;
        Ok(tables.into_iter().map(Self::convert_table_info).collect())
    }

    /// Get table columns.
    pub async fn get_table_columns(
        &self,
        table_name: &str,
        table_schema: &str,
    ) -> Result<QueryExecutionResult> {
        let conn_guard = self.connection.read().await;
        let conn = conn_guard
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Database not connected"))?;

        let columns = conn.get_columns(table_name, table_schema).await?;

        // Convert to QueryResult format
        let result_columns = vec![
            ResultColumnMetadata {
                name: "column_name".to_string(),
                type_name: "text".to_string(),
                ordinal: 0,
                table_name: None,
                is_nullable: Some(false),
            },
            ResultColumnMetadata {
                name: "data_type".to_string(),
                type_name: "text".to_string(),
                ordinal: 1,
                table_name: None,
                is_nullable: Some(false),
            },
            ResultColumnMetadata {
                name: "is_nullable".to_string(),
                type_name: "text".to_string(),
                ordinal: 2,
                table_name: None,
                is_nullable: Some(false),
            },
            ResultColumnMetadata {
                name: "column_default".to_string(),
                type_name: "text".to_string(),
                ordinal: 3,
                table_name: None,
                is_nullable: Some(true),
            },
            ResultColumnMetadata {
                name: "ordinal_position".to_string(),
                type_name: "int4".to_string(),
                ordinal: 4,
                table_name: None,
                is_nullable: Some(false),
            },
        ];

        let rows: Vec<ResultRow> = columns
            .into_iter()
            .map(|col| {
                ResultRow {
                    cells: vec![
                        ResultCell {
                            value: col.column_name.clone(),
                            is_null: false,
                            column_metadata: result_columns[0].clone(),
                        },
                        ResultCell {
                            value: col.data_type.clone(),
                            is_null: false,
                            column_metadata: result_columns[1].clone(),
                        },
                        ResultCell {
                            value: if col.is_nullable { "YES" } else { "NO" }.to_string(),
                            is_null: false,
                            column_metadata: result_columns[2].clone(),
                        },
                        ResultCell {
                            value: col.column_default.clone().unwrap_or_default(),
                            is_null: col.column_default.is_none(),
                            column_metadata: result_columns[3].clone(),
                        },
                        ResultCell {
                            value: col.ordinal_position.to_string(),
                            is_null: false,
                            column_metadata: result_columns[4].clone(),
                        },
                    ],
                }
            })
            .collect();

        let row_count = rows.len();
        Ok(QueryExecutionResult::Select(QueryResult {
            columns: result_columns,
            rows,
            row_count,
            execution_time_ms: 0,
            original_query: format!(
                "SELECT column_name, data_type, is_nullable, column_default, ordinal_position FROM columns WHERE table = '{}'",
                table_name
            ),
        }))
    }

    /// Get the database schema.
    pub async fn get_schema(&self, specific_tables: Option<Vec<String>>) -> Result<DatabaseSchema> {
        let conn_guard = self.connection.read().await;
        let conn = conn_guard
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Database not connected"))?;

        let schema = conn.get_schema(specific_tables).await?;
        Ok(Self::convert_database_schema(schema))
    }

    // ========================================================================
    // Type conversion helpers
    // ========================================================================

    fn convert_query_result(
        &self,
        result: TraitQueryResult,
        original_query: String,
    ) -> QueryExecutionResult {
        match result {
            TraitQueryResult::Select(select) => {
                QueryExecutionResult::Select(self.convert_select_result(select, original_query))
            }
            TraitQueryResult::Modified(modified) => {
                QueryExecutionResult::Modified(ModifiedResult {
                    rows_affected: modified.rows_affected,
                    execution_time_ms: modified.execution_time_ms,
                })
            }
            TraitQueryResult::Error(err) => QueryExecutionResult::Error(ErrorResult {
                message: err.message,
                execution_time_ms: err.execution_time_ms,
            }),
        }
    }

    fn convert_select_result(
        &self,
        select: TraitSelectResult,
        original_query: String,
    ) -> QueryResult {
        let columns: Vec<ResultColumnMetadata> = select
            .columns
            .iter()
            .map(|col| ResultColumnMetadata {
                name: col.name.clone(),
                type_name: col.type_name.clone(),
                ordinal: col.ordinal,
                table_name: None,
                is_nullable: None,
            })
            .collect();

        let rows: Vec<ResultRow> = select
            .rows
            .into_iter()
            .map(|row| {
                ResultRow {
                    cells: row
                        .cells
                        .into_iter()
                        .enumerate()
                        .map(|(i, cell)| {
                            let is_null = cell.value.is_null();
                            ResultCell {
                                value: cell.value.to_string(),
                                is_null,
                                column_metadata: columns.get(i).cloned().unwrap_or_else(|| {
                                    ResultColumnMetadata {
                                        name: format!("col_{}", i),
                                        type_name: "unknown".to_string(),
                                        ordinal: i,
                                        table_name: None,
                                        is_nullable: None,
                                    }
                                }),
                            }
                        })
                        .collect(),
                }
            })
            .collect();

        QueryResult {
            columns,
            rows,
            row_count: select.row_count,
            execution_time_ms: select.execution_time_ms,
            original_query,
        }
    }

    fn convert_database_info(info: TraitDatabaseInfo) -> DatabaseInfo {
        DatabaseInfo { datname: info.name }
    }

    fn convert_table_info(info: TraitTableInfo) -> TableInfo {
        TableInfo {
            table_name: info.table_name,
            table_schema: info.table_schema,
            table_type: info.table_type,
        }
    }

    fn convert_database_schema(schema: TraitDatabaseSchema) -> DatabaseSchema {
        DatabaseSchema {
            tables: schema
                .tables
                .into_iter()
                .map(Self::convert_table_schema)
                .collect(),
            total_tables: schema.total_tables,
        }
    }

    fn convert_table_schema(schema: TraitTableSchema) -> TableSchema {
        TableSchema {
            table_name: schema.table_name,
            table_schema: schema.table_schema,
            table_type: schema.table_type,
            columns: schema
                .columns
                .into_iter()
                .map(Self::convert_column_detail)
                .collect(),
            primary_keys: schema.primary_keys,
            foreign_keys: schema
                .foreign_keys
                .into_iter()
                .map(Self::convert_foreign_key)
                .collect(),
            indexes: schema.indexes.into_iter().map(Self::convert_index).collect(),
            constraints: schema
                .constraints
                .into_iter()
                .map(Self::convert_constraint)
                .collect(),
            description: schema.description,
        }
    }

    fn convert_column_detail(col: TraitColumnDetail) -> ColumnDetail {
        ColumnDetail {
            column_name: col.column_name,
            data_type: col.data_type,
            is_nullable: col.is_nullable,
            column_default: col.column_default,
            ordinal_position: col.ordinal_position,
            character_maximum_length: col.character_maximum_length,
            numeric_precision: col.numeric_precision,
            numeric_scale: col.numeric_scale,
            description: col.description,
        }
    }

    fn convert_foreign_key(fk: TraitForeignKeyInfo) -> ForeignKeyInfo {
        ForeignKeyInfo {
            constraint_name: fk.constraint_name,
            column_name: fk.column_name,
            foreign_table_schema: fk.foreign_table_schema,
            foreign_table_name: fk.foreign_table_name,
            foreign_column_name: fk.foreign_column_name,
        }
    }

    fn convert_index(idx: TraitIndexInfo) -> IndexInfo {
        IndexInfo {
            index_name: idx.index_name,
            columns: idx.columns,
            is_unique: idx.is_unique,
            is_primary: idx.is_primary,
            index_type: idx.index_type,
        }
    }

    fn convert_constraint(con: TraitConstraintInfo) -> ConstraintInfo {
        ConstraintInfo {
            constraint_name: con.constraint_name,
            constraint_type: con.constraint_type,
            columns: con.columns,
            check_clause: con.check_clause,
        }
    }
}

impl Default for MultiDatabaseManager {
    fn default() -> Self {
        Self::new()
    }
}
