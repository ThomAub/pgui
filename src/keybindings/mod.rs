//! Keyboard navigation and keybindings module.
//!
//! This module provides comprehensive keyboard navigation support for the PGUI application,
//! including:
//!
//! - Global shortcuts (mode switching, panel toggles)
//! - Context-aware navigation (vim-style j/k, arrow keys)
//! - Editor shortcuts (execute, format)
//! - Panel focus management
//!
//! # Usage
//!
//! ```rust,ignore
//! use crate::keybindings;
//!
//! // In main.rs, after opening the window:
//! keybindings::init(cx);
//! ```

pub mod actions;
pub mod bindings;

use gpui::App;

// Re-export action namespaces for convenience
pub use actions::connection;
pub use actions::editor;
pub use actions::focus;
pub use actions::global;
pub use bindings::register_keybindings;

/// Initialize the keybindings system.
///
/// This should be called once during application startup, after the window is opened.
pub fn init(cx: &mut App) {
    register_keybindings(cx);
}
