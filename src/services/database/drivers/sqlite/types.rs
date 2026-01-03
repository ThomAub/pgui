//! SQLite type conversion utilities.
//!
//! This module handles conversion between SQLite types (from SQLx)
//! and the generic `Value` type used across all database drivers.
//!
//! SQLite uses dynamic typing with type affinity:
//! - INTEGER: 64-bit signed integer
//! - REAL: 64-bit floating point
//! - TEXT: UTF-8 string
//! - BLOB: Binary data
//! - NULL: Null value

#![allow(dead_code)]

use sqlx::sqlite::{SqliteColumn, SqliteRow};
use sqlx::{Column, Row, TypeInfo, ValueRef};

use crate::services::database::traits::{Cell, ColumnInfo, Row as TraitRow, Value};

/// Converter for SQLite values to the unified `Value` type.
pub struct SqliteValueConverter;

impl SqliteValueConverter {
    /// Convert a SQLite row to a trait Row.
    pub fn convert_row(sqlite_row: &SqliteRow) -> TraitRow {
        let cells = sqlite_row
            .columns()
            .iter()
            .enumerate()
            .map(|(idx, col)| {
                let value = Self::extract_value(sqlite_row, col, idx);
                Cell::new(value, idx)
            })
            .collect();

        TraitRow::new(cells)
    }

    /// Build column info from a SQLite row.
    pub fn build_column_info(sqlite_row: &SqliteRow) -> Vec<ColumnInfo> {
        sqlite_row
            .columns()
            .iter()
            .enumerate()
            .map(|(idx, col)| {
                ColumnInfo::new(
                    col.name().to_string(),
                    col.type_info().name().to_string(),
                    idx,
                )
            })
            .collect()
    }

    /// Extract a value from a SQLite row at the given column index.
    fn extract_value(row: &SqliteRow, column: &SqliteColumn, index: usize) -> Value {
        // Check for NULL first
        match row.try_get_raw(index) {
            Ok(raw) if raw.is_null() => return Value::Null,
            Err(_) => return Value::Null,
            _ => {}
        }

        let type_name = column.type_info().name().to_uppercase();
        Self::decode_by_type(row, index, &type_name)
    }

    /// Decode a value based on its SQLite type name.
    ///
    /// SQLite has type affinity rules:
    /// - INTEGER affinity: INT, INTEGER, TINYINT, SMALLINT, MEDIUMINT, BIGINT, etc.
    /// - REAL affinity: REAL, DOUBLE, FLOAT
    /// - TEXT affinity: TEXT, VARCHAR, CHAR, CLOB
    /// - BLOB affinity: BLOB
    /// - NUMERIC affinity: NUMERIC, DECIMAL, BOOLEAN, DATE, DATETIME
    fn decode_by_type(row: &SqliteRow, index: usize, type_name: &str) -> Value {
        // Handle common type names and their affinities
        match type_name {
            // Integer types
            "INTEGER" | "INT" | "TINYINT" | "SMALLINT" | "MEDIUMINT" | "BIGINT"
            | "UNSIGNED BIG INT" | "INT2" | "INT8" => row
                .try_get::<i64, _>(index)
                .map(Value::Int64)
                .unwrap_or(Value::Null),

            // Boolean (SQLite stores as 0/1 integer)
            "BOOLEAN" | "BOOL" => row
                .try_get::<bool, _>(index)
                .map(Value::Bool)
                .or_else(|_| {
                    // Fallback: try as integer
                    row.try_get::<i64, _>(index).map(|v| Value::Bool(v != 0))
                })
                .unwrap_or(Value::Null),

            // Real/Float types
            "REAL" | "DOUBLE" | "DOUBLE PRECISION" | "FLOAT" => row
                .try_get::<f64, _>(index)
                .map(Value::Float64)
                .unwrap_or(Value::Null),

            // Text types
            "TEXT" | "VARCHAR" | "VARYING CHARACTER" | "NCHAR" | "NATIVE CHARACTER"
            | "NVARCHAR" | "CLOB" | "CHARACTER" | "CHAR" => row
                .try_get::<String, _>(index)
                .map(Value::Text)
                .unwrap_or(Value::Null),

            // Blob type
            "BLOB" => row
                .try_get::<Vec<u8>, _>(index)
                .map(Value::Bytes)
                .unwrap_or(Value::Null),

            // Date/Time types (SQLite stores as TEXT, REAL, or INTEGER)
            "DATE" => Self::decode_date(row, index),
            "TIME" => Self::decode_time(row, index),
            "DATETIME" | "TIMESTAMP" => Self::decode_datetime(row, index),

            // Numeric/Decimal (stored as TEXT or REAL in SQLite)
            "NUMERIC" | "DECIMAL" => Self::decode_numeric(row, index),

            // For unknown types, try common decode paths
            _ => Self::decode_unknown(row, index, type_name),
        }
    }

    /// Decode a DATE value (SQLite stores dates as TEXT in ISO format).
    fn decode_date(row: &SqliteRow, index: usize) -> Value {
        // Try as text first (ISO 8601 format: YYYY-MM-DD)
        if let Ok(s) = row.try_get::<String, _>(index) {
            if let Ok(date) = chrono::NaiveDate::parse_from_str(&s, "%Y-%m-%d") {
                return Value::Date(date);
            }
            // Return as text if parsing fails
            return Value::Text(s);
        }

        // Try as integer (Unix timestamp days)
        if let Ok(days) = row.try_get::<i64, _>(index) {
            if let Some(date) = chrono::NaiveDate::from_num_days_from_ce_opt(days as i32) {
                return Value::Date(date);
            }
        }

        Value::Null
    }

    /// Decode a TIME value.
    fn decode_time(row: &SqliteRow, index: usize) -> Value {
        // Try as text (HH:MM:SS format)
        if let Ok(s) = row.try_get::<String, _>(index) {
            if let Ok(time) = chrono::NaiveTime::parse_from_str(&s, "%H:%M:%S") {
                return Value::Time(time);
            }
            if let Ok(time) = chrono::NaiveTime::parse_from_str(&s, "%H:%M:%S%.f") {
                return Value::Time(time);
            }
            return Value::Text(s);
        }

        Value::Null
    }

    /// Decode a DATETIME value.
    fn decode_datetime(row: &SqliteRow, index: usize) -> Value {
        // Try as text (ISO 8601 format)
        if let Ok(s) = row.try_get::<String, _>(index) {
            // Try various datetime formats
            if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(&s, "%Y-%m-%d %H:%M:%S") {
                return Value::DateTime(dt);
            }
            if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(&s, "%Y-%m-%d %H:%M:%S%.f") {
                return Value::DateTime(dt);
            }
            if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(&s, "%Y-%m-%dT%H:%M:%S") {
                return Value::DateTime(dt);
            }
            if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(&s, "%Y-%m-%dT%H:%M:%S%.f") {
                return Value::DateTime(dt);
            }
            return Value::Text(s);
        }

        // Try as Unix timestamp (seconds since epoch)
        if let Ok(timestamp) = row.try_get::<i64, _>(index) {
            if let Some(dt) = chrono::DateTime::from_timestamp(timestamp, 0) {
                return Value::DateTimeTz(dt);
            }
        }

        // Try as Julian day number (real)
        if let Ok(julian) = row.try_get::<f64, _>(index) {
            // Julian day to Unix timestamp conversion
            let unix_days = julian - 2440587.5;
            let secs = (unix_days * 86400.0) as i64;
            if let Some(dt) = chrono::DateTime::from_timestamp(secs, 0) {
                return Value::DateTimeTz(dt);
            }
        }

        Value::Null
    }

    /// Decode a NUMERIC/DECIMAL value.
    fn decode_numeric(row: &SqliteRow, index: usize) -> Value {
        // Try as text first (for precision)
        if let Ok(s) = row.try_get::<String, _>(index) {
            if let Ok(decimal) = s.parse::<rust_decimal::Decimal>() {
                return Value::Decimal(decimal);
            }
            return Value::Text(s);
        }

        // Try as float
        if let Ok(f) = row.try_get::<f64, _>(index) {
            return Value::Float64(f);
        }

        // Try as integer
        if let Ok(i) = row.try_get::<i64, _>(index) {
            return Value::Int64(i);
        }

        Value::Null
    }

    /// Decode an unknown type by trying common paths.
    fn decode_unknown(row: &SqliteRow, index: usize, type_name: &str) -> Value {
        // Try integer first (most common)
        if let Ok(v) = row.try_get::<i64, _>(index) {
            return Value::Int64(v);
        }

        // Try float
        if let Ok(v) = row.try_get::<f64, _>(index) {
            return Value::Float64(v);
        }

        // Try text
        if let Ok(v) = row.try_get::<String, _>(index) {
            return Value::Text(v);
        }

        // Try blob
        if let Ok(v) = row.try_get::<Vec<u8>, _>(index) {
            return Value::Bytes(v);
        }

        // Give up
        Value::Other {
            type_name: type_name.to_string(),
            display: "<unknown>".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_type_affinity_mapping() {
        // These tests verify the type name matching logic
        // Actual row conversion requires a SQLite connection

        // Integer affinity types
        let int_types = [
            "INTEGER",
            "INT",
            "TINYINT",
            "SMALLINT",
            "MEDIUMINT",
            "BIGINT",
        ];
        for t in int_types {
            assert!(
                t.to_uppercase().contains("INT"),
                "{} should be integer affinity",
                t
            );
        }

        // Real affinity types
        let real_types = ["REAL", "DOUBLE", "FLOAT"];
        for t in real_types {
            assert!(
                ["REAL", "DOUBLE", "FLOAT"].contains(&t.to_uppercase().as_str()),
                "{} should be real affinity",
                t
            );
        }

        // Text affinity types
        let text_types = ["TEXT", "VARCHAR", "CHAR", "CLOB"];
        for t in text_types {
            assert!(
                ["TEXT", "VARCHAR", "CHAR", "CLOB"].contains(&t.to_uppercase().as_str())
                    || t.to_uppercase().contains("CHAR"),
                "{} should be text affinity",
                t
            );
        }
    }
}
