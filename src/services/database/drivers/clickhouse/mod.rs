//! ClickHouse database driver implementation.
//!
//! This module implements the `DatabaseConnection` and `SchemaIntrospection` traits
//! for ClickHouse, a column-oriented OLAP database management system.
//!
//! Features:
//! - HTTP-based connection (port 8123 by default)
//! - Async query execution
//! - Schema introspection
//! - Type conversion to unified Value enum

mod connection;
mod schema;
mod types;

pub use connection::ClickHouseConnection;
