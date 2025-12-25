//! S3 storage implementation using OpenDAL.
//!
//! This module provides S3 and S3-compatible storage support including:
//! - Amazon S3
//! - MinIO
//! - Cloudflare R2
//! - DigitalOcean Spaces
//! - Any S3-compatible service

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use bytes::Bytes;
use chrono::{DateTime, Utc};
use futures::stream::BoxStream;
use futures::StreamExt;
use opendal::layers::LoggingLayer;
use opendal::services::S3;
use opendal::{EntryMode, Metakey, Operator};
use std::sync::Arc;
use tokio::sync::RwLock;

use super::traits::StorageConnection;
use super::types::{ObjectInfo, StorageConfig, StorageParams, StorageType};

/// S3 storage connection implementation.
///
/// Uses OpenDAL for all S3 operations, providing support for AWS S3
/// and S3-compatible services like MinIO, R2, and DigitalOcean Spaces.
pub struct S3Storage {
    config: StorageConfig,
    operator: RwLock<Option<Operator>>,
    /// Secret access key (loaded from keyring separately)
    secret_key: RwLock<Option<String>>,
}

impl S3Storage {
    /// Create a new S3 storage connection.
    pub fn new(config: StorageConfig) -> Self {
        Self {
            config,
            operator: RwLock::new(None),
            secret_key: RwLock::new(None),
        }
    }

    /// Create a boxed S3 storage connection.
    pub fn boxed(config: StorageConfig) -> Box<dyn StorageConnection> {
        Box::new(Self::new(config))
    }

    /// Set the secret access key (loaded from keyring).
    pub async fn set_secret_key(&self, secret_key: String) {
        let mut guard = self.secret_key.write().await;
        *guard = Some(secret_key);
    }

    /// Get S3 params from config.
    fn get_s3_params(&self) -> Result<(&str, &str, Option<&str>, Option<&str>, bool, bool)> {
        match &self.config.params {
            StorageParams::S3 {
                endpoint,
                region,
                bucket,
                access_key_id,
                path_style,
                allow_anonymous,
                ..
            } => Ok((
                region.as_str(),
                bucket.as_str(),
                endpoint.as_deref(),
                access_key_id.as_deref(),
                *path_style,
                *allow_anonymous,
            )),
            _ => Err(anyhow!("Invalid storage params for S3")),
        }
    }

    /// Build the OpenDAL operator.
    async fn build_operator(&self) -> Result<Operator> {
        let (region, bucket, endpoint, access_key_id, path_style, allow_anonymous) =
            self.get_s3_params()?;

        let mut builder = S3::default();

        // Required settings
        builder = builder.bucket(bucket).region(region);

        // Custom endpoint for S3-compatible services
        if let Some(ep) = endpoint {
            if !ep.is_empty() {
                builder = builder.endpoint(ep);
            }
        }

        // Credentials
        if !allow_anonymous {
            if let Some(key_id) = access_key_id {
                builder = builder.access_key_id(key_id);

                // Get secret key
                let secret_guard = self.secret_key.read().await;
                if let Some(secret) = secret_guard.as_ref() {
                    builder = builder.secret_access_key(secret);
                }
            }
        }

        // Path style for MinIO and similar
        if path_style {
            builder = builder.enable_virtual_host_style();
        }

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

    /// Normalize path for S3 (ensure no leading slash, handle root).
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
                DateTime::<Utc>::from_timestamp(t.unix_timestamp(), 0).unwrap_or_default()
            }),
            content_type: metadata.content_type().map(|s| s.to_string()),
            etag: metadata.etag().map(|s| s.to_string()),
        }
    }
}

#[async_trait]
impl StorageConnection for S3Storage {
    fn storage_type(&self) -> StorageType {
        StorageType::S3
    }

    fn storage_config(&self) -> &StorageConfig {
        &self.config
    }

    async fn connect(&mut self) -> Result<()> {
        let op = self.build_operator().await?;

        // Test the connection by checking if we can access the bucket
        op.check().await.map_err(|e| {
            anyhow!(
                "Failed to connect to S3: {}. Check your credentials and bucket name.",
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

        // List with metadata
        let mut lister = op
            .lister_with(path)
            .metakey(Metakey::ContentLength | Metakey::LastModified | Metakey::ContentType)
            .await?;

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

        let mut lister = op
            .lister_with(path)
            .recursive(true)
            .metakey(Metakey::ContentLength | Metakey::LastModified | Metakey::ContentType)
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
        content_type: &str,
    ) -> Result<()> {
        let op = self.get_operator().await?;
        let path = Self::normalize_path(path);

        op.write_with(path, data)
            .content_type(content_type)
            .await?;
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
        if let StorageParams::S3 { bucket, .. } = &self.config.params {
            format!("s3://{}/{}", bucket, path)
        } else {
            format!("s3://unknown/{}", path)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_path() {
        assert_eq!(S3Storage::normalize_path("/data/file.txt"), "data/file.txt");
        assert_eq!(S3Storage::normalize_path("data/file.txt"), "data/file.txt");
        assert_eq!(S3Storage::normalize_path("/"), "");
        assert_eq!(S3Storage::normalize_path(""), "");
    }

    #[test]
    fn test_object_uri() {
        let config = StorageConfig::new(
            "test".to_string(),
            StorageType::S3,
            StorageParams::s3(
                None,
                "us-east-1".to_string(),
                "my-bucket".to_string(),
                None,
                false,
            ),
        );
        let storage = S3Storage::new(config);

        assert_eq!(
            storage.object_uri("/data/file.txt"),
            "s3://my-bucket/data/file.txt"
        );
    }
}
