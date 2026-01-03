use gpui::AsyncApp;

use crate::services::database::storage::ObjectInfo;
use crate::services::{ColumnDetail, DatabaseSchema, QueryExecutionResult, TableSchema};
use crate::state::StorageState;
use crate::{
    services::agent::{ToolCallData, ToolResultData},
    state::ConnectionState,
};

/// Execute tools with access to context
/// This is where you'll add database access, file system, etc.
pub async fn execute_tools(tool_calls: Vec<ToolCallData>, cx: &AsyncApp) -> Vec<ToolResultData> {
    let mut results = Vec::new();
    for call in tool_calls {
        let result = match call.name.as_str() {
            "get_schema" => {
                // Extract filter_tables as Option<Vec<String>>
                // If not provided, empty array, or null -> None (returns all tables)
                let filter_tables = call
                    .input
                    .get("filter_tables")
                    .and_then(|v| v.as_array())
                    .filter(|arr| !arr.is_empty())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str().map(String::from))
                            .collect::<Vec<String>>()
                    });

                let error_result = || ToolResultData {
                    tool_use_id: call.id.clone(),
                    content: "Failed to fetch schema".to_string(),
                    is_error: true,
                };

                match cx.read_global::<ConnectionState, _>(|state, _cx| state.db_manager.clone()) {
                    Ok(db) => match db.get_schema(filter_tables).await {
                        Ok(schema) => {
                            let formatted = format_schema_for_llm(&schema);
                            ToolResultData {
                                tool_use_id: call.id,
                                content: formatted,
                                is_error: false,
                            }
                        }
                        Err(_) => error_result(),
                    },
                    Err(_) => error_result(),
                }
            }

            "get_tables" => {
                let error_result = || ToolResultData {
                    tool_use_id: call.id.clone(),
                    content: "Failed to fetch tables".to_string(),
                    is_error: true,
                };

                match cx.read_global::<ConnectionState, _>(|state, _cx| state.db_manager.clone()) {
                    Ok(db) => match db.get_tables().await {
                        Ok(tables) => {
                            let formatted = tables
                                .iter()
                                .map(|t| {
                                    format!(
                                        "- {}.{} ({})",
                                        t.table_schema, t.table_name, t.table_type
                                    )
                                })
                                .collect::<Vec<_>>()
                                .join("\n");
                            ToolResultData {
                                tool_use_id: call.id,
                                content: format!("## Tables\n\n{}", formatted),
                                is_error: false,
                            }
                        }
                        Err(_) => error_result(),
                    },
                    Err(_) => error_result(),
                }
            }

            "get_table_columns" => {
                let table_name = call.input.get("table_name").and_then(|v| v.as_str());
                let table_schema = call
                    .input
                    .get("table_schema")
                    .and_then(|v| v.as_str())
                    .unwrap_or("public");

                let error_result = |msg: &str| ToolResultData {
                    tool_use_id: call.id.clone(),
                    content: msg.to_string(),
                    is_error: true,
                };

                match table_name {
                    Some(name) => {
                        match cx.read_global::<ConnectionState, _>(|state, _cx| {
                            state.db_manager.clone()
                        }) {
                            Ok(db) => match db.get_table_columns(name, table_schema).await {
                                Ok(result) => {
                                    let formatted = format_query_result_as_markdown(result);
                                    ToolResultData {
                                        tool_use_id: call.id,
                                        content: formatted,
                                        is_error: false,
                                    }
                                }
                                Err(e) => error_result(&format!("Failed to fetch columns: {}", e)),
                            },
                            Err(_) => error_result("Database not connected"),
                        }
                    }
                    None => error_result("table_name is required"),
                }
            }

            // ================================================================
            // Storage Tools
            // ================================================================
            "list_storage_files" => {
                let path = call
                    .input
                    .get("path")
                    .and_then(|v| v.as_str())
                    .unwrap_or("/");

                let error_result = |msg: &str| ToolResultData {
                    tool_use_id: call.id.clone(),
                    content: msg.to_string(),
                    is_error: true,
                };

                match cx.read_global::<StorageState, _>(|state, _cx| state.storage_manager.clone()) {
                    Ok(manager) => match manager.list(path).await {
                        Ok(objects) => {
                            let formatted = format_storage_list_for_llm(&objects);
                            ToolResultData {
                                tool_use_id: call.id,
                                content: formatted,
                                is_error: false,
                            }
                        }
                        Err(e) => error_result(&format!("Failed to list files: {}", e)),
                    },
                    Err(_) => error_result("Storage not connected"),
                }
            }

            "get_storage_info" => {
                let error_result = |msg: &str| ToolResultData {
                    tool_use_id: call.id.clone(),
                    content: msg.to_string(),
                    is_error: true,
                };

                match cx.read_global::<StorageState, _>(|state, _cx| {
                    (
                        state.storage_manager.clone(),
                        state.active_connection.clone(),
                    )
                }) {
                    Ok((manager, active_conn)) => {
                        let storage_type = manager.storage_type().await;
                        let is_connected = manager.is_connected().await;

                        let mut info = String::new();
                        info.push_str("## Storage Connection Info\n\n");
                        info.push_str(&format!(
                            "- **Status**: {}\n",
                            if is_connected {
                                "Connected"
                            } else {
                                "Disconnected"
                            }
                        ));

                        if let Some(conn) = active_conn {
                            info.push_str(&format!("- **Name**: {}\n", conn.name));
                            info.push_str(&format!(
                                "- **Type**: {}\n",
                                storage_type
                                    .map(|t| t.display_name().to_string())
                                    .unwrap_or_else(|| "Unknown".to_string())
                            ));

                            // Add bucket/path info based on type
                            match &conn.params {
                                crate::services::database::storage::StorageParams::S3 {
                                    bucket,
                                    region,
                                    ..
                                } => {
                                    info.push_str(&format!("- **Bucket**: {}\n", bucket));
                                    info.push_str(&format!("- **Region**: {}\n", region));
                                }
                                crate::services::database::storage::StorageParams::Gcs {
                                    bucket,
                                    ..
                                } => {
                                    info.push_str(&format!("- **Bucket**: {}\n", bucket));
                                }
                                crate::services::database::storage::StorageParams::LocalFs {
                                    root_path,
                                } => {
                                    info.push_str(&format!(
                                        "- **Root Path**: {}\n",
                                        root_path.display()
                                    ));
                                }
                                _ => {}
                            }
                        } else {
                            info.push_str("\nNo storage connection configured.\n");
                        }

                        ToolResultData {
                            tool_use_id: call.id,
                            content: info,
                            is_error: false,
                        }
                    }
                    Err(_) => error_result("Failed to read storage state"),
                }
            }

            "read_storage_file_preview" => {
                let path = call.input.get("path").and_then(|v| v.as_str());

                let error_result = |msg: &str| ToolResultData {
                    tool_use_id: call.id.clone(),
                    content: msg.to_string(),
                    is_error: true,
                };

                match path {
                    Some(file_path) => {
                        match cx
                            .read_global::<StorageState, _>(|state, _cx| state.storage_manager.clone())
                        {
                            Ok(manager) => {
                                // Read first 4KB
                                match manager.read_range(file_path, 0, 4096).await {
                                    Ok(data) => {
                                        // Try to convert to string, fallback to hex preview
                                        let content = match String::from_utf8(data.clone()) {
                                            Ok(text) => {
                                                let preview = if text.len() > 4000 {
                                                    format!("{}...\n\n_(truncated)_", &text[..4000])
                                                } else {
                                                    text
                                                };
                                                format!(
                                                    "## File Preview: {}\n\n```\n{}\n```",
                                                    file_path, preview
                                                )
                                            }
                                            Err(_) => {
                                                format!(
                                                    "## File Preview: {}\n\n_Binary file ({} bytes preview)_\n\nFirst 64 bytes (hex):\n```\n{}\n```",
                                                    file_path,
                                                    data.len(),
                                                    data.iter()
                                                        .take(64)
                                                        .map(|b| format!("{:02x}", b))
                                                        .collect::<Vec<_>>()
                                                        .join(" ")
                                                )
                                            }
                                        };
                                        ToolResultData {
                                            tool_use_id: call.id,
                                            content,
                                            is_error: false,
                                        }
                                    }
                                    Err(e) => {
                                        error_result(&format!("Failed to read file: {}", e))
                                    }
                                }
                            }
                            Err(_) => error_result("Storage not connected"),
                        }
                    }
                    None => error_result("path is required"),
                }
            }

            _ => ToolResultData {
                tool_use_id: call.id,
                content: format!("Unknown tool: {}", call.name),
                is_error: true,
            },
        };

        results.push(result);
    }

    results
}

/// Format a QueryExecutionResult as markdown
fn format_query_result_as_markdown(result: QueryExecutionResult) -> String {
    match result {
        QueryExecutionResult::Select(query_result) => {
            if query_result.columns.is_empty() {
                return "No columns found".to_string();
            }

            let mut md = String::new();

            // Header row
            md.push_str("| ");
            md.push_str(
                &query_result
                    .columns
                    .iter()
                    .map(|c| c.name.as_str())
                    .collect::<Vec<_>>()
                    .join(" | "),
            );
            md.push_str(" |\n");

            // Separator row
            md.push_str("| ");
            md.push_str(
                &query_result
                    .columns
                    .iter()
                    .map(|_| "---")
                    .collect::<Vec<_>>()
                    .join(" | "),
            );
            md.push_str(" |\n");

            // Data rows
            for row in &query_result.rows {
                md.push_str("| ");
                let values: Vec<String> = row
                    .cells
                    .iter()
                    .map(|cell| {
                        if cell.is_null {
                            "NULL".to_string()
                        } else {
                            cell.value.replace('|', "\\|")
                        }
                    })
                    .collect();
                md.push_str(&values.join(" | "));
                md.push_str(" |\n");
            }

            md.push_str(&format!("\n_{} row(s)_", query_result.row_count));
            md
        }

        QueryExecutionResult::Modified(modified_result) => {
            format!(
                "Query executed successfully. {} row(s) affected.",
                modified_result.rows_affected
            )
        }

        QueryExecutionResult::Error(error_result) => {
            format!("Error: {}", error_result.message)
        }
    }
}

/// Generates a human-readable schema description for LLM consumption
pub fn format_schema_for_llm(schema: &DatabaseSchema) -> String {
    let mut output = String::new();

    output.push_str(&format!(
        "# Database Schema ({} tables)\n\n",
        schema.total_tables
    ));

    for table in &schema.tables {
        format_table_for_llm(table, &mut output);
    }

    output
}

fn format_table_for_llm(table: &TableSchema, output: &mut String) {
    output.push_str(&format!(
        "## Table: {}.{}\n",
        table.table_schema, table.table_name
    ));
    output.push_str(&format!("Type: {}\n", table.table_type));

    if let Some(ref desc) = table.description {
        output.push_str(&format!("Description: {}\n", desc));
    }
    output.push('\n');

    // Columns
    output.push_str("### Columns:\n");
    for col in &table.columns {
        format_column_for_llm(col, output);
    }
    output.push('\n');

    // Primary Keys
    if !table.primary_keys.is_empty() {
        output.push_str(&format!(
            "### Primary Key: {}\n\n",
            table.primary_keys.join(", ")
        ));
    }

    // Foreign Keys
    if !table.foreign_keys.is_empty() {
        output.push_str("### Foreign Keys:\n");
        for fk in &table.foreign_keys {
            output.push_str(&format!(
                "- **{}** → {}.{}.{}\n",
                fk.column_name,
                fk.foreign_table_schema,
                fk.foreign_table_name,
                fk.foreign_column_name
            ));
        }
        output.push('\n');
    }

    // Indexes
    if !table.indexes.is_empty() {
        output.push_str("### Indexes:\n");
        for idx in &table.indexes {
            let unique = if idx.is_unique { "UNIQUE" } else { "" };
            let primary = if idx.is_primary { "PRIMARY" } else { "" };
            let flags = [unique, primary]
                .iter()
                .filter(|s| !s.is_empty())
                .copied()
                .collect::<Vec<_>>()
                .join(", ");
            let flags_str = if !flags.is_empty() {
                format!(" [{}]", flags)
            } else {
                String::new()
            };

            output.push_str(&format!(
                "- **{}** ({}):{} {}\n",
                idx.index_name,
                idx.index_type,
                flags_str,
                idx.columns.join(", ")
            ));
        }
        output.push('\n');
    }

    // Constraints
    if !table.constraints.is_empty() {
        output.push_str("### Constraints:\n");
        for constraint in &table.constraints {
            output.push_str(&format!(
                "- **{}** ({}): {}",
                constraint.constraint_name,
                constraint.constraint_type,
                constraint.columns.join(", ")
            ));

            if let Some(ref check) = constraint.check_clause {
                output.push_str(&format!(" WHERE {}", check));
            }
            output.push('\n');
        }
        output.push('\n');
    }

    output.push_str("---\n\n");
}

fn format_column_for_llm(col: &ColumnDetail, output: &mut String) {
    let nullable = if col.is_nullable { "NULL" } else { "NOT NULL" };
    let mut col_line = format!("- **{}**: {} {}", col.column_name, col.data_type, nullable);

    if let Some(ref default) = col.column_default {
        col_line.push_str(&format!(" DEFAULT {}", default));
    }

    if let Some(len) = col.character_maximum_length {
        col_line.push_str(&format!(" (max length: {})", len));
    }

    if let Some(prec) = col.numeric_precision {
        if let Some(scale) = col.numeric_scale {
            col_line.push_str(&format!(" (precision: {}, scale: {})", prec, scale));
        }
    }

    if let Some(ref desc) = col.description {
        col_line.push_str(&format!(" - {}", desc));
    }

    output.push_str(&col_line);
    output.push('\n');
}

// ============================================================================
// Storage Formatting Helpers
// ============================================================================

/// Format a list of storage objects for LLM consumption
fn format_storage_list_for_llm(objects: &[ObjectInfo]) -> String {
    if objects.is_empty() {
        return "## Storage Contents\n\n_No files or directories found._".to_string();
    }

    let mut output = String::new();
    output.push_str(&format!(
        "## Storage Contents ({} items)\n\n",
        objects.len()
    ));

    // Separate directories and files
    let (dirs, files): (Vec<_>, Vec<_>) = objects.iter().partition(|o| o.is_dir);

    // List directories first
    if !dirs.is_empty() {
        output.push_str("### Directories\n");
        for dir in dirs {
            output.push_str(&format!("- **{}**/\n", dir.path));
        }
        output.push('\n');
    }

    // List files with size and modification time
    if !files.is_empty() {
        output.push_str("### Files\n");
        output.push_str("| Name | Size | Last Modified |\n");
        output.push_str("|------|------|---------------|\n");
        for file in files {
            let size = file.size.map(format_file_size).unwrap_or_else(|| "-".to_string());
            let modified = file
                .last_modified
                .map(|dt| dt.format("%Y-%m-%d %H:%M").to_string())
                .unwrap_or_else(|| "-".to_string());
            output.push_str(&format!("| {} | {} | {} |\n", file.path, size, modified));
        }
    }

    output
}

/// Format file size in human-readable format
fn format_file_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}
