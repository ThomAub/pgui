//! Connection type definitions.
//!
//! This module contains:
//! - `SslMode` - SSL mode options for database connections
//! - `ConnectionInfo` - Database connection configuration (supports multiple database types)

#![allow(dead_code)]

use chrono::{DateTime, Utc};
use gpui::SharedString;
use gpui_component::select::SelectItem;
use serde::{Deserialize, Serialize};
use sqlx::postgres::{PgConnectOptions, PgSslMode};
use std::path::PathBuf;
use uuid::Uuid;

use crate::services::database::traits::{
    ConnectionConfig, ConnectionParams, DatabaseType, SslMode as TraitSslMode,
};

/// SSL mode options for PostgreSQL connections
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SslMode {
    Disable,
    Prefer,
    Require,
    VerifyCa,
    VerifyFull,
}

impl SelectItem for SslMode {
    type Value = &'static str;

    fn title(&self) -> SharedString {
        self.as_str().into()
    }

    fn value(&self) -> &Self::Value {
        match self {
            SslMode::Disable => &"disable",
            SslMode::Prefer => &"prefer",
            SslMode::Require => &"require",
            SslMode::VerifyCa => &"verify-ca",
            SslMode::VerifyFull => &"verify-full",
        }
    }
}

impl Default for SslMode {
    fn default() -> Self {
        SslMode::Prefer
    }
}

#[allow(dead_code)]
impl SslMode {
    /// Convert to sqlx PgSslMode
    pub fn to_pg_ssl_mode(&self) -> PgSslMode {
        match self {
            SslMode::Disable => PgSslMode::Disable,
            SslMode::Prefer => PgSslMode::Prefer,
            SslMode::Require => PgSslMode::Require,
            SslMode::VerifyCa => PgSslMode::VerifyCa,
            SslMode::VerifyFull => PgSslMode::VerifyFull,
        }
    }

    /// Get the display string for this SSL mode
    pub fn as_str(&self) -> &'static str {
        match self {
            SslMode::Disable => "Disable",
            SslMode::Prefer => "Prefer",
            SslMode::Require => "Require",
            SslMode::VerifyCa => "Verify CA",
            SslMode::VerifyFull => "Verify Full",
        }
    }

    /// Get a description of what this SSL mode does
    pub fn description(&self) -> &str {
        match self {
            SslMode::Disable => "No SSL connection",
            SslMode::Prefer => "Try SSL first, fall back to non-SSL",
            SslMode::Require => "Require SSL, don't verify certificates",
            SslMode::VerifyCa => "Require SSL and verify server certificate",
            SslMode::VerifyFull => "Require SSL, verify certificate and hostname",
        }
    }

    /// Get all available SSL modes
    pub fn all() -> Vec<SslMode> {
        vec![
            SslMode::Disable,
            SslMode::Prefer,
            SslMode::Require,
            SslMode::VerifyCa,
            SslMode::VerifyFull,
        ]
    }

    /// Create an SSL mode from a zero-based index
    pub fn from_index(index: usize) -> Self {
        match index {
            0 => SslMode::Disable,
            1 => SslMode::Prefer,
            2 => SslMode::Require,
            3 => SslMode::VerifyCa,
            4 => SslMode::VerifyFull,
            _ => SslMode::Prefer,
        }
    }

    /// Convert this SSL mode to a zero-based index
    pub fn to_index(&self) -> usize {
        match self {
            SslMode::Disable => 0,
            SslMode::Prefer => 1,
            SslMode::Require => 2,
            SslMode::VerifyCa => 3,
            SslMode::VerifyFull => 4,
        }
    }

    /// Parse an SSL mode from a database string
    pub fn from_db_str(s: &str) -> Self {
        match s {
            "disable" => SslMode::Disable,
            "prefer" => SslMode::Prefer,
            "require" => SslMode::Require,
            "verify-ca" => SslMode::VerifyCa,
            "verify-full" => SslMode::VerifyFull,
            _ => SslMode::Prefer, // Default fallback
        }
    }

    /// Convert this SSL mode to a database string
    pub fn to_db_str(&self) -> &'static str {
        match self {
            SslMode::Disable => "disable",
            SslMode::Prefer => "prefer",
            SslMode::Require => "require",
            SslMode::VerifyCa => "verify-ca",
            SslMode::VerifyFull => "verify-full",
        }
    }
}

/// Database connection configuration.
///
/// Supports both server-based databases (PostgreSQL, MySQL, ClickHouse) and
/// file-based databases (SQLite, DuckDB).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionInfo {
    #[serde(default = "Uuid::new_v4")]
    pub id: Uuid,
    pub name: String,

    /// The type of database (defaults to PostgreSQL for backward compatibility)
    #[serde(default)]
    pub database_type: DatabaseType,

    // Server-based connection fields (PostgreSQL, MySQL, ClickHouse)
    #[serde(default)]
    pub hostname: String,
    #[serde(default)]
    pub username: String,
    #[serde(skip_serializing_if = "String::is_empty", default)]
    pub password: String,
    #[serde(default)]
    pub database: String,
    #[serde(default = "default_port")]
    pub port: usize,
    #[serde(default)]
    pub ssl_mode: SslMode,

    // File-based connection fields (SQLite, DuckDB)
    /// Path to the database file (for SQLite, DuckDB)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_path: Option<PathBuf>,
    /// Open in read-only mode (for file-based databases)
    #[serde(default)]
    pub read_only: bool,
}

fn default_port() -> usize {
    5432
}

impl ConnectionInfo {
    /// Create a new server-based connection info (PostgreSQL, MySQL, ClickHouse)
    pub fn new(
        name: String,
        hostname: String,
        username: String,
        password: String,
        database: String,
        port: usize,
        ssl_mode: SslMode,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            name,
            database_type: DatabaseType::PostgreSQL,
            hostname,
            username,
            password,
            database,
            port,
            ssl_mode,
            file_path: None,
            read_only: false,
        }
    }

    /// Create a new server-based connection with specific database type
    pub fn new_server(
        name: String,
        database_type: DatabaseType,
        hostname: String,
        port: usize,
        username: String,
        password: String,
        database: String,
        ssl_mode: SslMode,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            name,
            database_type,
            hostname,
            username,
            password,
            database,
            port,
            ssl_mode,
            file_path: None,
            read_only: false,
        }
    }

    /// Create a new file-based connection (SQLite, DuckDB)
    pub fn new_file(
        name: String,
        database_type: DatabaseType,
        file_path: PathBuf,
        read_only: bool,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            name,
            database_type,
            hostname: String::new(),
            username: String::new(),
            password: String::new(),
            database: String::new(),
            port: 0,
            ssl_mode: SslMode::default(),
            file_path: Some(file_path),
            read_only,
        }
    }

    /// Check if this is a file-based connection
    pub fn is_file_based(&self) -> bool {
        self.database_type.is_file_based()
    }

    /// Check if this is a server-based connection
    pub fn is_server_based(&self) -> bool {
        self.database_type.is_server_based()
    }

    /// Convert to the new ConnectionConfig format
    pub fn to_connection_config(&self) -> ConnectionConfig {
        let params = if self.database_type.is_file_based() {
            if let Some(path) = &self.file_path {
                ConnectionParams::file(path.clone(), self.read_only)
            } else {
                // In-memory if no file path provided
                ConnectionParams::in_memory()
            }
        } else {
            let ssl_mode = match self.ssl_mode {
                SslMode::Disable => TraitSslMode::Disable,
                SslMode::Prefer => TraitSslMode::Prefer,
                SslMode::Require => TraitSslMode::Require,
                SslMode::VerifyCa => TraitSslMode::VerifyCa,
                SslMode::VerifyFull => TraitSslMode::VerifyFull,
            };

            ConnectionParams::Server {
                hostname: self.hostname.clone(),
                port: self.port as u16,
                username: self.username.clone(),
                password: self.password.clone(),
                database: self.database.clone(),
                ssl_mode,
                extra_options: std::collections::HashMap::new(),
            }
        };

        ConnectionConfig::with_id(
            self.id,
            self.name.clone(),
            self.database_type,
            params,
        )
    }

    /// Create connection options for sqlx (PostgreSQL only)
    ///
    /// # Panics
    ///
    /// Panics if called on a non-PostgreSQL connection.
    pub fn to_pg_connect_options(&self) -> PgConnectOptions {
        assert!(
            self.database_type == DatabaseType::PostgreSQL,
            "to_pg_connect_options() can only be called on PostgreSQL connections"
        );

        PgConnectOptions::new()
            .host(&self.hostname)
            .port(self.port as u16)
            .username(&self.username)
            .password(&self.password)
            .database(&self.database)
            .ssl_mode(self.ssl_mode.to_pg_ssl_mode())
    }

    /// Get the default port for the current database type
    pub fn default_port_for_type(&self) -> Option<u16> {
        self.database_type.default_port()
    }

    /// Get the display name for the connection (e.g., "user@host:port/db" or file path)
    pub fn display_info(&self) -> String {
        if self.database_type.is_file_based() {
            self.file_path
                .as_ref()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| ":memory:".to_string())
        } else {
            format!(
                "{}@{}:{}/{}",
                self.username, self.hostname, self.port, self.database
            )
        }
    }
}

impl Default for ConnectionInfo {
    fn default() -> Self {
        Self {
            id: Uuid::new_v4(),
            name: "New Connection".to_string(),
            database_type: DatabaseType::PostgreSQL,
            hostname: "localhost".to_string(),
            username: String::new(),
            password: String::new(),
            database: String::new(),
            port: 5432,
            ssl_mode: SslMode::default(),
            file_path: None,
            read_only: false,
        }
    }
}

impl Drop for ConnectionInfo {
    fn drop(&mut self) {
        // Zero out password memory when dropped for security
        use std::ptr;
        unsafe {
            ptr::write_volatile(&mut self.password, String::new());
        }
    }
}

/// Query history entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryHistoryEntry {
    pub id: Uuid,
    pub connection_id: Uuid,
    pub sql: String,
    pub execution_time_ms: i64,
    pub rows_affected: Option<i64>,
    pub success: bool,
    pub error_message: Option<String>,
    pub executed_at: DateTime<Utc>,
}
