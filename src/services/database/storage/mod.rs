//! Blob storage backend implementations.
//!
//! This module provides a unified interface for interacting with various
//! blob storage backends using Apache OpenDAL.
//!
//! Supported storage backends:
//!
//! - **Amazon S3** and S3-compatible services (MinIO, Cloudflare R2, DigitalOcean Spaces)
//! - **Google Cloud Storage (GCS)** for Google Cloud Platform
//! - **Local Filesystem** for development and testing
//!
//! Future support planned for:
//! - Azure Blob Storage
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │                    StorageManager                           │
//! │  - Manages active storage connection                        │
//! │  - Handles connect/disconnect lifecycle                     │
//! └─────────────────────────────────────────────────────────────┘
//!                              │
//!                              ▼
//! ┌─────────────────────────────────────────────────────────────┐
//! │                    StorageFactory                           │
//! │  - Creates appropriate storage connection from config       │
//! └─────────────────────────────────────────────────────────────┘
//!                              │
//!              ┌───────────────┼───────────────┐
//!              ▼               ▼               ▼
//! ┌──────────────────┐ ┌──────────────┐ ┌──────────────────┐
//! │   S3Storage      │ │  LocalFs     │ │  (Future: GCS,   │
//! │   (OpenDAL)      │ │  (OpenDAL)   │ │   Azure, etc.)   │
//! └──────────────────┘ └──────────────┘ └──────────────────┘
//! ```
//!
//! # Example
//!
//! ```ignore
//! use pgui::services::database::storage::{
//!     StorageConfig, StorageType, StorageParams, StorageFactory,
//! };
//!
//! // Create S3 storage connection
//! let config = StorageConfig::new(
//!     "my-s3".to_string(),
//!     StorageType::S3,
//!     StorageParams::s3(
//!         None,  // Use default AWS endpoint
//!         "us-east-1".to_string(),
//!         "my-bucket".to_string(),
//!         Some("AKIAIOSFODNN7EXAMPLE".to_string()),
//!         false,  // Virtual-hosted style
//!     ),
//! );
//!
//! let mut storage = StorageFactory::create(config)?;
//! storage.connect().await?;
//!
//! // List objects
//! let objects = storage.list("/data/").await?;
//! for obj in objects {
//!     println!("{}: {}", obj.name, obj.size_display());
//! }
//!
//! // Download a file
//! let data = storage.read("/data/config.json").await?;
//! ```

mod factory;
mod gcs;
mod local_fs;
mod manager;
mod s3;
mod traits;
mod types;

// Re-export main types
pub use factory::StorageFactory;
pub use manager::StorageManager;
pub use traits::{BoxedStorageConnection, BucketOperations, StorageConnection};
pub use types::{ObjectInfo, StorageConfig, StorageOperationResult, StorageParams, StorageType};

// Re-export storage implementations
pub use gcs::GcsStorage;
pub use local_fs::LocalFsStorage;
pub use s3::S3Storage;
