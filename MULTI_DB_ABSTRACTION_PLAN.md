# Multi-Database Abstraction Plan for PGUI

## Executive Summary

This document outlines the architectural plan to extend PGUI from a PostgreSQL-only client to a multi-database client supporting PostgreSQL, MySQL, SQLite, ClickHouse, DuckDB, and S3-compatible blob storage.

---

## 1. Current Architecture Analysis

### Current State (`src/services/database/`)

```
DatabaseManager
├── pool: Arc<RwLock<Option<PgPool>>>  ← PostgreSQL-specific
├── connect_with_options(PgConnectOptions)
├── execute_query_enhanced(sql) → QueryExecutionResult
├── stream_query(sql) → BoxStream<PgRow>
└── Schema introspection (PostgreSQL-specific SQL)
```

**Key Files:**
- `manager.rs` - Connection pooling (PostgreSQL-specific)
- `query.rs` - Query execution (PgRow-specific)
- `schema.rs` - Schema introspection (PostgreSQL catalog queries)
- `types.rs` - Result types (database-agnostic ✓)

### Limitations to Address
1. `DatabaseManager` is concrete, not trait-based
2. Connection types hardcoded to PostgreSQL (`PgPool`, `PgConnectOptions`, `PgRow`)
3. Schema introspection uses PostgreSQL-specific `information_schema` and `pg_catalog` queries
4. No abstraction for different SQL dialects

---

## 2. Proposed Architecture

### 2.1 Core Trait Hierarchy

```rust
// src/services/database/traits/connection.rs

/// Core trait for all database connections
#[async_trait]
pub trait DatabaseConnection: Send + Sync {
    /// Get the database type identifier
    fn database_type(&self) -> DatabaseType;

    /// Connect to the database
    async fn connect(&mut self) -> Result<()>;

    /// Disconnect from the database
    async fn disconnect(&mut self) -> Result<()>;

    /// Check if connected
    async fn is_connected(&self) -> bool;

    /// Execute a query and return results
    async fn execute_query(&self, sql: &str) -> Result<QueryExecutionResult>;

    /// Stream query results for large datasets
    async fn stream_query(&self, sql: &str) -> Result<BoxStream<'_, Result<Row>>>;

    /// Get connection info for display
    fn connection_info(&self) -> &ConnectionConfig;
}

/// Trait for databases that support schema introspection
#[async_trait]
pub trait SchemaIntrospection: DatabaseConnection {
    /// Get list of databases/schemas
    async fn get_databases(&self) -> Result<Vec<DatabaseInfo>>;

    /// Get list of tables
    async fn get_tables(&self) -> Result<Vec<TableInfo>>;

    /// Get full schema for specified tables
    async fn get_schema(&self, tables: Option<Vec<String>>) -> Result<DatabaseSchema>;

    /// Get columns for a specific table
    async fn get_columns(&self, table: &str, schema: &str) -> Result<Vec<ColumnInfo>>;

    /// Get primary keys for a table
    async fn get_primary_keys(&self, table: &str, schema: &str) -> Result<Vec<String>>;

    /// Get foreign keys for a table
    async fn get_foreign_keys(&self, table: &str, schema: &str) -> Result<Vec<ForeignKeyInfo>>;

    /// Get indexes for a table
    async fn get_indexes(&self, table: &str, schema: &str) -> Result<Vec<IndexInfo>>;
}

/// Trait for databases that support transactions
#[async_trait]
pub trait Transactional: DatabaseConnection {
    /// Begin a transaction
    async fn begin_transaction(&self) -> Result<Transaction>;

    /// Commit a transaction
    async fn commit(&self, tx: Transaction) -> Result<()>;

    /// Rollback a transaction
    async fn rollback(&self, tx: Transaction) -> Result<()>;
}
```

### 2.2 Database Type Enumeration

```rust
// src/services/database/traits/types.rs

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DatabaseType {
    PostgreSQL,
    MySQL,
    SQLite,
    ClickHouse,
    DuckDB,
}

impl DatabaseType {
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::PostgreSQL => "PostgreSQL",
            Self::MySQL => "MySQL",
            Self::SQLite => "SQLite",
            Self::ClickHouse => "ClickHouse",
            Self::DuckDB => "DuckDB",
        }
    }

    pub fn default_port(&self) -> Option<u16> {
        match self {
            Self::PostgreSQL => Some(5432),
            Self::MySQL => Some(3306),
            Self::SQLite => None,  // File-based
            Self::ClickHouse => Some(8123), // HTTP port
            Self::DuckDB => None,  // Embedded/file-based
        }
    }

    pub fn supports_schema_introspection(&self) -> bool {
        true // All supported databases have schema introspection
    }
}
```

### 2.3 Unified Connection Configuration

```rust
// src/services/database/config.rs

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionConfig {
    pub id: Uuid,
    pub name: String,
    pub database_type: DatabaseType,
    pub connection_params: ConnectionParams,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ConnectionParams {
    /// Server-based databases (PostgreSQL, MySQL, ClickHouse)
    Server {
        hostname: String,
        port: u16,
        username: String,
        #[serde(skip_serializing)]
        password: String,
        database: String,
        ssl_mode: SslMode,
        #[serde(default)]
        extra_options: HashMap<String, String>,
    },

    /// File-based databases (SQLite, DuckDB)
    File {
        path: PathBuf,
        #[serde(default)]
        read_only: bool,
        #[serde(default)]
        extra_options: HashMap<String, String>,
    },

    /// In-memory databases
    InMemory {
        #[serde(default)]
        extra_options: HashMap<String, String>,
    },
}
```

### 2.4 Unified Row/Result Types

```rust
// src/services/database/traits/row.rs

/// Database-agnostic row representation
#[derive(Debug, Clone)]
pub struct Row {
    pub cells: Vec<Cell>,
}

#[derive(Debug, Clone)]
pub struct Cell {
    pub value: Value,
    pub column_index: usize,
}

/// Unified value type across all databases
#[derive(Debug, Clone)]
pub enum Value {
    Null,
    Bool(bool),
    Int8(i8),
    Int16(i16),
    Int32(i32),
    Int64(i64),
    UInt8(u8),
    UInt16(u16),
    UInt32(u32),
    UInt64(u64),
    Float32(f32),
    Float64(f64),
    Text(String),
    Bytes(Vec<u8>),
    Date(NaiveDate),
    Time(NaiveTime),
    DateTime(NaiveDateTime),
    DateTimeTz(DateTime<Utc>),
    Decimal(Decimal),
    Uuid(Uuid),
    Json(serde_json::Value),
    Array(Vec<Value>),
    // Extensible for database-specific types
    Other { type_name: String, display: String },
}

impl Value {
    pub fn is_null(&self) -> bool {
        matches!(self, Value::Null)
    }

    pub fn to_display_string(&self) -> String {
        match self {
            Value::Null => "NULL".to_string(),
            Value::Bool(b) => b.to_string(),
            Value::Text(s) => s.clone(),
            // ... other conversions
        }
    }
}
```

---

## 3. Driver Implementations

### 3.1 Directory Structure

```
src/services/database/
├── traits/
│   ├── mod.rs
│   ├── connection.rs      # DatabaseConnection trait
│   ├── schema.rs          # SchemaIntrospection trait
│   ├── transaction.rs     # Transactional trait
│   ├── row.rs             # Row, Cell, Value types
│   └── types.rs           # DatabaseType, ConnectionParams
│
├── drivers/
│   ├── mod.rs             # Driver registry and factory
│   ├── postgres/
│   │   ├── mod.rs
│   │   ├── connection.rs  # PostgresConnection impl
│   │   ├── schema.rs      # PostgreSQL schema queries
│   │   ├── types.rs       # PgRow → Row conversion
│   │   └── dialect.rs     # PostgreSQL SQL dialect
│   │
│   ├── mysql/
│   │   ├── mod.rs
│   │   ├── connection.rs  # MySqlConnection impl
│   │   ├── schema.rs      # MySQL schema queries
│   │   └── types.rs       # MySqlRow → Row conversion
│   │
│   ├── sqlite/
│   │   ├── mod.rs
│   │   ├── connection.rs  # SqliteConnection impl
│   │   ├── schema.rs      # SQLite schema queries
│   │   └── types.rs       # SqliteRow → Row conversion
│   │
│   ├── clickhouse/
│   │   ├── mod.rs
│   │   ├── connection.rs  # ClickHouseConnection impl
│   │   ├── schema.rs      # ClickHouse schema queries
│   │   └── types.rs       # ClickHouse → Row conversion
│   │
│   └── duckdb/
│       ├── mod.rs
│       ├── connection.rs  # DuckDbConnection impl
│       ├── schema.rs      # DuckDB schema queries
│       └── types.rs       # DuckDB → Row conversion
│
├── storage/               # S3/Blob storage (see section 5)
│   ├── mod.rs
│   ├── traits.rs
│   └── s3.rs
│
├── manager.rs             # Updated DatabaseManager
├── factory.rs             # Connection factory
└── mod.rs
```

### 3.2 PostgreSQL Driver (SQLx)

```rust
// src/services/database/drivers/postgres/connection.rs

use sqlx::{PgPool, PgPoolOptions, postgres::PgConnectOptions};

pub struct PostgresConnection {
    config: ConnectionConfig,
    pool: Option<PgPool>,
}

#[async_trait]
impl DatabaseConnection for PostgresConnection {
    fn database_type(&self) -> DatabaseType {
        DatabaseType::PostgreSQL
    }

    async fn connect(&mut self) -> Result<()> {
        let ConnectionParams::Server {
            hostname, port, username, password, database, ssl_mode, ..
        } = &self.config.connection_params else {
            return Err(anyhow!("PostgreSQL requires server connection params"));
        };

        let options = PgConnectOptions::new()
            .host(hostname)
            .port(*port)
            .username(username)
            .password(password)
            .database(database)
            .ssl_mode(ssl_mode.to_pg_ssl_mode());

        let pool = PgPoolOptions::new()
            .max_connections(5)
            .acquire_timeout(Duration::from_secs(5))
            .connect_with(options)
            .await?;

        self.pool = Some(pool);
        Ok(())
    }

    async fn execute_query(&self, sql: &str) -> Result<QueryExecutionResult> {
        let pool = self.pool.as_ref().ok_or(anyhow!("Not connected"))?;

        if is_select_query(sql) {
            let rows = sqlx::query(sql).fetch_all(pool).await?;
            Ok(self.convert_pg_rows(rows))
        } else {
            let result = sqlx::query(sql).execute(pool).await?;
            Ok(QueryExecutionResult::Modified(ModifiedResult {
                rows_affected: result.rows_affected(),
                ..
            }))
        }
    }

    // ... other methods
}
```

### 3.3 MySQL Driver (SQLx)

```rust
// src/services/database/drivers/mysql/connection.rs

use sqlx::{MySqlPool, MySqlPoolOptions, mysql::MySqlConnectOptions};

pub struct MySqlConnection {
    config: ConnectionConfig,
    pool: Option<MySqlPool>,
}

#[async_trait]
impl DatabaseConnection for MySqlConnection {
    fn database_type(&self) -> DatabaseType {
        DatabaseType::MySQL
    }

    async fn connect(&mut self) -> Result<()> {
        let ConnectionParams::Server {
            hostname, port, username, password, database, ssl_mode, ..
        } = &self.config.connection_params else {
            return Err(anyhow!("MySQL requires server connection params"));
        };

        let options = MySqlConnectOptions::new()
            .host(hostname)
            .port(*port)
            .username(username)
            .password(password)
            .database(database)
            .ssl_mode(ssl_mode.to_mysql_ssl_mode());

        let pool = MySqlPoolOptions::new()
            .max_connections(5)
            .acquire_timeout(Duration::from_secs(5))
            .connect_with(options)
            .await?;

        self.pool = Some(pool);
        Ok(())
    }

    // ... similar to PostgreSQL
}

#[async_trait]
impl SchemaIntrospection for MySqlConnection {
    async fn get_tables(&self) -> Result<Vec<TableInfo>> {
        let pool = self.pool.as_ref().ok_or(anyhow!("Not connected"))?;

        let rows = sqlx::query(r#"
            SELECT
                TABLE_NAME as table_name,
                TABLE_SCHEMA as table_schema,
                TABLE_TYPE as table_type
            FROM information_schema.TABLES
            WHERE TABLE_SCHEMA = DATABASE()
            ORDER BY TABLE_NAME
        "#)
        .fetch_all(pool)
        .await?;

        // Convert to TableInfo...
    }

    // ... other schema methods
}
```

### 3.4 SQLite Driver (SQLx)

```rust
// src/services/database/drivers/sqlite/connection.rs

use sqlx::{SqlitePool, SqlitePoolOptions, sqlite::SqliteConnectOptions};

pub struct SqliteConnection {
    config: ConnectionConfig,
    pool: Option<SqlitePool>,
}

#[async_trait]
impl DatabaseConnection for SqliteConnection {
    fn database_type(&self) -> DatabaseType {
        DatabaseType::SQLite
    }

    async fn connect(&mut self) -> Result<()> {
        let options = match &self.config.connection_params {
            ConnectionParams::File { path, read_only, .. } => {
                SqliteConnectOptions::new()
                    .filename(path)
                    .read_only(*read_only)
                    .create_if_missing(!*read_only)
            }
            ConnectionParams::InMemory { .. } => {
                SqliteConnectOptions::from_str(":memory:")?
            }
            _ => return Err(anyhow!("SQLite requires file or in-memory params")),
        };

        let pool = SqlitePoolOptions::new()
            .max_connections(1)  // SQLite single-writer
            .connect_with(options)
            .await?;

        self.pool = Some(pool);
        Ok(())
    }

    // ...
}

#[async_trait]
impl SchemaIntrospection for SqliteConnection {
    async fn get_tables(&self) -> Result<Vec<TableInfo>> {
        let pool = self.pool.as_ref().ok_or(anyhow!("Not connected"))?;

        let rows = sqlx::query(r#"
            SELECT
                name as table_name,
                'main' as table_schema,
                type as table_type
            FROM sqlite_master
            WHERE type IN ('table', 'view')
            AND name NOT LIKE 'sqlite_%'
            ORDER BY name
        "#)
        .fetch_all(pool)
        .await?;

        // Convert...
    }

    async fn get_columns(&self, table: &str, _schema: &str) -> Result<Vec<ColumnInfo>> {
        // Use PRAGMA table_info(table_name)
    }
}
```

### 3.5 ClickHouse Driver

```rust
// src/services/database/drivers/clickhouse/connection.rs

use clickhouse::{Client, Row as ChRow};

pub struct ClickHouseConnection {
    config: ConnectionConfig,
    client: Option<Client>,
}

#[async_trait]
impl DatabaseConnection for ClickHouseConnection {
    fn database_type(&self) -> DatabaseType {
        DatabaseType::ClickHouse
    }

    async fn connect(&mut self) -> Result<()> {
        let ConnectionParams::Server {
            hostname, port, username, password, database, ..
        } = &self.config.connection_params else {
            return Err(anyhow!("ClickHouse requires server connection params"));
        };

        let url = format!("http://{}:{}", hostname, port);

        let client = Client::default()
            .with_url(&url)
            .with_user(username)
            .with_password(password)
            .with_database(database);

        // Test connection
        client.query("SELECT 1").execute().await?;

        self.client = Some(client);
        Ok(())
    }

    async fn execute_query(&self, sql: &str) -> Result<QueryExecutionResult> {
        let client = self.client.as_ref().ok_or(anyhow!("Not connected"))?;

        // ClickHouse uses different approach - query with RowBinary format
        // Need to handle dynamic column discovery
        let mut cursor = client.query(sql).fetch::<serde_json::Value>()?;

        let mut rows = Vec::new();
        while let Some(row) = cursor.next().await? {
            rows.push(self.convert_json_row(row));
        }

        Ok(QueryExecutionResult::Select(QueryResult {
            rows,
            // ...
        }))
    }
}

#[async_trait]
impl SchemaIntrospection for ClickHouseConnection {
    async fn get_tables(&self) -> Result<Vec<TableInfo>> {
        let client = self.client.as_ref().ok_or(anyhow!("Not connected"))?;

        // ClickHouse uses system.tables
        let sql = r#"
            SELECT
                name as table_name,
                database as table_schema,
                engine as table_type
            FROM system.tables
            WHERE database = currentDatabase()
            ORDER BY name
        "#;

        // Execute and convert...
    }

    async fn get_columns(&self, table: &str, schema: &str) -> Result<Vec<ColumnInfo>> {
        // Use system.columns
        let sql = format!(r#"
            SELECT
                name,
                type,
                default_kind,
                default_expression,
                comment
            FROM system.columns
            WHERE database = '{}' AND table = '{}'
            ORDER BY position
        "#, schema, table);

        // ...
    }
}
```

### 3.6 DuckDB Driver

```rust
// src/services/database/drivers/duckdb/connection.rs

use async_duckdb::{Client, ClientBuilder};

pub struct DuckDbConnection {
    config: ConnectionConfig,
    client: Option<Client>,
}

#[async_trait]
impl DatabaseConnection for DuckDbConnection {
    fn database_type(&self) -> DatabaseType {
        DatabaseType::DuckDB
    }

    async fn connect(&mut self) -> Result<()> {
        let client = match &self.config.connection_params {
            ConnectionParams::File { path, read_only, .. } => {
                let mut builder = ClientBuilder::new()
                    .path(path);

                if *read_only {
                    builder = builder.read_only();
                }

                builder.build().await?
            }
            ConnectionParams::InMemory { .. } => {
                ClientBuilder::new()
                    .build()
                    .await?
            }
            _ => return Err(anyhow!("DuckDB requires file or in-memory params")),
        };

        self.client = Some(client);
        Ok(())
    }

    async fn execute_query(&self, sql: &str) -> Result<QueryExecutionResult> {
        let client = self.client.as_ref().ok_or(anyhow!("Not connected"))?;

        let result = client.conn(move |conn| {
            let mut stmt = conn.prepare(&sql)?;
            let columns: Vec<_> = stmt.column_names().iter().map(|s| s.to_string()).collect();

            let rows: Vec<Vec<duckdb::types::Value>> = stmt
                .query_map([], |row| {
                    // Extract all columns
                })?
                .collect::<Result<_, _>>()?;

            Ok((columns, rows))
        }).await?;

        // Convert to QueryExecutionResult
    }
}

#[async_trait]
impl SchemaIntrospection for DuckDbConnection {
    async fn get_tables(&self) -> Result<Vec<TableInfo>> {
        // DuckDB supports information_schema
        self.execute_query(r#"
            SELECT
                table_name,
                table_schema,
                table_type
            FROM information_schema.tables
            WHERE table_schema NOT IN ('pg_catalog', 'information_schema')
            ORDER BY table_name
        "#).await?
        // Convert...
    }
}
```

---

## 4. Connection Factory & Manager

### 4.1 Connection Factory

```rust
// src/services/database/factory.rs

pub struct ConnectionFactory;

impl ConnectionFactory {
    /// Create a new database connection based on configuration
    pub fn create(config: ConnectionConfig) -> Result<Box<dyn DatabaseConnection>> {
        match config.database_type {
            DatabaseType::PostgreSQL => {
                Ok(Box::new(PostgresConnection::new(config)))
            }
            DatabaseType::MySQL => {
                Ok(Box::new(MySqlConnection::new(config)))
            }
            DatabaseType::SQLite => {
                Ok(Box::new(SqliteConnection::new(config)))
            }
            DatabaseType::ClickHouse => {
                Ok(Box::new(ClickHouseConnection::new(config)))
            }
            DatabaseType::DuckDB => {
                Ok(Box::new(DuckDbConnection::new(config)))
            }
        }
    }

    /// Create connection with schema introspection support
    pub fn create_with_schema(
        config: ConnectionConfig
    ) -> Result<Box<dyn SchemaIntrospection>> {
        // All supported databases implement SchemaIntrospection
        match config.database_type {
            DatabaseType::PostgreSQL => {
                Ok(Box::new(PostgresConnection::new(config)))
            }
            // ... others
        }
    }
}
```

### 4.2 Updated DatabaseManager

```rust
// src/services/database/manager.rs

pub struct DatabaseManager {
    connection: Arc<RwLock<Option<Box<dyn DatabaseConnection>>>>,
    schema_cache: Arc<RwLock<Option<DatabaseSchema>>>,
}

impl DatabaseManager {
    pub fn new() -> Self {
        Self {
            connection: Arc::new(RwLock::new(None)),
            schema_cache: Arc::new(RwLock::new(None)),
        }
    }

    pub async fn connect(&self, config: ConnectionConfig) -> Result<()> {
        let mut conn = ConnectionFactory::create(config)?;
        conn.connect().await?;

        let mut guard = self.connection.write().await;
        *guard = Some(conn);

        // Clear schema cache on new connection
        let mut cache_guard = self.schema_cache.write().await;
        *cache_guard = None;

        Ok(())
    }

    pub async fn disconnect(&self) -> Result<()> {
        let mut guard = self.connection.write().await;
        if let Some(mut conn) = guard.take() {
            conn.disconnect().await?;
        }
        Ok(())
    }

    pub async fn execute_query(&self, sql: &str) -> Result<QueryExecutionResult> {
        let guard = self.connection.read().await;
        let conn = guard.as_ref().ok_or(anyhow!("Not connected"))?;
        conn.execute_query(sql).await
    }

    pub async fn get_schema(&self) -> Result<DatabaseSchema> {
        // Check cache first
        {
            let cache = self.schema_cache.read().await;
            if let Some(schema) = cache.as_ref() {
                return Ok(schema.clone());
            }
        }

        // Fetch schema
        let guard = self.connection.read().await;
        let conn = guard.as_ref().ok_or(anyhow!("Not connected"))?;

        // Check if connection supports schema introspection
        // This is a simplified check - in practice use trait object downcasting
        let schema = self.fetch_schema_for_connection(conn.as_ref()).await?;

        // Cache it
        let mut cache = self.schema_cache.write().await;
        *cache = Some(schema.clone());

        Ok(schema)
    }

    pub fn database_type(&self) -> Option<DatabaseType> {
        // Would need sync access or cached value
    }
}
```

---

## 5. S3/Blob Storage with OpenDAL

### 5.1 Storage Traits

```rust
// src/services/database/storage/traits.rs

use opendal::Operator;

/// Trait for blob/object storage connections
#[async_trait]
pub trait StorageConnection: Send + Sync {
    /// Get the storage type
    fn storage_type(&self) -> StorageType;

    /// Connect/initialize the storage
    async fn connect(&mut self) -> Result<()>;

    /// List objects in a path
    async fn list(&self, path: &str) -> Result<Vec<ObjectInfo>>;

    /// Read an object
    async fn read(&self, path: &str) -> Result<Vec<u8>>;

    /// Read object as stream
    async fn read_stream(&self, path: &str) -> Result<BoxStream<'_, Result<Bytes>>>;

    /// Write an object
    async fn write(&self, path: &str, data: Vec<u8>) -> Result<()>;

    /// Delete an object
    async fn delete(&self, path: &str) -> Result<()>;

    /// Check if object exists
    async fn exists(&self, path: &str) -> Result<bool>;

    /// Get object metadata
    async fn stat(&self, path: &str) -> Result<ObjectInfo>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageType {
    S3,
    Gcs,
    AzureBlob,
    LocalFs,
}

#[derive(Debug, Clone)]
pub struct ObjectInfo {
    pub path: String,
    pub size: u64,
    pub last_modified: Option<DateTime<Utc>>,
    pub content_type: Option<String>,
    pub is_dir: bool,
}
```

### 5.2 S3 Storage Configuration

```rust
// src/services/database/storage/config.rs

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageConfig {
    pub id: Uuid,
    pub name: String,
    pub storage_type: StorageType,
    pub params: StorageParams,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum StorageParams {
    S3 {
        endpoint: Option<String>,  // For S3-compatible (MinIO, etc.)
        bucket: String,
        region: String,
        access_key_id: String,
        #[serde(skip_serializing)]
        secret_access_key: String,
        #[serde(default)]
        path_style: bool,  // For MinIO compatibility
    },
    Gcs {
        bucket: String,
        credential_path: Option<PathBuf>,
    },
    AzureBlob {
        container: String,
        account_name: String,
        #[serde(skip_serializing)]
        account_key: String,
    },
    LocalFs {
        root: PathBuf,
    },
}
```

### 5.3 S3 Storage Implementation

```rust
// src/services/database/storage/s3.rs

use opendal::{Operator, services::S3};

pub struct S3Storage {
    config: StorageConfig,
    operator: Option<Operator>,
}

impl S3Storage {
    pub fn new(config: StorageConfig) -> Self {
        Self {
            config,
            operator: None,
        }
    }
}

#[async_trait]
impl StorageConnection for S3Storage {
    fn storage_type(&self) -> StorageType {
        StorageType::S3
    }

    async fn connect(&mut self) -> Result<()> {
        let StorageParams::S3 {
            endpoint,
            bucket,
            region,
            access_key_id,
            secret_access_key,
            path_style,
        } = &self.config.params else {
            return Err(anyhow!("Invalid params for S3"));
        };

        let mut builder = S3::default();
        builder
            .bucket(bucket)
            .region(region)
            .access_key_id(access_key_id)
            .secret_access_key(secret_access_key);

        if let Some(ep) = endpoint {
            builder.endpoint(ep);
        }

        if *path_style {
            builder.enable_virtual_host_style();
        }

        let op = Operator::new(builder)?.finish();

        // Test connection by listing root
        op.list("/").await?;

        self.operator = Some(op);
        Ok(())
    }

    async fn list(&self, path: &str) -> Result<Vec<ObjectInfo>> {
        let op = self.operator.as_ref().ok_or(anyhow!("Not connected"))?;

        let entries = op.list(path).await?;

        let mut objects = Vec::new();
        for entry in entries {
            let meta = op.stat(entry.path()).await?;
            objects.push(ObjectInfo {
                path: entry.path().to_string(),
                size: meta.content_length(),
                last_modified: meta.last_modified(),
                content_type: meta.content_type().map(|s| s.to_string()),
                is_dir: meta.is_dir(),
            });
        }

        Ok(objects)
    }

    async fn read(&self, path: &str) -> Result<Vec<u8>> {
        let op = self.operator.as_ref().ok_or(anyhow!("Not connected"))?;
        let data = op.read(path).await?;
        Ok(data.to_vec())
    }

    async fn read_stream(&self, path: &str) -> Result<BoxStream<'_, Result<Bytes>>> {
        let op = self.operator.as_ref().ok_or(anyhow!("Not connected"))?;
        let reader = op.reader(path).await?;
        // Convert to stream
        Ok(reader.into_bytes_stream(..).await?.boxed())
    }

    async fn write(&self, path: &str, data: Vec<u8>) -> Result<()> {
        let op = self.operator.as_ref().ok_or(anyhow!("Not connected"))?;
        op.write(path, data).await?;
        Ok(())
    }

    async fn delete(&self, path: &str) -> Result<()> {
        let op = self.operator.as_ref().ok_or(anyhow!("Not connected"))?;
        op.delete(path).await?;
        Ok(())
    }

    async fn exists(&self, path: &str) -> Result<bool> {
        let op = self.operator.as_ref().ok_or(anyhow!("Not connected"))?;
        Ok(op.is_exist(path).await?)
    }

    async fn stat(&self, path: &str) -> Result<ObjectInfo> {
        let op = self.operator.as_ref().ok_or(anyhow!("Not connected"))?;
        let meta = op.stat(path).await?;
        Ok(ObjectInfo {
            path: path.to_string(),
            size: meta.content_length(),
            last_modified: meta.last_modified(),
            content_type: meta.content_type().map(|s| s.to_string()),
            is_dir: meta.is_dir(),
        })
    }
}
```

### 5.4 Storage Factory

```rust
// src/services/database/storage/factory.rs

pub struct StorageFactory;

impl StorageFactory {
    pub fn create(config: StorageConfig) -> Result<Box<dyn StorageConnection>> {
        match config.storage_type {
            StorageType::S3 => Ok(Box::new(S3Storage::new(config))),
            StorageType::Gcs => Ok(Box::new(GcsStorage::new(config))),
            StorageType::AzureBlob => Ok(Box::new(AzureBlobStorage::new(config))),
            StorageType::LocalFs => Ok(Box::new(LocalFsStorage::new(config))),
        }
    }
}
```

---

## 6. Dependencies to Add

```toml
# Cargo.toml additions

[dependencies]
# Async trait support
async-trait = "0.1"

# SQLx with multiple database support
sqlx = { version = "0.8", features = [
    "runtime-async-std",
    "tls-rustls",
    "postgres",
    "mysql",
    "sqlite",
]}

# ClickHouse
clickhouse = { version = "0.12", features = ["tls"] }

# DuckDB (async wrapper)
async-duckdb = "0.1"
duckdb = { version = "1.0", features = ["bundled"] }

# OpenDAL for S3/blob storage
opendal = { version = "0.50", features = [
    "services-s3",
    "services-gcs",
    "services-azblob",
    "services-fs",
]}

# Common utilities
chrono = { version = "0.4", features = ["serde"] }
rust_decimal = { version = "1.33", features = ["serde"] }
bytes = "1.5"
futures = "0.3"
```

---

## 7. Migration Path

### Phase 1: Core Abstractions (Week 1)
1. Create `traits/` module with `DatabaseConnection`, `SchemaIntrospection`, `Row`, `Value`
2. Create `ConnectionConfig` and `ConnectionParams`
3. Implement `ConnectionFactory`

### Phase 2: PostgreSQL Migration (Week 2)
1. Move existing PostgreSQL code to `drivers/postgres/`
2. Implement traits for PostgreSQL
3. Update `DatabaseManager` to use traits
4. Ensure all existing functionality works

### Phase 3: SQLx Databases (Week 3)
1. Implement MySQL driver
2. Implement SQLite driver
3. Add UI for selecting database type

### Phase 4: Non-SQLx Databases (Week 4)
1. Implement ClickHouse driver
2. Implement DuckDB driver
3. Handle driver-specific query syntax differences

### Phase 5: S3/Blob Storage (Week 5)
1. Add OpenDAL dependency
2. Implement `StorageConnection` trait
3. Implement S3, GCS, Azure, LocalFS backends
4. Add storage browser UI

### Phase 6: Polish & Testing (Week 6)
1. Comprehensive testing for all drivers
2. Connection testing UI
3. Error handling improvements
4. Documentation

---

## 8. UI Considerations

### 8.1 Connection Dialog Changes

The connection dialog needs to be updated to support:

1. **Database Type Selector** - Dropdown for PostgreSQL, MySQL, SQLite, ClickHouse, DuckDB
2. **Dynamic Form Fields** - Show/hide fields based on database type:
   - Server-based: hostname, port, username, password, database, SSL
   - File-based: file path, read-only toggle
3. **Storage Connections** - Separate tab/section for S3/blob storage
4. **Connection Testing** - Test button that validates connection before saving

### 8.2 Query Editor Adaptations

Consider database-specific:
- Syntax highlighting (different SQL dialects)
- Auto-completion (based on connected database type)
- Query execution limits (already implemented, may vary by database)

### 8.3 Schema Browser

Schema introspection display should handle:
- Database-specific object types (e.g., ClickHouse engines, DuckDB extensions)
- Different constraint/index representations

---

## 9. Testing Strategy

```rust
// tests/integration/drivers/mod.rs

#[cfg(test)]
mod tests {
    use super::*;

    // Test trait compliance for each driver
    async fn test_connection_lifecycle<T: DatabaseConnection>(mut conn: T) {
        assert!(!conn.is_connected().await);
        conn.connect().await.unwrap();
        assert!(conn.is_connected().await);
        conn.disconnect().await.unwrap();
        assert!(!conn.is_connected().await);
    }

    async fn test_basic_query<T: DatabaseConnection>(conn: &T) {
        let result = conn.execute_query("SELECT 1 as num").await.unwrap();
        // Assert result structure...
    }

    async fn test_schema_introspection<T: SchemaIntrospection>(conn: &T) {
        let tables = conn.get_tables().await.unwrap();
        // At least system tables should exist
        assert!(!tables.is_empty());
    }
}
```

---

## 10. Summary

This plan provides a clean, extensible architecture for supporting multiple databases:

| Component | Purpose |
|-----------|---------|
| `DatabaseConnection` trait | Core abstraction for all database operations |
| `SchemaIntrospection` trait | Optional schema discovery capability |
| `ConnectionConfig` | Unified configuration for all database types |
| `Value` enum | Type-safe, database-agnostic value representation |
| `ConnectionFactory` | Creates appropriate driver based on config |
| `StorageConnection` trait | Abstraction for blob/object storage |
| OpenDAL integration | S3-compatible and other cloud storage support |

**Key Benefits:**
- Clean separation between database-specific and generic code
- Easy to add new database drivers
- Consistent API across all databases
- Type-safe value handling
- Extensible storage layer for S3/cloud storage

**Sources:**
- [Apache OpenDAL Documentation](https://opendal.apache.org/docs/rust/opendal/)
- [OpenDAL S3 Service](https://opendal.apache.org/docs/rust/opendal/services/struct.S3.html)
- [SQLx GitHub](https://github.com/launchbadge/sqlx)
- [ClickHouse Rust Client](https://clickhouse.com/docs/integrations/rust)
- [Official ClickHouse-rs](https://github.com/ClickHouse/clickhouse-rs)
- [DuckDB Rust Client](https://duckdb.org/docs/stable/clients/rust)
- [async-duckdb](https://lib.rs/crates/async-duckdb)
- [Iceberg Rust OpenDAL Integration](https://www.hackintoshrao.com/one-interface-many-backends-the-design-of-iceberg-rusts-universal-storage-layer-with-opendal/)
