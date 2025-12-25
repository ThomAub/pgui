//! Connection factory for creating database connections.
//!
//! The factory pattern allows creating the appropriate database connection
//! based on the connection configuration's database type.

use anyhow::{anyhow, Result};

use super::postgres::PostgresConnection;
use crate::services::database::traits::{
    BoxedConnection, ConnectionConfig, DatabaseType, SchemaIntrospection,
};

/// Factory for creating database connections based on configuration.
///
/// # Example
///
/// ```ignore
/// use pgui::services::database::drivers::ConnectionFactory;
/// use pgui::services::database::traits::{ConnectionConfig, DatabaseType, ConnectionParams};
///
/// let config = ConnectionConfig::new(
///     "My DB".to_string(),
///     DatabaseType::PostgreSQL,
///     ConnectionParams::server("localhost".to_string(), 5432, "user".to_string(), "pass".to_string(), "db".to_string()),
/// );
///
/// let connection = ConnectionFactory::create(config)?;
/// ```
pub struct ConnectionFactory;

impl ConnectionFactory {
    /// Create a new database connection based on the configuration.
    ///
    /// # Arguments
    ///
    /// * `config` - The connection configuration specifying database type and parameters
    ///
    /// # Returns
    ///
    /// Returns a boxed database connection trait object.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The database type is not yet supported
    /// - The configuration is invalid for the database type
    pub fn create(config: ConnectionConfig) -> Result<BoxedConnection> {
        // Validate configuration
        config.validate().map_err(|e| anyhow!(e))?;

        match config.database_type {
            DatabaseType::PostgreSQL => Ok(PostgresConnection::boxed(config)),
            DatabaseType::MySQL => {
                // Will be implemented in Epic 4
                Err(anyhow!(
                    "MySQL support coming soon. \
                     Please check back after Epic 4 is complete."
                ))
            }
            DatabaseType::SQLite => {
                // Will be implemented in Epic 3
                Err(anyhow!(
                    "SQLite support coming soon. \
                     Please check back after Epic 3 is complete."
                ))
            }
            DatabaseType::ClickHouse => {
                // Will be implemented in Epic 6
                Err(anyhow!(
                    "ClickHouse support coming soon. \
                     Please check back after Epic 6 is complete."
                ))
            }
            DatabaseType::DuckDB => {
                // Will be implemented in Epic 5
                Err(anyhow!(
                    "DuckDB support coming soon. \
                     Please check back after Epic 5 is complete."
                ))
            }
        }
    }

    /// Create a connection that also supports schema introspection.
    ///
    /// All currently supported databases support schema introspection,
    /// so this is essentially the same as `create()` but returns a
    /// trait object that exposes schema methods.
    ///
    /// # Arguments
    ///
    /// * `config` - The connection configuration
    ///
    /// # Returns
    ///
    /// Returns a boxed schema introspection trait object.
    pub fn create_with_schema(
        config: ConnectionConfig,
    ) -> Result<Box<dyn SchemaIntrospection>> {
        // Validate configuration
        config.validate().map_err(|e| anyhow!(e))?;

        match config.database_type {
            DatabaseType::PostgreSQL => Ok(Box::new(PostgresConnection::new(config))),
            DatabaseType::MySQL => {
                Err(anyhow!("MySQL support coming soon."))
            }
            DatabaseType::SQLite => {
                Err(anyhow!("SQLite support coming soon."))
            }
            DatabaseType::ClickHouse => {
                Err(anyhow!("ClickHouse support coming soon."))
            }
            DatabaseType::DuckDB => {
                Err(anyhow!("DuckDB support coming soon."))
            }
        }
    }

    /// Check if a database type is currently supported.
    ///
    /// # Arguments
    ///
    /// * `db_type` - The database type to check
    ///
    /// # Returns
    ///
    /// Returns true if the database type has a driver implementation.
    pub fn is_supported(db_type: DatabaseType) -> bool {
        match db_type {
            DatabaseType::PostgreSQL => true,  // Epic 2 - Implemented
            DatabaseType::MySQL => false,      // Epic 4
            DatabaseType::SQLite => false,     // Epic 3
            DatabaseType::ClickHouse => false, // Epic 6
            DatabaseType::DuckDB => false,     // Epic 5
        }
    }

    /// Get a list of all supported database types.
    ///
    /// # Returns
    ///
    /// Returns a list of database types that have driver implementations.
    pub fn supported_types() -> Vec<DatabaseType> {
        DatabaseType::all()
            .into_iter()
            .filter(|t| Self::is_supported(*t))
            .collect()
    }

    /// Get a list of all database types (supported and unsupported).
    ///
    /// Useful for showing all options in the UI with some marked as "coming soon".
    pub fn all_types() -> Vec<(DatabaseType, bool)> {
        DatabaseType::all()
            .into_iter()
            .map(|t| (t, Self::is_supported(t)))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::database::traits::{ConnectionParams, SslMode};

    #[test]
    fn test_factory_validates_config() {
        // Invalid: PostgreSQL with file params
        let config = ConnectionConfig::new(
            "test".to_string(),
            DatabaseType::PostgreSQL,
            ConnectionParams::file(std::path::PathBuf::from("/tmp/test.db"), false),
        );

        let result = ConnectionFactory::create(config);
        assert!(result.is_err());
    }

    #[test]
    fn test_factory_creates_postgres() {
        // Valid: PostgreSQL with server params
        let config = ConnectionConfig::new(
            "test".to_string(),
            DatabaseType::PostgreSQL,
            ConnectionParams::server(
                "localhost".to_string(),
                5432,
                "postgres".to_string(),
                "password".to_string(),
                "postgres".to_string(),
                SslMode::Prefer,
            ),
        );

        let result = ConnectionFactory::create(config);
        assert!(result.is_ok());
    }

    #[test]
    fn test_is_supported() {
        // PostgreSQL is now implemented
        assert!(ConnectionFactory::is_supported(DatabaseType::PostgreSQL));
        assert!(!ConnectionFactory::is_supported(DatabaseType::MySQL));
        assert!(!ConnectionFactory::is_supported(DatabaseType::SQLite));
        assert!(!ConnectionFactory::is_supported(DatabaseType::ClickHouse));
        assert!(!ConnectionFactory::is_supported(DatabaseType::DuckDB));
    }

    #[test]
    fn test_supported_types() {
        let supported = ConnectionFactory::supported_types();
        assert_eq!(supported.len(), 1);
        assert!(supported.contains(&DatabaseType::PostgreSQL));
    }

    #[test]
    fn test_all_types() {
        let all = ConnectionFactory::all_types();
        assert_eq!(all.len(), 5);

        // Check PostgreSQL is supported
        let pg = all.iter().find(|(t, _)| *t == DatabaseType::PostgreSQL);
        assert!(pg.is_some());
        assert!(pg.unwrap().1); // PostgreSQL should be supported
    }
}
