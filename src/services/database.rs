use anyhow::Result;
use async_std::sync::RwLock;
use std::sync::Arc;

use crate::services::{
    database_adapter::{DatabaseAdapter, DatabaseType, QueryExecutionResult, QueryResult, TableInfo},
    database_factory::DatabaseFactory,
};

// Re-export types that were previously defined here but are now in database_adapter


#[derive(Clone)]
pub struct DatabaseManager {
    adapter: Arc<RwLock<Option<Box<dyn DatabaseAdapter>>>>,
    current_db_type: Arc<RwLock<Option<DatabaseType>>>,
}

impl DatabaseManager {
    pub fn new() -> Self {
        Self {
            adapter: Arc::new(RwLock::new(None)),
            current_db_type: Arc::new(RwLock::new(None)),
        }
    }

    pub async fn connect(&self, database_url: &str) -> Result<()> {
        // Create the appropriate adapter based on the URL
        let mut adapter = DatabaseFactory::create_adapter(database_url)?;
        let db_type = adapter.database_type();
        
        // Connect using the adapter
        adapter.connect(database_url).await?;
        
        // Store the adapter and database type
        let mut adapter_guard = self.adapter.write().await;
        *adapter_guard = Some(adapter);
        
        let mut db_type_guard = self.current_db_type.write().await;
        *db_type_guard = Some(db_type);
        
        Ok(())
    }

    pub async fn disconnect(&self) {
        let mut adapter_guard = self.adapter.write().await;
        if let Some(mut adapter) = adapter_guard.take() {
            let _ = adapter.disconnect().await;
        }
        
        let mut db_type_guard = self.current_db_type.write().await;
        *db_type_guard = None;
    }

    #[allow(dead_code)]
    pub async fn is_connected(&self) -> bool {
        let adapter_guard = self.adapter.read().await;
        match &*adapter_guard {
            Some(adapter) => adapter.is_connected().await,
            None => false,
        }
    }

    pub async fn get_tables(&self) -> Result<Vec<TableInfo>> {
        let adapter_guard = self.adapter.read().await;
        let adapter = adapter_guard
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Database not connected"))?;
        
        adapter.get_tables().await
    }

    pub async fn get_table_columns(
        &self,
        table_name: &str,
        table_schema: &str,
    ) -> Result<QueryResult> {
        let adapter_guard = self.adapter.read().await;
        let adapter = adapter_guard
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Database not connected"))?;
        
        adapter.get_table_columns(table_name, table_schema).await
    }

    pub async fn execute_query(&self, sql: &str) -> QueryExecutionResult {
        let adapter_guard = self.adapter.read().await;
        match &*adapter_guard {
            Some(adapter) => adapter.execute_query(sql).await,
            None => QueryExecutionResult::Error("Database not connected".to_string()),
        }
    }

    #[allow(dead_code)]
    pub async fn test_connection(&self) -> Result<bool> {
        let adapter_guard = self.adapter.read().await;
        let adapter = adapter_guard
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Database not connected"))?;
        
        adapter.test_connection().await
    }
    
    #[allow(dead_code)]
    pub async fn get_current_database_type(&self) -> Option<DatabaseType> {
        let db_type_guard = self.current_db_type.read().await;
        db_type_guard.clone()
    }
}
