//! Database type definitions and connection configuration.
//!
//! This module contains:
//! - `DatabaseType` - Enum of supported database types
//! - `ConnectionConfig` - Unified connection configuration
//! - `ConnectionParams` - Database-specific connection parameters

#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use uuid::Uuid;

/// Supported database types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum DatabaseType {
    #[default]
    PostgreSQL,
    MySQL,
    SQLite,
    ClickHouse,
    DuckDB,
}

impl DatabaseType {
    /// Get the display name for this database type
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::PostgreSQL => "PostgreSQL",
            Self::MySQL => "MySQL",
            Self::SQLite => "SQLite",
            Self::ClickHouse => "ClickHouse",
            Self::DuckDB => "DuckDB",
        }
    }

    /// Get the default port for server-based databases
    pub fn default_port(&self) -> Option<u16> {
        match self {
            Self::PostgreSQL => Some(5432),
            Self::MySQL => Some(3306),
            Self::SQLite => None,      // File-based
            Self::ClickHouse => Some(8123), // HTTP port
            Self::DuckDB => None,      // Embedded/file-based
        }
    }

    /// Check if this database type is file-based (SQLite, DuckDB)
    pub fn is_file_based(&self) -> bool {
        matches!(self, Self::SQLite | Self::DuckDB)
    }

    /// Check if this database type is server-based
    pub fn is_server_based(&self) -> bool {
        !self.is_file_based()
    }

    /// Get the icon name for this database type
    pub fn icon_name(&self) -> &'static str {
        match self {
            Self::PostgreSQL => "database",
            Self::MySQL => "database",
            Self::SQLite => "file",
            Self::ClickHouse => "database",
            Self::DuckDB => "file",
        }
    }

    /// Get all available database types
    pub fn all() -> Vec<DatabaseType> {
        vec![
            Self::PostgreSQL,
            Self::MySQL,
            Self::SQLite,
            Self::ClickHouse,
            Self::DuckDB,
        ]
    }

    /// Parse from a string representation
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "postgresql" | "postgres" | "pg" => Some(Self::PostgreSQL),
            "mysql" | "mariadb" => Some(Self::MySQL),
            "sqlite" | "sqlite3" => Some(Self::SQLite),
            "clickhouse" | "ch" => Some(Self::ClickHouse),
            "duckdb" | "duck" => Some(Self::DuckDB),
            _ => None,
        }
    }

    /// Convert to a string representation for storage
    pub fn to_db_str(&self) -> &'static str {
        match self {
            Self::PostgreSQL => "postgresql",
            Self::MySQL => "mysql",
            Self::SQLite => "sqlite",
            Self::ClickHouse => "clickhouse",
            Self::DuckDB => "duckdb",
        }
    }
}

impl std::fmt::Display for DatabaseType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display_name())
    }
}

/// SSL mode options (generic across databases)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum SslMode {
    /// No SSL connection
    Disable,
    /// Try SSL first, fall back to non-SSL
    #[default]
    Prefer,
    /// Require SSL, don't verify certificates
    Require,
    /// Require SSL and verify server certificate
    VerifyCa,
    /// Require SSL, verify certificate and hostname
    VerifyFull,
}

impl SslMode {
    /// Get the display string for this SSL mode
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Disable => "Disable",
            Self::Prefer => "Prefer",
            Self::Require => "Require",
            Self::VerifyCa => "Verify CA",
            Self::VerifyFull => "Verify Full",
        }
    }

    /// Get all available SSL modes
    pub fn all() -> Vec<SslMode> {
        vec![
            Self::Disable,
            Self::Prefer,
            Self::Require,
            Self::VerifyCa,
            Self::VerifyFull,
        ]
    }

    /// Parse from a database string
    pub fn from_db_str(s: &str) -> Self {
        match s {
            "disable" => Self::Disable,
            "prefer" => Self::Prefer,
            "require" => Self::Require,
            "verify-ca" => Self::VerifyCa,
            "verify-full" => Self::VerifyFull,
            _ => Self::Prefer,
        }
    }

    /// Convert to a database string
    pub fn to_db_str(&self) -> &'static str {
        match self {
            Self::Disable => "disable",
            Self::Prefer => "prefer",
            Self::Require => "require",
            Self::VerifyCa => "verify-ca",
            Self::VerifyFull => "verify-full",
        }
    }
}

/// Unified connection configuration for all database types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionConfig {
    /// Unique identifier for this connection
    pub id: Uuid,
    /// User-friendly name for this connection
    pub name: String,
    /// The type of database
    pub database_type: DatabaseType,
    /// Connection parameters (varies by database type)
    pub params: ConnectionParams,
}

impl ConnectionConfig {
    /// Create a new connection configuration
    pub fn new(name: String, database_type: DatabaseType, params: ConnectionParams) -> Self {
        Self {
            id: Uuid::new_v4(),
            name,
            database_type,
            params,
        }
    }

    /// Create a new connection configuration with a specific ID
    pub fn with_id(id: Uuid, name: String, database_type: DatabaseType, params: ConnectionParams) -> Self {
        Self {
            id,
            name,
            database_type,
            params,
        }
    }

    /// Validate that the params match the database type
    pub fn validate(&self) -> Result<(), String> {
        match (&self.database_type, &self.params) {
            (DatabaseType::SQLite | DatabaseType::DuckDB, ConnectionParams::Server { .. }) => {
                Err(format!(
                    "{} requires file or in-memory connection parameters",
                    self.database_type.display_name()
                ))
            }
            (DatabaseType::PostgreSQL | DatabaseType::MySQL | DatabaseType::ClickHouse, ConnectionParams::File { .. }) => {
                Err(format!(
                    "{} requires server connection parameters",
                    self.database_type.display_name()
                ))
            }
            _ => Ok(()),
        }
    }
}

/// Connection parameters for different database types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum ConnectionParams {
    /// Server-based databases (PostgreSQL, MySQL, ClickHouse)
    Server {
        /// Server hostname or IP address
        hostname: String,
        /// Server port
        port: u16,
        /// Username for authentication
        username: String,
        /// Password for authentication (loaded from secure storage)
        #[serde(skip_serializing, default)]
        password: String,
        /// Default database/schema to connect to
        database: String,
        /// SSL mode for the connection
        #[serde(default)]
        ssl_mode: SslMode,
        /// Additional driver-specific options
        #[serde(default)]
        extra_options: HashMap<String, String>,
    },

    /// File-based databases (SQLite, DuckDB)
    File {
        /// Path to the database file
        path: PathBuf,
        /// Open in read-only mode
        #[serde(default)]
        read_only: bool,
        /// Additional driver-specific options
        #[serde(default)]
        extra_options: HashMap<String, String>,
    },

    /// In-memory databases
    InMemory {
        /// Additional driver-specific options
        #[serde(default)]
        extra_options: HashMap<String, String>,
    },
}

impl ConnectionParams {
    /// Create new server connection parameters
    pub fn server(
        hostname: String,
        port: u16,
        username: String,
        password: String,
        database: String,
    ) -> Self {
        Self::Server {
            hostname,
            port,
            username,
            password,
            database,
            ssl_mode: SslMode::default(),
            extra_options: HashMap::new(),
        }
    }

    /// Create new file connection parameters
    pub fn file(path: PathBuf, read_only: bool) -> Self {
        Self::File {
            path,
            read_only,
            extra_options: HashMap::new(),
        }
    }

    /// Create new in-memory connection parameters
    pub fn in_memory() -> Self {
        Self::InMemory {
            extra_options: HashMap::new(),
        }
    }

    /// Get the hostname if this is a server connection
    pub fn hostname(&self) -> Option<&str> {
        match self {
            Self::Server { hostname, .. } => Some(hostname),
            _ => None,
        }
    }

    /// Get the port if this is a server connection
    pub fn port(&self) -> Option<u16> {
        match self {
            Self::Server { port, .. } => Some(*port),
            _ => None,
        }
    }

    /// Get the database name if this is a server connection
    pub fn database(&self) -> Option<&str> {
        match self {
            Self::Server { database, .. } => Some(database),
            _ => None,
        }
    }

    /// Get the file path if this is a file connection
    pub fn path(&self) -> Option<&PathBuf> {
        match self {
            Self::File { path, .. } => Some(path),
            _ => None,
        }
    }

    /// Check if this is a read-only connection
    pub fn is_read_only(&self) -> bool {
        match self {
            Self::File { read_only, .. } => *read_only,
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_database_type_display_names() {
        assert_eq!(DatabaseType::PostgreSQL.display_name(), "PostgreSQL");
        assert_eq!(DatabaseType::MySQL.display_name(), "MySQL");
        assert_eq!(DatabaseType::SQLite.display_name(), "SQLite");
        assert_eq!(DatabaseType::ClickHouse.display_name(), "ClickHouse");
        assert_eq!(DatabaseType::DuckDB.display_name(), "DuckDB");
    }

    #[test]
    fn test_database_type_default_ports() {
        assert_eq!(DatabaseType::PostgreSQL.default_port(), Some(5432));
        assert_eq!(DatabaseType::MySQL.default_port(), Some(3306));
        assert_eq!(DatabaseType::SQLite.default_port(), None);
        assert_eq!(DatabaseType::ClickHouse.default_port(), Some(8123));
        assert_eq!(DatabaseType::DuckDB.default_port(), None);
    }

    #[test]
    fn test_database_type_is_file_based() {
        assert!(!DatabaseType::PostgreSQL.is_file_based());
        assert!(!DatabaseType::MySQL.is_file_based());
        assert!(DatabaseType::SQLite.is_file_based());
        assert!(!DatabaseType::ClickHouse.is_file_based());
        assert!(DatabaseType::DuckDB.is_file_based());
    }

    #[test]
    fn test_connection_config_validation() {
        // Valid: PostgreSQL with server params
        let config = ConnectionConfig::new(
            "test".to_string(),
            DatabaseType::PostgreSQL,
            ConnectionParams::server(
                "localhost".to_string(),
                5432,
                "user".to_string(),
                "pass".to_string(),
                "db".to_string(),
            ),
        );
        assert!(config.validate().is_ok());

        // Invalid: PostgreSQL with file params
        let config = ConnectionConfig::new(
            "test".to_string(),
            DatabaseType::PostgreSQL,
            ConnectionParams::file(PathBuf::from("/tmp/test.db"), false),
        );
        assert!(config.validate().is_err());

        // Valid: SQLite with file params
        let config = ConnectionConfig::new(
            "test".to_string(),
            DatabaseType::SQLite,
            ConnectionParams::file(PathBuf::from("/tmp/test.db"), false),
        );
        assert!(config.validate().is_ok());

        // Invalid: SQLite with server params
        let config = ConnectionConfig::new(
            "test".to_string(),
            DatabaseType::SQLite,
            ConnectionParams::server(
                "localhost".to_string(),
                5432,
                "user".to_string(),
                "pass".to_string(),
                "db".to_string(),
            ),
        );
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_connection_config_serialization() {
        let config = ConnectionConfig::new(
            "test".to_string(),
            DatabaseType::PostgreSQL,
            ConnectionParams::server(
                "localhost".to_string(),
                5432,
                "user".to_string(),
                "pass".to_string(),
                "db".to_string(),
            ),
        );

        let json = serde_json::to_string(&config).unwrap();
        let deserialized: ConnectionConfig = serde_json::from_str(&json).unwrap();

        assert_eq!(config.id, deserialized.id);
        assert_eq!(config.name, deserialized.name);
        assert_eq!(config.database_type, deserialized.database_type);
    }
}
