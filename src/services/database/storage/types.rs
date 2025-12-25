//! Storage types and configuration.
//!
//! This module defines types for blob storage connections including
//! storage types, configuration, and object metadata.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use uuid::Uuid;

/// Supported storage backend types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StorageType {
    /// Amazon S3 and S3-compatible services (MinIO, R2, DigitalOcean Spaces)
    S3,
    /// Google Cloud Storage
    Gcs,
    /// Azure Blob Storage
    AzureBlob,
    /// Local filesystem
    LocalFs,
}

impl StorageType {
    /// Get the display name for this storage type.
    pub fn display_name(&self) -> &'static str {
        match self {
            StorageType::S3 => "Amazon S3",
            StorageType::Gcs => "Google Cloud Storage",
            StorageType::AzureBlob => "Azure Blob Storage",
            StorageType::LocalFs => "Local Filesystem",
        }
    }

    /// Get an icon name for this storage type.
    pub fn icon_name(&self) -> &'static str {
        match self {
            StorageType::S3 => "cloud",
            StorageType::Gcs => "cloud",
            StorageType::AzureBlob => "cloud",
            StorageType::LocalFs => "folder",
        }
    }

    /// Get all available storage types.
    pub fn all() -> Vec<StorageType> {
        vec![
            StorageType::S3,
            StorageType::Gcs,
            StorageType::AzureBlob,
            StorageType::LocalFs,
        ]
    }

    /// Check if this storage type requires credentials.
    pub fn requires_credentials(&self) -> bool {
        match self {
            StorageType::S3 | StorageType::Gcs | StorageType::AzureBlob => true,
            StorageType::LocalFs => false,
        }
    }
}

impl std::fmt::Display for StorageType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display_name())
    }
}

/// Configuration for a storage connection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageConfig {
    /// Unique identifier for this connection.
    pub id: Uuid,
    /// User-friendly name for the connection.
    pub name: String,
    /// The type of storage backend.
    pub storage_type: StorageType,
    /// Storage-specific parameters.
    pub params: StorageParams,
}

impl StorageConfig {
    /// Create a new storage configuration.
    pub fn new(name: String, storage_type: StorageType, params: StorageParams) -> Self {
        Self {
            id: Uuid::new_v4(),
            name,
            storage_type,
            params,
        }
    }

    /// Create a configuration with a specific ID (for loading from storage).
    pub fn with_id(id: Uuid, name: String, storage_type: StorageType, params: StorageParams) -> Self {
        Self {
            id,
            name,
            storage_type,
            params,
        }
    }

    /// Validate the configuration for the given storage type.
    pub fn validate(&self) -> Result<(), String> {
        match (&self.storage_type, &self.params) {
            (StorageType::S3, StorageParams::S3 { bucket, .. }) => {
                if bucket.is_empty() {
                    return Err("S3 bucket name is required".to_string());
                }
                Ok(())
            }
            (StorageType::Gcs, StorageParams::Gcs { bucket, .. }) => {
                if bucket.is_empty() {
                    return Err("GCS bucket name is required".to_string());
                }
                Ok(())
            }
            (StorageType::AzureBlob, StorageParams::AzureBlob { container, .. }) => {
                if container.is_empty() {
                    return Err("Azure container name is required".to_string());
                }
                Ok(())
            }
            (StorageType::LocalFs, StorageParams::LocalFs { root_path }) => {
                if root_path.as_os_str().is_empty() {
                    return Err("Local filesystem root path is required".to_string());
                }
                Ok(())
            }
            _ => Err(format!(
                "Parameter type mismatch: {:?} params for {:?} storage",
                self.params.param_type(),
                self.storage_type
            )),
        }
    }
}

/// Storage-specific connection parameters.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum StorageParams {
    /// S3 and S3-compatible storage parameters.
    S3 {
        /// S3 endpoint URL (leave empty for AWS, set for MinIO/R2/etc.)
        endpoint: Option<String>,
        /// AWS region (e.g., "us-east-1")
        region: String,
        /// Bucket name
        bucket: String,
        /// Access key ID (stored separately in keyring for security)
        access_key_id: Option<String>,
        /// Use path-style addressing (required for MinIO)
        path_style: bool,
        /// Allow unsigned/anonymous requests for public buckets
        allow_anonymous: bool,
        /// Additional options
        #[serde(default)]
        extra_options: HashMap<String, String>,
    },
    /// Google Cloud Storage parameters.
    Gcs {
        /// GCS bucket name
        bucket: String,
        /// Service account credentials JSON path
        credentials_path: Option<PathBuf>,
        /// Project ID
        project_id: Option<String>,
        /// Additional options
        #[serde(default)]
        extra_options: HashMap<String, String>,
    },
    /// Azure Blob Storage parameters.
    AzureBlob {
        /// Storage account name
        account_name: String,
        /// Container name
        container: String,
        /// Account key (stored separately in keyring for security)
        account_key: Option<String>,
        /// Additional options
        #[serde(default)]
        extra_options: HashMap<String, String>,
    },
    /// Local filesystem parameters.
    LocalFs {
        /// Root directory path
        root_path: PathBuf,
    },
}

impl StorageParams {
    /// Create S3 parameters.
    pub fn s3(
        endpoint: Option<String>,
        region: String,
        bucket: String,
        access_key_id: Option<String>,
        path_style: bool,
    ) -> Self {
        StorageParams::S3 {
            endpoint,
            region,
            bucket,
            access_key_id,
            path_style,
            allow_anonymous: false,
            extra_options: HashMap::new(),
        }
    }

    /// Create local filesystem parameters.
    pub fn local_fs(root_path: PathBuf) -> Self {
        StorageParams::LocalFs { root_path }
    }

    /// Get the parameter type name.
    pub fn param_type(&self) -> &'static str {
        match self {
            StorageParams::S3 { .. } => "s3",
            StorageParams::Gcs { .. } => "gcs",
            StorageParams::AzureBlob { .. } => "azure_blob",
            StorageParams::LocalFs { .. } => "local_fs",
        }
    }

    /// Get the bucket/container name if applicable.
    pub fn bucket_name(&self) -> Option<&str> {
        match self {
            StorageParams::S3 { bucket, .. } => Some(bucket),
            StorageParams::Gcs { bucket, .. } => Some(bucket),
            StorageParams::AzureBlob { container, .. } => Some(container),
            StorageParams::LocalFs { .. } => None,
        }
    }
}

/// Information about an object (file or directory) in storage.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObjectInfo {
    /// Full path to the object (relative to bucket/container root).
    pub path: String,
    /// Object name (filename or directory name).
    pub name: String,
    /// Whether this is a directory (prefix in S3 terms).
    pub is_dir: bool,
    /// Size in bytes (None for directories).
    pub size: Option<u64>,
    /// Last modified timestamp.
    pub last_modified: Option<DateTime<Utc>>,
    /// Content type / MIME type.
    pub content_type: Option<String>,
    /// ETag or version identifier.
    pub etag: Option<String>,
}

impl ObjectInfo {
    /// Create a new file object info.
    pub fn file(
        path: String,
        size: u64,
        last_modified: Option<DateTime<Utc>>,
        content_type: Option<String>,
        etag: Option<String>,
    ) -> Self {
        let name = path
            .rsplit('/')
            .next()
            .unwrap_or(&path)
            .to_string();
        Self {
            path,
            name,
            is_dir: false,
            size: Some(size),
            last_modified,
            content_type,
            etag,
        }
    }

    /// Create a new directory object info.
    pub fn directory(path: String) -> Self {
        let name = path
            .trim_end_matches('/')
            .rsplit('/')
            .next()
            .unwrap_or(&path)
            .to_string();
        Self {
            path,
            name,
            is_dir: true,
            size: None,
            last_modified: None,
            content_type: None,
            etag: None,
        }
    }

    /// Get a human-readable size string.
    pub fn size_display(&self) -> String {
        match self.size {
            Some(bytes) if bytes >= 1_073_741_824 => {
                format!("{:.1} GB", bytes as f64 / 1_073_741_824.0)
            }
            Some(bytes) if bytes >= 1_048_576 => {
                format!("{:.1} MB", bytes as f64 / 1_048_576.0)
            }
            Some(bytes) if bytes >= 1024 => {
                format!("{:.1} KB", bytes as f64 / 1024.0)
            }
            Some(bytes) => format!("{} B", bytes),
            None => "-".to_string(),
        }
    }

    /// Get the file extension if any.
    pub fn extension(&self) -> Option<&str> {
        if self.is_dir {
            return None;
        }
        self.name.rsplit('.').next()
    }

    /// Check if this is a previewable file type.
    pub fn is_previewable(&self) -> bool {
        match self.extension() {
            Some(ext) => matches!(
                ext.to_lowercase().as_str(),
                "txt" | "json" | "yaml" | "yml" | "toml" | "md" | "csv" | "xml" | "html" | "css" | "js" | "ts" | "py" | "rs" | "sql" | "sh" | "log"
            ),
            None => false,
        }
    }

    /// Check if this is an image file.
    pub fn is_image(&self) -> bool {
        match self.extension() {
            Some(ext) => matches!(
                ext.to_lowercase().as_str(),
                "png" | "jpg" | "jpeg" | "gif" | "webp" | "svg" | "ico" | "bmp"
            ),
            None => false,
        }
    }

    /// Check if this is a data file (Parquet, CSV, etc.)
    pub fn is_data_file(&self) -> bool {
        match self.extension() {
            Some(ext) => matches!(
                ext.to_lowercase().as_str(),
                "parquet" | "csv" | "tsv" | "json" | "jsonl" | "ndjson" | "avro" | "orc"
            ),
            None => false,
        }
    }
}

/// Result of a storage operation.
#[derive(Debug, Clone)]
pub struct StorageOperationResult {
    /// Whether the operation succeeded.
    pub success: bool,
    /// Number of objects affected.
    pub objects_affected: usize,
    /// Total bytes transferred (for upload/download).
    pub bytes_transferred: Option<u64>,
    /// Error message if failed.
    pub error: Option<String>,
}

impl StorageOperationResult {
    /// Create a success result.
    pub fn success(objects_affected: usize, bytes_transferred: Option<u64>) -> Self {
        Self {
            success: true,
            objects_affected,
            bytes_transferred,
            error: None,
        }
    }

    /// Create a failure result.
    pub fn failure(error: String) -> Self {
        Self {
            success: false,
            objects_affected: 0,
            bytes_transferred: None,
            error: Some(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_storage_type_display() {
        assert_eq!(StorageType::S3.display_name(), "Amazon S3");
        assert_eq!(StorageType::LocalFs.display_name(), "Local Filesystem");
    }

    #[test]
    fn test_storage_config_validation() {
        // Valid S3 config
        let config = StorageConfig::new(
            "test".to_string(),
            StorageType::S3,
            StorageParams::s3(None, "us-east-1".to_string(), "my-bucket".to_string(), None, false),
        );
        assert!(config.validate().is_ok());

        // Invalid S3 config (empty bucket)
        let config = StorageConfig::new(
            "test".to_string(),
            StorageType::S3,
            StorageParams::s3(None, "us-east-1".to_string(), "".to_string(), None, false),
        );
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_object_info_size_display() {
        let small = ObjectInfo::file("test.txt".to_string(), 500, None, None, None);
        assert_eq!(small.size_display(), "500 B");

        let medium = ObjectInfo::file("test.txt".to_string(), 1_500_000, None, None, None);
        assert_eq!(medium.size_display(), "1.4 MB");

        let large = ObjectInfo::file("test.txt".to_string(), 2_500_000_000, None, None, None);
        assert_eq!(large.size_display(), "2.3 GB");
    }

    #[test]
    fn test_object_info_extension() {
        let file = ObjectInfo::file("data/test.parquet".to_string(), 1000, None, None, None);
        assert_eq!(file.extension(), Some("parquet"));
        assert!(file.is_data_file());

        let dir = ObjectInfo::directory("data/".to_string());
        assert_eq!(dir.extension(), None);
    }
}
