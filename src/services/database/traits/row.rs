//! Database-agnostic row and value types.
//!
//! This module contains:
//! - `Value` - A unified value type that can represent any database value
//! - `Cell` - A cell in a query result row
//! - `Row` - A row of cells from a query result
//! - `ColumnInfo` - Metadata about a column in a result set

#![allow(dead_code)]

use chrono::{DateTime, NaiveDate, NaiveDateTime, NaiveTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A unified value type that can represent any database value across all supported databases.
///
/// This enum provides a common representation for values from PostgreSQL, MySQL, SQLite,
/// ClickHouse, and DuckDB, enabling database-agnostic query result handling.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value")]
pub enum Value {
    /// NULL value
    Null,

    // Boolean
    /// Boolean value (true/false)
    Bool(bool),

    // Signed integers
    /// 8-bit signed integer
    Int8(i8),
    /// 16-bit signed integer
    Int16(i16),
    /// 32-bit signed integer
    Int32(i32),
    /// 64-bit signed integer
    Int64(i64),

    // Unsigned integers (common in MySQL, ClickHouse)
    /// 8-bit unsigned integer
    UInt8(u8),
    /// 16-bit unsigned integer
    UInt16(u16),
    /// 32-bit unsigned integer
    UInt32(u32),
    /// 64-bit unsigned integer
    UInt64(u64),

    // Floating point
    /// 32-bit floating point
    Float32(f32),
    /// 64-bit floating point
    Float64(f64),

    // Text and binary
    /// Text/string value
    Text(String),
    /// Binary data
    Bytes(Vec<u8>),

    // Temporal types
    /// Date without time
    Date(NaiveDate),
    /// Time without date
    Time(NaiveTime),
    /// Date and time without timezone
    DateTime(NaiveDateTime),
    /// Date and time with timezone (stored as UTC)
    DateTimeTz(DateTime<Utc>),

    // Special types
    /// Decimal/numeric with arbitrary precision
    Decimal(Decimal),
    /// UUID
    Uuid(Uuid),
    /// JSON value
    Json(serde_json::Value),

    // Complex types
    /// Array of values (same type)
    Array(Vec<Value>),

    /// Database-specific type that doesn't map to a standard type.
    /// Contains the type name and a string representation for display.
    Other {
        /// The database-specific type name
        type_name: String,
        /// String representation for display
        display: String,
    },
}

impl Value {
    /// Check if this value is NULL
    pub fn is_null(&self) -> bool {
        matches!(self, Value::Null)
    }

    /// Get the type name for display purposes
    pub fn type_name(&self) -> &'static str {
        match self {
            Value::Null => "null",
            Value::Bool(_) => "bool",
            Value::Int8(_) => "int8",
            Value::Int16(_) => "int16",
            Value::Int32(_) => "int32",
            Value::Int64(_) => "int64",
            Value::UInt8(_) => "uint8",
            Value::UInt16(_) => "uint16",
            Value::UInt32(_) => "uint32",
            Value::UInt64(_) => "uint64",
            Value::Float32(_) => "float32",
            Value::Float64(_) => "float64",
            Value::Text(_) => "text",
            Value::Bytes(_) => "bytes",
            Value::Date(_) => "date",
            Value::Time(_) => "time",
            Value::DateTime(_) => "datetime",
            Value::DateTimeTz(_) => "datetimetz",
            Value::Decimal(_) => "decimal",
            Value::Uuid(_) => "uuid",
            Value::Json(_) => "json",
            Value::Array(_) => "array",
            Value::Other { .. } => "other",
        }
    }

    /// Convert this value to a display string
    pub fn to_display_string(&self) -> String {
        match self {
            Value::Null => "NULL".to_string(),
            Value::Bool(b) => b.to_string(),
            Value::Int8(v) => v.to_string(),
            Value::Int16(v) => v.to_string(),
            Value::Int32(v) => v.to_string(),
            Value::Int64(v) => v.to_string(),
            Value::UInt8(v) => v.to_string(),
            Value::UInt16(v) => v.to_string(),
            Value::UInt32(v) => v.to_string(),
            Value::UInt64(v) => v.to_string(),
            Value::Float32(v) => v.to_string(),
            Value::Float64(v) => v.to_string(),
            Value::Text(s) => s.clone(),
            Value::Bytes(b) => format!("\\x{}", hex::encode(b)),
            Value::Date(d) => d.format("%Y-%m-%d").to_string(),
            Value::Time(t) => t.format("%H:%M:%S%.f").to_string(),
            Value::DateTime(dt) => dt.format("%Y-%m-%d %H:%M:%S%.f").to_string(),
            Value::DateTimeTz(dt) => dt.format("%Y-%m-%d %H:%M:%S%.f %Z").to_string(),
            Value::Decimal(d) => d.to_string(),
            Value::Uuid(u) => u.to_string(),
            Value::Json(j) => serde_json::to_string(j).unwrap_or_else(|_| "{}".to_string()),
            Value::Array(arr) => {
                let items: Vec<String> = arr.iter().map(|v| v.to_display_string()).collect();
                format!("[{}]", items.join(", "))
            }
            Value::Other { display, .. } => display.clone(),
        }
    }

    /// Try to extract as a boolean
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Value::Bool(b) => Some(*b),
            _ => None,
        }
    }

    /// Try to extract as an i64 (will convert smaller integers)
    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Value::Int8(v) => Some(*v as i64),
            Value::Int16(v) => Some(*v as i64),
            Value::Int32(v) => Some(*v as i64),
            Value::Int64(v) => Some(*v),
            _ => None,
        }
    }

    /// Try to extract as an f64 (will convert f32)
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Value::Float32(v) => Some(*v as f64),
            Value::Float64(v) => Some(*v),
            _ => None,
        }
    }

    /// Try to extract as a string reference
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::Text(s) => Some(s),
            _ => None,
        }
    }

    /// Try to extract as bytes reference
    pub fn as_bytes(&self) -> Option<&[u8]> {
        match self {
            Value::Bytes(b) => Some(b),
            _ => None,
        }
    }
}

impl std::fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_display_string())
    }
}

impl Default for Value {
    fn default() -> Self {
        Value::Null
    }
}

// Convenient From implementations
impl From<bool> for Value {
    fn from(v: bool) -> Self {
        Value::Bool(v)
    }
}

impl From<i32> for Value {
    fn from(v: i32) -> Self {
        Value::Int32(v)
    }
}

impl From<i64> for Value {
    fn from(v: i64) -> Self {
        Value::Int64(v)
    }
}

impl From<f64> for Value {
    fn from(v: f64) -> Self {
        Value::Float64(v)
    }
}

impl From<String> for Value {
    fn from(v: String) -> Self {
        Value::Text(v)
    }
}

impl From<&str> for Value {
    fn from(v: &str) -> Self {
        Value::Text(v.to_string())
    }
}

impl<T: Into<Value>> From<Option<T>> for Value {
    fn from(v: Option<T>) -> Self {
        match v {
            Some(val) => val.into(),
            None => Value::Null,
        }
    }
}

/// Metadata about a column in a query result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnInfo {
    /// Column name
    pub name: String,
    /// Database-specific type name
    pub type_name: String,
    /// Column position (0-indexed)
    pub ordinal: usize,
    /// Source table name (if available)
    pub table_name: Option<String>,
    /// Source schema name (if available)
    pub schema_name: Option<String>,
    /// Whether the column allows NULL values
    pub is_nullable: Option<bool>,
}

impl ColumnInfo {
    /// Create a new column info
    pub fn new(name: String, type_name: String, ordinal: usize) -> Self {
        Self {
            name,
            type_name,
            ordinal,
            table_name: None,
            schema_name: None,
            is_nullable: None,
        }
    }

    /// Set the table name
    pub fn with_table(mut self, table_name: String) -> Self {
        self.table_name = Some(table_name);
        self
    }

    /// Set the schema name
    pub fn with_schema(mut self, schema_name: String) -> Self {
        self.schema_name = Some(schema_name);
        self
    }

    /// Set the nullable flag
    pub fn with_nullable(mut self, is_nullable: bool) -> Self {
        self.is_nullable = Some(is_nullable);
        self
    }
}

/// A cell in a query result row
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cell {
    /// The value of this cell
    pub value: Value,
    /// The column index (0-indexed)
    pub column_index: usize,
}

impl Cell {
    /// Create a new cell
    pub fn new(value: Value, column_index: usize) -> Self {
        Self { value, column_index }
    }

    /// Check if this cell is NULL
    pub fn is_null(&self) -> bool {
        self.value.is_null()
    }

    /// Get the display string for this cell
    pub fn to_display_string(&self) -> String {
        self.value.to_display_string()
    }
}

/// A row of cells from a query result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Row {
    /// The cells in this row
    pub cells: Vec<Cell>,
}

impl Row {
    /// Create a new row from cells
    pub fn new(cells: Vec<Cell>) -> Self {
        Self { cells }
    }

    /// Create a row from values (auto-assigns column indices)
    pub fn from_values(values: Vec<Value>) -> Self {
        let cells = values
            .into_iter()
            .enumerate()
            .map(|(idx, value)| Cell::new(value, idx))
            .collect();
        Self { cells }
    }

    /// Get the number of cells in this row
    pub fn len(&self) -> usize {
        self.cells.len()
    }

    /// Check if this row is empty
    pub fn is_empty(&self) -> bool {
        self.cells.is_empty()
    }

    /// Get a cell by index
    pub fn get(&self, index: usize) -> Option<&Cell> {
        self.cells.get(index)
    }

    /// Get a value by index
    pub fn get_value(&self, index: usize) -> Option<&Value> {
        self.cells.get(index).map(|c| &c.value)
    }

    /// Iterate over cells
    pub fn iter(&self) -> impl Iterator<Item = &Cell> {
        self.cells.iter()
    }

    /// Iterate over values
    pub fn values(&self) -> impl Iterator<Item = &Value> {
        self.cells.iter().map(|c| &c.value)
    }
}

impl IntoIterator for Row {
    type Item = Cell;
    type IntoIter = std::vec::IntoIter<Cell>;

    fn into_iter(self) -> Self::IntoIter {
        self.cells.into_iter()
    }
}

impl<'a> IntoIterator for &'a Row {
    type Item = &'a Cell;
    type IntoIter = std::slice::Iter<'a, Cell>;

    fn into_iter(self) -> Self::IntoIter {
        self.cells.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_value_null_check() {
        assert!(Value::Null.is_null());
        assert!(!Value::Bool(true).is_null());
        assert!(!Value::Int32(42).is_null());
        assert!(!Value::Text("hello".to_string()).is_null());
    }

    #[test]
    fn test_value_display_string() {
        assert_eq!(Value::Null.to_display_string(), "NULL");
        assert_eq!(Value::Bool(true).to_display_string(), "true");
        assert_eq!(Value::Bool(false).to_display_string(), "false");
        assert_eq!(Value::Int32(42).to_display_string(), "42");
        assert_eq!(Value::Int64(-123).to_display_string(), "-123");
        assert_eq!(Value::Float64(3.14).to_display_string(), "3.14");
        assert_eq!(Value::Text("hello".to_string()).to_display_string(), "hello");
    }

    #[test]
    fn test_value_bytes_display() {
        let bytes = Value::Bytes(vec![0xDE, 0xAD, 0xBE, 0xEF]);
        assert_eq!(bytes.to_display_string(), "\\xdeadbeef");
    }

    #[test]
    fn test_value_array_display() {
        let arr = Value::Array(vec![
            Value::Int32(1),
            Value::Int32(2),
            Value::Int32(3),
        ]);
        assert_eq!(arr.to_display_string(), "[1, 2, 3]");
    }

    #[test]
    fn test_value_from_option() {
        let some_val: Value = Some(42i32).into();
        assert_eq!(some_val, Value::Int32(42));

        let none_val: Value = Option::<i32>::None.into();
        assert_eq!(none_val, Value::Null);
    }

    #[test]
    fn test_row_from_values() {
        let row = Row::from_values(vec![
            Value::Int32(1),
            Value::Text("hello".to_string()),
            Value::Bool(true),
        ]);

        assert_eq!(row.len(), 3);
        assert_eq!(row.get_value(0), Some(&Value::Int32(1)));
        assert_eq!(row.get_value(1), Some(&Value::Text("hello".to_string())));
        assert_eq!(row.get_value(2), Some(&Value::Bool(true)));
        assert_eq!(row.get_value(3), None);
    }

    #[test]
    fn test_value_type_names() {
        assert_eq!(Value::Null.type_name(), "null");
        assert_eq!(Value::Bool(true).type_name(), "bool");
        assert_eq!(Value::Int32(0).type_name(), "int32");
        assert_eq!(Value::Int64(0).type_name(), "int64");
        assert_eq!(Value::Float64(0.0).type_name(), "float64");
        assert_eq!(Value::Text(String::new()).type_name(), "text");
        assert_eq!(Value::Uuid(Uuid::nil()).type_name(), "uuid");
    }

    #[test]
    fn test_cell_creation() {
        let cell = Cell::new(Value::Int32(42), 0);
        assert!(!cell.is_null());
        assert_eq!(cell.to_display_string(), "42");

        let null_cell = Cell::new(Value::Null, 1);
        assert!(null_cell.is_null());
        assert_eq!(null_cell.to_display_string(), "NULL");
    }
}
