//! PostgreSQL database driver implementation.
//!
//! This module provides a PostgreSQL driver that implements the `DatabaseConnection`
//! and `SchemaIntrospection` traits using SQLx.
//!
//! # Example
//!
//! ```ignore
//! use pgui::services::database::drivers::postgres::PostgresConnection;
//! use pgui::services::database::traits::{ConnectionConfig, DatabaseType, ConnectionParams, SslMode};
//!
//! let config = ConnectionConfig::new(
//!     "My PostgreSQL".to_string(),
//!     DatabaseType::PostgreSQL,
//!     ConnectionParams::server(
//!         "localhost".to_string(),
//!         5432,
//!         "user".to_string(),
//!         "password".to_string(),
//!         "mydb".to_string(),
//!         SslMode::Prefer,
//!     ),
//! );
//!
//! let mut conn = PostgresConnection::new(config);
//! conn.connect().await?;
//! ```

mod connection;
mod schema;
mod types;

pub use connection::PostgresConnection;
