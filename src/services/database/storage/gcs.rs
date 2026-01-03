//! Google Cloud Storage implementation using OpenDAL.
//!
//! This module provides GCS storage support for Google Cloud Platform.

use anyhow::{anyhow, Result};
use async_lock::RwLock;
use async_trait::async_trait;
use bytes::Bytes;
use chrono::{DateTime, Utc};
use futures::stream::BoxStream;
use futures::StreamExt;
use opendal::layers::LoggingLayer;
use opendal::services::Gcs;
use opendal::{EntryMode, Operator};

use super::traits::StorageConnection;
use super::types::{ObjectInfo, StorageConfig, StorageParams, StorageType};

/// Google Cloud Storage connection implementation.
///
/// Uses OpenDAL for all GCS operations, providing support for Google Cloud Storage
/// buckets with service account authentication.
pub struct GcsStorage {
    config: StorageConfig,
    operator: RwLock<Option<Operator>>,
}

impl GcsStorage {
    /// Create a new GCS storage connection.
    pub fn new(config: StorageConfig) -> Self {
        Self {
            config,
            operator: RwLock::new(None),
        }
    }

    /// Create a boxed GCS storage connection.
    pub fn boxed(config: StorageConfig) -> Box<dyn StorageConnection> {
        Box::new(Self::new(config))
    }

    /// Get GCS params from config.
    fn get_gcs_params(&self) -> Result<(&str, Option<&std::path::Path>, Option<&str>)> {
        match &self.config.params {
            StorageParams::Gcs {
                bucket,
                credentials_path,
                project_id,
                ..
            } => Ok((
                bucket.as_str(),
                credentials_path.as_deref(),
                project_id.as_deref(),
            )),
            _ => Err(anyhow!("Invalid storage params for GCS")),
        }
    }

    /// Build the OpenDAL operator.
    async fn build_operator(&self) -> Result<Operator> {
        let (bucket, credentials_path, _project_id) = self.get_gcs_params()?;

        let mut builder = Gcs::default();

        // Required: bucket name
        builder = builder.bucket(bucket);

        // Optional: service account credentials
        if let Some(creds_path) = credentials_path {
            let creds_path_str = creds_path
                .to_str()
                .ok_or_else(|| anyhow!("Invalid credentials path"))?;
            builder = builder.credential_path(creds_path_str);
        }

        // Optional: project ID (required for some operations)
        // Note: GCS builder might not have project_id method in all versions
        // The project is usually inferred from credentials

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

    /// Normalize path for GCS (ensure no leading slash, handle root).
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
            etag: metadata.etag().map(|s| s.to_string()),
        }
    }
}

#[async_trait]
impl StorageConnection for GcsStorage {
    fn storage_type(&self) -> StorageType {
        StorageType::Gcs
    }

    fn storage_config(&self) -> &StorageConfig {
        &self.config
    }

    async fn connect(&mut self) -> Result<()> {
        let op = self.build_operator().await?;

        // Test the connection by checking if we can access the bucket
        op.check().await.map_err(|e| {
            anyhow!(
                "Failed to connect to GCS: {}. Check your credentials and bucket name.",
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
        let op = self.build_operator().await?;
        op.check().await.map_err(|e| {
            anyhow!(
                "Connection test failed: {}. Check your credentials and bucket access.",
                e
            )
        })?;
        Ok(())
    }

    async fn list(&self, path: &str) -> Result<Vec<ObjectInfo>> {
        let op = self.get_operator().await?;
        let path = Self::normalize_path(path);

        let mut lister = op.lister_with(path).await?;

        let mut objects = Vec::new();

        while let Some(entry) = lister.next().await {
            let entry = entry?;
            let metadata = entry.metadata();
            let entry_path = entry.path().to_string();

            // Skip the path itself if it's a directory marker
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

        let mut lister = op.lister_with(path).recursive(true).await?;

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
        content_type: &str,
    ) -> Result<()> {
        let op = self.get_operator().await?;
        let path = Self::normalize_path(path);

        op.write_with(path, data).content_type(content_type).await?;
        Ok(())
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

    async fn presigned_url(&self, path: &str, expires_in_secs: u64) -> Result<String> {
        let op = self.get_operator().await?;
        let path = Self::normalize_path(path);

        let duration = std::time::Duration::from_secs(expires_in_secs);
        let url = op.presign_read(path, duration).await?;

        Ok(url.uri().to_string())
    }

    fn object_uri(&self, path: &str) -> String {
        let path = Self::normalize_path(path);
        if let StorageParams::Gcs { bucket, .. } = &self.config.params {
            format!("gs://{}/{}", bucket, path)
        } else {
            format!("gs://unknown/{}", path)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn create_test_config() -> StorageConfig {
        StorageConfig::new(
            "test".to_string(),
            StorageType::Gcs,
            StorageParams::Gcs {
                bucket: "my-bucket".to_string(),
                credentials_path: None,
                project_id: Some("my-project".to_string()),
                extra_options: HashMap::new(),
            },
        )
    }

    #[test]
    fn test_normalize_path() {
        assert_eq!(GcsStorage::normalize_path("/data/file.txt"), "data/file.txt");
        assert_eq!(GcsStorage::normalize_path("data/file.txt"), "data/file.txt");
        assert_eq!(GcsStorage::normalize_path("/"), "");
        assert_eq!(GcsStorage::normalize_path(""), "");
    }

    #[test]
    fn test_object_uri() {
        let config = create_test_config();
        let storage = GcsStorage::new(config);

        assert_eq!(
            storage.object_uri("/data/file.txt"),
            "gs://my-bucket/data/file.txt"
        );
    }

    #[test]
    fn test_storage_type() {
        let config = create_test_config();
        let storage = GcsStorage::new(config);

        assert_eq!(storage.storage_type(), StorageType::Gcs);
    }
}
