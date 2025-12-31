//! Action definitions for keyboard navigation.
//!
//! This module defines all the action types that can be triggered via keyboard shortcuts.

// ============================================================================
// Global Actions - Work in any context
// ============================================================================

pub mod global {
    use gpui::actions;

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
}

// ============================================================================
// Editor Actions - SQL editor specific
// ============================================================================

pub mod editor {
    use gpui::actions;

    actions!(
        editor,
        [
            ExecuteQuery,
            FormatQuery,
            FocusEditor,
        ]
    );
}

// ============================================================================
// Navigation Actions - List/tree navigation
// ============================================================================

pub mod navigation {
    use gpui::actions;

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
}

// ============================================================================
// Panel Focus Actions
// ============================================================================

pub mod focus {
    use gpui::actions;

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
}

// ============================================================================
// Results Panel Actions
// ============================================================================

pub mod results {
    use gpui::actions;

    actions!(
        results,
        [
            CopyCell,
            CopyRow,
            ExportResults,
        ]
    );
}

// ============================================================================
// Storage Browser Actions
// ============================================================================

pub mod storage {
    use gpui::actions;

    actions!(
        storage,
        [
            NavigateUp,
            CopyPath,
            DownloadFile,
        ]
    );
}

// ============================================================================
// Connection Actions
// ============================================================================

pub mod connection {
    use gpui::actions;

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
}
