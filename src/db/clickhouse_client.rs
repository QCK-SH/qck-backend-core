// ClickHouse client configuration and connection management
// Centralizes all ClickHouse authentication and connection settings

use clickhouse::Client;
use std::sync::Arc;
use tracing::info;

/// ClickHouse client wrapper with proper authentication
#[derive(Clone)]
pub struct ClickHouseClient {
    client: Client,
    database: String,
}

impl ClickHouseClient {
    /// Create a new ClickHouse client from app configuration
    pub fn from_config() -> Self {
        let config = crate::app_config::config();

        let client = Client::default()
            .with_url(&config.clickhouse_url)
            .with_database(&config.clickhouse_database)
            .with_user(&config.clickhouse_user)
            .with_password(&config.clickhouse_password);

        info!(
            "ClickHouse client initialized for database: {}",
            config.clickhouse_database
        );

        Self {
            client,
            database: config.clickhouse_database.clone(),
        }
    }

    /// Get the underlying clickhouse::Client
    pub fn client(&self) -> &Client {
        &self.client
    }

    /// Get the database name
    pub fn database(&self) -> &str {
        &self.database
    }

    /// Test the connection
    pub async fn health_check(&self) -> Result<(), clickhouse::error::Error> {
        self.client.query("SELECT 1").fetch_one::<u8>().await?;
        Ok(())
    }

    /// Insert link events into the specified table (with or without database prefix)
    /// This is a convenience method that delegates to the insert builder
    pub async fn insert_link_events(
        &self,
        table: &str,
        events: &[crate::services::click_tracking::ClickEvent],
    ) -> Result<(), clickhouse::error::Error> {
        // Use the full table name if no database is specified
        let full_table = if table.contains('.') {
            table.to_string()
        } else {
            format!("{}.{}", self.database, table)
        };

        crate::db::insert_link_events(&self.client, &full_table, events).await
    }
}

/// Create a shared ClickHouse client instance
pub fn create_clickhouse_client() -> Arc<ClickHouseClient> {
    Arc::new(ClickHouseClient::from_config())
}
