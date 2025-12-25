mod manager;
mod query;
mod schema;
mod types;

// New multi-database abstraction modules
pub mod drivers;
pub mod traits;

pub use manager::DatabaseManager;

// Re-export driver factory for convenience
pub use drivers::ConnectionFactory;

// Re-export database drivers
pub use drivers::PostgresConnection;
pub use drivers::SqliteConnection;

// Re-export commonly used trait types
pub use traits::{
    ConnectionConfig, ConnectionParams, DatabaseType, SslMode as TraitSslMode,
    // Connection traits
    DatabaseConnection, Transactional,
    // Schema traits
    SchemaIntrospection,
    // Value types
    Cell, ColumnInfo, Row, Value,
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
