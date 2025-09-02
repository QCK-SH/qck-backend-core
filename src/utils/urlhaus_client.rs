// URLhaus Threat Intelligence Client with ClickHouse backend
// Free malicious URL database from abuse.ch

use crate::db::ClickHouseClient;
use crate::CONFIG;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use thiserror::Error;
use tracing::{error, info};
use url::Url;

// =============================================================================
// ERROR TYPES
// =============================================================================

#[derive(Debug, Error)]
pub enum UrlhausError {
    #[error("ClickHouse error: {0}")]
    ClickHouse(#[from] clickhouse::error::Error),

    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),

    #[error("Parse error: {0}")]
    Parse(String),
}

// =============================================================================
// DATA STRUCTURES
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UrlhausThreat {
    pub urlhaus_id: u64,
    pub url: String,
    pub url_host: String,
    pub threat_type: String,
    pub tags: Vec<String>,
    pub url_status: String,
    pub first_seen: DateTime<Utc>,
    pub last_seen: DateTime<Utc>,
    pub reporter: String,
    pub urlhaus_link: String,
}

// =============================================================================
// URLHAUS CLIENT
// =============================================================================

pub struct UrlhausClient {
    clickhouse: Arc<ClickHouseClient>,
    http_client: reqwest::Client,
}

impl UrlhausClient {
    /// Create new URLhaus client with shared ClickHouse backend
    pub fn new(clickhouse_client: Arc<ClickHouseClient>) -> Self {
        let http_client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .user_agent("QCK-URLhaus-Client/1.0")
            .build()
            .unwrap_or_default();

        Self {
            clickhouse: clickhouse_client,
            http_client,
        }
    }

    /// Check if a URL is in the URLhaus threat database and get threat details
    pub async fn check_url(&self, url: &str) -> Result<bool, UrlhausError> {
        // Validate URL format
        if Url::parse(url).is_err() {
            return Ok(false); // Invalid URL, not in threat DB
        }

        // Use parameterized query to prevent SQL injection
        let mut cursor = self
            .clickhouse
            .client()
            .query(
                "SELECT COUNT(*) as count FROM urlhaus_threats 
                    WHERE url = ? 
                      AND url_status = 'online'
                    LIMIT 1",
            )
            .bind(url)
            .fetch::<u64>()?;

        if let Some(count) = cursor.next().await? {
            if count > 0 {
                tracing::warn!("URLhaus threat detected: URL={}", url);
                Ok(true)
            } else {
                Ok(false)
            }
        } else {
            Ok(false)
        }
    }

    /// Get detailed threat information for a URL
    /// Currently not used but kept for future API needs
    pub async fn get_url_threat_details(
        &self,
        url: &str,
    ) -> Result<Option<UrlhausThreat>, UrlhausError> {
        // Since we removed Row derive from UrlhausThreat, this method would need
        // to be rewritten with manual deserialization if needed in the future
        // For now, return None as this method is not currently called
        let _ = url;
        Ok(None)
    }

    /// Check if a domain is hosting malicious content
    pub async fn check_domain(&self, domain: &str) -> Result<u32, UrlhausError> {
        // Use parameterized query to prevent SQL injection
        let mut cursor = self
            .clickhouse
            .client()
            .query(
                "SELECT COUNT(*) as count FROM urlhaus_threats 
                    WHERE url_host = ? 
                      AND url_status = 'online'
                      AND last_checked >= now() - INTERVAL 24 HOUR",
            )
            .bind(domain)
            .fetch::<u32>()?;

        Ok(cursor.next().await?.unwrap_or(0))
    }

    /// Update URLhaus threat database from abuse.ch feed
    /// Always uses the online feed for simplicity
    pub async fn update_from_feed(&self) -> Result<u32, UrlhausError> {
        if !CONFIG.security.urlhaus_enabled {
            return Err(UrlhausError::Parse(
                "URLhaus is disabled in configuration".to_string(),
            ));
        }

        // Always use the online feed - it only contains currently active threats
        let feed_url = &CONFIG.security.urlhaus_feed_url;
        info!("Fetching URLhaus threat feed from {}", feed_url);

        // Fetch the online threats CSV
        let response = self.http_client.get(feed_url).send().await?;

        let text = response.text().await?;
        let mut threats = Vec::new();
        let mut online_count = 0u32;

        // Parse CSV and collect all threats first
        for line in text.lines() {
            if line.starts_with('#') || line.is_empty() {
                continue;
            }

            // Parse CSV line (handle quoted fields properly)
            let threat = self.parse_csv_line(line)?;
            if threat.url_status == "online" {
                online_count += 1;
            }
            threats.push(threat);

            // Respect max cache size from config
            if online_count >= CONFIG.security.urlhaus_max_cache_size as u32 {
                info!(
                    "Reached max cache size limit: {}",
                    CONFIG.security.urlhaus_max_cache_size
                );
                break;
            }
        }

        // Use atomic table swap to avoid security gap
        // 1. Create temporary table with new data
        self.clickhouse
            .client()
            .query("CREATE TABLE IF NOT EXISTS urlhaus_threats_temp AS urlhaus_threats")
            .execute()
            .await?;
        self.clickhouse
            .client()
            .query("TRUNCATE TABLE urlhaus_threats_temp")
            .execute()
            .await?;

        // 2. Insert all new threats into temp table
        for chunk in threats.chunks(1000) {
            self.insert_threats_to_table(chunk, "urlhaus_threats_temp")
                .await?;
        }

        // 3. Atomic swap: rename tables (nearly instantaneous)
        self.clickhouse.client().query("RENAME TABLE urlhaus_threats TO urlhaus_threats_old, urlhaus_threats_temp TO urlhaus_threats").execute().await?;

        // 4. Clean up old table
        self.clickhouse
            .client()
            .query("DROP TABLE IF EXISTS urlhaus_threats_old")
            .execute()
            .await?;

        info!("URLhaus update complete: {} threats loaded", online_count);

        Ok(online_count)
    }

    /// Parse CSV line from URLhaus feed with proper quote handling
    fn parse_csv_line(&self, line: &str) -> Result<UrlhausThreat, UrlhausError> {
        // Proper CSV parser that handles quoted fields with commas
        let mut parts = Vec::new();
        let mut current = String::new();
        let mut in_quotes = false;
        let mut chars = line.chars().peekable();

        while let Some(ch) = chars.next() {
            match ch {
                '"' => {
                    // Toggle quote state
                    in_quotes = !in_quotes;
                },
                ',' if !in_quotes => {
                    // End of field
                    parts.push(current.trim().to_string());
                    current = String::new();
                },
                _ => {
                    current.push(ch);
                },
            }
        }
        // Don't forget the last field
        parts.push(current.trim().to_string());

        if parts.len() < 9 {
            return Err(UrlhausError::Parse(format!(
                "Invalid CSV line with {} fields: {}",
                parts.len(),
                line
            )));
        }

        // Extract host from URL
        let url = &parts[2];
        let url_host = if let Ok(parsed) = Url::parse(url) {
            parsed.host_str().unwrap_or("").to_string()
        } else {
            String::new()
        };

        // Parse tags (already a comma-separated field, but within the CSV)
        let tags: Vec<String> = if parts.len() > 6 && !parts[6].is_empty() {
            parts[6]
                .split('|')  // URLhaus uses pipe separator for tags
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect()
        } else {
            Vec::new()
        };

        Ok(UrlhausThreat {
            urlhaus_id: parts[0].parse().unwrap_or(0),
            url: parts[2].clone(),
            url_host,
            threat_type: parts[5].clone(),
            tags,
            url_status: parts[3].clone(),
            first_seen: parts[1].parse().unwrap_or_else(|_| Utc::now()),
            last_seen: parts[4].parse().unwrap_or_else(|_| Utc::now()),
            reporter: if parts.len() > 8 {
                parts[8].clone()
            } else {
                String::new()
            },
            urlhaus_link: if parts.len() > 7 {
                parts[7].clone()
            } else {
                String::new()
            },
        })
    }

    /// Insert threats into ClickHouse table
    async fn insert_threats_to_table(
        &self,
        threats: &[UrlhausThreat],
        table_name: &str,
    ) -> Result<(), UrlhausError> {
        if threats.is_empty() {
            return Ok(());
        }

        // Build parameterized query with placeholders
        let placeholders: Vec<String> = threats
            .iter()
            .enumerate()
            .map(|(_, _)| format!("(?, ?, ?, ?, ?, ?, ?, ?, now(), ?, ?)"))
            .collect();

        let query = format!(
            "INSERT INTO {} (urlhaus_id, url, url_host, threat_type, tags, url_status, first_seen, last_seen, last_checked, reporter, urlhaus_link) VALUES {}",
            table_name,
            placeholders.join(",")
        );

        let mut insert = self.clickhouse.client().query(&query);

        // Bind parameters for each threat
        for threat in threats {
            // Format DateTime without fractional seconds for ClickHouse
            let first_seen_str = threat.first_seen.format("%Y-%m-%d %H:%M:%S").to_string();
            let last_seen_str = threat.last_seen.format("%Y-%m-%d %H:%M:%S").to_string();

            insert = insert
                .bind(threat.urlhaus_id)
                .bind(&threat.url)
                .bind(&threat.url_host)
                .bind(&threat.threat_type)
                .bind(threat.tags.clone())  // ClickHouse handles arrays natively
                .bind(&threat.url_status)
                .bind(first_seen_str)
                .bind(last_seen_str)
                .bind(&threat.reporter)
                .bind(&threat.urlhaus_link);
        }

        insert.execute().await?;

        Ok(())
    }
}

// Removed ThreatCheckRow - no longer needed after simplifying query to COUNT(*)

// =============================================================================
// BACKGROUND UPDATER
// =============================================================================

/// Spawn a background task to periodically update URLhaus data
/// Updates based on configured interval (default: daily)
pub fn spawn_urlhaus_updater() {
    if !CONFIG.security.urlhaus_enabled {
        info!("URLhaus threat intelligence is disabled in configuration");
        return;
    }

    tokio::spawn(async move {
        let clickhouse_client = crate::db::clickhouse_client::create_clickhouse_client();
        let client = UrlhausClient::new(clickhouse_client);

        // Initial update on startup
        info!("Running initial URLhaus feed update...");
        match client.update_from_feed().await {
            Ok(count) => {
                info!(
                    "Initial URLhaus update successful: {} threats loaded",
                    count
                );
            },
            Err(e) => {
                error!("Initial URLhaus update failed: {}", e);
            },
        }

        // Then update based on configured interval
        let update_interval_secs = CONFIG.security.urlhaus_update_interval_hours as u64 * 3600;
        let mut interval =
            tokio::time::interval(std::time::Duration::from_secs(update_interval_secs));

        // Skip the first tick since we just did the initial update
        interval.tick().await;

        loop {
            interval.tick().await;

            info!("Starting scheduled URLhaus feed update...");
            match client.update_from_feed().await {
                Ok(count) => {
                    info!("URLhaus daily update successful: {} threats loaded", count);
                },
                Err(e) => {
                    error!("URLhaus daily update failed: {}", e);
                },
            }
        }
    });
}
