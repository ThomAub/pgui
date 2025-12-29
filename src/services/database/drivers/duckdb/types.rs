//! DuckDB value type conversions.
//!
//! This module handles converting DuckDB-specific types to our unified Value type.

use crate::services::database::traits::{ColumnInfo, Row as TraitRow, Value};
use duckdb::types::ValueRef;
use duckdb::Row;

/// Converter for DuckDB values to unified Value types.
pub struct DuckDbValueConverter;

impl DuckDbValueConverter {
    /// Convert a DuckDB row to a trait Row.
    pub fn convert_row(duckdb_row: &Row<'_>, column_count: usize) -> TraitRow {
        let values: Vec<Value> = (0..column_count)
            .map(|i| Self::extract_value(duckdb_row, i))
            .collect();

        TraitRow::from_values(values)
    }

    /// Build column info from DuckDB statement.
    pub fn build_column_info(stmt: &duckdb::Statement<'_>) -> Vec<ColumnInfo> {
        let count = stmt.column_count();
        (0..count)
            .map(|i| {
                let name = stmt.column_name(i).map_or("?", |v| v).to_string();
                let type_name = format!("{:?}", stmt.column_type(i));

                ColumnInfo::new(name, type_name, i)
            })
            .collect()
    }

    /// Extract a value from a DuckDB row at the given index.
    fn extract_value(row: &Row<'_>, index: usize) -> Value {
        let value_ref = match row.get_ref(index) {
            Ok(v) => v,
            Err(_) => return Value::Null,
        };

        Self::value_ref_to_value(value_ref)
    }

    /// Convert a DuckDB ValueRef to our Value type.
    fn value_ref_to_value(value_ref: ValueRef<'_>) -> Value {
        match value_ref {
            ValueRef::Null => Value::Null,
            ValueRef::Boolean(b) => Value::Bool(b),
            ValueRef::TinyInt(i) => Value::Int8(i),
            ValueRef::SmallInt(i) => Value::Int16(i),
            ValueRef::Int(i) => Value::Int32(i),
            ValueRef::BigInt(i) => Value::Int64(i),
            ValueRef::HugeInt(i) => {
                Value::Other {
                    type_name: "hugeint".to_string(),
                    display: i.to_string(),
                }
            }
            ValueRef::UTinyInt(i) => Value::UInt8(i),
            ValueRef::USmallInt(i) => Value::UInt16(i),
            ValueRef::UInt(i) => Value::UInt32(i),
            ValueRef::UBigInt(i) => Value::UInt64(i),
            ValueRef::Float(f) => Value::Float32(f),
            ValueRef::Double(f) => Value::Float64(f),
            ValueRef::Decimal(d) => {
                Value::Other {
                    type_name: "decimal".to_string(),
                    display: format!("{}", d),
                }
            }
            ValueRef::Text(bytes) => {
                Value::Text(String::from_utf8_lossy(bytes).to_string())
            }
            ValueRef::Blob(bytes) => Value::Bytes(bytes.to_vec()),
            ValueRef::Date32(days) => {
                let date = chrono::NaiveDate::from_num_days_from_ce_opt(days + 719163);
                match date {
                    Some(d) => Value::Date(d),
                    None => Value::Other {
                        type_name: "date".to_string(),
                        display: format!("DATE({})", days),
                    },
                }
            }
            ValueRef::Time64(_, micros) => {
                let secs = (micros / 1_000_000) as u32;
                let nanos = ((micros % 1_000_000) * 1000) as u32;
                match chrono::NaiveTime::from_num_seconds_from_midnight_opt(secs, nanos) {
                    Some(t) => Value::Time(t),
                    None => Value::Other {
                        type_name: "time".to_string(),
                        display: format!("TIME({})", micros),
                    },
                }
            }
            ValueRef::Timestamp(unit, value) => {
                let micros = match unit {
                    duckdb::types::TimeUnit::Second => value * 1_000_000,
                    duckdb::types::TimeUnit::Millisecond => value * 1_000,
                    duckdb::types::TimeUnit::Microsecond => value,
                    duckdb::types::TimeUnit::Nanosecond => value / 1_000,
                };
                match chrono::DateTime::from_timestamp_micros(micros) {
                    Some(dt) => Value::DateTime(dt.naive_utc()),
                    None => Value::Other {
                        type_name: "timestamp".to_string(),
                        display: format!("TIMESTAMP({})", value),
                    },
                }
            }
            ValueRef::Interval { months, days, nanos } => {
                Value::Other {
                    type_name: "interval".to_string(),
                    display: format!("{} months {} days {} ns", months, days, nanos),
                }
            }
            ValueRef::List(_, _) => {
                Value::Other {
                    type_name: "list".to_string(),
                    display: "[LIST]".to_string(),
                }
            }
            ValueRef::Enum(_, idx) => Value::Int64(idx as i64),
            ValueRef::Struct(_, _) => {
                Value::Other {
                    type_name: "struct".to_string(),
                    display: "[STRUCT]".to_string(),
                }
            }
            ValueRef::Map(_, _) => {
                Value::Other {
                    type_name: "map".to_string(),
                    display: "[MAP]".to_string(),
                }
            }
            ValueRef::Array(_, _) => {
                Value::Other {
                    type_name: "array".to_string(),
                    display: "[ARRAY]".to_string(),
                }
            }
            ValueRef::Union(_, _) => {
                Value::Other {
                    type_name: "union".to_string(),
                    display: "[UNION]".to_string(),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_value_conversion_null() {
        let value = DuckDbValueConverter::value_ref_to_value(ValueRef::Null);
        assert!(matches!(value, Value::Null));
    }

    #[test]
    fn test_value_conversion_bool() {
        let value = DuckDbValueConverter::value_ref_to_value(ValueRef::Boolean(true));
        assert!(matches!(value, Value::Bool(true)));
    }

    #[test]
    fn test_value_conversion_int() {
        let value = DuckDbValueConverter::value_ref_to_value(ValueRef::BigInt(42));
        assert!(matches!(value, Value::Int64(42)));
    }

    #[test]
    fn test_value_conversion_float() {
        let value = DuckDbValueConverter::value_ref_to_value(ValueRef::Double(3.14));
        match value {
            Value::Float64(f) => assert!((f - 3.14).abs() < 0.001),
            _ => panic!("Expected Float64"),
        }
    }

    #[test]
    fn test_value_conversion_text() {
        let value = DuckDbValueConverter::value_ref_to_value(ValueRef::Text(b"hello"));
        assert!(matches!(value, Value::Text(ref s) if s == "hello"));
    }
}
