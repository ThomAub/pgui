//! ClickHouse type conversion utilities.
//!
//! This module handles conversion between ClickHouse-specific types
//! and the generic `Value` type used across all database drivers.
//!
//! ClickHouse has a rich type system including:
//! - UInt8, UInt16, UInt32, UInt64, UInt128, UInt256
//! - Int8, Int16, Int32, Int64, Int128, Int256
//! - Float32, Float64
//! - Decimal(P, S)
//! - String, FixedString(N)
//! - Date, Date32, DateTime, DateTime64
//! - UUID
//! - Array(T), Nullable(T), Tuple(...)
//! - Enum8, Enum16
//! - LowCardinality(T)

#![allow(dead_code)]

use chrono::{NaiveDate, NaiveDateTime};
use serde_json::Value as JsonValue;

use crate::services::database::traits::{Cell, ColumnInfo, Row as TraitRow, SslMode, Value};

/// Converter for ClickHouse JSON values to the unified `Value` type.
///
/// ClickHouse queries using JSONEachRow format return data as JSON,
/// which we parse and convert to our unified Value enum.
pub struct ClickHouseValueConverter;

impl ClickHouseValueConverter {
    /// Convert a JSON row (from JSONEachRow format) to a trait Row.
    ///
    /// The columns parameter provides the expected column order.
    pub fn convert_json_row(json_row: &JsonValue, columns: &[ColumnInfo]) -> TraitRow {
        let cells = columns
            .iter()
            .enumerate()
            .map(|(idx, col)| {
                let json_value = json_row.get(&col.name).cloned().unwrap_or(JsonValue::Null);
                let value = Self::json_to_value(&json_value, &col.type_name);
                Cell::new(value, idx)
            })
            .collect();

        TraitRow::new(cells)
    }

    /// Convert a JSON value to our unified Value type.
    ///
    /// Uses the ClickHouse type name to guide conversion.
    pub fn json_to_value(json: &JsonValue, type_name: &str) -> Value {
        match json {
            JsonValue::Null => Value::Null,
            JsonValue::Bool(b) => Value::Bool(*b),
            JsonValue::Number(n) => Self::convert_number(n, type_name),
            JsonValue::String(s) => Self::convert_string(s, type_name),
            JsonValue::Array(arr) => Self::convert_array(arr, type_name),
            JsonValue::Object(_) => Value::Json(json.clone()),
        }
    }

    /// Convert a JSON number based on the ClickHouse type.
    fn convert_number(n: &serde_json::Number, type_name: &str) -> Value {
        let type_lower = type_name.to_lowercase();

        // Handle Nullable wrapper
        let inner_type = if type_lower.starts_with("nullable(") && type_lower.ends_with(')') {
            &type_lower[9..type_lower.len() - 1]
        } else {
            &type_lower
        };

        // Handle LowCardinality wrapper
        let inner_type = if inner_type.starts_with("lowcardinality(") && inner_type.ends_with(')') {
            &inner_type[15..inner_type.len() - 1]
        } else {
            inner_type
        };

        match inner_type {
            "uint8" => n.as_u64().map(|v| Value::UInt8(v as u8)).unwrap_or(Value::Null),
            "uint16" => n.as_u64().map(|v| Value::UInt16(v as u16)).unwrap_or(Value::Null),
            "uint32" => n.as_u64().map(|v| Value::UInt32(v as u32)).unwrap_or(Value::Null),
            "uint64" => n.as_u64().map(Value::UInt64).unwrap_or(Value::Null),
            "int8" => n.as_i64().map(|v| Value::Int8(v as i8)).unwrap_or(Value::Null),
            "int16" => n.as_i64().map(|v| Value::Int16(v as i16)).unwrap_or(Value::Null),
            "int32" => n.as_i64().map(|v| Value::Int32(v as i32)).unwrap_or(Value::Null),
            "int64" => n.as_i64().map(Value::Int64).unwrap_or(Value::Null),
            "float32" => n.as_f64().map(|v| Value::Float32(v as f32)).unwrap_or(Value::Null),
            "float64" => n.as_f64().map(Value::Float64).unwrap_or(Value::Null),
            _ if inner_type.starts_with("decimal") => {
                // Decimal types come as numbers, convert to string for precision
                if let Some(f) = n.as_f64() {
                    match rust_decimal::Decimal::try_from(f) {
                        Ok(d) => Value::Decimal(d),
                        Err(_) => Value::Float64(f),
                    }
                } else {
                    Value::Null
                }
            }
            _ => {
                // Default: try i64, then f64
                if let Some(i) = n.as_i64() {
                    Value::Int64(i)
                } else if let Some(f) = n.as_f64() {
                    Value::Float64(f)
                } else {
                    Value::Null
                }
            }
        }
    }

    /// Convert a JSON string based on the ClickHouse type.
    fn convert_string(s: &str, type_name: &str) -> Value {
        let type_lower = type_name.to_lowercase();

        // Handle Nullable wrapper
        let inner_type = if type_lower.starts_with("nullable(") && type_lower.ends_with(')') {
            &type_lower[9..type_lower.len() - 1]
        } else {
            &type_lower
        };

        // Handle LowCardinality wrapper
        let inner_type = if inner_type.starts_with("lowcardinality(") && inner_type.ends_with(')') {
            &inner_type[15..inner_type.len() - 1]
        } else {
            inner_type
        };

        match inner_type {
            "uuid" => {
                uuid::Uuid::parse_str(s)
                    .map(Value::Uuid)
                    .unwrap_or_else(|_| Value::Text(s.to_string()))
            }
            "date" | "date32" => {
                NaiveDate::parse_from_str(s, "%Y-%m-%d")
                    .map(Value::Date)
                    .unwrap_or_else(|_| Value::Text(s.to_string()))
            }
            "datetime" | "datetime64" | _ if inner_type.starts_with("datetime64") => {
                // Try various datetime formats
                NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S")
                    .or_else(|_| NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S%.f"))
                    .map(Value::DateTime)
                    .unwrap_or_else(|_| Value::Text(s.to_string()))
            }
            _ if inner_type.starts_with("fixedstring") => Value::Text(s.to_string()),
            _ if inner_type.starts_with("enum8") || inner_type.starts_with("enum16") => {
                Value::Text(s.to_string())
            }
            "string" => Value::Text(s.to_string()),
            "ipv4" | "ipv6" => Value::Text(s.to_string()),
            _ => Value::Text(s.to_string()),
        }
    }

    /// Convert a JSON array based on the ClickHouse type.
    fn convert_array(arr: &[JsonValue], type_name: &str) -> Value {
        // Extract inner type from Array(T)
        let type_lower = type_name.to_lowercase();
        let inner_type = if type_lower.starts_with("array(") && type_lower.ends_with(')') {
            &type_lower[6..type_lower.len() - 1]
        } else {
            "string" // fallback
        };

        let values: Vec<Value> = arr
            .iter()
            .map(|v| Self::json_to_value(v, inner_type))
            .collect();

        Value::Array(values)
    }

    /// Parse column metadata from ClickHouse query result.
    ///
    /// When using FORMAT JSONCompactColumns or similar, metadata is in separate fields.
    pub fn parse_columns_from_meta(meta: &[JsonValue]) -> Vec<ColumnInfo> {
        meta.iter()
            .enumerate()
            .filter_map(|(idx, m)| {
                let name = m.get("name")?.as_str()?.to_string();
                let type_name = m.get("type")?.as_str()?.to_string();
                Some(ColumnInfo::new(name, type_name, idx))
            })
            .collect()
    }

    /// Map SSL mode to ClickHouse HTTP scheme and settings.
    ///
    /// Returns (use_https, verify_certificate).
    pub fn map_ssl_mode(mode: &SslMode) -> (bool, bool) {
        match mode {
            SslMode::Disable => (false, false),
            SslMode::Prefer => (false, false), // Try without, but could use
            SslMode::Require => (true, false),
            SslMode::VerifyCa | SslMode::VerifyFull => (true, true),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_json_to_value_null() {
        let value = ClickHouseValueConverter::json_to_value(&JsonValue::Null, "String");
        assert!(matches!(value, Value::Null));
    }

    #[test]
    fn test_json_to_value_bool() {
        let value = ClickHouseValueConverter::json_to_value(&json!(true), "UInt8");
        assert!(matches!(value, Value::Bool(true)));
    }

    #[test]
    fn test_json_to_value_integers() {
        let value = ClickHouseValueConverter::json_to_value(&json!(42), "Int32");
        assert!(matches!(value, Value::Int32(42)));

        let value = ClickHouseValueConverter::json_to_value(&json!(1000), "UInt64");
        assert!(matches!(value, Value::UInt64(1000)));
    }

    #[test]
    fn test_json_to_value_float() {
        let value = ClickHouseValueConverter::json_to_value(&json!(3.14), "Float64");
        match value {
            Value::Float64(f) => assert!((f - 3.14).abs() < 0.001),
            _ => panic!("Expected Float64"),
        }
    }

    #[test]
    fn test_json_to_value_string() {
        let value = ClickHouseValueConverter::json_to_value(&json!("hello"), "String");
        assert!(matches!(value, Value::Text(s) if s == "hello"));
    }

    #[test]
    fn test_json_to_value_date() {
        let value = ClickHouseValueConverter::json_to_value(&json!("2024-01-15"), "Date");
        match value {
            Value::Date(d) => {
                assert_eq!(d.format("%Y-%m-%d").to_string(), "2024-01-15");
            }
            _ => panic!("Expected Date"),
        }
    }

    #[test]
    fn test_json_to_value_datetime() {
        let value = ClickHouseValueConverter::json_to_value(
            &json!("2024-01-15 10:30:00"),
            "DateTime",
        );
        match value {
            Value::DateTime(dt) => {
                assert_eq!(dt.format("%Y-%m-%d %H:%M:%S").to_string(), "2024-01-15 10:30:00");
            }
            _ => panic!("Expected DateTime"),
        }
    }

    #[test]
    fn test_json_to_value_uuid() {
        let value = ClickHouseValueConverter::json_to_value(
            &json!("550e8400-e29b-41d4-a716-446655440000"),
            "UUID",
        );
        match value {
            Value::Uuid(u) => {
                assert_eq!(u.to_string(), "550e8400-e29b-41d4-a716-446655440000");
            }
            _ => panic!("Expected UUID"),
        }
    }

    #[test]
    fn test_json_to_value_array() {
        let value = ClickHouseValueConverter::json_to_value(
            &json!([1, 2, 3]),
            "Array(Int32)",
        );
        match value {
            Value::Array(arr) => {
                assert_eq!(arr.len(), 3);
                assert!(matches!(arr[0], Value::Int64(1)));
            }
            _ => panic!("Expected Array"),
        }
    }

    #[test]
    fn test_json_to_value_nullable() {
        let value = ClickHouseValueConverter::json_to_value(&json!(42), "Nullable(Int32)");
        assert!(matches!(value, Value::Int32(42)));

        let value = ClickHouseValueConverter::json_to_value(&JsonValue::Null, "Nullable(Int32)");
        assert!(matches!(value, Value::Null));
    }

    #[test]
    fn test_ssl_mode_mapping() {
        let (https, verify) = ClickHouseValueConverter::map_ssl_mode(&SslMode::Disable);
        assert!(!https);
        assert!(!verify);

        let (https, verify) = ClickHouseValueConverter::map_ssl_mode(&SslMode::Require);
        assert!(https);
        assert!(!verify);

        let (https, verify) = ClickHouseValueConverter::map_ssl_mode(&SslMode::VerifyFull);
        assert!(https);
        assert!(verify);
    }
}
