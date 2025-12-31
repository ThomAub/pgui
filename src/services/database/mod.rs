mod manager;
mod query;
mod schema;
mod types;

// New multi-database abstraction modules
pub mod drivers;
pub mod storage;
pub mod traits;

pub use manager::DatabaseManager;

// Re-export storage types (public API for multi-database abstraction)
#[allow(unused_imports)]
pub use storage::{
    ObjectInfo, StorageConfig, StorageFactory, StorageManager, StorageParams, StorageType,
};

// Legacy types (will be migrated in Epic 2)
#[allow(unused_imports)]
pub use types::{
    ColumnDetail, ConstraintInfo, DatabaseInfo, DatabaseSchema, ErrorResult, ForeignKeyInfo,
    IndexInfo, QueryExecutionResult, QueryResult, ResultCell, ResultColumnMetadata, ResultRow,
    TableInfo, TableSchema,
};

// TableMetadata is internal only
#[allow(unused_imports)]
pub(crate) use types::TableMetadata;
