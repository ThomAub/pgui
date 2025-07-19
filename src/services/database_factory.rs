use crate::services::{
    adapters::{ClickHouseAdapter, PostgresAdapter, SqliteAdapter},
    database_adapter::{DatabaseAdapter, DatabaseType},
};
use anyhow::Result;

pub struct DatabaseFactory;

impl DatabaseFactory {
    pub fn create_adapter(connection_url: &str) -> Result<Box<dyn DatabaseAdapter>> {
        let db_type = DatabaseType::from_url(connection_url)?;
        
        match db_type {
            DatabaseType::PostgreSQL => Ok(Box::new(PostgresAdapter::new())),
            DatabaseType::SQLite => Ok(Box::new(SqliteAdapter::new())),
            DatabaseType::ClickHouse => Ok(Box::new(ClickHouseAdapter::new())),
        }
    }

    pub fn get_placeholder_url(db_type: DatabaseType) -> &'static str {
        match db_type {
            DatabaseType::PostgreSQL => "postgres://username:password@localhost:5432/database",
            DatabaseType::SQLite => "sqlite://path/to/database.db",
            DatabaseType::ClickHouse => "clickhouse://username:password@localhost:9000/database",
        }
    }

    pub fn get_database_type_name(db_type: DatabaseType) -> &'static str {
        match db_type {
            DatabaseType::PostgreSQL => "PostgreSQL",
            DatabaseType::SQLite => "SQLite",
            DatabaseType::ClickHouse => "ClickHouse",
        }
    }

    pub fn validate_connection_url(url: &str) -> Result<DatabaseType> {
        DatabaseType::from_url(url)
    }
}