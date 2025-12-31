//! PostgreSQL type conversion utilities.
//!
//! This module handles conversion between PostgreSQL-specific types (from SQLx)
//! and the generic `Value` type used across all database drivers.

#![allow(dead_code)]

use chrono::{DateTime, NaiveDate, NaiveDateTime, NaiveTime, Utc};
use rust_decimal::Decimal;
use sqlx::postgres::{PgColumn, PgRow};
use sqlx::{Column, Row, TypeInfo, ValueRef};
use uuid::Uuid;

use crate::services::database::traits::{Cell, ColumnInfo, Row as TraitRow, Value};

/// Converter for PostgreSQL values to the unified `Value` type.
pub struct PgValueConverter;

impl PgValueConverter {
    /// Convert a PostgreSQL row to a trait Row.
    pub fn convert_row(pg_row: &PgRow) -> TraitRow {
        let cells = pg_row
            .columns()
            .iter()
            .enumerate()
            .map(|(idx, col)| {
                let value = Self::extract_value(pg_row, col, idx);
                Cell::new(value, idx)
            })
            .collect();

        TraitRow::new(cells)
    }

    /// Build column info from a PostgreSQL row.
    pub fn build_column_info(pg_row: &PgRow) -> Vec<ColumnInfo> {
        pg_row
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

    /// Extract a value from a PostgreSQL row at the given column index.
    fn extract_value(row: &PgRow, column: &PgColumn, index: usize) -> Value {
        // Check for NULL first
        match row.try_get_raw(index) {
            Ok(raw) if raw.is_null() => return Value::Null,
            Err(_) => return Value::Null,
            _ => {}
        }

        let type_name = column.type_info().name();
        Self::decode_by_type(row, index, type_name)
    }

    /// Decode a value based on its PostgreSQL type name.
    fn decode_by_type(row: &PgRow, index: usize, type_name: &str) -> Value {
        match type_name {
            // Boolean
            "BOOL" => row
                .try_get::<bool, _>(index)
                .map(Value::Bool)
                .unwrap_or(Value::Null),

            // Integers
            "INT2" | "SMALLINT" | "SMALLSERIAL" => row
                .try_get::<i16, _>(index)
                .map(Value::Int16)
                .unwrap_or(Value::Null),

            "INT4" | "INT" | "INTEGER" | "SERIAL" => row
                .try_get::<i32, _>(index)
                .map(Value::Int32)
                .unwrap_or(Value::Null),

            "INT8" | "BIGINT" | "BIGSERIAL" => row
                .try_get::<i64, _>(index)
                .map(Value::Int64)
                .unwrap_or(Value::Null),

            // Floating point
            "FLOAT4" | "REAL" => row
                .try_get::<f32, _>(index)
                .map(Value::Float32)
                .unwrap_or(Value::Null),

            "FLOAT8" | "DOUBLE PRECISION" => row
                .try_get::<f64, _>(index)
                .map(Value::Float64)
                .unwrap_or(Value::Null),

            // Numeric/Decimal
            "NUMERIC" | "DECIMAL" => row
                .try_get::<Decimal, _>(index)
                .map(Value::Decimal)
                .unwrap_or(Value::Null),

            // Text types
            "TEXT" | "VARCHAR" | "CHAR" | "BPCHAR" | "NAME" => row
                .try_get::<String, _>(index)
                .map(Value::Text)
                .unwrap_or(Value::Null),

            // Binary
            "BYTEA" => row
                .try_get::<Vec<u8>, _>(index)
                .map(Value::Bytes)
                .unwrap_or(Value::Null),

            // Date/Time types
            "DATE" => row
                .try_get::<NaiveDate, _>(index)
                .map(Value::Date)
                .unwrap_or(Value::Null),

            "TIME" | "TIMETZ" => row
                .try_get::<NaiveTime, _>(index)
                .map(Value::Time)
                .unwrap_or(Value::Null),

            "TIMESTAMP" => row
                .try_get::<NaiveDateTime, _>(index)
                .map(Value::DateTime)
                .unwrap_or(Value::Null),

            "TIMESTAMPTZ" => row
                .try_get::<DateTime<Utc>, _>(index)
                .map(Value::DateTimeTz)
                .unwrap_or(Value::Null),

            // UUID
            "UUID" => row
                .try_get::<Uuid, _>(index)
                .map(Value::Uuid)
                .unwrap_or(Value::Null),

            // JSON types
            "JSON" | "JSONB" => row
                .try_get::<serde_json::Value, _>(index)
                .map(Value::Json)
                .unwrap_or(Value::Null),

            // Array types - handle common ones
            "_INT4" | "INT4[]" => Self::decode_int_array(row, index),
            "_INT8" | "INT8[]" => Self::decode_bigint_array(row, index),
            "_TEXT" | "TEXT[]" | "_VARCHAR" | "VARCHAR[]" => Self::decode_text_array(row, index),
            "_BOOL" | "BOOL[]" => Self::decode_bool_array(row, index),
            "_FLOAT8" | "FLOAT8[]" => Self::decode_float_array(row, index),

            // For unknown types, try to get as string representation
            _ => Self::decode_as_string_fallback(row, index, type_name),
        }
    }

    /// Decode an integer array.
    fn decode_int_array(row: &PgRow, index: usize) -> Value {
        row.try_get::<Vec<i32>, _>(index)
            .map(|arr| Value::Array(arr.into_iter().map(Value::Int32).collect()))
            .unwrap_or(Value::Null)
    }

    /// Decode a bigint array.
    fn decode_bigint_array(row: &PgRow, index: usize) -> Value {
        row.try_get::<Vec<i64>, _>(index)
            .map(|arr| Value::Array(arr.into_iter().map(Value::Int64).collect()))
            .unwrap_or(Value::Null)
    }

    /// Decode a text array.
    fn decode_text_array(row: &PgRow, index: usize) -> Value {
        row.try_get::<Vec<String>, _>(index)
            .map(|arr| Value::Array(arr.into_iter().map(Value::Text).collect()))
            .unwrap_or(Value::Null)
    }

    /// Decode a boolean array.
    fn decode_bool_array(row: &PgRow, index: usize) -> Value {
        row.try_get::<Vec<bool>, _>(index)
            .map(|arr| Value::Array(arr.into_iter().map(Value::Bool).collect()))
            .unwrap_or(Value::Null)
    }

    /// Decode a float array.
    fn decode_float_array(row: &PgRow, index: usize) -> Value {
        row.try_get::<Vec<f64>, _>(index)
            .map(|arr| Value::Array(arr.into_iter().map(Value::Float64).collect()))
            .unwrap_or(Value::Null)
    }

    /// Fallback: try to decode as string representation for unknown types.
    fn decode_as_string_fallback(row: &PgRow, index: usize, type_name: &str) -> Value {
        // Try to get as string first (PostgreSQL can convert most types to text)
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

    /// Map PostgreSQL SSL mode to trait SSL mode.
    pub fn map_ssl_mode(
        mode: &crate::services::database::traits::SslMode,
    ) -> sqlx::postgres::PgSslMode {
        use crate::services::database::traits::SslMode;
        match mode {
            SslMode::Disable => sqlx::postgres::PgSslMode::Disable,
            SslMode::Prefer => sqlx::postgres::PgSslMode::Prefer,
            SslMode::Require => sqlx::postgres::PgSslMode::Require,
            SslMode::VerifyCa => sqlx::postgres::PgSslMode::VerifyCa,
            SslMode::VerifyFull => sqlx::postgres::PgSslMode::VerifyFull,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::database::traits::SslMode;

    #[test]
    fn test_ssl_mode_mapping() {
        assert!(matches!(
            PgValueConverter::map_ssl_mode(&SslMode::Disable),
            sqlx::postgres::PgSslMode::Disable
        ));
        assert!(matches!(
            PgValueConverter::map_ssl_mode(&SslMode::Prefer),
            sqlx::postgres::PgSslMode::Prefer
        ));
        assert!(matches!(
            PgValueConverter::map_ssl_mode(&SslMode::Require),
            sqlx::postgres::PgSslMode::Require
        ));
    }
}
