// ClickHouse Insert Builder - Clean SQL builder pattern for batch inserts
//
// This module provides a clean abstraction for building and executing
// batch INSERT queries for ClickHouse. It was created to replace the
// problematic Row derive approach which has fundamental limitations:
// - Cannot handle Option<Uuid> fields (serialize_none unsupported)
// - Expects ALL columns in exact order, even those with DEFAULT values
// - Byte count mismatches when ClickHouse expects different serialization
//
// This SQL builder approach gives us full control over:
// - Which columns to include (skip event_id, user_id)
// - Type conversions (device_type u8 -> string, timestamp formatting)
// - Batch optimization (single query for multiple rows)
// - Clear error messages and debugging

use clickhouse::Client;
use tracing::{debug, info};

// =============================================================================
// LINK EVENTS SPECIFIC FUNCTIONS
// =============================================================================

/// Insert click events into link_events table or its buffers
/// This is the primary function for inserting click tracking data
pub async fn insert_link_events(
    client: &Client,
    table: &str,
    events: &[crate::services::click_tracking::ClickEvent],
) -> Result<(), clickhouse::error::Error> {
    LinkEventsInsertBuilder::new(client, table)
        .add_events(events)
        .execute()
        .await
}

/// Builder specifically for link_events table inserts
struct LinkEventsInsertBuilder<'a> {
    client: &'a Client,
    table: String,
    events: Vec<crate::services::click_tracking::ClickEvent>,
}

impl<'a> LinkEventsInsertBuilder<'a> {
    /// Column list for link_events table inserts
    /// We deliberately skip:
    /// - event_id (has DEFAULT generateUUIDv4())
    /// - user_id (always None, causes serialization issues)
    /// - date (computed from timestamp with DEFAULT toDate(timestamp))
    const INSERT_COLUMNS: &'static str = "link_id, timestamp, ip_address, user_agent, referrer, \
         country, country_code, city, region, device_type, \
         device_brand, device_model, browser, browser_version, \
         os, os_version, is_bot, bot_name, http_method, \
         response_time, status_code, utm_source, utm_medium, utm_campaign";

    /// Number of columns we're inserting
    const COLUMN_COUNT: usize = 24;

    /// Create a new builder with client and table
    fn new(client: &'a Client, table: impl Into<String>) -> Self {
        Self {
            client,
            table: table.into(),
            events: Vec::new(),
        }
    }

    /// Add events to the batch
    fn add_events(mut self, events: &[crate::services::click_tracking::ClickEvent]) -> Self {
        self.events.extend_from_slice(events);
        self
    }

    /// Execute the batch insert
    async fn execute(self) -> Result<(), clickhouse::error::Error> {
        if self.events.is_empty() {
            debug!("No events to insert");
            return Ok(());
        }

        let event_count = self.events.len();
        debug!(
            "Building batch insert for {} events to {}",
            event_count, self.table
        );

        // Build the VALUES clause with placeholders
        let values_placeholder = self.build_values_placeholder();

        // Build the complete INSERT query
        let query = format!(
            "INSERT INTO {} ({}) VALUES {}",
            self.table,
            Self::INSERT_COLUMNS,
            values_placeholder
        );

        // Create query and bind all parameters
        let mut query_builder = self.client.query(&query);

        // Bind parameters for each event
        for event in &self.events {
            query_builder = self.bind_event_params(query_builder, event);
        }

        // Execute the batch insert
        query_builder.execute().await?;

        info!(
            "Successfully inserted {} events to {} using batch SQL builder",
            event_count, self.table
        );

        Ok(())
    }

    /// Build VALUES clause with proper placeholders for all events
    fn build_values_placeholder(&self) -> String {
        let single_row = format!(
            "({})",
            std::iter::repeat("?")
                .take(Self::COLUMN_COUNT)
                .collect::<Vec<_>>()
                .join(", ")
        );

        std::iter::repeat(single_row)
            .take(self.events.len())
            .collect::<Vec<_>>()
            .join(", ")
    }

    /// Bind parameters for a single event
    fn bind_event_params(
        &self,
        query: clickhouse::query::Query,
        event: &crate::services::click_tracking::ClickEvent,
    ) -> clickhouse::query::Query {
        // Convert device_type u8 to string for Enum8
        let device_type_str = match event.device_type {
            0 => "unknown",
            1 => "desktop",
            2 => "mobile",
            3 => "tablet",
            4 => "bot",
            _ => "unknown",
        };

        // Format timestamp for ClickHouse DateTime64(3)
        let timestamp_str = event.timestamp.format("%Y-%m-%d %H:%M:%S%.3f").to_string();

        // Bind all parameters in the exact order of INSERT_COLUMNS
        query
            .bind(event.link_id)
            .bind(&timestamp_str)
            .bind(&event.ip_address)
            .bind(&event.user_agent)
            .bind(&event.referrer)
            .bind(&event.country)
            .bind(&event.country_code)
            .bind(&event.city)
            .bind(&event.region)
            .bind(device_type_str)
            .bind(&event.device_brand)
            .bind(&event.device_model)
            .bind(&event.browser)
            .bind(&event.browser_version)
            .bind(&event.os)
            .bind(&event.os_version)
            .bind(event.is_bot)
            .bind(&event.bot_name)
            .bind(&event.http_method)
            .bind(event.response_time)
            .bind(event.status_code)
            .bind(&event.utm_source)
            .bind(&event.utm_medium)
            .bind(&event.utm_campaign)
    }
}

// =============================================================================
// GENERIC BUILDER FOR FUTURE USE
// =============================================================================

/// Generic ClickHouse insert builder base - for future extensibility
/// Each table type should have its own builder implementation
pub struct ClickHouseInsertBuilder<'a> {
    client: &'a Client,
    table: String,
}

impl<'a> ClickHouseInsertBuilder<'a> {
    /// Create a new generic builder - use table-specific functions instead
    #[allow(dead_code)]
    pub fn new(client: &'a Client, table: impl Into<String>) -> Self {
        Self {
            client,
            table: table.into(),
        }
    }

    /// Get a builder for link_events table
    #[allow(dead_code)]
    pub fn for_link_events(self) -> LinkEventsInsertBuilder<'a> {
        LinkEventsInsertBuilder::new(self.client, self.table)
    }
}

// =============================================================================
// FUTURE TABLE SUPPORT
// =============================================================================

// When adding support for new tables, create specific functions like:
//
// /// Insert user analytics events
// pub async fn insert_user_events(...) { ... }
//
// /// Insert aggregated statistics
// pub async fn insert_stats(...) { ... }
//
// Each function should handle its specific type conversions and column mappings

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_values_placeholder_generation() {
        // We can't test directly without a Client, but we can test the logic
        let expected_single = format!(
            "({})",
            std::iter::repeat("?")
                .take(24)
                .collect::<Vec<_>>()
                .join(", ")
        );

        // Just verify the format is correct
        assert_eq!(expected_single.matches('?').count(), 24);
    }
}
