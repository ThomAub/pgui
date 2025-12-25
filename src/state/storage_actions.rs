//! Storage state actions.
//!
//! This module contains actions that modify the global StorageState.

use gpui::*;
use gpui_component::{notification::NotificationType, WindowExt as _};

use crate::services::{
    database::storage::{StorageConfig, StorageManager},
    storage::StorageConnectionsRepository,
    AppStore,
};

use super::storage::{StorageConnectionStatus, StorageState};

/// Connect to a storage backend.
pub fn storage_connect(config: &StorageConfig, cx: &mut App) {
    let config = config.clone();

    cx.update_global::<StorageState, _>(|state, _cx| {
        state.connection_status = StorageConnectionStatus::Connecting;
        state.active_connection = Some(config.clone());
    });

    let manager = cx.global::<StorageState>().storage_manager.clone();

    cx.spawn(async move |cx| {
        // Get secret from keyring if available
        let secret = StorageConnectionsRepository::get_connection_secret(&config.id).ok();

        // Create config with secret if needed
        let mut connect_config = config.clone();
        if let Some(secret) = secret {
            // Inject secret into params
            match &mut connect_config.params {
                crate::services::database::storage::StorageParams::S3 { .. } => {
                    // Secret is the secret_access_key, handled by StorageFactory
                }
                _ => {}
            }
        }

        let result = manager.connect(connect_config).await;

        let _ = cx.update_global::<StorageState, _>(|state, _cx| match result {
            Ok(_) => {
                state.connection_status = StorageConnectionStatus::Connected;
                state.current_path = "/".to_string();
                tracing::info!("Connected to storage: {}", config.name);
            }
            Err(e) => {
                state.connection_status = StorageConnectionStatus::Disconnected;
                state.active_connection = None;
                tracing::error!("Failed to connect to storage: {}", e);
            }
        });
    })
    .detach();
}

/// Disconnect from current storage backend.
pub fn storage_disconnect(cx: &mut App) {
    cx.update_global::<StorageState, _>(|state, _cx| {
        state.connection_status = StorageConnectionStatus::Disconnecting;
    });

    let manager = cx.global::<StorageState>().storage_manager.clone();

    cx.spawn(async move |cx| {
        let _ = manager.disconnect().await;

        let _ = cx.update_global::<StorageState, _>(|state, _cx| {
            state.connection_status = StorageConnectionStatus::Disconnected;
            state.active_connection = None;
            state.current_path = "/".to_string();
        });
    })
    .detach();
}

/// Add a new storage connection to saved connections.
pub fn add_storage_connection(config: StorageConfig, secret: Option<String>, cx: &mut App) {
    let config_clone = config.clone();

    cx.spawn(async move |cx| {
        match AppStore::singleton().await {
            Ok(store) => {
                match store
                    .storage_connections()
                    .create(&config_clone, secret.as_deref())
                    .await
                {
                    Ok(_) => {
                        // Reload connections
                        if let Ok(connections) = store.storage_connections().load_all().await {
                            let _ = cx.update_global::<StorageState, _>(|state, _cx| {
                                state.saved_connections = connections;
                            });
                        }
                        tracing::info!("Storage connection saved: {}", config_clone.name);
                    }
                    Err(e) => {
                        tracing::error!("Failed to save storage connection: {}", e);
                    }
                }
            }
            Err(e) => {
                tracing::error!("Failed to get app store: {}", e);
            }
        }
    })
    .detach();
}

/// Update an existing storage connection.
pub fn update_storage_connection(config: StorageConfig, secret: Option<String>, cx: &mut App) {
    let config_clone = config.clone();

    cx.spawn(async move |cx| {
        match AppStore::singleton().await {
            Ok(store) => {
                match store
                    .storage_connections()
                    .update(&config_clone, secret.as_deref())
                    .await
                {
                    Ok(_) => {
                        // Reload connections
                        if let Ok(connections) = store.storage_connections().load_all().await {
                            let _ = cx.update_global::<StorageState, _>(|state, _cx| {
                                state.saved_connections = connections;
                            });
                        }
                        tracing::info!("Storage connection updated: {}", config_clone.name);
                    }
                    Err(e) => {
                        tracing::error!("Failed to update storage connection: {}", e);
                    }
                }
            }
            Err(e) => {
                tracing::error!("Failed to get app store: {}", e);
            }
        }
    })
    .detach();
}

/// Delete a storage connection.
pub fn delete_storage_connection(config: StorageConfig, cx: &mut App) {
    let config_clone = config.clone();

    cx.spawn(async move |cx| {
        match AppStore::singleton().await {
            Ok(store) => {
                match store.storage_connections().delete(&config_clone.id).await {
                    Ok(_) => {
                        // Reload connections
                        if let Ok(connections) = store.storage_connections().load_all().await {
                            let _ = cx.update_global::<StorageState, _>(|state, _cx| {
                                state.saved_connections = connections;
                            });
                        }
                        tracing::info!("Storage connection deleted: {}", config_clone.name);
                    }
                    Err(e) => {
                        tracing::error!("Failed to delete storage connection: {}", e);
                    }
                }
            }
            Err(e) => {
                tracing::error!("Failed to get app store: {}", e);
            }
        }
    })
    .detach();
}

/// Test a storage connection without saving.
pub async fn test_storage_connection(config: StorageConfig) -> Result<(), String> {
    let manager = StorageManager::new();
    manager
        .test_connection(config)
        .await
        .map_err(|e| e.to_string())
}
