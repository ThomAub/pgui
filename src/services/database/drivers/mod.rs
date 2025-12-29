//! Database driver implementations.
//!
//! This module contains driver implementations for different database types:
//!
//! - **PostgreSQL** (Epic 2): Full-featured PostgreSQL support via SQLx
//! - **MySQL** (Epic 4): MySQL/MariaDB support via SQLx
//! - **SQLite** (Epic 3): Embedded SQLite support via SQLx
//! - **ClickHouse** (Epic 6): ClickHouse analytics database support
//! - **DuckDB** (Epic 5): Embedded DuckDB analytics support
//!
//! Each driver implements the `DatabaseConnection` and `SchemaIntrospection` traits.

mod factory;

// Driver implementations
pub mod duckdb;
pub mod mysql;
pub mod postgres;
pub mod sqlite;

// Driver modules to be added as they are implemented:
// pub mod clickhouse; // Epic 6

pub use factory::ConnectionFactory;

// Re-export main driver types
pub use duckdb::DuckDbConnection;
pub use mysql::MySqlConnection;
pub use postgres::PostgresConnection;
pub use sqlite::SqliteConnection;
