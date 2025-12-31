//! Database abstraction traits and types.
//!
//! This module provides a unified interface for interacting with different database types.
//! It defines:
//!
//! - **Types** (`types`): Database type enum, connection configuration, SSL modes
//! - **Row/Value** (`row`): Database-agnostic value representation
//! - **Connection** (`connection`): Core connection trait and query results
//! - **Schema** (`schema`): Schema introspection trait and metadata types
//!
//! # Example
//!
//! ```ignore
//! use pgui::services::database::traits::{
//!     DatabaseConnection, DatabaseType, ConnectionConfig, ConnectionParams,
//! };
//!
//! // Create a PostgreSQL connection config
//! let config = ConnectionConfig::new(
//!     "My Database".to_string(),
//!     DatabaseType::PostgreSQL,
//!     ConnectionParams::server(
//!         "localhost".to_string(),
//!         5432,
//!         "user".to_string(),
//!         "password".to_string(),
//!         "mydb".to_string(),
//!     ),
//! );
//! ```

pub mod connection;
pub mod row;
pub mod schema;
pub mod types;

// Re-export commonly used types
pub use connection::{
    BoxedConnection, DatabaseConnection, ErrorResult, ModifiedResult, QueryExecutionResult,
    SelectResult,
};

pub use row::{Cell, ColumnInfo, Row, Value};

pub use schema::{
    ColumnDetail, ConstraintInfo, DatabaseInfo, DatabaseSchema,
    ForeignKeyInfo, IndexInfo, SchemaIntrospection, TableInfo, TableSchema,
};

pub use types::{ConnectionConfig, ConnectionParams, DatabaseType, SslMode};
