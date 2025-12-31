//! Schema introspection traits.
//!
//! This module defines the `SchemaIntrospection` trait for databases that support
//! querying their schema metadata (tables, columns, indexes, etc.).

#![allow(dead_code)]

use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use super::connection::DatabaseConnection;

/// Information about a database/catalog
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseInfo {
    /// Database name
    pub name: String,
    /// Whether this is the currently connected database
    #[serde(default)]
    pub is_current: bool,
}

impl DatabaseInfo {
    /// Create a new database info
    pub fn new(name: String) -> Self {
        Self {
            name,
            is_current: false,
        }
    }

    /// Mark this as the current database
    pub fn as_current(mut self) -> Self {
        self.is_current = true;
        self
    }
}

/// Information about a table
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableInfo {
    /// Table name
    pub table_name: String,
    /// Schema/namespace name
    pub table_schema: String,
    /// Table type (TABLE, VIEW, etc.)
    pub table_type: String,
    /// Optional description/comment
    #[serde(default)]
    pub description: Option<String>,
}

impl TableInfo {
    /// Create a new table info
    pub fn new(table_name: String, table_schema: String, table_type: String) -> Self {
        Self {
            table_name,
            table_schema,
            table_type,
            description: None,
        }
    }

    /// Get the fully qualified name (schema.table)
    pub fn full_name(&self) -> String {
        format!("{}.{}", self.table_schema, self.table_name)
    }

    /// Check if this is a view
    pub fn is_view(&self) -> bool {
        self.table_type.to_uppercase().contains("VIEW")
    }
}

/// Detailed information about a column
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnDetail {
    /// Column name
    pub column_name: String,
    /// Data type
    pub data_type: String,
    /// Whether NULL values are allowed
    pub is_nullable: bool,
    /// Default value expression
    pub column_default: Option<String>,
    /// Position in the table (1-indexed)
    pub ordinal_position: i32,
    /// Maximum character length for string types
    pub character_maximum_length: Option<i32>,
    /// Numeric precision for numeric types
    pub numeric_precision: Option<i32>,
    /// Numeric scale for decimal types
    pub numeric_scale: Option<i32>,
    /// Column description/comment
    pub description: Option<String>,
}

impl ColumnDetail {
    /// Create a new column detail with minimal info
    pub fn new(column_name: String, data_type: String, ordinal_position: i32) -> Self {
        Self {
            column_name,
            data_type,
            is_nullable: true,
            column_default: None,
            ordinal_position,
            character_maximum_length: None,
            numeric_precision: None,
            numeric_scale: None,
            description: None,
        }
    }

    /// Set nullable flag
    pub fn with_nullable(mut self, is_nullable: bool) -> Self {
        self.is_nullable = is_nullable;
        self
    }

    /// Set default value
    pub fn with_default(mut self, default: String) -> Self {
        self.column_default = Some(default);
        self
    }
}

/// Information about a foreign key relationship
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForeignKeyInfo {
    /// Constraint name
    pub constraint_name: String,
    /// Column name in the source table
    pub column_name: String,
    /// Schema of the referenced table
    pub foreign_table_schema: String,
    /// Name of the referenced table
    pub foreign_table_name: String,
    /// Column name in the referenced table
    pub foreign_column_name: String,
}

impl ForeignKeyInfo {
    /// Create a new foreign key info
    pub fn new(
        constraint_name: String,
        column_name: String,
        foreign_table_schema: String,
        foreign_table_name: String,
        foreign_column_name: String,
    ) -> Self {
        Self {
            constraint_name,
            column_name,
            foreign_table_schema,
            foreign_table_name,
            foreign_column_name,
        }
    }

    /// Get the referenced table's full name
    pub fn referenced_table_full_name(&self) -> String {
        format!("{}.{}", self.foreign_table_schema, self.foreign_table_name)
    }
}

/// Information about an index
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexInfo {
    /// Index name
    pub index_name: String,
    /// Columns included in the index
    pub columns: Vec<String>,
    /// Whether the index enforces uniqueness
    pub is_unique: bool,
    /// Whether this is the primary key index
    pub is_primary: bool,
    /// Index type (btree, hash, gin, etc.)
    pub index_type: String,
}

impl IndexInfo {
    /// Create a new index info
    pub fn new(index_name: String, columns: Vec<String>, index_type: String) -> Self {
        Self {
            index_name,
            columns,
            is_unique: false,
            is_primary: false,
            index_type,
        }
    }

    /// Mark as unique index
    pub fn as_unique(mut self) -> Self {
        self.is_unique = true;
        self
    }

    /// Mark as primary key index
    pub fn as_primary(mut self) -> Self {
        self.is_primary = true;
        self.is_unique = true;
        self
    }
}

/// Information about a constraint
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConstraintInfo {
    /// Constraint name
    pub constraint_name: String,
    /// Constraint type (PRIMARY KEY, UNIQUE, CHECK, etc.)
    pub constraint_type: String,
    /// Columns involved in the constraint
    pub columns: Vec<String>,
    /// Check clause for CHECK constraints
    pub check_clause: Option<String>,
}

impl ConstraintInfo {
    /// Create a new constraint info
    pub fn new(constraint_name: String, constraint_type: String, columns: Vec<String>) -> Self {
        Self {
            constraint_name,
            constraint_type,
            columns,
            check_clause: None,
        }
    }

    /// Set check clause
    pub fn with_check_clause(mut self, clause: String) -> Self {
        self.check_clause = Some(clause);
        self
    }
}

/// Complete schema information for a table
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableSchema {
    /// Basic table info
    pub table_name: String,
    pub table_schema: String,
    pub table_type: String,
    /// Column details
    pub columns: Vec<ColumnDetail>,
    /// Primary key column names
    pub primary_keys: Vec<String>,
    /// Foreign key relationships
    pub foreign_keys: Vec<ForeignKeyInfo>,
    /// Indexes on the table
    pub indexes: Vec<IndexInfo>,
    /// Other constraints
    pub constraints: Vec<ConstraintInfo>,
    /// Table description/comment
    pub description: Option<String>,
}

impl TableSchema {
    /// Create a new table schema from basic info
    pub fn new(table_name: String, table_schema: String, table_type: String) -> Self {
        Self {
            table_name,
            table_schema,
            table_type,
            columns: Vec::new(),
            primary_keys: Vec::new(),
            foreign_keys: Vec::new(),
            indexes: Vec::new(),
            constraints: Vec::new(),
            description: None,
        }
    }

    /// Get the fully qualified name
    pub fn full_name(&self) -> String {
        format!("{}.{}", self.table_schema, self.table_name)
    }
}

/// Complete database schema (multiple tables)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseSchema {
    /// All table schemas
    pub tables: Vec<TableSchema>,
    /// Total number of tables
    pub total_tables: usize,
}

impl DatabaseSchema {
    /// Create a new database schema
    pub fn new(tables: Vec<TableSchema>) -> Self {
        let total_tables = tables.len();
        Self {
            tables,
            total_tables,
        }
    }

    /// Get a table by name
    pub fn get_table(&self, table_name: &str) -> Option<&TableSchema> {
        self.tables.iter().find(|t| t.table_name == table_name)
    }

    /// Get a table by full name (schema.table)
    pub fn get_table_by_full_name(&self, full_name: &str) -> Option<&TableSchema> {
        self.tables.iter().find(|t| t.full_name() == full_name)
    }
}

/// Trait for databases that support schema introspection.
///
/// This trait provides methods to query metadata about the database structure,
/// including tables, columns, indexes, and relationships.
#[async_trait]
pub trait SchemaIntrospection: DatabaseConnection {
    /// Get a list of available databases/catalogs.
    ///
    /// # Returns
    ///
    /// Returns a list of database names available on the server.
    /// For file-based databases, this typically returns a single entry.
    async fn get_databases(&self) -> Result<Vec<DatabaseInfo>>;

    /// Get a list of tables in the current database.
    ///
    /// # Returns
    ///
    /// Returns table metadata for all tables and views in the current database.
    async fn get_tables(&self) -> Result<Vec<TableInfo>>;

    /// Get the complete schema for specified tables.
    ///
    /// # Arguments
    ///
    /// * `tables` - Optional list of table names to get schema for.
    ///              If None, returns schema for all tables.
    ///
    /// # Returns
    ///
    /// Returns complete schema information including columns, keys, and indexes.
    async fn get_schema(&self, tables: Option<Vec<String>>) -> Result<DatabaseSchema>;

    /// Get columns for a specific table.
    ///
    /// # Arguments
    ///
    /// * `table` - The table name
    /// * `schema` - The schema/namespace name
    ///
    /// # Returns
    ///
    /// Returns detailed column information for the specified table.
    async fn get_columns(&self, table: &str, schema: &str) -> Result<Vec<ColumnDetail>>;

    /// Get primary key columns for a table.
    ///
    /// # Arguments
    ///
    /// * `table` - The table name
    /// * `schema` - The schema/namespace name
    ///
    /// # Returns
    ///
    /// Returns the names of columns that make up the primary key.
    async fn get_primary_keys(&self, table: &str, schema: &str) -> Result<Vec<String>>;

    /// Get foreign key relationships for a table.
    ///
    /// # Arguments
    ///
    /// * `table` - The table name
    /// * `schema` - The schema/namespace name
    ///
    /// # Returns
    ///
    /// Returns foreign key relationship information.
    async fn get_foreign_keys(&self, table: &str, schema: &str) -> Result<Vec<ForeignKeyInfo>>;

    /// Get indexes for a table.
    ///
    /// # Arguments
    ///
    /// * `table` - The table name
    /// * `schema` - The schema/namespace name
    ///
    /// # Returns
    ///
    /// Returns index information for the table.
    async fn get_indexes(&self, table: &str, schema: &str) -> Result<Vec<IndexInfo>>;

    /// Get constraints for a table.
    ///
    /// # Arguments
    ///
    /// * `table` - The table name
    /// * `schema` - The schema/namespace name
    ///
    /// # Returns
    ///
    /// Returns constraint information (UNIQUE, CHECK, etc.).
    async fn get_constraints(&self, table: &str, schema: &str) -> Result<Vec<ConstraintInfo>>;

    /// Refresh the schema cache (if any).
    ///
    /// Some implementations may cache schema information for performance.
    /// This method forces a refresh of that cache.
    async fn refresh_schema(&self) -> Result<()> {
        // Default implementation does nothing
        Ok(())
    }
}

/// A boxed schema introspection trait object.
pub type BoxedSchemaIntrospection = Box<dyn SchemaIntrospection>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_table_info_full_name() {
        let table = TableInfo::new(
            "users".to_string(),
            "public".to_string(),
            "TABLE".to_string(),
        );
        assert_eq!(table.full_name(), "public.users");
    }

    #[test]
    fn test_table_info_is_view() {
        let table = TableInfo::new(
            "users".to_string(),
            "public".to_string(),
            "TABLE".to_string(),
        );
        assert!(!table.is_view());

        let view = TableInfo::new(
            "active_users".to_string(),
            "public".to_string(),
            "VIEW".to_string(),
        );
        assert!(view.is_view());
    }

    #[test]
    fn test_database_schema_get_table() {
        let schema = DatabaseSchema::new(vec![
            TableSchema::new("users".to_string(), "public".to_string(), "TABLE".to_string()),
            TableSchema::new("posts".to_string(), "public".to_string(), "TABLE".to_string()),
        ]);

        assert!(schema.get_table("users").is_some());
        assert!(schema.get_table("posts").is_some());
        assert!(schema.get_table("comments").is_none());
    }

    #[test]
    fn test_index_info_builder() {
        let index = IndexInfo::new(
            "idx_users_email".to_string(),
            vec!["email".to_string()],
            "btree".to_string(),
        )
        .as_unique();

        assert!(index.is_unique);
        assert!(!index.is_primary);

        let pk_index = IndexInfo::new(
            "pk_users".to_string(),
            vec!["id".to_string()],
            "btree".to_string(),
        )
        .as_primary();

        assert!(pk_index.is_unique);
        assert!(pk_index.is_primary);
    }
}
