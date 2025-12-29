//! MySQL database driver implementation.
//!
//! This module provides a MySQL driver that implements the `DatabaseConnection`
//! and `SchemaIntrospection` traits using SQLx.
//!
//! # Example
//!
//! ```ignore
//! use pgui::services::database::drivers::mysql::MySqlConnection;
//! use pgui::services::database::traits::{ConnectionConfig, DatabaseType, ConnectionParams, SslMode};
//!
//! let config = ConnectionConfig::new(
//!     "My MySQL".to_string(),
//!     DatabaseType::MySQL,
//!     ConnectionParams::server(
//!         "localhost".to_string(),
//!         3306,
//!         "user".to_string(),
//!         "password".to_string(),
//!         "mydb".to_string(),
//!         SslMode::Prefer,
//!     ),
//! );
//!
//! let mut conn = MySqlConnection::new(config);
//! conn.connect().await?;
//! ```

mod connection;
mod schema;
mod types;

pub use connection::MySqlConnection;
pub use types::MySqlValueConverter;
