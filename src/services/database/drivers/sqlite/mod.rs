//! SQLite database driver implementation.
//!
//! This module provides a SQLite driver that implements the `DatabaseConnection`
//! and `SchemaIntrospection` traits using SQLx.
//!
//! SQLite is a file-based embedded database that supports:
//! - File-based databases (`.db`, `.sqlite`, `.sqlite3`)
//! - In-memory databases (`:memory:`)
//! - Read-only mode
//!
//! # Example
//!
//! ```ignore
//! use pgui::services::database::drivers::sqlite::SqliteConnection;
//! use pgui::services::database::traits::{ConnectionConfig, DatabaseType, ConnectionParams};
//! use std::path::PathBuf;
//!
//! // File-based connection
//! let config = ConnectionConfig::new(
//!     "My SQLite DB".to_string(),
//!     DatabaseType::SQLite,
//!     ConnectionParams::file(PathBuf::from("/path/to/database.db"), false),
//! );
//!
//! let mut conn = SqliteConnection::new(config);
//! conn.connect().await?;
//!
//! // In-memory connection
//! let memory_config = ConnectionConfig::new(
//!     "In-Memory DB".to_string(),
//!     DatabaseType::SQLite,
//!     ConnectionParams::in_memory(),
//! );
//! ```

mod connection;
mod schema;
mod types;

pub use connection::SqliteConnection;
