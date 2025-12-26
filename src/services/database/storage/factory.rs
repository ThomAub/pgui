//! Storage connection factory.
//!
//! The factory pattern allows creating the appropriate storage connection
//! based on the storage configuration's type.

use anyhow::{anyhow, Result};

use super::local_fs::LocalFsStorage;
use super::s3::S3Storage;
use super::traits::BoxedStorageConnection;
use super::types::{StorageConfig, StorageType};

/// Factory for creating storage connections based on configuration.
///
/// # Example
///
/// ```ignore
/// use pgui::services::database::storage::{StorageFactory, StorageConfig, StorageType, StorageParams};
///
/// let config = StorageConfig::new(
///     "My S3".to_string(),
///     StorageType::S3,
///     StorageParams::s3(None, "us-east-1".to_string(), "my-bucket".to_string(), None, false),
/// );
///
/// let connection = StorageFactory::create(config)?;
/// ```
pub struct StorageFactory;

impl StorageFactory {
    /// Create a new storage connection based on the configuration.
    ///
    /// # Arguments
    ///
    /// * `config` - The storage configuration specifying type and parameters
    ///
    /// # Returns
    ///
    /// Returns a boxed storage connection trait object.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The storage type is not yet supported
    /// - The configuration is invalid for the storage type
    pub fn create(config: StorageConfig) -> Result<BoxedStorageConnection> {
        // Validate configuration
        config.validate().map_err(|e| anyhow!(e))?;

        match config.storage_type {
            StorageType::S3 => Ok(S3Storage::boxed(config)),
            StorageType::LocalFs => Ok(LocalFsStorage::boxed(config)),
            StorageType::Gcs => {
                Err(anyhow!(
                    "Google Cloud Storage support coming soon."
                ))
            }
            StorageType::AzureBlob => {
                Err(anyhow!(
                    "Azure Blob Storage support coming soon."
                ))
            }
        }
    }

    /// Check if a storage type is currently supported.
    ///
    /// # Arguments
    ///
    /// * `storage_type` - The storage type to check
    ///
    /// # Returns
    ///
    /// Returns true if the storage type has a driver implementation.
    pub fn is_supported(storage_type: StorageType) -> bool {
        match storage_type {
            StorageType::S3 => true,
            StorageType::LocalFs => true,
            StorageType::Gcs => false,
            StorageType::AzureBlob => false,
        }
    }

    /// Get a list of all supported storage types.
    ///
    /// # Returns
    ///
    /// Returns a list of storage types that have driver implementations.
    pub fn supported_types() -> Vec<StorageType> {
        StorageType::all()
            .into_iter()
            .filter(|t| Self::is_supported(*t))
            .collect()
    }

    /// Get a list of all storage types (supported and unsupported).
    ///
    /// Useful for showing all options in the UI with some marked as "coming soon".
    pub fn all_types() -> Vec<(StorageType, bool)> {
        StorageType::all()
            .into_iter()
            .map(|t| (t, Self::is_supported(t)))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::database::storage::types::StorageParams;
    use std::path::PathBuf;

    #[test]
    fn test_factory_validates_config() {
        // Invalid: S3 with empty bucket
        let config = StorageConfig::new(
            "test".to_string(),
            StorageType::S3,
            StorageParams::s3(None, "us-east-1".to_string(), "".to_string(), None, false),
        );

        let result = StorageFactory::create(config);
        assert!(result.is_err());
    }

    #[test]
    fn test_factory_creates_s3() {
        // Valid: S3 with bucket
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

        let result = StorageFactory::create(config);
        assert!(result.is_ok());
    }

    #[test]
    fn test_factory_creates_local_fs() {
        // Valid: LocalFs with path
        let config = StorageConfig::new(
            "test".to_string(),
            StorageType::LocalFs,
            StorageParams::local_fs(PathBuf::from("/tmp/test")),
        );

        let result = StorageFactory::create(config);
        assert!(result.is_ok());
    }

    #[test]
    fn test_is_supported() {
        assert!(StorageFactory::is_supported(StorageType::S3));
        assert!(StorageFactory::is_supported(StorageType::LocalFs));
        assert!(!StorageFactory::is_supported(StorageType::Gcs));
        assert!(!StorageFactory::is_supported(StorageType::AzureBlob));
    }

    #[test]
    fn test_supported_types() {
        let supported = StorageFactory::supported_types();
        assert_eq!(supported.len(), 2);
        assert!(supported.contains(&StorageType::S3));
        assert!(supported.contains(&StorageType::LocalFs));
    }

    #[test]
    fn test_all_types() {
        let all = StorageFactory::all_types();
        assert_eq!(all.len(), 4);

        // Check S3 is supported
        let s3 = all.iter().find(|(t, _)| *t == StorageType::S3);
        assert!(s3.is_some());
        assert!(s3.unwrap().1);

        // Check GCS is not supported
        let gcs = all.iter().find(|(t, _)| *t == StorageType::Gcs);
        assert!(gcs.is_some());
        assert!(!gcs.unwrap().1);
    }
}
