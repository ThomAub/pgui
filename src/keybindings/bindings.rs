//! Default keybinding definitions.
//!
//! This module defines all the default keyboard shortcuts for the application.

use gpui::{App, KeyBinding};

use super::actions::global::*;
use super::actions::editor::*;
use super::actions::navigation::*;
use super::actions::focus::*;
use super::actions::results::*;
use super::actions::storage::*;
use super::actions::connection::*;

/// Register all default keybindings with the application.
pub fn register_keybindings(cx: &mut App) {
    cx.bind_keys(global_bindings());
    cx.bind_keys(editor_bindings());
    cx.bind_keys(navigation_bindings());
    cx.bind_keys(focus_bindings());
    cx.bind_keys(results_bindings());
    cx.bind_keys(storage_bindings());
    cx.bind_keys(connection_bindings());
}

/// Global keybindings that work in any context.
fn global_bindings() -> Vec<KeyBinding> {
    vec![
        // Mode switching
        KeyBinding::new("cmd-1", SwitchToDatabase, None),
        KeyBinding::new("cmd-2", SwitchToStorage, None),
        // Panel toggles
        KeyBinding::new("cmd-b", ToggleSidebar, None),
        KeyBinding::new("cmd-\\", ToggleRightPanel, None),
        KeyBinding::new("cmd-j", ToggleHistory, None),
        // Help
        KeyBinding::new("shift-/", ShowHelp, None), // ? key
        // Escape
        KeyBinding::new("escape", Escape, None),
    ]
}

/// Editor-specific keybindings.
fn editor_bindings() -> Vec<KeyBinding> {
    vec![
        KeyBinding::new("cmd-enter", ExecuteQuery, Some("Editor")),
        KeyBinding::new("cmd-shift-f", FormatQuery, Some("Editor")),
        KeyBinding::new("cmd-e", FocusEditor, None),
    ]
}

/// Navigation keybindings for lists and trees.
fn navigation_bindings() -> Vec<KeyBinding> {
    vec![
        // Arrow key navigation
        KeyBinding::new("up", MoveUp, Some("NavigableList")),
        KeyBinding::new("down", MoveDown, Some("NavigableList")),
        KeyBinding::new("left", MoveLeft, Some("NavigableList")),
        KeyBinding::new("right", MoveRight, Some("NavigableList")),
        // Vim-style navigation
        KeyBinding::new("k", MoveUp, Some("NavigableList")),
        KeyBinding::new("j", MoveDown, Some("NavigableList")),
        KeyBinding::new("h", MoveLeft, Some("NavigableList")),
        KeyBinding::new("l", MoveRight, Some("NavigableList")),
        // Selection and expansion
        KeyBinding::new("enter", Select, Some("NavigableList")),
        KeyBinding::new("space", Expand, Some("NavigableList")),
        // Jump navigation
        KeyBinding::new("g g", GoToFirst, Some("NavigableList")),
        KeyBinding::new("shift-g", GoToLast, Some("NavigableList")),
        // Actions
        KeyBinding::new("d", Delete, Some("NavigableList")),
        KeyBinding::new("e", Edit, Some("NavigableList")),
        KeyBinding::new("n", New, Some("NavigableList")),
        KeyBinding::new("r", Refresh, Some("NavigableList")),
    ]
}

/// Panel focus keybindings.
fn focus_bindings() -> Vec<KeyBinding> {
    vec![
        KeyBinding::new("cmd-t", FocusTables, None),
        KeyBinding::new("cmd-r", FocusResults, None),
        KeyBinding::new("cmd-shift-a", FocusAgent, None),
        KeyBinding::new("cmd-shift-h", FocusHistory, None),
    ]
}

/// Results panel keybindings.
fn results_bindings() -> Vec<KeyBinding> {
    vec![
        KeyBinding::new("cmd-c", CopyCell, Some("ResultsPanel")),
        KeyBinding::new("cmd-shift-c", CopyRow, Some("ResultsPanel")),
        KeyBinding::new("cmd-shift-e", ExportResults, Some("ResultsPanel")),
    ]
}

/// Storage browser keybindings.
fn storage_bindings() -> Vec<KeyBinding> {
    vec![
        KeyBinding::new("backspace", NavigateUp, Some("StorageBrowser")),
        KeyBinding::new("cmd-c", CopyPath, Some("StorageBrowser")),
        KeyBinding::new("cmd-d", DownloadFile, Some("StorageBrowser")),
    ]
}

/// Connection list keybindings.
fn connection_bindings() -> Vec<KeyBinding> {
    vec![
        KeyBinding::new("enter", Connect, Some("ConnectionList")),
        KeyBinding::new("cmd-shift-d", Disconnect, None),
        KeyBinding::new("e", EditConnection, Some("ConnectionList")),
        KeyBinding::new("d", DeleteConnection, Some("ConnectionList")),
        KeyBinding::new("n", NewConnection, Some("ConnectionList")),
    ]
}

/// Helper struct for displaying keybinding information in the help overlay.
#[derive(Clone)]
pub struct KeybindingInfo {
    pub key: &'static str,
    pub description: &'static str,
    pub context: Option<&'static str>,
}

/// Get all keybindings for display in help overlay.
pub fn get_all_keybindings() -> Vec<(&'static str, Vec<KeybindingInfo>)> {
    vec![
        (
            "Global",
            vec![
                KeybindingInfo {
                    key: "Cmd+1",
                    description: "Switch to Database mode",
                    context: None,
                },
                KeybindingInfo {
                    key: "Cmd+2",
                    description: "Switch to Storage mode",
                    context: None,
                },
                KeybindingInfo {
                    key: "Cmd+B",
                    description: "Toggle sidebar",
                    context: None,
                },
                KeybindingInfo {
                    key: "Cmd+\\",
                    description: "Toggle right panel",
                    context: None,
                },
                KeybindingInfo {
                    key: "Cmd+J",
                    description: "Toggle history",
                    context: None,
                },
                KeybindingInfo {
                    key: "?",
                    description: "Show this help",
                    context: None,
                },
                KeybindingInfo {
                    key: "Esc",
                    description: "Cancel / Unfocus",
                    context: None,
                },
            ],
        ),
        (
            "Editor",
            vec![
                KeybindingInfo {
                    key: "Cmd+Enter",
                    description: "Execute query",
                    context: Some("Editor"),
                },
                KeybindingInfo {
                    key: "Cmd+Shift+F",
                    description: "Format SQL",
                    context: Some("Editor"),
                },
                KeybindingInfo {
                    key: "Cmd+E",
                    description: "Focus editor",
                    context: None,
                },
            ],
        ),
        (
            "Navigation",
            vec![
                KeybindingInfo {
                    key: "j / Down",
                    description: "Move down",
                    context: Some("Lists"),
                },
                KeybindingInfo {
                    key: "k / Up",
                    description: "Move up",
                    context: Some("Lists"),
                },
                KeybindingInfo {
                    key: "Enter",
                    description: "Select / Connect",
                    context: Some("Lists"),
                },
                KeybindingInfo {
                    key: "gg",
                    description: "Go to first",
                    context: Some("Lists"),
                },
                KeybindingInfo {
                    key: "G",
                    description: "Go to last",
                    context: Some("Lists"),
                },
                KeybindingInfo {
                    key: "n",
                    description: "New item",
                    context: Some("Lists"),
                },
                KeybindingInfo {
                    key: "e",
                    description: "Edit item",
                    context: Some("Lists"),
                },
                KeybindingInfo {
                    key: "d",
                    description: "Delete item",
                    context: Some("Lists"),
                },
                KeybindingInfo {
                    key: "r",
                    description: "Refresh",
                    context: Some("Lists"),
                },
            ],
        ),
        (
            "Focus",
            vec![
                KeybindingInfo {
                    key: "Cmd+T",
                    description: "Focus tables",
                    context: None,
                },
                KeybindingInfo {
                    key: "Cmd+R",
                    description: "Focus results",
                    context: None,
                },
                KeybindingInfo {
                    key: "Cmd+Shift+A",
                    description: "Focus agent",
                    context: None,
                },
                KeybindingInfo {
                    key: "Cmd+Shift+H",
                    description: "Focus history",
                    context: None,
                },
            ],
        ),
        (
            "Results",
            vec![
                KeybindingInfo {
                    key: "Cmd+C",
                    description: "Copy cell",
                    context: Some("Results"),
                },
                KeybindingInfo {
                    key: "Cmd+Shift+C",
                    description: "Copy row",
                    context: Some("Results"),
                },
                KeybindingInfo {
                    key: "Cmd+Shift+E",
                    description: "Export results",
                    context: Some("Results"),
                },
            ],
        ),
        (
            "Storage",
            vec![
                KeybindingInfo {
                    key: "Backspace",
                    description: "Go to parent folder",
                    context: Some("Storage"),
                },
                KeybindingInfo {
                    key: "Cmd+C",
                    description: "Copy path",
                    context: Some("Storage"),
                },
                KeybindingInfo {
                    key: "Cmd+D",
                    description: "Download file",
                    context: Some("Storage"),
                },
            ],
        ),
    ]
}
