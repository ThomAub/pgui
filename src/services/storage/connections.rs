//! Connection repository using SQLite and system keyring.

use anyhow::{Context, Result};
use keyring::Entry;
use sqlx::SqlitePool;
use std::path::PathBuf;
use uuid::Uuid;

use crate::services::database::traits::DatabaseType;
use super::types::{ConnectionInfo, SslMode};

const KEYRING_SERVICE: &str = "pgui";

/// Repository for connection CRUD operations.
///
/// Passwords are stored securely in the system keyring, while connection
/// metadata (host, port, username, etc.) is stored in SQLite.
#[derive(Debug, Clone)]
pub struct ConnectionsRepository {
    pool: SqlitePool,
}

impl ConnectionsRepository {
    pub(crate) fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    // ========== Keyring Methods ==========

    fn get_keyring_entry(connection_id: &Uuid) -> Result<Entry> {
        Entry::new(KEYRING_SERVICE, &connection_id.to_string())
            .context("Failed to create keyring entry")
    }

    fn store_password(connection_id: &Uuid, password: &str) -> Result<()> {
        let entry = Self::get_keyring_entry(connection_id)?;
        entry
            .set_password(password)
            .context("Failed to store password in keyring")
    }

    fn get_password(connection_id: &Uuid) -> Result<String> {
        let entry = Self::get_keyring_entry(connection_id)?;
        entry
            .get_password()
            .context("Failed to retrieve password from keyring")
    }

    fn delete_password(connection_id: &Uuid) -> Result<()> {
        let entry = Self::get_keyring_entry(connection_id)?;
        let _ = entry.delete_credential();
        Ok(())
    }

    // ========== CRUD Methods ==========

    /// Load all saved connections from the database
    pub async fn load_all(&self) -> Result<Vec<ConnectionInfo>> {
        // Query includes all fields including new multi-database fields
        let rows = sqlx::query_as::<_, (String, String, String, String, String, String, i64, String, Option<String>, i64)>(
            "SELECT id, name, database_type, hostname, username, database, port, ssl_mode, file_path, read_only
             FROM connections
             ORDER BY name",
        )
        .fetch_all(&self.pool)
        .await?;

        let mut connections = Vec::new();
        for (id_str, name, db_type_str, hostname, username, database, port, ssl_mode_str, file_path, read_only) in rows {
            let id = Uuid::parse_str(&id_str).context("Invalid UUID in database")?;
            let password = String::new(); // Load on-demand to avoid keychain prompts

            let database_type = DatabaseType::from_str(&db_type_str)
                .unwrap_or(DatabaseType::PostgreSQL);

            connections.push(ConnectionInfo {
                id,
                name,
                database_type,
                hostname,
                username,
                password,
                database,
                port: port as usize,
                ssl_mode: SslMode::from_db_str(&ssl_mode_str),
                file_path: file_path.map(PathBuf::from),
                read_only: read_only != 0,
            });
        }

        Ok(connections)
    }

    /// Create a new connection
    pub async fn create(&self, connection: &ConnectionInfo) -> Result<()> {
        if self.exists_by_name(&connection.name).await? {
            anyhow::bail!(
                "A connection with the name '{}' already exists",
                connection.name
            );
        }

        if !connection.password.is_empty() {
            Self::store_password(&connection.id, &connection.password)?;
        }

        let file_path_str = connection.file_path.as_ref().map(|p| p.display().to_string());

        sqlx::query(
            r#"
            INSERT INTO connections (id, name, database_type, hostname, username, database, port, ssl_mode, file_path, read_only, updated_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, CURRENT_TIMESTAMP)
            "#,
        )
        .bind(connection.id.to_string())
        .bind(&connection.name)
        .bind(connection.database_type.to_db_str())
        .bind(&connection.hostname)
        .bind(&connection.username)
        .bind(&connection.database)
        .bind(connection.port as i64)
        .bind(connection.ssl_mode.to_db_str())
        .bind(&file_path_str)
        .bind(if connection.read_only { 1i64 } else { 0i64 })
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Update an existing connection
    pub async fn update(&self, connection: &ConnectionInfo) -> Result<()> {
        let existing = sqlx::query_scalar::<_, String>(
            "SELECT id FROM connections WHERE name = ?1 AND id != ?2",
        )
        .bind(&connection.name)
        .bind(connection.id.to_string())
        .fetch_optional(&self.pool)
        .await?;

        if existing.is_some() {
            anyhow::bail!(
                "A connection with the name '{}' already exists",
                connection.name
            );
        }

        if !connection.password.is_empty() {
            Self::store_password(&connection.id, &connection.password)?;
        }

        let file_path_str = connection.file_path.as_ref().map(|p| p.display().to_string());

        sqlx::query(
            r#"
            UPDATE connections
            SET name = ?2, database_type = ?3, hostname = ?4, username = ?5, database = ?6,
                port = ?7, ssl_mode = ?8, file_path = ?9, read_only = ?10, updated_at = CURRENT_TIMESTAMP
            WHERE id = ?1
            "#,
        )
        .bind(connection.id.to_string())
        .bind(&connection.name)
        .bind(connection.database_type.to_db_str())
        .bind(&connection.hostname)
        .bind(&connection.username)
        .bind(&connection.database)
        .bind(connection.port as i64)
        .bind(connection.ssl_mode.to_db_str())
        .bind(&file_path_str)
        .bind(if connection.read_only { 1i64 } else { 0i64 })
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Delete a connection by ID
    pub async fn delete(&self, id: &Uuid) -> Result<()> {
        Self::delete_password(id)?;
        sqlx::query("DELETE FROM connections WHERE id = ?1")
            .bind(id.to_string())
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Get a single connection by ID
    #[allow(dead_code)]
    pub async fn get(&self, id: &Uuid) -> Result<Option<ConnectionInfo>> {
        let result = sqlx::query_as::<_, (String, String, String, String, String, String, i64, String, Option<String>, i64)>(
            "SELECT id, name, database_type, hostname, username, database, port, ssl_mode, file_path, read_only
             FROM connections WHERE id = ?1",
        )
        .bind(id.to_string())
        .fetch_optional(&self.pool)
        .await?;

        Ok(result.map(
            |(id_str, name, db_type_str, hostname, username, database, port, ssl_mode_str, file_path, read_only)| {
                let database_type = DatabaseType::from_str(&db_type_str)
                    .unwrap_or(DatabaseType::PostgreSQL);

                ConnectionInfo {
                    id: Uuid::parse_str(&id_str).unwrap_or_else(|_| Uuid::new_v4()),
                    name,
                    database_type,
                    hostname,
                    username,
                    password: String::new(),
                    database,
                    port: port as usize,
                    ssl_mode: SslMode::from_db_str(&ssl_mode_str),
                    file_path: file_path.map(PathBuf::from),
                    read_only: read_only != 0,
                }
            },
        ))
    }

    /// Get password for a connection from keyring (on-demand)
    pub fn get_connection_password(connection_id: &Uuid) -> Result<String> {
        Self::get_password(connection_id)
    }

    /// Check if a connection with the given name exists
    pub async fn exists_by_name(&self, name: &str) -> Result<bool> {
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM connections WHERE name = ?1")
            .bind(name)
            .fetch_one(&self.pool)
            .await?;
        Ok(count > 0)
    }
}
