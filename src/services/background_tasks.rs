// Background task scheduler for DEV-124 & DEV-105
// Handles periodic maintenance tasks for link management

use tracing::info;

use crate::app::AppState;

/// Background task manager for link services
pub struct BackgroundTaskManager {
    state: AppState,
}

impl BackgroundTaskManager {
    pub fn new(state: AppState) -> Self {
        Self { state }
    }

    /// Start all background tasks
    pub async fn start_all_tasks(&self) {
        info!("Starting background tasks for link management");

        // Click count sync is no longer needed since we fetch from ClickHouse directly
        // Add other background tasks here as needed

        // Example: Could add a task to periodically refresh ClickHouse materialized views
        // or cleanup expired links
    }
}

/// Initialize background tasks (call this in main.rs)
pub async fn initialize_background_tasks(state: AppState) {
    let task_manager = BackgroundTaskManager::new(state);
    task_manager.start_all_tasks().await;
}
