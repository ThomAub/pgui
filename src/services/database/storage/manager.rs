//! Storage connection manager.
//!
//! This module provides a manager for storage connections, handling
//! the lifecycle of connections and providing a unified API for storage operations.

use anyhow::{anyhow, Result};
use bytes::Bytes;
use futures::stream::BoxStream;
use std::sync::Arc;
use tokio::sync::RwLock;

use super::factory::StorageFactory;
use super::traits::BoxedStorageConnection;
use super::types::{ObjectInfo, StorageConfig, StorageType};

/// Connection status for storage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StorageConnectionStatus {
    /// Not connected to any storage.
    Disconnected,
    /// Currently connecting.
    Connecting,
    /// Connected to storage.
    Connected,
    /// Connection failed with error.
    Failed(String),
}

/// Manager for storage connections.
///
/// Provides a singleton-like interface for managing the active storage
/// connection and performing storage operations.
///
/// # Example
///
/// ```ignore
/// use pgui::services::database::storage::{StorageManager, StorageConfig, StorageType, StorageParams};
///
/// let manager = StorageManager::new();
///
/// let config = StorageConfig::new(
///     "My S3".to_string(),
///     StorageType::S3,
///     StorageParams::s3(None, "us-east-1".to_string(), "my-bucket".to_string(), None, false),
/// );
///
/// manager.connect(config).await?;
///
/// let objects = manager.list("/").await?;
/// ```
pub struct StorageManager {
    connection: Arc<RwLock<Option<BoxedStorageConnection>>>,
    config: Arc<RwLock<Option<StorageConfig>>>,
    status: Arc<RwLock<StorageConnectionStatus>>,
}

impl Default for StorageManager {
    fn default() -> Self {
        Self::new()
    }
}

impl StorageManager {
    /// Create a new storage manager.
    pub fn new() -> Self {
        Self {
            connection: Arc::new(RwLock::new(None)),
            config: Arc::new(RwLock::new(None)),
            status: Arc::new(RwLock::new(StorageConnectionStatus::Disconnected)),
        }
    }

    /// Get the current connection status.
    pub async fn status(&self) -> StorageConnectionStatus {
        self.status.read().await.clone()
    }

    /// Get the current connection config.
    pub async fn current_config(&self) -> Option<StorageConfig> {
        self.config.read().await.clone()
    }

    /// Check if connected.
    pub async fn is_connected(&self) -> bool {
        matches!(*self.status.read().await, StorageConnectionStatus::Connected)
    }

    /// Connect to storage using the provided configuration.
    ///
    /// If already connected, disconnects first.
    pub async fn connect(&self, config: StorageConfig) -> Result<()> {
        // Disconnect if already connected
        if self.is_connected().await {
            self.disconnect().await?;
        }

        // Update status
        {
            let mut status = self.status.write().await;
            *status = StorageConnectionStatus::Connecting;
        }

        // Create connection
        let mut connection = match StorageFactory::create(config.clone()) {
            Ok(conn) => conn,
            Err(e) => {
                let mut status = self.status.write().await;
                *status = StorageConnectionStatus::Failed(e.to_string());
                return Err(e);
            }
        };

        // Connect
        if let Err(e) = connection.connect().await {
            let mut status = self.status.write().await;
            *status = StorageConnectionStatus::Failed(e.to_string());
            return Err(e);
        }

        // Store connection and config
        {
            let mut conn_guard = self.connection.write().await;
            *conn_guard = Some(connection);
        }
        {
            let mut config_guard = self.config.write().await;
            *config_guard = Some(config);
        }
        {
            let mut status = self.status.write().await;
            *status = StorageConnectionStatus::Connected;
        }

        Ok(())
    }

    /// Disconnect from current storage.
    pub async fn disconnect(&self) -> Result<()> {
        let mut conn_guard = self.connection.write().await;
        if let Some(mut conn) = conn_guard.take() {
            conn.disconnect().await?;
        }

        {
            let mut config_guard = self.config.write().await;
            *config_guard = None;
        }
        {
            let mut status = self.status.write().await;
            *status = StorageConnectionStatus::Disconnected;
        }

        Ok(())
    }

    /// Test a connection without fully connecting.
    pub async fn test_connection(&self, config: StorageConfig) -> Result<()> {
        let connection = StorageFactory::create(config)?;
        connection.test_connection().await
    }

    /// Get the storage type of the current connection.
    pub async fn storage_type(&self) -> Option<StorageType> {
        let guard = self.connection.read().await;
        guard.as_ref().map(|c| c.storage_type())
    }

    // Delegated storage operations

    /// List objects at the given path.
    pub async fn list(&self, path: &str) -> Result<Vec<ObjectInfo>> {
        let guard = self.connection.read().await;
        let conn = guard
            .as_ref()
            .ok_or_else(|| anyhow!("Not connected to storage"))?;
        conn.list(path).await
    }

    /// List objects recursively at the given path.
    pub async fn list_recursive(&self, path: &str) -> Result<Vec<ObjectInfo>> {
        let guard = self.connection.read().await;
        let conn = guard
            .as_ref()
            .ok_or_else(|| anyhow!("Not connected to storage"))?;
        conn.list_recursive(path).await
    }

    /// Read the contents of an object.
    pub async fn read(&self, path: &str) -> Result<Vec<u8>> {
        let guard = self.connection.read().await;
        let conn = guard
            .as_ref()
            .ok_or_else(|| anyhow!("Not connected to storage"))?;
        conn.read(path).await
    }

    /// Read an object as a stream.
    pub async fn read_stream(&self, path: &str) -> Result<BoxStream<'static, Result<Bytes>>> {
        let guard = self.connection.read().await;
        let conn = guard
            .as_ref()
            .ok_or_else(|| anyhow!("Not connected to storage"))?;
        conn.read_stream(path).await
    }

    /// Read a range of bytes from an object.
    pub async fn read_range(&self, path: &str, offset: u64, length: u64) -> Result<Vec<u8>> {
        let guard = self.connection.read().await;
        let conn = guard
            .as_ref()
            .ok_or_else(|| anyhow!("Not connected to storage"))?;
        conn.read_range(path, offset, length).await
    }

    /// Write data to an object.
    pub async fn write(&self, path: &str, data: Vec<u8>) -> Result<()> {
        let guard = self.connection.read().await;
        let conn = guard
            .as_ref()
            .ok_or_else(|| anyhow!("Not connected to storage"))?;
        conn.write(path, data).await
    }

    /// Write data to an object with content type.
    pub async fn write_with_content_type(
        &self,
        path: &str,
        data: Vec<u8>,
        content_type: &str,
    ) -> Result<()> {
        let guard = self.connection.read().await;
        let conn = guard
            .as_ref()
            .ok_or_else(|| anyhow!("Not connected to storage"))?;
        conn.write_with_content_type(path, data, content_type).await
    }

    /// Delete an object.
    pub async fn delete(&self, path: &str) -> Result<()> {
        let guard = self.connection.read().await;
        let conn = guard
            .as_ref()
            .ok_or_else(|| anyhow!("Not connected to storage"))?;
        conn.delete(path).await
    }

    /// Delete multiple objects.
    pub async fn delete_many(&self, paths: &[String]) -> Result<()> {
        let guard = self.connection.read().await;
        let conn = guard
            .as_ref()
            .ok_or_else(|| anyhow!("Not connected to storage"))?;
        conn.delete_many(paths).await
    }

    /// Check if an object exists.
    pub async fn exists(&self, path: &str) -> Result<bool> {
        let guard = self.connection.read().await;
        let conn = guard
            .as_ref()
            .ok_or_else(|| anyhow!("Not connected to storage"))?;
        conn.exists(path).await
    }

    /// Get metadata for an object.
    pub async fn stat(&self, path: &str) -> Result<ObjectInfo> {
        let guard = self.connection.read().await;
        let conn = guard
            .as_ref()
            .ok_or_else(|| anyhow!("Not connected to storage"))?;
        conn.stat(path).await
    }

    /// Create a directory.
    pub async fn create_dir(&self, path: &str) -> Result<()> {
        let guard = self.connection.read().await;
        let conn = guard
            .as_ref()
            .ok_or_else(|| anyhow!("Not connected to storage"))?;
        conn.create_dir(path).await
    }

    /// Copy an object.
    pub async fn copy(&self, src: &str, dst: &str) -> Result<()> {
        let guard = self.connection.read().await;
        let conn = guard
            .as_ref()
            .ok_or_else(|| anyhow!("Not connected to storage"))?;
        conn.copy(src, dst).await
    }

    /// Rename/move an object.
    pub async fn rename(&self, src: &str, dst: &str) -> Result<()> {
        let guard = self.connection.read().await;
        let conn = guard
            .as_ref()
            .ok_or_else(|| anyhow!("Not connected to storage"))?;
        conn.rename(src, dst).await
    }

    /// Get a presigned URL for an object.
    pub async fn presigned_url(&self, path: &str, expires_in_secs: u64) -> Result<String> {
        let guard = self.connection.read().await;
        let conn = guard
            .as_ref()
            .ok_or_else(|| anyhow!("Not connected to storage"))?;
        conn.presigned_url(path, expires_in_secs).await
    }

    /// Get the full URI for an object.
    pub async fn object_uri(&self, path: &str) -> Result<String> {
        let guard = self.connection.read().await;
        let conn = guard
            .as_ref()
            .ok_or_else(|| anyhow!("Not connected to storage"))?;
        Ok(conn.object_uri(path))
    }
}

impl Clone for StorageManager {
    fn clone(&self) -> Self {
        Self {
            connection: Arc::clone(&self.connection),
            config: Arc::clone(&self.config),
            status: Arc::clone(&self.status),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_manager_initial_state() {
        let manager = StorageManager::new();
        // Initial state should be disconnected
        // (async test would be needed to fully test)
    }

    #[test]
    fn test_manager_clone() {
        let manager = StorageManager::new();
        let _cloned = manager.clone();
        // Should be able to clone the manager
    }
}
