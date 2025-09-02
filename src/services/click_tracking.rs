// Click event tracking service for ClickHouse analytics
// Implements high-performance event tracking with 3-buffer configuration

use crate::db::ClickHouseClient;
use chrono::{DateTime, NaiveDate, Utc};
use std::net::IpAddr;
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{error, info};
use uuid::Uuid;
use woothee::parser::Parser;

// =============================================================================
// CLICK EVENT STRUCTURE
// =============================================================================

/// Click event for internal processing
#[derive(Debug, Clone)]
pub struct ClickEvent {
    pub event_id: Uuid,
    pub link_id: Uuid,
    pub user_id: Option<Uuid>,    // Nullable(UUID) in ClickHouse
    pub timestamp: DateTime<Utc>, // DateTime64(3) in ClickHouse
    pub date: NaiveDate,          // Date in ClickHouse

    // Request information
    pub ip_address: String, // IPv6/IPv4 in ClickHouse - send as string
    pub user_agent: String, // LowCardinality(String) in CH
    pub referrer: String,

    // Geographic data
    pub country: String,
    pub country_code: String,
    pub city: String,
    pub region: String,

    // Device and browser - Enum8 in CH, needs to be u8 in Rust
    pub device_type: u8, // 0=unknown, 1=desktop, 2=mobile, 3=tablet, 4=bot
    pub device_brand: String,
    pub device_model: String,
    pub browser: String, // LowCardinality(String) in CH
    pub browser_version: String,
    pub os: String, // LowCardinality(String) in CH
    pub os_version: String,

    // Bot detection
    pub is_bot: bool,
    pub bot_name: String,

    // Performance metrics
    pub http_method: String, // LowCardinality(String) in CH
    pub response_time: u16,
    pub status_code: u16,

    // Campaign tracking
    pub utm_source: String,   // LowCardinality(String) in CH
    pub utm_medium: String,   // LowCardinality(String) in CH
    pub utm_campaign: String, // LowCardinality(String) in CH
}

// Note: ClickEventInsert struct removed - we use the SQL builder pattern instead
// The Row derive approach has fundamental limitations with Option<T> fields
// and byte count mismatches, so we use ClickHouseInsertBuilder for clean SQL generation

impl ClickEvent {
    /// Create a new click event from request data
    pub fn new(
        link_id: Uuid,
        ip: IpAddr,
        user_agent: &str,
        referrer: Option<&str>,
        method: &str,
        response_time_ms: u16,
        status_code: u16,
    ) -> Self {
        let timestamp = Utc::now();
        let date = timestamp.date_naive();

        // Parse user agent for device/browser info
        let parser = Parser::new();
        let ua_result = parser.parse(user_agent);

        // Determine device type as u8 for Enum8 in ClickHouse
        let device_type = if let Some(result) = &ua_result {
            match &*result.category {
                "pc" => 1,                         // desktop
                "smartphone" | "mobilephone" => 2, // mobile
                "tablet" => 3,                     // tablet
                "crawler" => 4,                    // bot
                _ => 0,                            // unknown
            }
        } else {
            0 // unknown
        };

        // Extract browser info
        let (browser, browser_version) = if let Some(result) = &ua_result {
            (result.name.to_string(), result.version.to_string())
        } else {
            (String::new(), String::new())
        };

        // Extract OS info
        let (os, os_version) = if let Some(result) = &ua_result {
            (result.os.to_string(), result.os_version.to_string())
        } else {
            (String::new(), String::new())
        };

        // Bot detection
        let is_bot = ua_result
            .as_ref()
            .map(|r| r.category == "crawler")
            .unwrap_or(false);

        let bot_name = if is_bot {
            ua_result
                .as_ref()
                .map(|r| r.name.to_string())
                .unwrap_or_else(|| String::new())
        } else {
            String::new()
        };

        // Extract UTM parameters from referrer if present
        let (utm_source, utm_medium, utm_campaign) = Self::extract_utm_params(referrer);

        Self {
            event_id: Uuid::new_v4(),
            link_id,
            user_id: None, // No authenticated user tracking for now
            timestamp,
            date,
            ip_address: ip.to_string(),
            user_agent: user_agent.to_string(),
            referrer: referrer.unwrap_or("").to_string(),
            country: String::new(),      // Will be populated by GeoIP later
            country_code: String::new(), // Will be populated by GeoIP later
            city: String::new(),         // Will be populated by GeoIP later
            region: String::new(),       // Will be populated by GeoIP later
            device_type,
            device_brand: String::new(), // TODO: Extract from user agent
            device_model: String::new(), // TODO: Extract from user agent
            browser,
            browser_version,
            os,
            os_version,
            is_bot,
            bot_name,
            http_method: method.to_string(),
            response_time: response_time_ms,
            status_code,
            utm_source,
            utm_medium,
            utm_campaign,
        }
    }

    /// Extract UTM parameters from URL
    fn extract_utm_params(referrer: Option<&str>) -> (String, String, String) {
        if let Some(url_str) = referrer {
            if let Ok(url) = url::Url::parse(url_str) {
                let params: std::collections::HashMap<_, _> = url.query_pairs().collect();
                return (
                    params
                        .get("utm_source")
                        .map(|s| s.to_string())
                        .unwrap_or_default(),
                    params
                        .get("utm_medium")
                        .map(|s| s.to_string())
                        .unwrap_or_default(),
                    params
                        .get("utm_campaign")
                        .map(|s| s.to_string())
                        .unwrap_or_default(),
                );
            }
        }
        (String::new(), String::new(), String::new())
    }
}

// =============================================================================
// CLICKHOUSE CLIENT SERVICE
// =============================================================================

/// ClickHouse client for writing click events
pub struct ClickTrackingService {
    clickhouse_client: Arc<ClickHouseClient>,
    // Channel for async event batching
    tx: mpsc::UnboundedSender<ClickEvent>,
}

impl ClickTrackingService {
    /// Create a new click tracking service with ClickHouse client
    pub fn new(clickhouse_client: Arc<ClickHouseClient>) -> Self {
        let (tx, mut rx) = mpsc::unbounded_channel::<ClickEvent>();

        // Spawn background task to batch and write events
        let client_for_task = clickhouse_client.clone();
        tokio::spawn(async move {
            let mut batch = Vec::with_capacity(1000);
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(2));

            loop {
                tokio::select! {
                    Some(event) = rx.recv() => {
                        batch.push(event);

                        // Write batch if it reaches 1000 events
                        if batch.len() >= 1000 {
                            Self::write_batch(&client_for_task, &mut batch).await;
                        }
                    }
                    _ = interval.tick() => {
                        // Write batch every 2 seconds if not empty
                        if !batch.is_empty() {
                            Self::write_batch(&client_for_task, &mut batch).await;
                        }
                    }
                }
            }
        });

        Self {
            clickhouse_client,
            tx,
        }
    }

    /// Track a click event (fire-and-forget with batching)
    pub fn track_click(&self, event: ClickEvent) {
        // Determine which buffer to use based on hash of event_id
        let buffer_table = Self::select_buffer(&event);

        // Send to background task for batching
        if let Err(e) = self.tx.send(event.clone()) {
            error!("Failed to queue click event: {}", e);

            // Fallback: Try direct write as last resort
            let client = self.clickhouse_client.clone();
            tokio::spawn(async move {
                if let Err(e) = Self::write_single(&client, &event, &buffer_table).await {
                    error!("Failed to write click event directly: {}", e);
                }
            });
        }
    }

    /// Select buffer table based on event characteristics
    fn select_buffer(event: &ClickEvent) -> String {
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
    async fn write_batch(clickhouse_client: &ClickHouseClient, batch: &mut Vec<ClickEvent>) {
        if batch.is_empty() {
            return;
        }

        info!(
            "Writing batch of {} click events to ClickHouse",
            batch.len()
        );

        // Group events by buffer table
        let mut buffer1_events = Vec::new();
        let mut buffer2_events = Vec::new();
        let mut buffer3_events = Vec::new();

        for event in batch.drain(..) {
            match Self::select_buffer(&event).as_str() {
                "link_events_buffer1" => buffer1_events.push(event),
                "link_events_buffer2" => buffer2_events.push(event),
                "link_events_buffer3" => buffer3_events.push(event),
                _ => buffer1_events.push(event), // Default fallback
            }
        }

        // Write to each buffer - the client handles database prefixing
        if !buffer1_events.is_empty() {
            if let Err(e) =
                Self::write_to_buffer(clickhouse_client, &buffer1_events, "link_events_buffer1")
                    .await
            {
                error!(
                    "Failed to write {} events to buffer1: {}",
                    buffer1_events.len(),
                    e
                );
            }
        }

        if !buffer2_events.is_empty() {
            if let Err(e) =
                Self::write_to_buffer(clickhouse_client, &buffer2_events, "link_events_buffer2")
                    .await
            {
                error!(
                    "Failed to write {} events to buffer2: {}",
                    buffer2_events.len(),
                    e
                );
            }
        }

        if !buffer3_events.is_empty() {
            if let Err(e) =
                Self::write_to_buffer(clickhouse_client, &buffer3_events, "link_events_buffer3")
                    .await
            {
                error!(
                    "Failed to write {} events to buffer3: {}",
                    buffer3_events.len(),
                    e
                );
            }
        }
    }

    /// Write events to a specific buffer table using the SQL builder
    async fn write_to_buffer(
        clickhouse_client: &ClickHouseClient,
        events: &[ClickEvent],
        table: &str,
    ) -> Result<(), clickhouse::error::Error> {
        if events.is_empty() {
            return Ok(());
        }

        info!("Writing batch of {} events to {}", events.len(), table);

        // Use the ClickHouseClient's insert method - cleaner API
        clickhouse_client.insert_link_events(table, events).await
    }

    /// Write a single event (fallback method)
    async fn write_single(
        clickhouse_client: &ClickHouseClient,
        event: &ClickEvent,
        buffer_name: &str,
    ) -> Result<(), clickhouse::error::Error> {
        // Use the ClickHouseClient's insert method - handles database prefixing
        clickhouse_client
            .insert_link_events(buffer_name, &[event.clone()])
            .await
    }
}

impl Clone for ClickTrackingService {
    fn clone(&self) -> Self {
        Self {
            clickhouse_client: self.clickhouse_client.clone(),
            tx: self.tx.clone(),
        }
    }
}

// =============================================================================
// GEOIP SERVICE (PLACEHOLDER)
// =============================================================================

/// Populate geographic data from IP address
pub async fn enrich_with_geoip(event: &mut ClickEvent) {
    // TODO: Implement GeoIP lookup
    // For now, set defaults
    if event.country_code.is_empty() {
        event.country_code = "US".to_string();
    }
    if event.city.is_empty() {
        event.city = "Unknown".to_string();
    }
}
