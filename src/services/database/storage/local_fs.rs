//! Local filesystem storage implementation using OpenDAL.
//!
//! This module provides local filesystem storage support for development
//! and testing purposes. It uses the same trait interface as cloud storage.

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use bytes::Bytes;
use chrono::{DateTime, Utc};
use futures::stream::BoxStream;
use futures::StreamExt;
use opendal::layers::LoggingLayer;
use opendal::services::Fs;
use opendal::{EntryMode, Operator};
use std::path::PathBuf;
use async_lock::RwLock;

use super::traits::StorageConnection;
use super::types::{ObjectInfo, StorageConfig, StorageParams, StorageType};

/// Local filesystem storage connection implementation.
///
/// Uses OpenDAL for all filesystem operations, providing a consistent
/// interface with cloud storage backends.
pub struct LocalFsStorage {
    config: StorageConfig,
    operator: RwLock<Option<Operator>>,
}

impl LocalFsStorage {
    /// Create a new local filesystem storage connection.
    pub fn new(config: StorageConfig) -> Self {
        Self {
            config,
            operator: RwLock::new(None),
        }
    }

    /// Create a boxed local filesystem storage connection.
    pub fn boxed(config: StorageConfig) -> Box<dyn StorageConnection> {
        Box::new(Self::new(config))
    }

    /// Get the root path from config.
    fn get_root_path(&self) -> Result<&PathBuf> {
        match &self.config.params {
            StorageParams::LocalFs { root_path } => Ok(root_path),
            _ => Err(anyhow!("Invalid storage params for LocalFs")),
        }
    }

    /// Build the OpenDAL operator.
    fn build_operator(&self) -> Result<Operator> {
        let root_path = self.get_root_path()?;

        let mut builder = Fs::default();
        builder = builder.root(
            root_path
                .to_str()
                .ok_or_else(|| anyhow!("Invalid path encoding"))?,
        );

        let op = Operator::new(builder)?
            .layer(LoggingLayer::default())
            .finish();

        Ok(op)
    }

    /// Get the operator, returning an error if not connected.
    async fn get_operator(&self) -> Result<Operator> {
        let guard = self.operator.read().await;
        guard
            .as_ref()
            .cloned()
            .ok_or_else(|| anyhow!("Storage not connected"))
    }

    /// Normalize path (ensure no leading slash for OpenDAL).
    fn normalize_path(path: &str) -> &str {
        let path = path.trim_start_matches('/');
        if path.is_empty() {
            ""
        } else {
            path
        }
    }

    /// Convert OpenDAL entry to ObjectInfo.
    fn entry_to_object_info(path: String, metadata: &opendal::Metadata) -> ObjectInfo {
        let is_dir = metadata.mode() == EntryMode::DIR;
        let name = path
            .trim_end_matches('/')
            .rsplit('/')
            .next()
            .unwrap_or(&path)
            .to_string();

        ObjectInfo {
            path: path.clone(),
            name,
            is_dir,
            size: if is_dir {
                None
            } else {
                Some(metadata.content_length())
            },
            last_modified: metadata.last_modified().map(|t| {
                DateTime::<Utc>::from_timestamp(t.timestamp(), 0).unwrap_or_default()
            }),
            content_type: metadata.content_type().map(|s| s.to_string()),
            etag: None,
        }
    }
}

#[async_trait]
impl StorageConnection for LocalFsStorage {
    fn storage_type(&self) -> StorageType {
        StorageType::LocalFs
    }

    fn storage_config(&self) -> &StorageConfig {
        &self.config
    }

    async fn connect(&mut self) -> Result<()> {
        let op = self.build_operator()?;

        // Verify the root path exists
        op.check().await.map_err(|e| {
            anyhow!(
                "Failed to access filesystem path: {}. Check that the path exists and is accessible.",
                e
            )
        })?;

        let mut guard = self.operator.write().await;
        *guard = Some(op);

        Ok(())
    }

    async fn disconnect(&mut self) -> Result<()> {
        let mut guard = self.operator.write().await;
        *guard = None;
        Ok(())
    }

    async fn is_connected(&self) -> bool {
        let guard = self.operator.read().await;
        guard.is_some()
    }

    async fn test_connection(&self) -> Result<()> {
        let op = self.build_operator()?;
        op.check().await.map_err(|e| {
            anyhow!(
                "Connection test failed: {}. Check that the path exists.",
                e
            )
        })?;
        Ok(())
    }

    async fn list(&self, path: &str) -> Result<Vec<ObjectInfo>> {
        let op = self.get_operator().await?;
        let path = Self::normalize_path(path);

        let mut lister = op
            .lister_with(path)
            .await?;

        let mut objects = Vec::new();

        while let Some(entry) = lister.next().await {
            let entry = entry?;
            let metadata = entry.metadata();
            let entry_path = entry.path().to_string();

            // Skip the path itself
            if entry_path == path || entry_path == format!("{}/", path) {
                continue;
            }

            objects.push(Self::entry_to_object_info(entry_path, metadata));
        }

        // Sort: directories first, then by name
        objects.sort_by(|a, b| match (a.is_dir, b.is_dir) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
        });

        Ok(objects)
    }

    async fn list_recursive(&self, path: &str) -> Result<Vec<ObjectInfo>> {
        let op = self.get_operator().await?;
        let path = Self::normalize_path(path);

        let mut lister = op
            .lister_with(path)
            .recursive(true)
            .await?;

        let mut objects = Vec::new();

        while let Some(entry) = lister.next().await {
            let entry = entry?;
            let metadata = entry.metadata();
            let entry_path = entry.path().to_string();

            objects.push(Self::entry_to_object_info(entry_path, metadata));
        }

        Ok(objects)
    }

    async fn read(&self, path: &str) -> Result<Vec<u8>> {
        let op = self.get_operator().await?;
        let path = Self::normalize_path(path);

        let data = op.read(path).await?.to_vec();
        Ok(data)
    }

    async fn read_stream(&self, path: &str) -> Result<BoxStream<'static, Result<Bytes>>> {
        let op = self.get_operator().await?;
        let path = Self::normalize_path(path).to_string();

        let reader = op.reader(&path).await?;
        let stream = reader
            .into_bytes_stream(0..u64::MAX)
            .await?
            .map(|result| result.map_err(|e| anyhow!("Read error: {}", e)));

        Ok(Box::pin(stream))
    }

    async fn read_range(&self, path: &str, offset: u64, length: u64) -> Result<Vec<u8>> {
        let op = self.get_operator().await?;
        let path = Self::normalize_path(path);

        let data = op.read_with(path).range(offset..offset + length).await?;
        Ok(data.to_vec())
    }

    async fn write(&self, path: &str, data: Vec<u8>) -> Result<()> {
        let op = self.get_operator().await?;
        let path = Self::normalize_path(path);

        op.write(path, data).await?;
        Ok(())
    }

    async fn write_with_content_type(
        &self,
        path: &str,
        data: Vec<u8>,
        _content_type: &str,
    ) -> Result<()> {
        // Local filesystem doesn't support content-type metadata
        self.write(path, data).await
    }

    async fn delete(&self, path: &str) -> Result<()> {
        let op = self.get_operator().await?;
        let path = Self::normalize_path(path);

        op.delete(path).await?;
        Ok(())
    }

    async fn delete_many(&self, paths: &[String]) -> Result<()> {
        let op = self.get_operator().await?;

        for path in paths {
            let path = Self::normalize_path(path);
            op.delete(path).await?;
        }

        Ok(())
    }

    async fn exists(&self, path: &str) -> Result<bool> {
        let op = self.get_operator().await?;
        let path = Self::normalize_path(path);

        Ok(op.exists(path).await?)
    }

    async fn stat(&self, path: &str) -> Result<ObjectInfo> {
        let op = self.get_operator().await?;
        let path = Self::normalize_path(path);

        let metadata = op.stat(path).await?;
        Ok(Self::entry_to_object_info(path.to_string(), &metadata))
    }

    async fn create_dir(&self, path: &str) -> Result<()> {
        let op = self.get_operator().await?;
        let mut path = Self::normalize_path(path).to_string();

        // Ensure path ends with /
        if !path.ends_with('/') {
            path.push('/');
        }

        op.create_dir(&path).await?;
        Ok(())
    }

    async fn copy(&self, src: &str, dst: &str) -> Result<()> {
        let op = self.get_operator().await?;
        let src = Self::normalize_path(src);
        let dst = Self::normalize_path(dst);

        op.copy(src, dst).await?;
        Ok(())
    }

    async fn rename(&self, src: &str, dst: &str) -> Result<()> {
        let op = self.get_operator().await?;
        let src = Self::normalize_path(src);
        let dst = Self::normalize_path(dst);

        op.rename(src, dst).await?;
        Ok(())
    }

    async fn presigned_url(&self, path: &str, _expires_in_secs: u64) -> Result<String> {
        // Local filesystem doesn't support presigned URLs
        // Return a file:// URL instead
        let root = self.get_root_path()?;
        let path = Self::normalize_path(path);
        let full_path = root.join(path);

        Ok(format!("file://{}", full_path.display()))
    }

    fn object_uri(&self, path: &str) -> String {
        let root = self.get_root_path().ok();
        let path = Self::normalize_path(path);

        if let Some(root) = root {
            format!("file://{}/{}", root.display(), path)
        } else {
            format!("file:///{}", path)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_normalize_path() {
        assert_eq!(
            LocalFsStorage::normalize_path("/data/file.txt"),
            "data/file.txt"
        );
        assert_eq!(
            LocalFsStorage::normalize_path("data/file.txt"),
            "data/file.txt"
        );
        assert_eq!(LocalFsStorage::normalize_path("/"), "");
        assert_eq!(LocalFsStorage::normalize_path(""), "");
    }

    #[test]
    fn test_object_uri() {
        let config = StorageConfig::new(
            "test".to_string(),
            StorageType::LocalFs,
            StorageParams::local_fs(PathBuf::from("/home/user/data")),
        );
        let storage = LocalFsStorage::new(config);

        assert_eq!(
            storage.object_uri("/subdir/file.txt"),
            "file:///home/user/data/subdir/file.txt"
        );
    }
}
