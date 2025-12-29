//! MySQL type conversion utilities.
//!
//! This module handles conversion between MySQL-specific types (from SQLx)
//! and the generic `Value` type used across all database drivers.

use chrono::{DateTime, NaiveDate, NaiveDateTime, NaiveTime, Utc};
use rust_decimal::Decimal;
use sqlx::mysql::{MySqlColumn, MySqlRow, MySqlTypeInfo};
use sqlx::{Column, Row, TypeInfo, ValueRef};

use crate::services::database::traits::{Cell, ColumnInfo, Row as TraitRow, Value};

/// Converter for MySQL values to the unified `Value` type.
pub struct MySqlValueConverter;

impl MySqlValueConverter {
    /// Convert a MySQL row to a trait Row.
    pub fn convert_row(mysql_row: &MySqlRow) -> TraitRow {
        let cells = mysql_row
            .columns()
            .iter()
            .enumerate()
            .map(|(idx, col)| {
                let value = Self::extract_value(mysql_row, col, idx);
                Cell::new(value, idx)
            })
            .collect();

        TraitRow::new(cells)
    }

    /// Build column info from a MySQL row.
    pub fn build_column_info(mysql_row: &MySqlRow) -> Vec<ColumnInfo> {
        mysql_row
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

    /// Extract a value from a MySQL row at the given column index.
    fn extract_value(row: &MySqlRow, column: &MySqlColumn, index: usize) -> Value {
        // Check for NULL first
        match row.try_get_raw(index) {
            Ok(raw) if raw.is_null() => return Value::Null,
            Err(_) => return Value::Null,
            _ => {}
        }

        let type_name = column.type_info().name();
        Self::decode_by_type(row, index, type_name)
    }

    /// Decode a value based on its MySQL type name.
    fn decode_by_type(row: &MySqlRow, index: usize, type_name: &str) -> Value {
        match type_name {
            // Boolean (MySQL uses TINYINT(1) for booleans)
            "BOOLEAN" | "BOOL" => row
                .try_get::<bool, _>(index)
                .map(Value::Bool)
                .unwrap_or(Value::Null),

            // Integers
            "TINYINT" => row
                .try_get::<i8, _>(index)
                .map(|v| Value::Int16(v as i16))
                .unwrap_or(Value::Null),

            "TINYINT UNSIGNED" => row
                .try_get::<u8, _>(index)
                .map(|v| Value::Int16(v as i16))
                .unwrap_or(Value::Null),

            "SMALLINT" => row
                .try_get::<i16, _>(index)
                .map(Value::Int16)
                .unwrap_or(Value::Null),

            "SMALLINT UNSIGNED" => row
                .try_get::<u16, _>(index)
                .map(|v| Value::Int32(v as i32))
                .unwrap_or(Value::Null),

            "MEDIUMINT" | "INT" | "INTEGER" => row
                .try_get::<i32, _>(index)
                .map(Value::Int32)
                .unwrap_or(Value::Null),

            "MEDIUMINT UNSIGNED" | "INT UNSIGNED" | "INTEGER UNSIGNED" => row
                .try_get::<u32, _>(index)
                .map(|v| Value::Int64(v as i64))
                .unwrap_or(Value::Null),

            "BIGINT" => row
                .try_get::<i64, _>(index)
                .map(Value::Int64)
                .unwrap_or(Value::Null),

            "BIGINT UNSIGNED" => row
                .try_get::<u64, _>(index)
                .map(|v| Value::Other {
                    type_name: "BIGINT UNSIGNED".to_string(),
                    display: v.to_string(),
                })
                .unwrap_or(Value::Null),

            // Floating point
            "FLOAT" => row
                .try_get::<f32, _>(index)
                .map(Value::Float32)
                .unwrap_or(Value::Null),

            "DOUBLE" | "DOUBLE PRECISION" | "REAL" => row
                .try_get::<f64, _>(index)
                .map(Value::Float64)
                .unwrap_or(Value::Null),

            // Numeric/Decimal
            "DECIMAL" | "NUMERIC" | "DEC" | "FIXED" => row
                .try_get::<Decimal, _>(index)
                .map(Value::Decimal)
                .unwrap_or(Value::Null),

            // Text types
            "CHAR" | "VARCHAR" | "TINYTEXT" | "TEXT" | "MEDIUMTEXT" | "LONGTEXT" => row
                .try_get::<String, _>(index)
                .map(Value::Text)
                .unwrap_or(Value::Null),

            // Binary types
            "BINARY" | "VARBINARY" | "TINYBLOB" | "BLOB" | "MEDIUMBLOB" | "LONGBLOB" => row
                .try_get::<Vec<u8>, _>(index)
                .map(Value::Bytes)
                .unwrap_or(Value::Null),

            // Date/Time types
            "DATE" => row
                .try_get::<NaiveDate, _>(index)
                .map(Value::Date)
                .unwrap_or(Value::Null),

            "TIME" => row
                .try_get::<NaiveTime, _>(index)
                .map(Value::Time)
                .unwrap_or(Value::Null),

            "DATETIME" => row
                .try_get::<NaiveDateTime, _>(index)
                .map(Value::DateTime)
                .unwrap_or(Value::Null),

            "TIMESTAMP" => row
                .try_get::<DateTime<Utc>, _>(index)
                .map(Value::DateTimeTz)
                .or_else(|_| {
                    row.try_get::<NaiveDateTime, _>(index)
                        .map(Value::DateTime)
                })
                .unwrap_or(Value::Null),

            "YEAR" => row
                .try_get::<i16, _>(index)
                .map(Value::Int16)
                .unwrap_or(Value::Null),

            // JSON type
            "JSON" => row
                .try_get::<serde_json::Value, _>(index)
                .map(Value::Json)
                .unwrap_or(Value::Null),

            // ENUM and SET types - treat as strings
            _ if type_name.starts_with("ENUM") => row
                .try_get::<String, _>(index)
                .map(Value::Text)
                .unwrap_or(Value::Null),

            _ if type_name.starts_with("SET") => row
                .try_get::<String, _>(index)
                .map(Value::Text)
                .unwrap_or(Value::Null),

            // For unknown types, try to get as string representation
            _ => Self::decode_as_string_fallback(row, index, type_name),
        }
    }

    /// Fallback: try to decode as string representation for unknown types.
    fn decode_as_string_fallback(row: &MySqlRow, index: usize, type_name: &str) -> Value {
        // Try to get as string first
        if let Ok(s) = row.try_get::<String, _>(index) {
            return Value::Other {
                type_name: type_name.to_string(),
                display: s,
            };
        }

        // Try common numeric types
        if let Ok(v) = row.try_get::<i64, _>(index) {
            return Value::Other {
                type_name: type_name.to_string(),
                display: v.to_string(),
            };
        }

        if let Ok(v) = row.try_get::<f64, _>(index) {
            return Value::Other {
                type_name: type_name.to_string(),
                display: v.to_string(),
            };
        }

        // Give up - return an unknown representation
        Value::Other {
            type_name: type_name.to_string(),
            display: "<unknown>".to_string(),
        }
    }

    /// Map SSL mode to MySQL SSL mode.
    ///
    /// MySQL's SSL mode is configured via connection options rather than
    /// a dedicated enum, so we return a tuple of (require_ssl, verify_mode).
    pub fn map_ssl_mode(
        mode: &crate::services::database::traits::SslMode,
    ) -> (bool, bool) {
        use crate::services::database::traits::SslMode;
        match mode {
            SslMode::Disable => (false, false),
            SslMode::Prefer => (false, false), // Don't require, but use if available
            SslMode::Require => (true, false),
            SslMode::VerifyCa | SslMode::VerifyFull => (true, true),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::database::traits::SslMode;

    #[test]
    fn test_ssl_mode_mapping() {
        let (require, verify) = MySqlValueConverter::map_ssl_mode(&SslMode::Disable);
        assert!(!require);
        assert!(!verify);

        let (require, verify) = MySqlValueConverter::map_ssl_mode(&SslMode::Require);
        assert!(require);
        assert!(!verify);

        let (require, verify) = MySqlValueConverter::map_ssl_mode(&SslMode::VerifyFull);
        assert!(require);
        assert!(verify);
    }
}
