//! Action definitions for keyboard navigation.
//!
//! This module defines all the action types that can be triggered via keyboard shortcuts.

use gpui::actions;

// ============================================================================
// Global Actions - Work in any context
// ============================================================================

actions!(
    global,
    [
        // Mode switching
        SwitchToDatabase,
        SwitchToStorage,
        // Panel toggles
        ToggleSidebar,
        ToggleRightPanel,
        ToggleHistory,
        ToggleAgent,
        // Focus
        Escape,
        // Help
        ShowHelp,
        HideHelp,
    ]
);

// ============================================================================
// Editor Actions - SQL editor specific
// ============================================================================

actions!(
    editor,
    [
        ExecuteQuery,
        FormatQuery,
        FocusEditor,
    ]
);

// ============================================================================
// Navigation Actions - List/tree navigation
// ============================================================================

actions!(
    navigation,
    [
        // Directional navigation
        MoveUp,
        MoveDown,
        MoveLeft,
        MoveRight,
        // Selection
        Select,
        Expand,
        Collapse,
        // Jump navigation
        GoToFirst,
        GoToLast,
        // Actions
        Delete,
        Edit,
        New,
        Refresh,
    ]
);

// ============================================================================
// Panel Focus Actions
// ============================================================================

actions!(
    focus,
    [
        FocusTables,
        FocusResults,
        FocusAgent,
        FocusHistory,
        FocusConnectionList,
        FocusStorageBrowser,
    ]
);

// ============================================================================
// Results Panel Actions
// ============================================================================

actions!(
    results,
    [
        CopyCell,
        CopyRow,
        ExportResults,
    ]
);

// ============================================================================
// Storage Browser Actions
// ============================================================================

actions!(
    storage,
    [
        NavigateUp,
        CopyPath,
        DownloadFile,
    ]
);

// ============================================================================
// Connection Actions
// ============================================================================

actions!(
    connection,
    [
        Connect,
        Disconnect,
        EditConnection,
        DeleteConnection,
        NewConnection,
    ]
);
