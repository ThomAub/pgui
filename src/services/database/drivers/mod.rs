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

// Driver modules will be added as they are implemented:
// pub mod postgres;  // Epic 2
// pub mod sqlite;    // Epic 3
// pub mod mysql;     // Epic 4
// pub mod duckdb;    // Epic 5
// pub mod clickhouse; // Epic 6

pub use factory::ConnectionFactory;
