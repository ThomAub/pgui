//! ClickHouse connection implementation.
//!
//! This module implements the `DatabaseConnection` trait for ClickHouse
//! using HTTP-based queries with JSON format for dynamic result handling.
//!
//! ClickHouse supports multiple interfaces:
//! - HTTP (default port 8123) - used here for simplicity and JSON support
//! - Native TCP (port 9000) - higher performance but binary protocol
//! - MySQL protocol (port 9004) - for MySQL compatibility

#![allow(dead_code)]

use anyhow::{anyhow, Result};
use async_lock::RwLock;
use async_trait::async_trait;
use futures::stream::BoxStream;
use serde_json::Value as JsonValue;

use super::types::ClickHouseValueConverter;
use crate::services::database::traits::{
    BoxedConnection, Cell, ColumnInfo, ConnectionConfig, ConnectionParams, DatabaseConnection,
    DatabaseType, ErrorResult, ModifiedResult, QueryExecutionResult, Row, SelectResult,
};

/// ClickHouse database connection.
///
/// Uses HTTP interface for flexibility with dynamic query results.
/// Queries are executed with JSONEachRow format to support unknown schemas.
pub struct ClickHouseConnection {
    config: ConnectionConfig,
    /// HTTP client configuration
    client: RwLock<Option<ClickHouseClient>>,
}

/// Internal HTTP client configuration for ClickHouse
#[derive(Clone)]
struct ClickHouseClient {
    base_url: String,
    username: String,
    password: String,
    database: String,
}

impl std::fmt::Debug for ClickHouseConnection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ClickHouseConnection")
            .field("config", &self.config)
            .field("client", &"<ClickHouseClient>")
            .finish()
    }
}

impl ClickHouseConnection {
    /// Create a new ClickHouse connection from configuration.
    ///
    /// This does not connect immediately - call `connect()` to establish the connection.
    pub fn new(config: ConnectionConfig) -> Self {
        Self {
            config,
            client: RwLock::new(None),
        }
    }

    /// Create a boxed connection (for factory use).
    pub fn boxed(config: ConnectionConfig) -> BoxedConnection {
        Box::new(Self::new(config))
    }

    /// Build the HTTP client from configuration.
    fn build_client(&self) -> Result<ClickHouseClient> {
        match &self.config.params {
            ConnectionParams::Server {
                hostname,
                port,
                username,
                password,
                database,
                ssl_mode,
                ..
            } => {
                let (use_https, _verify) = ClickHouseValueConverter::map_ssl_mode(ssl_mode);
                let scheme = if use_https { "https" } else { "http" };
                let base_url = format!("{}://{}:{}", scheme, hostname, port);

                Ok(ClickHouseClient {
                    base_url,
                    username: username.clone(),
                    password: password.clone(),
                    database: database.clone(),
                })
            }
            ConnectionParams::File { .. } | ConnectionParams::InMemory { .. } => Err(anyhow!(
                "ClickHouse does not support file-based or in-memory connections"
            )),
        }
    }

    /// Get the HTTP client, returning an error if not connected.
    async fn get_client(&self) -> Result<ClickHouseClient> {
        let guard = self.client.read().await;
        guard
            .clone()
            .ok_or_else(|| anyhow!("Database not connected"))
    }

    /// Get a reference to the client (internal helper for schema module).
    pub(crate) async fn get_client_internal(&self) -> Result<(String, String, String, String)> {
        let client = self.get_client().await?;
        Ok((
            client.base_url,
            client.username,
            client.password,
            client.database,
        ))
    }

    /// Execute a query and return raw response.
    async fn execute_query_http(&self, sql: &str, format: &str) -> Result<String> {
        let client = self.get_client().await?;

        // Build the URL with query parameters
        let url = format!(
            "{}/?database={}&default_format={}",
            client.base_url,
            urlencoding::encode(&client.database),
            format
        );

        let auth_header = build_auth_header(&client.username, &client.password);
        let sql_body = sql.to_string();

        // Use smol::unblock for sync HTTP client
        let body = smol::unblock(move || {
            let response = smolhttp::Client::new(&url)
                .map_err(|e| anyhow!("Failed to create HTTP client: {}", e))?
                .post()
                .headers(vec![
                    ("Authorization".to_string(), auth_header),
                    ("Content-Type".to_string(), "text/plain".to_string()),
                ])
                .body(sql_body.into_bytes())
                .send()
                .map_err(|e| anyhow!("HTTP request failed: {}", e))?;

            Ok::<String, anyhow::Error>(response.text())
        })
        .await?;

        // Check for ClickHouse error responses
        if body.contains("Code:") && body.contains("DB::Exception") {
            return Err(anyhow!("ClickHouse error: {}", body.trim()));
        }

        Ok(body)
    }

    /// Check if the query is a SELECT-type query.
    fn is_select_query(sql: &str) -> bool {
        let lower = sql.to_lowercase();
        let trimmed = lower.trim_start();
        trimmed.starts_with("select")
            || trimmed.starts_with("with")
            || trimmed.starts_with("show")
            || trimmed.starts_with("describe")
            || trimmed.starts_with("explain")
    }

    /// Execute a SELECT query.
    async fn execute_select(&self, sql: &str) -> QueryExecutionResult {
        let start_time = std::time::Instant::now();
        let original_query = sql.to_string();

        // Add LIMIT if not present to prevent massive result sets
        let limited_sql = if !sql.to_lowercase().contains(" limit ")
            && !sql.to_lowercase().trim_start().starts_with("show")
            && !sql.to_lowercase().trim_start().starts_with("describe")
            && !sql.to_lowercase().trim_start().starts_with("explain")
        {
            format!("{} LIMIT {}", sql.trim_end_matches(';'), 1_000)
        } else {
            sql.to_string()
        };

        // Use JSONCompact format which includes metadata
        match self.execute_query_http(&limited_sql, "JSONCompact").await {
            Ok(response) => {
                let execution_time_ms = start_time.elapsed().as_millis();

                // Parse JSON response
                match serde_json::from_str::<JsonValue>(&response) {
                    Ok(json) => {
                        let (columns, rows) = Self::parse_json_compact_response(&json);
                        QueryExecutionResult::Select(SelectResult::new(
                            columns,
                            rows,
                            execution_time_ms,
                            original_query,
                        ))
                    }
                    Err(e) => QueryExecutionResult::Error(ErrorResult::new(
                        format!("Failed to parse response: {}", e),
                        execution_time_ms,
                    )),
                }
            }
            Err(e) => {
                let execution_time_ms = start_time.elapsed().as_millis();
                QueryExecutionResult::Error(ErrorResult::new(
                    format!("Query failed: {}", e),
                    execution_time_ms,
                ))
            }
        }
    }

    /// Parse JSONCompact format response.
    fn parse_json_compact_response(json: &JsonValue) -> (Vec<ColumnInfo>, Vec<Row>) {
        let columns = if let Some(meta) = json.get("meta").and_then(|m| m.as_array()) {
            meta.iter()
                .enumerate()
                .filter_map(|(idx, m)| {
                    let name = m.get("name")?.as_str()?.to_string();
                    let type_name = m.get("type")?.as_str()?.to_string();
                    Some(ColumnInfo::new(name, type_name, idx))
                })
                .collect()
        } else {
            vec![]
        };

        let rows = if let Some(data) = json.get("data").and_then(|d| d.as_array()) {
            data.iter()
                .map(|row| {
                    if let Some(row_arr) = row.as_array() {
                        let cells: Vec<Cell> = row_arr
                            .iter()
                            .enumerate()
                            .map(|(idx, val)| {
                                let type_name = columns
                                    .get(idx)
                                    .map(|c| c.type_name.as_str())
                                    .unwrap_or("String");
                                let value =
                                    ClickHouseValueConverter::json_to_value(val, type_name);
                                Cell::new(value, idx)
                            })
                            .collect();
                        Row::new(cells)
                    } else {
                        Row::new(vec![])
                    }
                })
                .collect()
        } else {
            vec![]
        };

        (columns, rows)
    }

    /// Execute a modification query (INSERT, ALTER, DROP, etc.).
    async fn execute_modification(&self, sql: &str) -> QueryExecutionResult {
        let start_time = std::time::Instant::now();

        match self.execute_query_http(sql, "JSONCompact").await {
            Ok(_) => {
                let execution_time_ms = start_time.elapsed().as_millis();
                // ClickHouse doesn't return rows affected for most DDL
                QueryExecutionResult::Modified(ModifiedResult::new(0, execution_time_ms))
            }
            Err(e) => {
                let execution_time_ms = start_time.elapsed().as_millis();
                QueryExecutionResult::Error(ErrorResult::new(
                    format!("Query failed: {}", e),
                    execution_time_ms,
                ))
            }
        }
    }
}

/// Build the HTTP Basic Auth header
fn build_auth_header(username: &str, password: &str) -> String {
    let auth = format!("{}:{}", username, password);
    format!("Basic {}", base64_encode(&auth))
}

/// Simple base64 encoding for auth header
fn base64_encode(input: &str) -> String {
    use std::io::Write;
    let mut output = Vec::new();
    let mut encoder = Base64Encoder {
        output: &mut output,
        buffer: 0,
        bits: 0,
    };
    encoder.write_all(input.as_bytes()).unwrap();
    drop(encoder);
    String::from_utf8(output).unwrap()
}

struct Base64Encoder<'a> {
    output: &'a mut Vec<u8>,
    buffer: u32,
    bits: u8,
}

impl<'a> std::io::Write for Base64Encoder<'a> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        const ALPHABET: &[u8] =
            b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

        for &byte in buf {
            self.buffer = (self.buffer << 8) | byte as u32;
            self.bits += 8;

            while self.bits >= 6 {
                self.bits -= 6;
                let idx = ((self.buffer >> self.bits) & 0x3F) as usize;
                self.output.push(ALPHABET[idx]);
            }
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        if self.bits > 0 {
            const ALPHABET: &[u8] =
                b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
            let idx = ((self.buffer << (6 - self.bits)) & 0x3F) as usize;
            self.output.push(ALPHABET[idx]);

            // Add padding
            let padding = (3 - (self.bits / 8 + 1) % 3) % 3;
            for _ in 0..padding {
                self.output.push(b'=');
            }
        }
        Ok(())
    }
}

impl<'a> Drop for Base64Encoder<'a> {
    fn drop(&mut self) {
        let _ = std::io::Write::flush(self);
    }
}

/// URL encoding helper
mod urlencoding {
    pub fn encode(s: &str) -> String {
        let mut result = String::with_capacity(s.len() * 3);
        for byte in s.bytes() {
            match byte {
                b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                    result.push(byte as char);
                }
                _ => {
                    result.push('%');
                    result.push_str(&format!("{:02X}", byte));
                }
            }
        }
        result
    }
}

#[async_trait]
impl DatabaseConnection for ClickHouseConnection {
    fn database_type(&self) -> DatabaseType {
        DatabaseType::ClickHouse
    }

    fn connection_config(&self) -> &ConnectionConfig {
        &self.config
    }

    async fn connect(&mut self) -> Result<()> {
        let client = self.build_client()?;

        // Test connection with a simple query
        let test_url = format!(
            "{}/?database={}&query=SELECT%201",
            client.base_url,
            urlencoding::encode(&client.database)
        );

        let auth_header = build_auth_header(&client.username, &client.password);

        let body = smol::unblock(move || {
            let response = smolhttp::Client::new(&test_url)
                .map_err(|e| anyhow!("Failed to create HTTP client: {}", e))?
                .get()
                .headers(vec![("Authorization".to_string(), auth_header)])
                .send()
                .map_err(|e| anyhow!("Connection failed: {}", e))?;

            Ok::<String, anyhow::Error>(response.text())
        })
        .await?;

        // Check for error response
        if body.contains("Code:") && body.contains("DB::Exception") {
            return Err(anyhow!("Failed to connect to ClickHouse: {}", body.trim()));
        }

        // Store the client config (rebuild because we moved it)
        let client = self.build_client()?;
        let mut guard = self.client.write().await;
        *guard = Some(client);

        Ok(())
    }

    async fn disconnect(&mut self) -> Result<()> {
        let mut guard = self.client.write().await;
        if guard.take().is_some() {
            Ok(())
        } else {
            Err(anyhow!("No active database connection to disconnect"))
        }
    }

    async fn is_connected(&self) -> bool {
        let guard = self.client.read().await;
        if let Some(client) = guard.as_ref() {
            // Verify connection is still alive
            let test_url = format!(
                "{}/?database={}&query=SELECT%201",
                client.base_url,
                urlencoding::encode(&client.database)
            );

            let auth_header = build_auth_header(&client.username, &client.password);

            if let Some(body) = smol::unblock(move || {
                let response = smolhttp::Client::new(&test_url)
                    .ok()?
                    .get()
                    .headers(vec![("Authorization".to_string(), auth_header)])
                    .send()
                    .ok()?;
                Some(response.text())
            })
            .await
            {
                // Check for success (body should be "1\n" for SELECT 1)
                return !body.contains("DB::Exception");
            }
        }
        false
    }

    async fn execute_query(&self, sql: &str) -> Result<QueryExecutionResult> {
        let _client = self.get_client().await?;

        let sql = sql.trim();
        if sql.is_empty() {
            return Ok(QueryExecutionResult::Error(ErrorResult::new(
                "Empty query".to_string(),
                0,
            )));
        }

        if Self::is_select_query(sql) {
            Ok(self.execute_select(sql).await)
        } else {
            Ok(self.execute_modification(sql).await)
        }
    }

    async fn stream_query<'a>(&'a self, sql: &'a str) -> Result<BoxStream<'a, Result<Row>>> {
        let client = self.get_client().await?;

        // For streaming, use JSONEachRow format
        let url = format!(
            "{}/?database={}&default_format=JSONEachRow",
            client.base_url,
            urlencoding::encode(&client.database)
        );

        let auth_header = build_auth_header(&client.username, &client.password);
        let sql_body = sql.to_string();

        let body = smol::unblock(move || {
            let response = smolhttp::Client::new(&url)
                .map_err(|e| anyhow!("Failed to create HTTP client: {}", e))?
                .post()
                .headers(vec![
                    ("Authorization".to_string(), auth_header),
                    ("Content-Type".to_string(), "text/plain".to_string()),
                ])
                .body(sql_body.into_bytes())
                .send()
                .map_err(|e| anyhow!("HTTP request failed: {}", e))?;

            Ok::<String, anyhow::Error>(response.text())
        })
        .await?;

        // Check for ClickHouse error responses
        if body.contains("Code:") && body.contains("DB::Exception") {
            return Err(anyhow!("Query failed: {}", body.trim()));
        }

        // Parse each line as a JSON object
        let rows: Vec<Result<Row>> = body
            .lines()
            .filter(|line| !line.trim().is_empty())
            .map(|line| {
                let json: JsonValue = serde_json::from_str(line)?;
                if let JsonValue::Object(obj) = json {
                    let cells: Vec<Cell> = obj
                        .iter()
                        .enumerate()
                        .map(|(idx, (_key, val))| {
                            let value = ClickHouseValueConverter::json_to_value(val, "String");
                            Cell::new(value, idx)
                        })
                        .collect();
                    Ok(Row::new(cells))
                } else {
                    Ok(Row::new(vec![]))
                }
            })
            .collect();

        Ok(Box::pin(futures::stream::iter(rows)))
    }

    async fn test_connection(config: &ConnectionConfig) -> Result<()> {
        let (base_url, username, password, database) = match &config.params {
            ConnectionParams::Server {
                hostname,
                port,
                username,
                password,
                database,
                ssl_mode,
                ..
            } => {
                let (use_https, _verify) = ClickHouseValueConverter::map_ssl_mode(ssl_mode);
                let scheme = if use_https { "https" } else { "http" };
                let base_url = format!("{}://{}:{}", scheme, hostname, port);
                (
                    base_url,
                    username.clone(),
                    password.clone(),
                    database.clone(),
                )
            }
            _ => return Err(anyhow!("ClickHouse requires server connection parameters")),
        };

        let test_url = format!(
            "{}/?database={}&query=SELECT%201",
            base_url,
            urlencoding::encode(&database)
        );

        let auth_header = build_auth_header(&username, &password);

        let body = smol::unblock(move || {
            let response = smolhttp::Client::new(&test_url)
                .map_err(|e| anyhow!("Failed to create HTTP client: {}", e))?
                .get()
                .headers(vec![("Authorization".to_string(), auth_header)])
                .send()
                .map_err(|e| anyhow!("Connection test failed: {}", e))?;

            Ok::<String, anyhow::Error>(response.text())
        })
        .await?;

        // Check for error response
        if body.contains("Code:") && body.contains("DB::Exception") {
            return Err(anyhow!("Connection test failed: {}", body.trim()));
        }

        Ok(())
    }
}

// Ensure ClickHouseConnection can be sent between threads
unsafe impl Send for ClickHouseConnection {}
unsafe impl Sync for ClickHouseConnection {}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_config() -> ConnectionConfig {
        ConnectionConfig::new(
            "test".to_string(),
            DatabaseType::ClickHouse,
            ConnectionParams::server(
                "localhost".to_string(),
                8123,
                "default".to_string(),
                "".to_string(),
                "default".to_string(),
            ),
        )
    }

    #[test]
    fn test_clickhouse_connection_new() {
        let config = create_test_config();
        let conn = ClickHouseConnection::new(config.clone());

        assert_eq!(conn.database_type(), DatabaseType::ClickHouse);
        assert_eq!(conn.connection_config().name, "test");
    }

    #[test]
    fn test_is_select_query() {
        assert!(ClickHouseConnection::is_select_query("SELECT * FROM users"));
        assert!(ClickHouseConnection::is_select_query("select * from users"));
        assert!(ClickHouseConnection::is_select_query("  SELECT * FROM users"));
        assert!(ClickHouseConnection::is_select_query(
            "WITH cte AS (SELECT 1) SELECT * FROM cte"
        ));
        assert!(ClickHouseConnection::is_select_query("SHOW TABLES"));
        assert!(ClickHouseConnection::is_select_query("DESCRIBE users"));
        assert!(ClickHouseConnection::is_select_query(
            "EXPLAIN SELECT * FROM users"
        ));

        assert!(!ClickHouseConnection::is_select_query(
            "INSERT INTO users VALUES (1)"
        ));
        assert!(!ClickHouseConnection::is_select_query(
            "ALTER TABLE users ADD COLUMN x Int32"
        ));
        assert!(!ClickHouseConnection::is_select_query("DROP TABLE users"));
    }

    #[test]
    fn test_build_client() {
        let config = create_test_config();
        let conn = ClickHouseConnection::new(config);

        let result = conn.build_client();
        assert!(result.is_ok());

        let client = result.unwrap();
        assert_eq!(client.base_url, "http://localhost:8123");
        assert_eq!(client.username, "default");
        assert_eq!(client.database, "default");
    }

    #[test]
    fn test_file_params_rejected() {
        let config = ConnectionConfig::new(
            "test".to_string(),
            DatabaseType::ClickHouse,
            ConnectionParams::file(std::path::PathBuf::from("/tmp/test.db"), false),
        );
        let conn = ClickHouseConnection::new(config);

        let result = conn.build_client();
        assert!(result.is_err());
    }

    #[test]
    fn test_base64_encode() {
        assert_eq!(base64_encode(""), "");
        assert_eq!(base64_encode("f"), "Zg==");
        assert_eq!(base64_encode("fo"), "Zm8=");
        assert_eq!(base64_encode("foo"), "Zm9v");
        assert_eq!(base64_encode("user:pass"), "dXNlcjpwYXNz");
    }

    #[test]
    fn test_url_encoding() {
        assert_eq!(urlencoding::encode("hello"), "hello");
        assert_eq!(urlencoding::encode("hello world"), "hello%20world");
        assert_eq!(urlencoding::encode("foo=bar&baz"), "foo%3Dbar%26baz");
    }

    #[test]
    fn test_parse_json_compact_response() {
        let json = serde_json::json!({
            "meta": [
                {"name": "id", "type": "Int32"},
                {"name": "name", "type": "String"}
            ],
            "data": [
                [1, "Alice"],
                [2, "Bob"]
            ],
            "rows": 2
        });

        let (columns, rows) = ClickHouseConnection::parse_json_compact_response(&json);

        assert_eq!(columns.len(), 2);
        assert_eq!(columns[0].name, "id");
        assert_eq!(columns[0].type_name, "Int32");
        assert_eq!(columns[1].name, "name");
        assert_eq!(columns[1].type_name, "String");

        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].len(), 2);
    }
}
