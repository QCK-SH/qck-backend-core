// ClickHouse Analytics Service
// Unified service for ALL ClickHouse operations - analytics and event tracking
// Built on top of the ClickHouse Query Builder for clean abstraction

use crate::db::{ClickHouseClient, ClickHouseQueryBuilder, SingleLinkStats};
use crate::services::click_tracking::ClickEvent;
use crate::services::link::LinkClickStats;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{error, info, warn};
use uuid::Uuid;

/// Unified ClickHouse service for analytics and event tracking
pub struct ClickHouseAnalyticsService {
    client: Arc<ClickHouseClient>,
    query_builder: ClickHouseQueryBuilder,
    // Event tracking channel for async batching
    event_tx: Option<mpsc::UnboundedSender<ClickEvent>>,
}

impl ClickHouseAnalyticsService {
    /// Create a new ClickHouse analytics service with event tracking
    pub fn new(client: Arc<ClickHouseClient>) -> Self {
        let query_builder = ClickHouseQueryBuilder::new(&client.database());

        // Setup event tracking channel and background task
        let (tx, mut rx) = mpsc::unbounded_channel::<ClickEvent>();

        // Spawn background task for event batching
        let client_for_task = client.clone();
        tokio::spawn(async move {
            let mut batch = Vec::with_capacity(1000);
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(2));

            loop {
                tokio::select! {
                    Some(event) = rx.recv() => {
                        batch.push(event);

                        // Write batch if it reaches 1000 events
                        if batch.len() >= 1000 {
                            Self::write_event_batch(&client_for_task, &mut batch).await;
                        }
                    }
                    _ = interval.tick() => {
                        // Write batch every 2 seconds if not empty
                        if !batch.is_empty() {
                            Self::write_event_batch(&client_for_task, &mut batch).await;
                        }
                    }
                }
            }
        });

        Self {
            client,
            query_builder,
            event_tx: Some(tx),
        }
    }

    /// Get the underlying ClickHouse client
    pub fn client(&self) -> Arc<ClickHouseClient> {
        self.client.clone()
    }

    /// Get statistics for a single link
    pub async fn get_link_stats(&self, link_id: &Uuid) -> Option<LinkClickStats> {
        let query = self.query_builder.build_single_link_stats(link_id);

        let stats_result = self
            .client
            .client()
            .query(&query)
            .fetch_one::<SingleLinkStats>()
            .await;

        match stats_result {
            Ok((total_clicks, unique_visitors, bot_clicks)) => {
                if total_clicks > 0 || unique_visitors > 0 || bot_clicks > 0 {
                    Some(LinkClickStats {
                        total_clicks,
                        unique_visitors,
                        bot_clicks,
                        last_accessed_at: Some(chrono::Utc::now()),
                    })
                } else {
                    None // No stats available
                }
            },
            Err(e) => {
                warn!("Failed to fetch ClickHouse stats for {}: {:?}", link_id, e);
                None
            },
        }
    }

    /// Get statistics for multiple links
    pub async fn get_bulk_link_stats(&self, link_ids: &[Uuid]) -> HashMap<Uuid, LinkClickStats> {
        let mut stats_map = HashMap::new();

        if link_ids.is_empty() {
            return stats_map;
        }

        info!("Fetching ClickHouse stats for {} links", link_ids.len());

        // Execute individual queries for reliability (avoids deserialization issues)
        for link_id in link_ids {
            if let Some(stats) = self.get_link_stats(link_id).await {
                stats_map.insert(*link_id, stats);
            }
        }

        if !stats_map.is_empty() {
            info!(
                "Retrieved ClickHouse stats for {} active links",
                stats_map.len()
            );
        }

        stats_map
    }

    /// Check if ClickHouse has any events for a link
    pub async fn has_events(&self, link_id: &Uuid) -> bool {
        let query = self.query_builder.build_link_exists_check(link_id);

        match self.client.client().query(&query).fetch_one::<u64>().await {
            Ok(count) => count > 0,
            Err(_) => false,
        }
    }

    /// Get health check - total events count
    pub async fn health_check(&self) -> Result<u64, String> {
        let query = self.query_builder.build_health_check_query();

        match self.client.client().query(&query).fetch_one::<u64>().await {
            Ok(count) => Ok(count),
            Err(e) => Err(format!("ClickHouse health check failed: {:?}", e)),
        }
    }

    /// Get top performing links for a user
    pub async fn get_top_links(&self, user_id: Option<&Uuid>, limit: u32) -> Vec<(Uuid, u64)> {
        let _query = self.query_builder.build_top_links_query(user_id, limit);

        // For simplicity, return empty vec for now since this would need custom deserialization
        // Can be implemented later with specific tuple types
        warn!("get_top_links not yet implemented - requires custom query handling");
        vec![]
    }

    // =============================================================================
    // EVENT TRACKING METHODS (unified from ClickTrackingService)
    // =============================================================================

    /// Track a click event (fire-and-forget with batching)
    pub fn track_click(&self, event: ClickEvent) {
        if let Some(ref tx) = self.event_tx {
            // Send to background task for batching
            if let Err(e) = tx.send(event.clone()) {
                error!("Failed to queue click event: {}", e);

                // Fallback: Try direct write as last resort
                let client = self.client.clone();
                let buffer_table = Self::select_buffer_table(&event);
                tokio::spawn(async move {
                    if let Err(e) = Self::write_single_event(&client, &event, &buffer_table).await {
                        error!("Failed to write click event directly: {}", e);
                    }
                });
            }
        } else {
            warn!("Event tracking not initialized");
        }
    }

    /// Select buffer table based on event characteristics
    fn select_buffer_table(event: &ClickEvent) -> String {
        if event.is_bot {
            // Bot traffic goes to buffer2 (isolated)
            "link_events_buffer2".to_string()
        } else {
            // User traffic distributed between buffer1 and buffer3
            let hash = event.event_id.as_u128();
            if hash % 2 == 0 {
                "link_events_buffer1".to_string()
            } else {
                "link_events_buffer3".to_string()
            }
        }
    }

    /// Write a batch of events to ClickHouse
    async fn write_event_batch(client: &ClickHouseClient, batch: &mut Vec<ClickEvent>) {
        if batch.is_empty() {
            return;
        }

        // Group events by buffer table
        let mut buffer_groups: HashMap<String, Vec<ClickEvent>> = HashMap::new();
        for event in batch.drain(..) {
            let buffer = Self::select_buffer_table(&event);
            buffer_groups
                .entry(buffer)
                .or_insert_with(Vec::new)
                .push(event);
        }

        // Write each buffer group
        for (buffer_table, events) in buffer_groups {
            // Use the public insert_link_events function
            match crate::db::clickhouse_insert_builder::insert_link_events(
                client.client(),
                &buffer_table,
                &events,
            )
            .await
            {
                Ok(()) => {
                    info!(
                        "Successfully wrote {} events to {}",
                        events.len(),
                        buffer_table
                    );
                },
                Err(e) => {
                    error!("Failed to write batch to {}: {}", buffer_table, e);
                },
            }
        }
    }

    /// Write a single event to ClickHouse (fallback method)
    async fn write_single_event(
        client: &ClickHouseClient,
        event: &ClickEvent,
        buffer_table: &str,
    ) -> Result<(), String> {
        let events = vec![event.clone()];

        crate::db::clickhouse_insert_builder::insert_link_events(
            client.client(),
            buffer_table,
            &events,
        )
        .await
        .map_err(|e| format!("Failed to write single event: {}", e))
    }
}

/// Factory function to create ClickHouse Analytics Service
pub fn create_clickhouse_analytics_service() -> ClickHouseAnalyticsService {
    let client = crate::db::clickhouse_client::create_clickhouse_client();
    ClickHouseAnalyticsService::new(client)
}
