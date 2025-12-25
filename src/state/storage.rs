//! Storage connection state management.
//!
//! This module manages the global state for blob storage connections (S3, GCS, Azure, etc.).

use gpui::*;

use crate::services::{
    database::storage::{StorageConfig, StorageManager, StorageType},
    storage::StorageConnectionsRepository,
    AppStore,
};

/// Connection status for storage backends.
#[derive(Clone, PartialEq)]
pub enum StorageConnectionStatus {
    Disconnected,
    Disconnecting,
    Connecting,
    Connected,
}

/// Global state for storage connections.
pub struct StorageState {
    /// List of saved storage connections.
    pub saved_connections: Vec<StorageConfig>,
    /// Currently active storage connection.
    pub active_connection: Option<StorageConfig>,
    /// Storage manager for operations.
    pub storage_manager: StorageManager,
    /// Current connection status.
    pub connection_status: StorageConnectionStatus,
    /// Current path being browsed.
    pub current_path: String,
}

impl Global for StorageState {}

impl StorageState {
    /// Initialize the global storage state.
    pub fn init(cx: &mut App) {
        let storage_manager = StorageManager::new();
        let this = StorageState {
            saved_connections: vec![],
            active_connection: None,
            storage_manager,
            connection_status: StorageConnectionStatus::Disconnected,
            current_path: "/".to_string(),
        };
        cx.set_global(this);

        // Load saved storage connections on startup
        cx.spawn(async move |cx| {
            if let Ok(store) = AppStore::singleton().await {
                if let Ok(connections) = store.storage_connections().load_all().await {
                    let _ = cx.update_global::<StorageState, _>(|state, _cx| {
                        state.saved_connections = connections;
                    });
                }
            }
        })
        .detach();
    }

    /// Check if connected to any storage.
    pub fn is_connected(&self) -> bool {
        matches!(self.connection_status, StorageConnectionStatus::Connected)
    }

    /// Get the current storage type if connected.
    pub fn current_storage_type(&self) -> Option<StorageType> {
        self.active_connection.as_ref().map(|c| c.storage_type)
    }
}
