# Multi-Database Implementation Epics

This document breaks down the multi-database abstraction into 7 epics with detailed backend implementation, frontend implementation, and test plans for each.

---

## Epic Overview

| Epic | Name | Dependencies | Estimated Complexity |
|------|------|--------------|---------------------|
| 1 | Core Abstractions & Infrastructure | None | High |
| 2 | PostgreSQL Migration | Epic 1 | Medium |
| 3 | SQLite Support | Epic 2 | Low |
| 4 | MySQL Support | Epic 2 | Low |
| 5 | DuckDB Support | Epic 3 | Medium |
| 6 | ClickHouse Support | Epic 2 | Medium |
| 7 | S3/Blob Storage (OpenDAL) | Epic 1 | High |

---

# Epic 1: Core Abstractions & Infrastructure

## Goal
Create the foundational trait hierarchy, unified types, and factory pattern that all database drivers will implement.

## Backend Implementation

### Step 1.1: Create Traits Module Structure
**Files to create:**
- `src/services/database/traits/mod.rs`
- `src/services/database/traits/connection.rs`
- `src/services/database/traits/schema.rs`
- `src/services/database/traits/types.rs`
- `src/services/database/traits/row.rs`

**Tasks:**
1. Create `src/services/database/traits/` directory
2. Define `DatabaseType` enum in `types.rs`:
   ```rust
   #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
   pub enum DatabaseType {
       PostgreSQL,
       MySQL,
       SQLite,
       ClickHouse,
       DuckDB,
   }
   ```
3. Add helper methods: `display_name()`, `default_port()`, `is_file_based()`, `icon_name()`

### Step 1.2: Define Core Value Types
**File:** `src/services/database/traits/row.rs`

**Tasks:**
1. Create `Value` enum with all supported types:
   - Primitives: `Null`, `Bool`, `Int8-64`, `UInt8-64`, `Float32/64`
   - Text: `Text`, `Bytes`
   - Temporal: `Date`, `Time`, `DateTime`, `DateTimeTz`
   - Complex: `Decimal`, `Uuid`, `Json`, `Array`
   - Fallback: `Other { type_name, display }`
2. Implement `Display` trait for `Value`
3. Implement `Value::is_null()`, `Value::type_name()`, `Value::to_display_string()`
4. Create `Cell` struct with `value: Value` and `column_index: usize`
5. Create `Row` struct with `cells: Vec<Cell>`

### Step 1.3: Define Connection Configuration
**File:** `src/services/database/traits/types.rs`

**Tasks:**
1. Create `ConnectionConfig` struct:
   ```rust
   pub struct ConnectionConfig {
       pub id: Uuid,
       pub name: String,
       pub database_type: DatabaseType,
       pub params: ConnectionParams,
   }
   ```
2. Create `ConnectionParams` enum:
   ```rust
   pub enum ConnectionParams {
       Server { hostname, port, username, password, database, ssl_mode, extra_options },
       File { path, read_only, extra_options },
       InMemory { extra_options },
   }
   ```
3. Create `SslMode` enum (reuse existing, make generic)
4. Add validation methods for each variant

### Step 1.4: Define DatabaseConnection Trait
**File:** `src/services/database/traits/connection.rs`

**Tasks:**
1. Add `async-trait` dependency to `Cargo.toml`
2. Define the core trait:
   ```rust
   #[async_trait]
   pub trait DatabaseConnection: Send + Sync {
       fn database_type(&self) -> DatabaseType;
       fn connection_config(&self) -> &ConnectionConfig;
       async fn connect(&mut self) -> Result<()>;
       async fn disconnect(&mut self) -> Result<()>;
       async fn is_connected(&self) -> bool;
       async fn execute_query(&self, sql: &str) -> Result<QueryExecutionResult>;
       async fn stream_query(&self, sql: &str) -> Result<BoxStream<'_, Result<Row>>>;
   }
   ```
3. Define `Transactional` trait (optional for databases that support it)

### Step 1.5: Define SchemaIntrospection Trait
**File:** `src/services/database/traits/schema.rs`

**Tasks:**
1. Define the trait:
   ```rust
   #[async_trait]
   pub trait SchemaIntrospection: DatabaseConnection {
       async fn get_databases(&self) -> Result<Vec<DatabaseInfo>>;
       async fn get_tables(&self) -> Result<Vec<TableInfo>>;
       async fn get_schema(&self, tables: Option<Vec<String>>) -> Result<DatabaseSchema>;
       async fn get_columns(&self, table: &str, schema: &str) -> Result<Vec<ColumnInfo>>;
       async fn get_primary_keys(&self, table: &str, schema: &str) -> Result<Vec<String>>;
       async fn get_foreign_keys(&self, table: &str, schema: &str) -> Result<Vec<ForeignKeyInfo>>;
       async fn get_indexes(&self, table: &str, schema: &str) -> Result<Vec<IndexInfo>>;
   }
   ```
2. Ensure existing schema types (`TableInfo`, `ColumnInfo`, etc.) are compatible or create new ones

### Step 1.6: Create Drivers Module Structure
**Files to create:**
- `src/services/database/drivers/mod.rs`
- `src/services/database/drivers/factory.rs`

**Tasks:**
1. Create `drivers/` directory structure
2. Create `ConnectionFactory` struct with `create()` method:
   ```rust
   impl ConnectionFactory {
       pub fn create(config: ConnectionConfig) -> Result<Box<dyn DatabaseConnection>> {
           match config.database_type {
               DatabaseType::PostgreSQL => { /* Epic 2 */ }
               _ => Err(anyhow!("Database type not yet supported"))
           }
       }
   }
   ```
3. Export all public types from `drivers/mod.rs`

### Step 1.7: Update Cargo.toml Dependencies
**File:** `Cargo.toml`

**Tasks:**
1. Add new dependencies:
   ```toml
   async-trait = "0.1"
   rust_decimal = { version = "1.33", features = ["serde"] }
   bytes = "1.5"
   ```
2. Update sqlx features for future MySQL/SQLite support:
   ```toml
   sqlx = { version = "0.8", features = [
       "runtime-async-std",
       "tls-rustls",
       "postgres",
       "mysql",    # Add
       "sqlite",   # Add
   ]}
   ```

### Step 1.8: Create Mod Exports
**File:** `src/services/database/mod.rs`

**Tasks:**
1. Add `pub mod traits;`
2. Add `pub mod drivers;`
3. Re-export key types for convenience

## Frontend Implementation

### Step 1.9: Update ConnectionInfo to Support Multiple Types
**File:** `src/services/storage/types.rs`

**Tasks:**
1. Add `database_type: DatabaseType` field to `ConnectionInfo`
2. Update `to_pg_connect_options()` to return `Result<ConnectionConfig>` instead
3. Create `ConnectionInfo::to_connection_config()` method
4. Maintain backward compatibility with existing saved connections (default to PostgreSQL)

### Step 1.10: Update Storage Layer for New Fields
**File:** `src/services/storage/connections.rs`

**Tasks:**
1. Update SQLite schema to add `database_type` column:
   ```sql
   ALTER TABLE connections ADD COLUMN database_type TEXT NOT NULL DEFAULT 'postgresql';
   ```
2. Handle migration for existing connections
3. Update `create()`, `update()`, `get_all()` methods

## Test Plan

### Unit Tests
**File:** `src/services/database/traits/tests.rs`

| Test Name | Description |
|-----------|-------------|
| `test_database_type_display_names` | Verify all DatabaseType variants have correct display names |
| `test_database_type_default_ports` | Verify default ports for server-based databases |
| `test_database_type_is_file_based` | Verify file-based detection for SQLite/DuckDB |
| `test_value_null_check` | Verify `Value::is_null()` works correctly |
| `test_value_display_string` | Verify `Value::to_display_string()` for all variants |
| `test_connection_params_validation` | Verify Server params require hostname, File params require path |
| `test_connection_config_serialization` | Verify JSON serialization/deserialization roundtrip |

### Integration Tests
**File:** `tests/traits_integration.rs`

| Test Name | Description |
|-----------|-------------|
| `test_connection_factory_unknown_type` | Verify factory returns error for unimplemented types |
| `test_connection_config_from_legacy` | Verify old ConnectionInfo converts to new ConnectionConfig |

### Frontend Tests (Manual)
| Test Case | Steps | Expected Result |
|-----------|-------|-----------------|
| Existing connections load | 1. Start app with existing connections | Connections load with PostgreSQL type |
| Database type persists | 1. Create connection 2. Restart app | Database type preserved |

---

# Epic 2: PostgreSQL Migration

## Goal
Migrate existing PostgreSQL implementation to the new trait-based architecture while maintaining all existing functionality.

## Backend Implementation

### Step 2.1: Create PostgreSQL Driver Directory
**Files to create:**
- `src/services/database/drivers/postgres/mod.rs`
- `src/services/database/drivers/postgres/connection.rs`
- `src/services/database/drivers/postgres/schema.rs`
- `src/services/database/drivers/postgres/types.rs`

**Tasks:**
1. Create directory structure
2. Create `mod.rs` with public exports

### Step 2.2: Implement PostgresConnection
**File:** `src/services/database/drivers/postgres/connection.rs`

**Tasks:**
1. Create `PostgresConnection` struct:
   ```rust
   pub struct PostgresConnection {
       config: ConnectionConfig,
       pool: Option<PgPool>,
   }
   ```
2. Implement `DatabaseConnection` trait:
   - `connect()`: Create PgPool from ConnectionConfig
   - `disconnect()`: Close pool
   - `is_connected()`: Check pool state
   - `execute_query()`: Move logic from existing `manager.rs`
   - `stream_query()`: Move existing stream logic
3. Add `new(config: ConnectionConfig)` constructor

### Step 2.3: Implement Type Conversions
**File:** `src/services/database/drivers/postgres/types.rs`

**Tasks:**
1. Create `PgValueConverter` to convert `PgRow` → `Row`:
   ```rust
   impl PgValueConverter {
       pub fn convert_row(pg_row: &PgRow, metadata: &TableMetadata) -> Row {
           // Convert each column to Value enum
       }

       fn convert_value(pg_value: &PgValue, type_info: &PgTypeInfo) -> Value {
           match type_info.name() {
               "INT4" => Value::Int32(pg_value.decode()),
               "TEXT" | "VARCHAR" => Value::Text(pg_value.decode()),
               // ... handle all PostgreSQL types
           }
       }
   }
   ```
2. Handle all PostgreSQL types including arrays, JSON, UUID, etc.
3. Map PostgreSQL type names to generic type names

### Step 2.4: Implement SchemaIntrospection for PostgreSQL
**File:** `src/services/database/drivers/postgres/schema.rs`

**Tasks:**
1. Move schema queries from existing `src/services/database/schema.rs`
2. Implement `SchemaIntrospection` trait:
   - `get_databases()`: Query `pg_database`
   - `get_tables()`: Query `information_schema.tables`
   - `get_columns()`: Query `information_schema.columns`
   - `get_primary_keys()`: Query `table_constraints`
   - `get_foreign_keys()`: Query `key_column_usage`
   - `get_indexes()`: Query `pg_indexes`
3. Keep PostgreSQL-specific SQL queries here

### Step 2.5: Update ConnectionFactory
**File:** `src/services/database/drivers/factory.rs`

**Tasks:**
1. Add PostgreSQL case to factory:
   ```rust
   DatabaseType::PostgreSQL => {
       Ok(Box::new(PostgresConnection::new(config)))
   }
   ```
2. Add `create_with_schema()` for schema-capable connections

### Step 2.6: Update DatabaseManager
**File:** `src/services/database/manager.rs`

**Tasks:**
1. Replace `PgPool` with `Box<dyn DatabaseConnection>`:
   ```rust
   pub struct DatabaseManager {
       connection: Arc<RwLock<Option<Box<dyn DatabaseConnection>>>>,
       schema_cache: Arc<RwLock<Option<DatabaseSchema>>>,
   }
   ```
2. Update `connect()` to use `ConnectionFactory`
3. Update all methods to use trait methods instead of direct PgPool access
4. Keep public API compatible with existing code

### Step 2.7: Update Query Execution
**File:** `src/services/database/query.rs`

**Tasks:**
1. Update `QueryExecutionResult` to use new `Row` type (or keep adapter)
2. Ensure `ResultColumnMetadata` works with generic column info
3. Update any PostgreSQL-specific logic to go through the driver

### Step 2.8: Remove Old PostgreSQL-Specific Code
**Tasks:**
1. Remove direct `PgPool` usage from `manager.rs`
2. Remove inline SQL from `manager.rs` (moved to driver)
3. Keep backward-compatible public API

## Frontend Implementation

### Step 2.9: Update State Actions
**File:** `src/state/actions.rs`

**Tasks:**
1. Update `connect()` to create `ConnectionConfig` from `ConnectionInfo`
2. Update `connect_async()` to use new `DatabaseManager::connect(config)`
3. Ensure connection monitoring still works with trait methods

### Step 2.10: Verify All Existing UI Works
**Tasks:**
1. Test connection creation flow
2. Test query execution
3. Test schema browser
4. Test database switching
5. Test disconnect/reconnect

## Test Plan

### Unit Tests
**File:** `src/services/database/drivers/postgres/tests.rs`

| Test Name | Description |
|-----------|-------------|
| `test_postgres_connection_new` | Verify PostgresConnection creates from valid config |
| `test_postgres_connection_invalid_params` | Verify error for non-server params |
| `test_pg_value_converter_integers` | Test INT2, INT4, INT8 conversion |
| `test_pg_value_converter_floats` | Test FLOAT4, FLOAT8 conversion |
| `test_pg_value_converter_text` | Test TEXT, VARCHAR, CHAR conversion |
| `test_pg_value_converter_temporal` | Test DATE, TIME, TIMESTAMP conversion |
| `test_pg_value_converter_uuid` | Test UUID conversion |
| `test_pg_value_converter_json` | Test JSON, JSONB conversion |
| `test_pg_value_converter_arrays` | Test array type conversion |
| `test_pg_value_converter_null` | Test NULL handling |

### Integration Tests
**File:** `tests/postgres_integration.rs`

| Test Name | Description | Requires |
|-----------|-------------|----------|
| `test_postgres_connect_disconnect` | Connect and disconnect lifecycle | Live PostgreSQL |
| `test_postgres_simple_query` | Execute SELECT 1 | Live PostgreSQL |
| `test_postgres_select_with_types` | Query with various data types | Live PostgreSQL |
| `test_postgres_schema_tables` | Get tables list | Live PostgreSQL |
| `test_postgres_schema_columns` | Get column info | Live PostgreSQL |
| `test_postgres_modification_query` | Execute INSERT/UPDATE | Live PostgreSQL |
| `test_postgres_connection_failure` | Verify error on bad credentials | - |

### Frontend Tests (Manual)
| Test Case | Steps | Expected Result |
|-----------|-------|-----------------|
| Connect to PostgreSQL | 1. Create new connection 2. Enter valid credentials 3. Click Connect | Connection established, tables load |
| Execute SELECT query | 1. Connect 2. Enter "SELECT * FROM users LIMIT 10" 3. Execute | Results display in table |
| Execute INSERT query | 1. Connect 2. Execute INSERT 3. Check results | "N rows affected" message |
| View table schema | 1. Connect 2. Click table in sidebar | Columns display |
| Switch database | 1. Connect 2. Select different database from dropdown | Reconnects to new database |
| Test connection button | 1. Enter credentials 2. Click "Test Connection" | Success/failure notification |
| Handle connection error | 1. Enter wrong password 2. Try to connect | Error message displayed |

---

# Epic 3: SQLite Support

## Goal
Add SQLite as the first file-based database, introducing the file picker UI pattern.

## Backend Implementation

### Step 3.1: Create SQLite Driver
**Files to create:**
- `src/services/database/drivers/sqlite/mod.rs`
- `src/services/database/drivers/sqlite/connection.rs`
- `src/services/database/drivers/sqlite/schema.rs`
- `src/services/database/drivers/sqlite/types.rs`

### Step 3.2: Implement SqliteConnection
**File:** `src/services/database/drivers/sqlite/connection.rs`

**Tasks:**
1. Create `SqliteConnection` struct with `SqlitePool`
2. Implement `DatabaseConnection`:
   - `connect()`: Handle File and InMemory params
   - Use `SqliteConnectOptions::new().filename(path)`
   - Set `create_if_missing` based on read_only flag
3. Implement connection pooling (note: SQLite has single-writer limitation)

### Step 3.3: Implement SQLite Schema Introspection
**File:** `src/services/database/drivers/sqlite/schema.rs`

**Tasks:**
1. Implement `SchemaIntrospection`:
   - `get_databases()`: Return single "main" database (SQLite limitation)
   - `get_tables()`: Query `sqlite_master`
   - `get_columns()`: Use `PRAGMA table_info(table_name)`
   - `get_primary_keys()`: Parse from `PRAGMA table_info`
   - `get_foreign_keys()`: Use `PRAGMA foreign_key_list(table_name)`
   - `get_indexes()`: Use `PRAGMA index_list(table_name)`

### Step 3.4: Implement SQLite Type Conversion
**File:** `src/services/database/drivers/sqlite/types.rs`

**Tasks:**
1. Handle SQLite's dynamic typing:
   - `INTEGER` → `Value::Int64`
   - `REAL` → `Value::Float64`
   - `TEXT` → `Value::Text`
   - `BLOB` → `Value::Bytes`
   - `NULL` → `Value::Null`
2. Handle type affinity rules

### Step 3.5: Update ConnectionFactory
**File:** `src/services/database/drivers/factory.rs`

**Tasks:**
1. Add SQLite case:
   ```rust
   DatabaseType::SQLite => {
       Ok(Box::new(SqliteConnection::new(config)))
   }
   ```

## Frontend Implementation

### Step 3.6: Add Database Type Selector to ConnectionForm
**File:** `src/workspace/connections/connection_form.rs`

**Tasks:**
1. Add `database_type_select: Entity<SelectState<DatabaseType>>` field
2. Create database type dropdown at top of form:
   ```rust
   Select::new(&self.database_type_select)
       .items(vec![
           ("PostgreSQL", DatabaseType::PostgreSQL),
           ("SQLite", DatabaseType::SQLite),
       ])
   ```
3. Subscribe to selection changes
4. Update form layout based on selected type

### Step 3.7: Create Conditional Form Fields
**File:** `src/workspace/connections/connection_form.rs`

**Tasks:**
1. Create `render_server_fields()` for PostgreSQL/MySQL/ClickHouse:
   - hostname, port, username, password, database, ssl_mode
2. Create `render_file_fields()` for SQLite/DuckDB:
   - file_path (with file picker button)
   - read_only checkbox
3. Conditionally render based on `database_type`:
   ```rust
   match self.selected_database_type {
       DatabaseType::PostgreSQL | DatabaseType::MySQL => self.render_server_fields(cx),
       DatabaseType::SQLite | DatabaseType::DuckDB => self.render_file_fields(cx),
       // ...
   }
   ```

### Step 3.8: Implement File Picker for SQLite
**File:** `src/workspace/connections/connection_form.rs`

**Tasks:**
1. Add file_path input field
2. Add "Browse" button next to file path:
   ```rust
   Button::new("browse")
       .label("Browse...")
       .on_click(cx.listener(|this, _, window, cx| {
           let receiver = cx.prompt_for_paths(/* options */);
           // Handle file selection
       }))
   ```
3. Support both opening existing files and creating new ones
4. Filter for `.db`, `.sqlite`, `.sqlite3` extensions

### Step 3.9: Update ConnectionInfo Persistence
**File:** `src/services/storage/types.rs`

**Tasks:**
1. Add fields for file-based connections:
   ```rust
   pub file_path: Option<PathBuf>,
   pub read_only: Option<bool>,
   ```
2. Update `to_connection_config()` to handle both types

### Step 3.10: Update Storage Schema
**File:** `src/services/storage/connections.rs`

**Tasks:**
1. Add migration for new columns:
   ```sql
   ALTER TABLE connections ADD COLUMN file_path TEXT;
   ALTER TABLE connections ADD COLUMN read_only INTEGER DEFAULT 0;
   ```
2. Update CRUD operations

## Test Plan

### Unit Tests
**File:** `src/services/database/drivers/sqlite/tests.rs`

| Test Name | Description |
|-----------|-------------|
| `test_sqlite_connection_file` | Create connection with file path |
| `test_sqlite_connection_memory` | Create in-memory connection |
| `test_sqlite_connection_invalid` | Verify error for server params |
| `test_sqlite_type_integer` | Test INTEGER conversion |
| `test_sqlite_type_real` | Test REAL conversion |
| `test_sqlite_type_text` | Test TEXT conversion |
| `test_sqlite_type_blob` | Test BLOB conversion |
| `test_sqlite_type_null` | Test NULL handling |

### Integration Tests
**File:** `tests/sqlite_integration.rs`

| Test Name | Description | Requires |
|-----------|-------------|----------|
| `test_sqlite_create_database` | Create new SQLite file | tempfile |
| `test_sqlite_open_existing` | Open existing SQLite file | test fixture |
| `test_sqlite_memory_database` | Create in-memory database | - |
| `test_sqlite_schema_tables` | Get tables from sqlite_master | tempfile |
| `test_sqlite_schema_columns` | Get columns via PRAGMA | tempfile |
| `test_sqlite_crud_operations` | INSERT, SELECT, UPDATE, DELETE | tempfile |
| `test_sqlite_read_only_mode` | Verify writes fail in read-only | tempfile |

### Frontend Tests (Manual)
| Test Case | Steps | Expected Result |
|-----------|-------|-----------------|
| Select SQLite type | 1. New connection 2. Select SQLite from dropdown | File fields appear, server fields hide |
| Browse for file | 1. Select SQLite 2. Click Browse | File picker opens |
| Connect to SQLite | 1. Select/create SQLite file 2. Connect | Connection established |
| Create new SQLite | 1. Enter new file path 2. Connect | New .sqlite file created |
| Read-only mode | 1. Enable read-only 2. Try INSERT | Error message |
| Switch between types | 1. Select PostgreSQL 2. Select SQLite 3. Select PostgreSQL | Form fields update correctly |

---

# Epic 4: MySQL Support

## Goal
Add MySQL support, demonstrating the ease of adding SQLx-based databases.

## Backend Implementation

### Step 4.1: Create MySQL Driver
**Files to create:**
- `src/services/database/drivers/mysql/mod.rs`
- `src/services/database/drivers/mysql/connection.rs`
- `src/services/database/drivers/mysql/schema.rs`
- `src/services/database/drivers/mysql/types.rs`

### Step 4.2: Implement MySqlConnection
**File:** `src/services/database/drivers/mysql/connection.rs`

**Tasks:**
1. Create `MySqlConnection` struct with `MySqlPool`
2. Implement `DatabaseConnection`:
   - Similar to PostgreSQL, use `MySqlConnectOptions`
   - Handle MySQL-specific SSL modes
3. Handle MySQL-specific connection parameters (charset, etc.)

### Step 4.3: Implement MySQL Schema Introspection
**File:** `src/services/database/drivers/mysql/schema.rs`

**Tasks:**
1. Implement `SchemaIntrospection`:
   - `get_databases()`: Query `SHOW DATABASES`
   - `get_tables()`: Query `information_schema.TABLES` with `TABLE_SCHEMA = DATABASE()`
   - `get_columns()`: Query `information_schema.COLUMNS`
   - `get_primary_keys()`: Query `information_schema.KEY_COLUMN_USAGE`
   - `get_foreign_keys()`: Query `information_schema.REFERENTIAL_CONSTRAINTS`
   - `get_indexes()`: Query `information_schema.STATISTICS`

### Step 4.4: Implement MySQL Type Conversion
**File:** `src/services/database/drivers/mysql/types.rs`

**Tasks:**
1. Handle MySQL types:
   - `TINYINT`, `SMALLINT`, `MEDIUMINT`, `INT`, `BIGINT` → Int variants
   - `FLOAT`, `DOUBLE` → Float variants
   - `DECIMAL` → `Value::Decimal`
   - `VARCHAR`, `TEXT`, `CHAR` → `Value::Text`
   - `BLOB`, `BINARY` → `Value::Bytes`
   - `DATE`, `TIME`, `DATETIME`, `TIMESTAMP` → Temporal variants
   - `JSON` → `Value::Json`
   - `ENUM`, `SET` → `Value::Text`

### Step 4.5: Update ConnectionFactory
**File:** `src/services/database/drivers/factory.rs`

**Tasks:**
1. Add MySQL case to factory

## Frontend Implementation

### Step 4.6: Add MySQL to Database Type Selector
**File:** `src/workspace/connections/connection_form.rs`

**Tasks:**
1. Add MySQL to dropdown options
2. MySQL uses server fields (same as PostgreSQL)
3. Update port default to 3306 when MySQL selected

### Step 4.7: Handle MySQL-Specific Options
**Tasks:**
1. Consider adding charset selector (optional)
2. Ensure SSL mode options work for MySQL

## Test Plan

### Unit Tests
**File:** `src/services/database/drivers/mysql/tests.rs`

| Test Name | Description |
|-----------|-------------|
| `test_mysql_connection_new` | Create connection from valid config |
| `test_mysql_type_integers` | Test TINYINT through BIGINT |
| `test_mysql_type_decimals` | Test DECIMAL precision |
| `test_mysql_type_temporal` | Test DATE, TIME, DATETIME |
| `test_mysql_type_json` | Test JSON column handling |
| `test_mysql_type_enum` | Test ENUM type handling |

### Integration Tests
**File:** `tests/mysql_integration.rs`

| Test Name | Description | Requires |
|-----------|-------------|----------|
| `test_mysql_connect_disconnect` | Connection lifecycle | Live MySQL |
| `test_mysql_simple_query` | Execute SELECT 1 | Live MySQL |
| `test_mysql_show_databases` | List available databases | Live MySQL |
| `test_mysql_schema_tables` | Get tables list | Live MySQL |
| `test_mysql_crud_operations` | Full CRUD cycle | Live MySQL |

### Frontend Tests (Manual)
| Test Case | Steps | Expected Result |
|-----------|-------|-----------------|
| Select MySQL type | 1. New connection 2. Select MySQL | Port changes to 3306 |
| Connect to MySQL | 1. Enter MySQL credentials 2. Connect | Connection established |
| Execute MySQL query | 1. Connect 2. Run SELECT query | Results display |
| MySQL-specific syntax | 1. Run `SHOW TABLES` | Tables listed |
| Switch databases | 1. Connect 2. Change database | Reconnects to new database |

---

# Epic 5: DuckDB Support

## Goal
Add DuckDB as an embedded analytical database with async wrapper.

## Backend Implementation

### Step 5.1: Add DuckDB Dependencies
**File:** `Cargo.toml`

**Tasks:**
1. Add dependencies:
   ```toml
   duckdb = { version = "1.0", features = ["bundled"] }
   async-duckdb = "0.1"
   ```

### Step 5.2: Create DuckDB Driver
**Files to create:**
- `src/services/database/drivers/duckdb/mod.rs`
- `src/services/database/drivers/duckdb/connection.rs`
- `src/services/database/drivers/duckdb/schema.rs`
- `src/services/database/drivers/duckdb/types.rs`

### Step 5.3: Implement DuckDbConnection
**File:** `src/services/database/drivers/duckdb/connection.rs`

**Tasks:**
1. Use `async_duckdb::Client` for async operations
2. Implement `DatabaseConnection`:
   - `connect()`: Handle file and in-memory modes
   - Note: DuckDB has special concurrency model (read_only for pools)
3. Execute queries through async wrapper:
   ```rust
   let result = client.conn(move |conn| {
       let mut stmt = conn.prepare(&sql)?;
       // Execute and collect results
   }).await?;
   ```

### Step 5.4: Implement DuckDB Schema Introspection
**File:** `src/services/database/drivers/duckdb/schema.rs`

**Tasks:**
1. DuckDB supports `information_schema` (PostgreSQL-compatible)
2. Implement `SchemaIntrospection`:
   - `get_databases()`: Query `pragma_database_list`
   - `get_tables()`: Query `information_schema.tables`
   - `get_columns()`: Query `information_schema.columns`
   - Also consider DuckDB extensions (Parquet, CSV support)

### Step 5.5: Implement DuckDB Type Conversion
**File:** `src/services/database/drivers/duckdb/types.rs`

**Tasks:**
1. Handle DuckDB types:
   - Standard SQL types similar to PostgreSQL
   - `HUGEINT` (128-bit) → String representation or custom handling
   - `LIST`, `STRUCT`, `MAP` → Complex types
   - `INTERVAL` → Duration representation

### Step 5.6: Update ConnectionFactory
**File:** `src/services/database/drivers/factory.rs`

**Tasks:**
1. Add DuckDB case to factory

## Frontend Implementation

### Step 5.7: Add DuckDB to Type Selector
**File:** `src/workspace/connections/connection_form.rs`

**Tasks:**
1. Add DuckDB to dropdown (uses file fields like SQLite)
2. Filter for `.duckdb`, `.db` extensions

### Step 5.8: DuckDB-Specific Options (Optional)
**Tasks:**
1. Consider adding options for:
   - Memory limit
   - Thread count
   - Extensions to load

## Test Plan

### Unit Tests
**File:** `src/services/database/drivers/duckdb/tests.rs`

| Test Name | Description |
|-----------|-------------|
| `test_duckdb_connection_file` | Create file-based connection |
| `test_duckdb_connection_memory` | Create in-memory connection |
| `test_duckdb_type_numeric` | Test numeric type conversion |
| `test_duckdb_type_temporal` | Test date/time types |
| `test_duckdb_type_list` | Test LIST type handling |
| `test_duckdb_type_struct` | Test STRUCT type handling |

### Integration Tests
**File:** `tests/duckdb_integration.rs`

| Test Name | Description | Requires |
|-----------|-------------|----------|
| `test_duckdb_create_database` | Create new DuckDB file | tempfile |
| `test_duckdb_memory_database` | In-memory operations | - |
| `test_duckdb_parquet_query` | Query Parquet file directly | test fixture |
| `test_duckdb_csv_query` | Query CSV file directly | test fixture |
| `test_duckdb_schema_introspection` | Get tables and columns | tempfile |
| `test_duckdb_analytical_query` | Window functions, aggregations | tempfile |

### Frontend Tests (Manual)
| Test Case | Steps | Expected Result |
|-----------|-------|-----------------|
| Select DuckDB type | 1. New connection 2. Select DuckDB | File picker fields appear |
| Create DuckDB database | 1. Enter new path 2. Connect | New .duckdb file created |
| Query Parquet file | 1. Connect 2. Run `SELECT * FROM 'file.parquet'` | Parquet data displayed |
| Analytical query | 1. Run query with window function | Results with calculations |

---

# Epic 6: ClickHouse Support

## Goal
Add ClickHouse as a server-based analytical database using the official Rust client.

## Backend Implementation

### Step 6.1: Add ClickHouse Dependencies
**File:** `Cargo.toml`

**Tasks:**
1. Add dependencies:
   ```toml
   clickhouse = { version = "0.12", features = ["tls"] }
   ```

### Step 6.2: Create ClickHouse Driver
**Files to create:**
- `src/services/database/drivers/clickhouse/mod.rs`
- `src/services/database/drivers/clickhouse/connection.rs`
- `src/services/database/drivers/clickhouse/schema.rs`
- `src/services/database/drivers/clickhouse/types.rs`

### Step 6.3: Implement ClickHouseConnection
**File:** `src/services/database/drivers/clickhouse/connection.rs`

**Tasks:**
1. Use `clickhouse::Client` (HTTP-based)
2. Implement `DatabaseConnection`:
   - `connect()`: Build client with URL, credentials
   - Default port 8123 (HTTP) or 8443 (HTTPS)
3. Query execution returns results via cursor:
   ```rust
   let mut cursor = client.query(sql).fetch::<serde_json::Value>()?;
   while let Some(row) = cursor.next().await? {
       // Process row
   }
   ```

### Step 6.4: Implement ClickHouse Schema Introspection
**File:** `src/services/database/drivers/clickhouse/schema.rs`

**Tasks:**
1. Implement `SchemaIntrospection`:
   - `get_databases()`: Query `system.databases`
   - `get_tables()`: Query `system.tables`
   - `get_columns()`: Query `system.columns`
2. Note: ClickHouse doesn't have traditional primary keys, foreign keys
3. Return engine info instead (MergeTree, ReplicatedMergeTree, etc.)

### Step 6.5: Implement ClickHouse Type Conversion
**File:** `src/services/database/drivers/clickhouse/types.rs`

**Tasks:**
1. Handle ClickHouse types:
   - `UInt8-64`, `Int8-64` → Unsigned/signed integers
   - `Float32`, `Float64` → Floats
   - `String`, `FixedString(N)` → Text
   - `Date`, `DateTime`, `DateTime64` → Temporal
   - `Nullable(T)` → Handle null wrapper
   - `Array(T)` → Arrays
   - `LowCardinality(T)` → Treat as underlying type
   - `Enum8`, `Enum16` → Text representation
   - `UUID` → UUID
   - `IPv4`, `IPv6` → Text representation

### Step 6.6: Update ConnectionFactory
**File:** `src/services/database/drivers/factory.rs`

**Tasks:**
1. Add ClickHouse case to factory

## Frontend Implementation

### Step 6.7: Add ClickHouse to Type Selector
**File:** `src/workspace/connections/connection_form.rs`

**Tasks:**
1. Add ClickHouse to dropdown (uses server fields)
2. Default port to 8123
3. Note: ClickHouse uses HTTP, not traditional database port

### Step 6.8: ClickHouse-Specific Options
**Tasks:**
1. Consider adding:
   - HTTP vs HTTPS toggle
   - Compression option
   - Async insert option

## Test Plan

### Unit Tests
**File:** `src/services/database/drivers/clickhouse/tests.rs`

| Test Name | Description |
|-----------|-------------|
| `test_clickhouse_connection_new` | Create connection from config |
| `test_clickhouse_type_integers` | Test UInt/Int type conversion |
| `test_clickhouse_type_nullable` | Test Nullable wrapper handling |
| `test_clickhouse_type_array` | Test Array type conversion |
| `test_clickhouse_type_datetime64` | Test DateTime64 precision |
| `test_clickhouse_type_lowcardinality` | Test LowCardinality unwrapping |

### Integration Tests
**File:** `tests/clickhouse_integration.rs`

| Test Name | Description | Requires |
|-----------|-------------|----------|
| `test_clickhouse_connect` | Establish connection | Live ClickHouse |
| `test_clickhouse_simple_query` | Execute SELECT 1 | Live ClickHouse |
| `test_clickhouse_system_tables` | Query system.tables | Live ClickHouse |
| `test_clickhouse_insert_select` | Insert and retrieve data | Live ClickHouse |
| `test_clickhouse_aggregation` | Run analytical query | Live ClickHouse |

### Frontend Tests (Manual)
| Test Case | Steps | Expected Result |
|-----------|-------|-----------------|
| Select ClickHouse type | 1. New connection 2. Select ClickHouse | Port changes to 8123 |
| Connect to ClickHouse | 1. Enter credentials 2. Connect | Connection established |
| View system tables | 1. Connect 2. View tables | Tables with engine info |
| Analytical query | 1. Run GROUP BY query | Aggregated results display |
| ClickHouse-specific syntax | 1. Run `SELECT * FROM system.functions` | Function list displayed |

---

# Epic 7: S3/Blob Storage with OpenDAL

## Goal
Add S3-compatible blob storage support using Apache OpenDAL, enabling users to browse and manage files in cloud storage.

---

## UI Design: ASCII Diagrams

### Design Option A: Sidebar Tab-Based Approach

The storage browser appears as a new tab alongside database connections in the left sidebar.

```
┌──────────────────────────────────────────────────────────────────────────────┐
│  pgui - Database & Storage Manager                                      [_][x]│
├──────────────────────────────────────────────────────────────────────────────┤
│ ┌─────────────────────┐ ┌──────────────────────────────────────────────────┐ │
│ │ [🗄️ DB] [📦 Storage] │ │  Query Editor / File Preview                     │ │
│ ├─────────────────────┤ │                                                  │ │
│ │                     │ │                                                  │ │
│ │ ▾ 🪣 my-bucket      │ │                                                  │ │
│ │   ▾ 📁 data/        │ │                                                  │ │
│ │     📁 2024/        │ │                                                  │ │
│ │     📁 2025/        │ │                                                  │ │
│ │     📄 config.json  │ │                                                  │ │
│ │   ▸ 📁 backups/     │ │                                                  │ │
│ │   ▸ 📁 logs/        │ │                                                  │ │
│ │   📄 readme.txt     │ │                                                  │ │
│ │                     │ │                                                  │ │
│ │ ▸ 🪣 archive-bucket │ │                                                  │ │
│ │ ▸ 🪣 public-assets  │ │                                                  │ │
│ │                     │ │                                                  │ │
│ ├─────────────────────┤ │                                                  │ │
│ │ [+ Add Storage]     │ │                                                  │ │
│ └─────────────────────┘ └──────────────────────────────────────────────────┘ │
├──────────────────────────────────────────────────────────────────────────────┤
│ Connected: my-s3-prod │ 3 buckets │ 1,234 objects                            │
└──────────────────────────────────────────────────────────────────────────────┘
```

### Design Option B: Dual-Panel File Browser

When a storage connection is active, show a dedicated file browser panel.

```
┌──────────────────────────────────────────────────────────────────────────────┐
│  Storage Browser - my-s3-prod                                           [_][x]│
├──────────────────────────────────────────────────────────────────────────────┤
│ 📍 my-bucket / data / 2025 /                            [🔍 Search] [⚙️ Filter]│
├──────────────────────────────────────────────────────────────────────────────┤
│ [⬆️ Upload] [📁 New Folder] [⬇️ Download] [🗑️ Delete] [🔄 Refresh]            │
├────────────────────────────────────┬─────────────────────────────────────────┤
│                                    │                                         │
│  Name                    Size      │  📄 File Details                        │
│  ─────────────────────────────     │  ─────────────────────────────          │
│  📁 ..                             │                                         │
│  📁 january/              -        │  Name:     sales_q1.parquet             │
│  📁 february/             -        │  Path:     data/2025/sales_q1.parquet   │
│  📁 march/                -        │  Size:     15.4 MB                      │
│  📄 sales_q1.parquet   15.4 MB  ◀──│  Modified: 2025-01-15 14:32:00          │
│  📄 users.csv           2.1 MB     │  Type:     application/parquet          │
│  📄 config.yaml         1.2 KB     │  ETag:     "abc123..."                  │
│  📄 notes.md            856 B      │                                         │
│                                    │  ─────────────────────────────          │
│                                    │  [⬇️ Download] [👁️ Preview] [🗑️ Delete]  │
│                                    │                                         │
├────────────────────────────────────┴─────────────────────────────────────────┤
│ 4 folders, 4 files │ Selected: sales_q1.parquet (15.4 MB)                    │
└──────────────────────────────────────────────────────────────────────────────┘
```

### Design Option C: Column-Based Navigation (Finder-style)

Navigate through directories in a column-based view.

```
┌──────────────────────────────────────────────────────────────────────────────┐
│  Storage: my-s3-prod                                                    [_][x]│
├──────────────────────────────────────────────────────────────────────────────┤
│ [⬆️ Upload] [📁 New Folder] [🔄 Refresh]           🪣 my-bucket / data / 2025 │
├─────────────────┬──────────────────┬─────────────────┬───────────────────────┤
│  🪣 Buckets      │  📁 data/        │  📁 2025/       │  📄 Preview           │
│  ───────────    │  ───────────     │  ───────────    │  ───────────          │
│                 │                  │                 │                       │
│  my-bucket    ▸ │  2024/         ▸ │  january/     ▸ │  sales_q1.parquet     │
│  archive      ▸ │  2025/         ▸ │  february/    ▸ │  ─────────────        │
│  public-assets▸ │  config.json     │  march/       ▸ │                       │
│                 │  readme.md       │  sales_q1.par…  │  Size: 15.4 MB        │
│                 │                  │  users.csv      │  Modified: Jan 15     │
│                 │                  │  config.yaml    │                       │
│                 │                  │  notes.md       │  ┌─────────────────┐  │
│                 │                  │                 │  │ Column Headers: │  │
│                 │                  │                 │  │ date, product,  │  │
│                 │                  │                 │  │ region, sales,  │  │
│                 │                  │                 │  │ quantity...     │  │
│                 │                  │                 │  └─────────────────┘  │
│                 │                  │                 │                       │
│                 │                  │                 │  [Download] [Delete]  │
├─────────────────┴──────────────────┴─────────────────┴───────────────────────┤
│ Path: s3://my-bucket/data/2025/sales_q1.parquet                              │
└──────────────────────────────────────────────────────────────────────────────┘
```

---

### Storage Connection Form

```
┌─────────────────────────────────────────────────────────────────┐
│  Add Storage Connection                                    [×]  │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  Connection Name:  ┌────────────────────────────────────────┐   │
│                    │ my-s3-prod                             │   │
│                    └────────────────────────────────────────┘   │
│                                                                 │
│  Storage Type:     ┌────────────────────────────────────────┐   │
│                    │ Amazon S3                          [▾] │   │
│                    └────────────────────────────────────────┘   │
│                    ○ Amazon S3                                  │
│                    ○ Google Cloud Storage                       │
│                    ○ Azure Blob Storage                         │
│                    ○ S3-Compatible (MinIO, R2, etc.)            │
│                    ○ Local Filesystem                           │
│                                                                 │
│  ─── S3 Configuration ──────────────────────────────────────   │
│                                                                 │
│  Endpoint:         ┌────────────────────────────────────────┐   │
│                    │ https://s3.amazonaws.com               │   │
│                    └────────────────────────────────────────┘   │
│                    (Leave blank for AWS, set for MinIO/R2)      │
│                                                                 │
│  Region:           ┌────────────────────────────────────────┐   │
│                    │ us-east-1                          [▾] │   │
│                    └────────────────────────────────────────┘   │
│                                                                 │
│  Bucket:           ┌────────────────────────────────────────┐   │
│                    │ my-bucket                              │   │
│                    └────────────────────────────────────────┘   │
│                                                                 │
│  Access Key ID:    ┌────────────────────────────────────────┐   │
│                    │ AKIAIOSFODNN7EXAMPLE                   │   │
│                    └────────────────────────────────────────┘   │
│                                                                 │
│  Secret Key:       ┌────────────────────────────────────────┐   │
│                    │ ••••••••••••••••••••                   │   │
│                    └────────────────────────────────────────┘   │
│                                                                 │
│  ☐ Use path-style addressing (required for MinIO)              │
│  ☐ Allow unsigned requests (public buckets)                    │
│                                                                 │
├─────────────────────────────────────────────────────────────────┤
│              [Test Connection]     [Cancel]  [Save Connection]  │
└─────────────────────────────────────────────────────────────────┘
```

---

### File Operations & Context Menu

```
┌─────────────────────────────────┐
│  📄 sales_q1.parquet            │
├─────────────────────────────────┤
│  👁️  Preview                    │
│  ⬇️  Download                   │
│  📋  Copy Path                  │
│  📋  Copy S3 URI                │
│  ───────────────────────────    │
│  ✏️  Rename                     │
│  📁  Move to...                 │
│  📄  Copy to...                 │
│  ───────────────────────────    │
│  🗑️  Delete                     │
└─────────────────────────────────┘

┌─────────────────────────────────┐
│  📁 data/                       │
├─────────────────────────────────┤
│  📂  Open                       │
│  📋  Copy Path                  │
│  ───────────────────────────    │
│  📁  New Subfolder              │
│  ⬆️  Upload to Here             │
│  ───────────────────────────    │
│  ✏️  Rename                     │
│  🗑️  Delete (recursive)         │
└─────────────────────────────────┘
```

---

### File Preview Modes

**Text/JSON Preview:**
```
┌──────────────────────────────────────────────────────────────────┐
│  📄 config.yaml                                   [Download] [×] │
├──────────────────────────────────────────────────────────────────┤
│  1 │ database:                                                   │
│  2 │   host: localhost                                           │
│  3 │   port: 5432                                                │
│  4 │   name: production                                          │
│  5 │                                                             │
│  6 │ storage:                                                    │
│  7 │   type: s3                                                  │
│  8 │   bucket: my-bucket                                         │
│  9 │   region: us-east-1                                         │
│ 10 │                                                             │
│    │                                                             │
├──────────────────────────────────────────────────────────────────┤
│ YAML │ 856 bytes │ UTF-8                                         │
└──────────────────────────────────────────────────────────────────┘
```

**CSV/Parquet Table Preview:**
```
┌──────────────────────────────────────────────────────────────────┐
│  📄 sales_q1.parquet                      [Download] [Query] [×] │
├──────────────────────────────────────────────────────────────────┤
│  Showing first 100 rows of 15,432                                │
├────────┬────────────┬──────────┬───────────┬─────────────────────┤
│ Row    │ date       │ product  │ region    │ sales    │ quantity │
├────────┼────────────┼──────────┼───────────┼──────────┼──────────┤
│ 1      │ 2025-01-01 │ Widget A │ US-East   │ 1,234.56 │ 42       │
│ 2      │ 2025-01-01 │ Widget B │ US-West   │ 987.65   │ 31       │
│ 3      │ 2025-01-02 │ Widget A │ EU-West   │ 2,345.67 │ 78       │
│ 4      │ 2025-01-02 │ Widget C │ US-East   │ 456.78   │ 15       │
│ 5      │ 2025-01-03 │ Widget B │ APAC      │ 3,456.78 │ 112      │
│ ...    │ ...        │ ...      │ ...       │ ...      │ ...      │
├────────┴────────────┴──────────┴───────────┴──────────┴──────────┤
│ Parquet │ 6 columns │ 15,432 rows │ 15.4 MB                      │
└──────────────────────────────────────────────────────────────────┘
```

**Image Preview:**
```
┌──────────────────────────────────────────────────────────────────┐
│  📄 logo.png                                      [Download] [×] │
├──────────────────────────────────────────────────────────────────┤
│                                                                  │
│                    ┌────────────────────┐                        │
│                    │                    │                        │
│                    │    [Image         │                        │
│                    │     Preview]       │                        │
│                    │                    │                        │
│                    │    512 × 512 px    │                        │
│                    │                    │                        │
│                    └────────────────────┘                        │
│                                                                  │
├──────────────────────────────────────────────────────────────────┤
│ PNG │ 512×512 │ 45.2 KB │ image/png                              │
└──────────────────────────────────────────────────────────────────┘
```

---

### Upload Progress Dialog

```
┌─────────────────────────────────────────────────────────────────┐
│  Uploading Files                                           [×]  │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  📄 large_dataset.parquet                                       │
│  ████████████████████░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░  45%        │
│  45.2 MB / 100.5 MB  •  12.3 MB/s  •  ~4 sec remaining          │
│                                                                 │
│  ─────────────────────────────────────────────────────────────  │
│                                                                 │
│  📄 config.yaml                                     ✓ Complete  │
│  📄 readme.md                                       ✓ Complete  │
│  📄 data.csv                                        ⏳ Pending   │
│                                                                 │
├─────────────────────────────────────────────────────────────────┤
│  2 of 4 files complete                              [Cancel]    │
└─────────────────────────────────────────────────────────────────┘
```

---

### Recommended Design: Hybrid Approach

Combine **Option A** (sidebar tabs) with **Option B** (dual-panel when browsing):

```
┌──────────────────────────────────────────────────────────────────────────────┐
│  pgui                                                                   [_][x]│
├──────────────────────────────────────────────────────────────────────────────┤
│ ┌─────────────────────┐ ┌────────────────────────────────────────────────────┤
│ │ [🗄️ DB] [📦 Storage] │ │  🪣 my-bucket / data / 2025 /              [🔄] [⚙️]│
│ ├─────────────────────┤ ├────────────────────────────────────────────────────┤
│ │                     │ │ [⬆️ Upload] [📁 New Folder] [⬇️] [🗑️]              │
│ │ ▾ 🔌 my-s3-prod     │ ├────────────────────────────┬───────────────────────┤
│ │   ▾ 🪣 my-bucket    │ │                            │ 📄 sales_q1.parquet   │
│ │     ▸ 📁 data/      │ │  Name              Size    │ ─────────────────     │
│ │     ▸ 📁 backups/   │ │  ────────────────────────  │ Size:  15.4 MB        │
│ │   ▸ 🪣 archive      │ │  📁 ..                     │ Modified: Jan 15      │
│ │   ▸ 🪣 public       │ │  📁 january/        -      │ Type: parquet         │
│ │                     │ │  📁 february/       -      │                       │
│ │ ▸ 🔌 backup-storage │ │  📄 sales_q1... ◀ 15.4 MB  │ ─────────────────     │
│ │                     │ │  📄 users.csv     2.1 MB   │ [⬇️ Download]         │
│ │                     │ │  📄 config.yaml   1.2 KB   │ [👁️ Preview]          │
│ │                     │ │                            │ [🗑️ Delete]           │
│ ├─────────────────────┤ ├────────────────────────────┴───────────────────────┤
│ │ [+ Add Connection]  │ │ 3 folders, 3 files │ Selected: sales_q1.parquet   │
│ └─────────────────────┘ └────────────────────────────────────────────────────┘
└──────────────────────────────────────────────────────────────────────────────┘
```

**Key Design Decisions:**

1. **Tab-based sidebar**: Switch between DB and Storage modes
2. **Tree navigation**: Connections → Buckets → Folders in sidebar
3. **Dual-panel main area**: File list + Details panel
4. **Breadcrumb navigation**: Shows current path, clickable segments
5. **Toolbar**: Common actions always visible
6. **Context menus**: Right-click for additional actions
7. **Preview integration**: Reuse query results panel for file preview
8. **Progress dialogs**: Modal for uploads/downloads

---

## Backend Implementation

### Step 7.1: Add OpenDAL Dependencies
**File:** `Cargo.toml`

**Tasks:**
1. Add dependencies:
   ```toml
   opendal = { version = "0.50", features = [
       "services-s3",
       "services-gcs",
       "services-azblob",
       "services-fs",
   ]}
   ```

### Step 7.2: Create Storage Module Structure
**Files to create:**
- `src/services/database/storage/mod.rs`
- `src/services/database/storage/traits.rs`
- `src/services/database/storage/config.rs`
- `src/services/database/storage/s3.rs`
- `src/services/database/storage/factory.rs`

### Step 7.3: Define Storage Traits
**File:** `src/services/database/storage/traits.rs`

**Tasks:**
1. Define `StorageConnection` trait:
   ```rust
   #[async_trait]
   pub trait StorageConnection: Send + Sync {
       fn storage_type(&self) -> StorageType;
       async fn connect(&mut self) -> Result<()>;
       async fn list(&self, path: &str) -> Result<Vec<ObjectInfo>>;
       async fn read(&self, path: &str) -> Result<Vec<u8>>;
       async fn read_stream(&self, path: &str) -> Result<BoxStream<'_, Result<Bytes>>>;
       async fn write(&self, path: &str, data: Vec<u8>) -> Result<()>;
       async fn delete(&self, path: &str) -> Result<()>;
       async fn exists(&self, path: &str) -> Result<bool>;
       async fn stat(&self, path: &str) -> Result<ObjectInfo>;
       async fn create_dir(&self, path: &str) -> Result<()>;
   }
   ```
2. Define `StorageType` enum: `S3`, `Gcs`, `AzureBlob`, `LocalFs`
3. Define `ObjectInfo` struct: path, size, last_modified, content_type, is_dir

### Step 7.4: Define Storage Configuration
**File:** `src/services/database/storage/config.rs`

**Tasks:**
1. Create `StorageConfig`:
   ```rust
   pub struct StorageConfig {
       pub id: Uuid,
       pub name: String,
       pub storage_type: StorageType,
       pub params: StorageParams,
   }
   ```
2. Create `StorageParams` enum with variants for each storage type

### Step 7.5: Implement S3 Storage
**File:** `src/services/database/storage/s3.rs`

**Tasks:**
1. Create `S3Storage` struct with `opendal::Operator`
2. Implement `StorageConnection`:
   - `connect()`: Build S3 operator with credentials
   - `list()`: Use `operator.list(path)`
   - `read()`: Use `operator.read(path)`
   - `write()`: Use `operator.write(path, data)`
   - `delete()`: Use `operator.delete(path)`
3. Handle S3-compatible services (MinIO, DigitalOcean Spaces, etc.) via endpoint override
4. Support path-style vs virtual-hosted-style

### Step 7.6: Implement StorageFactory
**File:** `src/services/database/storage/factory.rs`

**Tasks:**
1. Create factory for storage connections:
   ```rust
   pub fn create(config: StorageConfig) -> Result<Box<dyn StorageConnection>> {
       match config.storage_type {
           StorageType::S3 => Ok(Box::new(S3Storage::new(config))),
           // Future: Gcs, AzureBlob, LocalFs
       }
   }
   ```

### Step 7.7: Create StorageManager
**File:** `src/services/database/storage/manager.rs`

**Tasks:**
1. Create `StorageManager` similar to `DatabaseManager`:
   ```rust
   pub struct StorageManager {
       connection: Arc<RwLock<Option<Box<dyn StorageConnection>>>>,
   }
   ```
2. Implement connect, disconnect, and delegating methods

### Step 7.8: Persist Storage Connections
**File:** `src/services/storage/connections.rs`

**Tasks:**
1. Create separate table for storage connections:
   ```sql
   CREATE TABLE storage_connections (
       id TEXT PRIMARY KEY,
       name TEXT NOT NULL UNIQUE,
       storage_type TEXT NOT NULL,
       endpoint TEXT,
       bucket TEXT,
       region TEXT,
       access_key_id TEXT,
       path_style INTEGER DEFAULT 0,
       created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
   )
   ```
2. Store secrets in keyring

## Frontend Implementation

### Step 7.9: Create Storage State
**File:** `src/state/storage.rs`

**Tasks:**
1. Create `StorageState`:
   ```rust
   pub struct StorageState {
       pub saved_connections: Vec<StorageConfig>,
       pub active_connection: Option<StorageConfig>,
       pub storage_manager: StorageManager,
       pub connection_status: ConnectionStatus,
   }
   ```
2. Initialize in `state::init()`

### Step 7.10: Create StorageConnectionForm
**File:** `src/workspace/storage/connection_form.rs`

**Tasks:**
1. Create form similar to `ConnectionForm` but for storage:
   - Storage type dropdown (S3, GCS, Azure, Local)
   - S3 fields: endpoint, bucket, region, access_key_id, secret_access_key
   - Path-style checkbox for MinIO compatibility
2. Test connection button
3. Save/update/delete buttons

### Step 7.11: Create StorageBrowser Component
**File:** `src/workspace/storage/browser.rs`

**Tasks:**
1. Create tree-based file browser:
   ```rust
   pub struct StorageBrowser {
       tree_state: Entity<TreeState>,
       current_path: String,
       storage_manager: StorageManager,
   }
   ```
2. Load directories on expand
3. Display files with size, last modified
4. Support navigation (breadcrumb or path input)

### Step 7.12: Create StorageActions
**File:** `src/workspace/storage/actions.rs`

**Tasks:**
1. Implement file operations:
   - Download file (prompt for save location)
   - Upload file (file picker)
   - Delete file (confirmation dialog)
   - Create directory
   - Rename (delete + create)
2. Progress indicators for large transfers
3. Error handling with notifications

### Step 7.13: Create StoragePanel
**File:** `src/workspace/storage/panel.rs`

**Tasks:**
1. Create panel for storage browser (similar to sidebar)
2. Toolbar: refresh, upload, create folder
3. Status bar: connected bucket, object count
4. Context menu on right-click (download, delete, etc.)

### Step 7.14: Integrate into Workspace
**File:** `src/workspace/workspace.rs`

**Tasks:**
1. Add storage panel as optional sidebar section
2. Add "Storage" tab or button to switch between databases and storage
3. Handle storage connection independently from database connection

### Step 7.15: File Preview (Optional)
**Tasks:**
1. Preview common file types:
   - Text files (.txt, .json, .csv)
   - Images (.png, .jpg)
   - Parquet metadata
2. Download for other types

## Test Plan

### Unit Tests
**File:** `src/services/database/storage/tests.rs`

| Test Name | Description |
|-----------|-------------|
| `test_s3_storage_config` | Verify S3Config creation |
| `test_storage_factory_s3` | Factory creates S3Storage |
| `test_storage_factory_unknown` | Factory errors on unknown type |
| `test_object_info_is_dir` | Verify directory detection |

### Integration Tests
**File:** `tests/s3_integration.rs`

| Test Name | Description | Requires |
|-----------|-------------|----------|
| `test_s3_connect` | Connect to S3/MinIO | MinIO |
| `test_s3_list_root` | List bucket root | MinIO |
| `test_s3_upload_download` | Upload and download file | MinIO |
| `test_s3_delete` | Delete object | MinIO |
| `test_s3_create_directory` | Create pseudo-directory | MinIO |
| `test_s3_nested_list` | List nested paths | MinIO |
| `test_s3_large_file_stream` | Stream large file | MinIO |

### Frontend Tests (Manual)
| Test Case | Steps | Expected Result |
|-----------|-------|-----------------|
| Add S3 connection | 1. Open storage connections 2. Enter S3 credentials 3. Test 4. Save | Connection saved |
| Browse S3 bucket | 1. Connect to S3 2. View browser | Files and folders displayed |
| Navigate directories | 1. Click folder 2. Go back | Navigation works correctly |
| Upload file | 1. Click upload 2. Select file | File appears in browser |
| Download file | 1. Select file 2. Click download | File saved locally |
| Delete file | 1. Select file 2. Delete 3. Confirm | File removed from list |
| MinIO compatibility | 1. Enter MinIO endpoint 2. Enable path-style 3. Connect | Connection works |
| Handle large files | 1. Upload 100MB file | Progress indicator, completion |

---

# Implementation Order Recommendation

```
Epic 1 (Core Abstractions)
    │
    ├── Epic 2 (PostgreSQL Migration) ──┬── Epic 3 (SQLite)
    │                                   │
    │                                   ├── Epic 4 (MySQL)
    │                                   │
    │                                   ├── Epic 5 (DuckDB)
    │                                   │
    │                                   └── Epic 6 (ClickHouse)
    │
    └── Epic 7 (S3/OpenDAL) [Can start after Step 1.6]
```

**Recommended Order:**
1. Epic 1 → Epic 2 (foundation + first migration)
2. Epic 3 (SQLite - simplest file-based, validates abstraction)
3. Epic 4 (MySQL - validates SQLx pattern reuse)
4. Epic 7 (S3 - independent track, can parallelize)
5. Epic 5 (DuckDB - file-based with unique features)
6. Epic 6 (ClickHouse - server-based analytical)

---

# Testing Infrastructure Setup

Before starting implementation, set up testing infrastructure:

## Step 0.1: Create Test Module Structure
```
tests/
├── common/
│   ├── mod.rs
│   ├── fixtures.rs        # Test data and setup
│   └── helpers.rs         # Common test utilities
├── postgres_integration.rs
├── sqlite_integration.rs
├── mysql_integration.rs
├── duckdb_integration.rs
├── clickhouse_integration.rs
└── s3_integration.rs
```

## Step 0.2: Create Docker Compose for Test Databases
**File:** `docker-compose.test.yml`
```yaml
version: '3.8'
services:
  postgres:
    image: postgres:16
    environment:
      POSTGRES_PASSWORD: test
    ports:
      - "5432:5432"

  mysql:
    image: mysql:8
    environment:
      MYSQL_ROOT_PASSWORD: test
    ports:
      - "3306:3306"

  clickhouse:
    image: clickhouse/clickhouse-server
    ports:
      - "8123:8123"

  minio:
    image: minio/minio
    command: server /data
    environment:
      MINIO_ROOT_USER: minioadmin
      MINIO_ROOT_PASSWORD: minioadmin
    ports:
      - "9000:9000"
```

## Step 0.3: Create Test Fixtures
**File:** `tests/common/fixtures.rs`
- Sample tables for each database type
- Test data insertions
- Cleanup functions

## Step 0.4: Create CI Configuration
**File:** `.github/workflows/test.yml`
- Run unit tests on every PR
- Run integration tests with Docker services
- Cache dependencies
