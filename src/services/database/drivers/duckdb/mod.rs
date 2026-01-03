//! DuckDB driver implementation.
//!
//! This module provides DuckDB database support using the duckdb-rs crate.
//!
//! # Example
//!
//! ```ignore
//! use pgui::services::database::drivers::DuckDbConnection;
//! use pgui::services::database::traits::{ConnectionConfig, DatabaseType, ConnectionParams};
//!
//! // File-based DuckDB
//! let config = ConnectionConfig::new(
//!     "My DuckDB".to_string(),
//!     DatabaseType::DuckDB,
//!     ConnectionParams::file("/path/to/database.duckdb".into(), false),
//! );
//!
//! let connection = DuckDbConnection::new(config);
//! ```

mod connection;
mod schema;
mod types;

pub use connection::DuckDbConnection;
