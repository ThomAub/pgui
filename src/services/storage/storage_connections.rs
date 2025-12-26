//! Storage connection repository using SQLite and system keyring.

use anyhow::{Context, Result};
#[cfg(feature = "keyring")]
use keyring::Entry;
use sqlx::SqlitePool;
use std::collections::HashMap;
use std::path::PathBuf;
use uuid::Uuid;

use crate::services::database::storage::{StorageConfig, StorageParams, StorageType};

#[cfg(feature = "keyring")]
const KEYRING_SERVICE: &str = "pgui-storage";

/// Repository for storage connection CRUD operations.
///
/// Secrets (access keys, etc.) are stored securely in the system keyring,
/// while connection metadata is stored in SQLite.
#[derive(Debug, Clone)]
pub struct StorageConnectionsRepository {
    pool: SqlitePool,
}

impl StorageConnectionsRepository {
    pub(crate) fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    // ========== Keyring Methods (feature-gated) ==========

    #[cfg(feature = "keyring")]
    fn get_keyring_entry(connection_id: &Uuid) -> Result<Entry> {
        Entry::new(KEYRING_SERVICE, &connection_id.to_string())
            .context("Failed to create keyring entry")
    }

    #[cfg(feature = "keyring")]
    fn store_secret(connection_id: &Uuid, secret: &str) -> Result<()> {
        let entry = Self::get_keyring_entry(connection_id)?;
        entry
            .set_password(secret)
            .context("Failed to store secret in keyring")
    }

    #[cfg(not(feature = "keyring"))]
    fn store_secret(_connection_id: &Uuid, _secret: &str) -> Result<()> {
        tracing::warn!("Keyring feature disabled - secret will not be stored securely");
        Ok(())
    }

    #[cfg(feature = "keyring")]
    fn get_secret(connection_id: &Uuid) -> Result<String> {
        let entry = Self::get_keyring_entry(connection_id)?;
        entry
            .get_password()
            .context("Failed to retrieve secret from keyring")
    }

    #[cfg(not(feature = "keyring"))]
    fn get_secret(_connection_id: &Uuid) -> Result<String> {
        tracing::warn!("Keyring feature disabled - cannot retrieve stored secret");
        Ok(String::new())
    }

    #[cfg(feature = "keyring")]
    fn delete_secret(connection_id: &Uuid) -> Result<()> {
        let entry = Self::get_keyring_entry(connection_id)?;
        let _ = entry.delete_credential();
        Ok(())
    }

    #[cfg(not(feature = "keyring"))]
    fn delete_secret(_connection_id: &Uuid) -> Result<()> {
        Ok(())
    }

    // ========== CRUD Methods ==========

    /// Load all saved storage connections from the database
    pub async fn load_all(&self) -> Result<Vec<StorageConfig>> {
        let rows = sqlx::query_as::<_, (String, String, String, Option<String>, String, String, Option<String>, i64, i64, Option<String>)>(
            "SELECT id, name, storage_type, endpoint, region, bucket, access_key_id, path_style, allow_anonymous, root_path
             FROM storage_connections
             ORDER BY name",
        )
        .fetch_all(&self.pool)
        .await?;

        let mut connections = Vec::new();
        for (id_str, name, storage_type_str, endpoint, region, bucket, access_key_id, path_style, allow_anonymous, root_path) in rows
        {
            let id = Uuid::parse_str(&id_str).context("Invalid UUID in database")?;

            let storage_type = match storage_type_str.as_str() {
                "s3" => StorageType::S3,
                "gcs" => StorageType::Gcs,
                "azure_blob" => StorageType::AzureBlob,
                "local_fs" => StorageType::LocalFs,
                _ => StorageType::S3, // Default fallback
            };

            let params = match storage_type {
                StorageType::S3 => StorageParams::S3 {
                    endpoint: if endpoint.as_ref().map(|e| e.is_empty()).unwrap_or(true) {
                        None
                    } else {
                        endpoint
                    },
                    region,
                    bucket,
                    access_key_id,
                    path_style: path_style != 0,
                    allow_anonymous: allow_anonymous != 0,
                    extra_options: HashMap::new(),
                },
                StorageType::LocalFs => {
                    StorageParams::local_fs(PathBuf::from(root_path.unwrap_or_default()))
                }
                StorageType::Gcs => StorageParams::Gcs {
                    bucket,
                    credentials_path: None,
                    project_id: None,
                    extra_options: HashMap::new(),
                },
                StorageType::AzureBlob => StorageParams::AzureBlob {
                    account_name: region, // Reuse region field for account name
                    container: bucket,
                    account_key: None,
                    extra_options: HashMap::new(),
                },
            };

            connections.push(StorageConfig::with_id(id, name, storage_type, params));
        }

        Ok(connections)
    }

    /// Create a new storage connection
    pub async fn create(&self, config: &StorageConfig, secret: Option<&str>) -> Result<()> {
        if self.exists_by_name(&config.name).await? {
            anyhow::bail!(
                "A storage connection with the name '{}' already exists",
                config.name
            );
        }

        // Store secret in keyring if provided
        if let Some(secret) = secret {
            if !secret.is_empty() {
                Self::store_secret(&config.id, secret)?;
            }
        }

        // Extract params for SQL
        let (storage_type_str, endpoint, region, bucket, access_key_id, path_style, allow_anonymous, root_path) = match &config.params {
            StorageParams::S3 {
                endpoint,
                region,
                bucket,
                access_key_id,
                path_style,
                allow_anonymous,
                ..
            } => (
                "s3",
                endpoint.clone(),
                region.clone(),
                bucket.clone(),
                access_key_id.clone(),
                if *path_style { 1i64 } else { 0i64 },
                if *allow_anonymous { 1i64 } else { 0i64 },
                None::<String>,
            ),
            StorageParams::LocalFs { root_path } => (
                "local_fs",
                None,
                String::new(),
                String::new(),
                None,
                0i64,
                0i64,
                Some(root_path.display().to_string()),
            ),
            StorageParams::Gcs { bucket, .. } => (
                "gcs",
                None,
                String::new(),
                bucket.clone(),
                None,
                0i64,
                0i64,
                None,
            ),
            StorageParams::AzureBlob {
                account_name,
                container,
                ..
            } => (
                "azure_blob",
                None,
                account_name.clone(),
                container.clone(),
                None,
                0i64,
                0i64,
                None,
            ),
        };

        sqlx::query(
            r#"
            INSERT INTO storage_connections (id, name, storage_type, endpoint, region, bucket, access_key_id, path_style, allow_anonymous, root_path, updated_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, CURRENT_TIMESTAMP)
            "#,
        )
        .bind(config.id.to_string())
        .bind(&config.name)
        .bind(storage_type_str)
        .bind(&endpoint)
        .bind(&region)
        .bind(&bucket)
        .bind(&access_key_id)
        .bind(path_style)
        .bind(allow_anonymous)
        .bind(&root_path)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Update an existing storage connection
    pub async fn update(&self, config: &StorageConfig, secret: Option<&str>) -> Result<()> {
        let existing = sqlx::query_scalar::<_, String>(
            "SELECT id FROM storage_connections WHERE name = ?1 AND id != ?2",
        )
        .bind(&config.name)
        .bind(config.id.to_string())
        .fetch_optional(&self.pool)
        .await?;

        if existing.is_some() {
            anyhow::bail!(
                "A storage connection with the name '{}' already exists",
                config.name
            );
        }

        // Update secret if provided
        if let Some(secret) = secret {
            if !secret.is_empty() {
                Self::store_secret(&config.id, secret)?;
            }
        }

        // Extract params
        let (storage_type_str, endpoint, region, bucket, access_key_id, path_style, allow_anonymous, root_path) = match &config.params {
            StorageParams::S3 {
                endpoint,
                region,
                bucket,
                access_key_id,
                path_style,
                allow_anonymous,
                ..
            } => (
                "s3",
                endpoint.clone(),
                region.clone(),
                bucket.clone(),
                access_key_id.clone(),
                if *path_style { 1i64 } else { 0i64 },
                if *allow_anonymous { 1i64 } else { 0i64 },
                None::<String>,
            ),
            StorageParams::LocalFs { root_path } => (
                "local_fs",
                None,
                String::new(),
                String::new(),
                None,
                0i64,
                0i64,
                Some(root_path.display().to_string()),
            ),
            StorageParams::Gcs { bucket, .. } => (
                "gcs",
                None,
                String::new(),
                bucket.clone(),
                None,
                0i64,
                0i64,
                None,
            ),
            StorageParams::AzureBlob {
                account_name,
                container,
                ..
            } => (
                "azure_blob",
                None,
                account_name.clone(),
                container.clone(),
                None,
                0i64,
                0i64,
                None,
            ),
        };

        sqlx::query(
            r#"
            UPDATE storage_connections
            SET name = ?2, storage_type = ?3, endpoint = ?4, region = ?5, bucket = ?6,
                access_key_id = ?7, path_style = ?8, allow_anonymous = ?9, root_path = ?10,
                updated_at = CURRENT_TIMESTAMP
            WHERE id = ?1
            "#,
        )
        .bind(config.id.to_string())
        .bind(&config.name)
        .bind(storage_type_str)
        .bind(&endpoint)
        .bind(&region)
        .bind(&bucket)
        .bind(&access_key_id)
        .bind(path_style)
        .bind(allow_anonymous)
        .bind(&root_path)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Delete a storage connection by ID
    pub async fn delete(&self, id: &Uuid) -> Result<()> {
        Self::delete_secret(id)?;
        sqlx::query("DELETE FROM storage_connections WHERE id = ?1")
            .bind(id.to_string())
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Get secret for a storage connection from keyring
    pub fn get_connection_secret(connection_id: &Uuid) -> Result<String> {
        Self::get_secret(connection_id)
    }

    /// Check if a storage connection with the given name exists
    pub async fn exists_by_name(&self, name: &str) -> Result<bool> {
        let count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM storage_connections WHERE name = ?1")
                .bind(name)
                .fetch_one(&self.pool)
                .await?;
        Ok(count > 0)
    }
}
