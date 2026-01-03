//! Database driver implementations.
//!
//! This module contains driver implementations for different database types:
//!
//! - **PostgreSQL** (Epic 2): Full-featured PostgreSQL support via SQLx
//! - **MySQL** (Epic 4): MySQL/MariaDB support via SQLx
//! - **SQLite** (Epic 3): Embedded SQLite support via SQLx
//! - **ClickHouse** (Epic 6): ClickHouse analytics database support via HTTP
//! - **DuckDB** (Epic 5): Embedded DuckDB analytics support
//!
//! Each driver implements the `DatabaseConnection` and `SchemaIntrospection` traits.

mod factory;

// Driver implementations
pub mod clickhouse;
pub mod duckdb;
pub mod mysql;
pub mod postgres;
pub mod sqlite;

// Re-export for public API (will be used once multi-database is fully integrated)
#[allow(unused_imports)]
pub use factory::ConnectionFactory;
