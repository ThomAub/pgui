//! Storage connection traits.
//!
//! This module defines the core trait for storage backends, providing
//! a unified interface for blob storage operations across different providers.

use anyhow::Result;
use async_trait::async_trait;
use bytes::Bytes;
use futures::stream::BoxStream;

use super::types::{ObjectInfo, StorageConfig, StorageType};

/// Core trait for storage connections.
///
/// This trait provides a unified interface for interacting with blob storage
/// backends like S3, GCS, Azure Blob, and local filesystem.
///
/// # Example
///
/// ```ignore
/// use pgui::services::database::storage::{StorageConnection, StorageConfig, StorageType, StorageParams};
///
/// let config = StorageConfig::new(
///     "my-s3".to_string(),
///     StorageType::S3,
///     StorageParams::s3(None, "us-east-1".to_string(), "my-bucket".to_string(), None, false),
/// );
///
/// let mut storage = S3Storage::new(config);
/// storage.connect().await?;
///
/// let objects = storage.list("/data/").await?;
/// for obj in objects {
///     println!("{}: {}", obj.name, obj.size_display());
/// }
/// ```
#[async_trait]
pub trait StorageConnection: Send + Sync {
    /// Get the storage type for this connection.
    fn storage_type(&self) -> StorageType;

    /// Get the connection configuration.
    fn storage_config(&self) -> &StorageConfig;

    /// Connect to the storage backend.
    ///
    /// This initializes the connection and validates credentials.
    async fn connect(&mut self) -> Result<()>;

    /// Disconnect from the storage backend.
    async fn disconnect(&mut self) -> Result<()>;

    /// Check if currently connected.
    async fn is_connected(&self) -> bool;

    /// Test the connection without fully connecting.
    ///
    /// Useful for validating credentials before saving a connection.
    async fn test_connection(&self) -> Result<()>;

    /// List objects at the given path.
    ///
    /// # Arguments
    ///
    /// * `path` - The path to list (empty string or "/" for root)
    ///
    /// # Returns
    ///
    /// A list of objects (files and directories) at the given path.
    async fn list(&self, path: &str) -> Result<Vec<ObjectInfo>>;

    /// List objects recursively at the given path.
    ///
    /// # Arguments
    ///
    /// * `path` - The path to list recursively
    ///
    /// # Returns
    ///
    /// A list of all objects under the given path.
    async fn list_recursive(&self, path: &str) -> Result<Vec<ObjectInfo>>;

    /// Read the contents of an object.
    ///
    /// # Arguments
    ///
    /// * `path` - The path to the object
    ///
    /// # Returns
    ///
    /// The object contents as bytes.
    async fn read(&self, path: &str) -> Result<Vec<u8>>;

    /// Read an object as a stream.
    ///
    /// Useful for large files to avoid loading everything into memory.
    ///
    /// # Arguments
    ///
    /// * `path` - The path to the object
    ///
    /// # Returns
    ///
    /// A stream of bytes.
    async fn read_stream(&self, path: &str) -> Result<BoxStream<'static, Result<Bytes>>>;

    /// Read a range of bytes from an object.
    ///
    /// # Arguments
    ///
    /// * `path` - The path to the object
    /// * `offset` - Starting byte offset
    /// * `length` - Number of bytes to read
    ///
    /// # Returns
    ///
    /// The requested byte range.
    async fn read_range(&self, path: &str, offset: u64, length: u64) -> Result<Vec<u8>>;

    /// Write data to an object.
    ///
    /// # Arguments
    ///
    /// * `path` - The path where to write
    /// * `data` - The data to write
    ///
    /// # Returns
    ///
    /// Ok(()) on success.
    async fn write(&self, path: &str, data: Vec<u8>) -> Result<()>;

    /// Write data to an object with content type.
    ///
    /// # Arguments
    ///
    /// * `path` - The path where to write
    /// * `data` - The data to write
    /// * `content_type` - The MIME type of the content
    ///
    /// # Returns
    ///
    /// Ok(()) on success.
    async fn write_with_content_type(
        &self,
        path: &str,
        data: Vec<u8>,
        content_type: &str,
    ) -> Result<()>;

    /// Delete an object.
    ///
    /// # Arguments
    ///
    /// * `path` - The path to delete
    ///
    /// # Returns
    ///
    /// Ok(()) on success.
    async fn delete(&self, path: &str) -> Result<()>;

    /// Delete multiple objects.
    ///
    /// # Arguments
    ///
    /// * `paths` - The paths to delete
    ///
    /// # Returns
    ///
    /// Ok(()) on success.
    async fn delete_many(&self, paths: &[String]) -> Result<()>;

    /// Check if an object exists.
    ///
    /// # Arguments
    ///
    /// * `path` - The path to check
    ///
    /// # Returns
    ///
    /// true if the object exists.
    async fn exists(&self, path: &str) -> Result<bool>;

    /// Get metadata for an object.
    ///
    /// # Arguments
    ///
    /// * `path` - The path to the object
    ///
    /// # Returns
    ///
    /// Object information including size, last modified, etc.
    async fn stat(&self, path: &str) -> Result<ObjectInfo>;

    /// Create a directory (prefix in S3 terms).
    ///
    /// Note: In object storage, directories are virtual and are created
    /// implicitly when objects are written. This method creates an empty
    /// marker object to make the directory visible in listings.
    ///
    /// # Arguments
    ///
    /// * `path` - The directory path (should end with /)
    ///
    /// # Returns
    ///
    /// Ok(()) on success.
    async fn create_dir(&self, path: &str) -> Result<()>;

    /// Copy an object to a new location.
    ///
    /// # Arguments
    ///
    /// * `src` - Source path
    /// * `dst` - Destination path
    ///
    /// # Returns
    ///
    /// Ok(()) on success.
    async fn copy(&self, src: &str, dst: &str) -> Result<()>;

    /// Rename/move an object.
    ///
    /// Note: This is typically implemented as copy + delete in object storage.
    ///
    /// # Arguments
    ///
    /// * `src` - Source path
    /// * `dst` - Destination path
    ///
    /// # Returns
    ///
    /// Ok(()) on success.
    async fn rename(&self, src: &str, dst: &str) -> Result<()>;

    /// Get a presigned URL for direct access (if supported).
    ///
    /// # Arguments
    ///
    /// * `path` - The path to the object
    /// * `expires_in_secs` - How long the URL should be valid
    ///
    /// # Returns
    ///
    /// A presigned URL string, or an error if not supported.
    async fn presigned_url(&self, path: &str, expires_in_secs: u64) -> Result<String>;

    /// Get the full URI for an object.
    ///
    /// # Arguments
    ///
    /// * `path` - The path to the object
    ///
    /// # Returns
    ///
    /// The full URI (e.g., "s3://bucket/path/to/object")
    fn object_uri(&self, path: &str) -> String;
}

/// A boxed storage connection for dynamic dispatch.
pub type BoxedStorageConnection = Box<dyn StorageConnection>;

/// Trait for storage connections that support bucket operations.
///
/// Not all storage backends support listing buckets (e.g., local filesystem).
#[async_trait]
pub trait BucketOperations: StorageConnection {
    /// List all buckets accessible with current credentials.
    async fn list_buckets(&self) -> Result<Vec<String>>;

    /// Check if a bucket exists.
    async fn bucket_exists(&self, bucket: &str) -> Result<bool>;
}
