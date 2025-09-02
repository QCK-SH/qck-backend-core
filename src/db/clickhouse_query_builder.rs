// ClickHouse Query Builder
// Provides a safe, flexible way to build ClickHouse queries without struct deserialization
// Uses raw queries with primitive types to bypass clickhouse-rs deserialization issues

use uuid::Uuid;

/// ClickHouse Query Builder for analytics queries
/// Bypasses clickhouse-rs deserialization by using primitive types
pub struct ClickHouseQueryBuilder {
    database: String,
}

impl ClickHouseQueryBuilder {
    pub fn new(database: &str) -> Self {
        Self {
            database: database.to_string(),
        }
    }

    /// Build a query to get link statistics for a single link
    pub fn build_single_link_stats(&self, link_id: &Uuid) -> String {
        // Query the new AggregatingMergeTree table using -Merge functions
        format!(
            "SELECT 
                COALESCE(sumMerge(total_clicks), 0) as total_clicks,
                COALESCE(uniqMerge(unique_visitors), 0) as unique_visitors,
                COALESCE(sumMerge(total_bots), 0) as bot_clicks
            FROM {}.link_totals 
            WHERE link_id = '{}'",
            self.database, link_id
        )
    }

    /// Build a query to get link statistics for multiple links
    pub fn build_bulk_link_stats(&self, link_ids: &[Uuid]) -> String {
        let link_id_list: Vec<String> = link_ids.iter().map(|id| format!("'{}'", id)).collect();

        // Query the new AggregatingMergeTree table using -Merge functions
        format!(
            "SELECT 
                link_id,
                sumMerge(total_clicks) as total_clicks,
                uniqMerge(unique_visitors) as unique_visitors,
                sumMerge(total_bots) as bot_clicks
            FROM {}.link_totals 
            WHERE link_id IN ({})
            GROUP BY link_id",
            self.database,
            link_id_list.join(", ")
        )
    }

    /// Build a query to check if any events exist for a link
    pub fn build_link_exists_check(&self, link_id: &Uuid) -> String {
        format!(
            "SELECT COUNT(*) FROM {}.link_events WHERE link_id = '{}'",
            self.database, link_id
        )
    }

    /// Build a query to get top links by clicks
    pub fn build_top_links_query(&self, user_id: Option<&Uuid>, limit: u32) -> String {
        let user_filter = match user_id {
            Some(uid) => format!(" AND user_id = '{}'", uid),
            None => String::new(),
        };

        format!(
            "SELECT 
                link_id,
                COUNT(*) as total_clicks,
                COUNT(DISTINCT ip_address) as unique_visitors,
                MAX(timestamp) as last_click
            FROM {}.link_events 
            WHERE timestamp >= now() - INTERVAL 30 DAY{}
            GROUP BY link_id
            ORDER BY total_clicks DESC
            LIMIT {}",
            self.database, user_filter, limit
        )
    }

    /// Build a query to get time-based analytics
    pub fn build_time_series_query(&self, link_id: &Uuid, days: u32) -> String {
        format!(
            "SELECT 
                toDate(timestamp) as date,
                COUNT(*) as clicks,
                COUNT(DISTINCT ip_address) as unique_visitors
            FROM {}.link_events 
            WHERE link_id = '{}' 
                AND timestamp >= now() - INTERVAL {} DAY
            GROUP BY date
            ORDER BY date ASC",
            self.database, link_id, days
        )
    }

    /// Build a query to get geographic analytics
    pub fn build_geo_analytics(&self, link_id: &Uuid) -> String {
        format!(
            "SELECT 
                country,
                country_code,
                COUNT(*) as clicks,
                COUNT(DISTINCT ip_address) as unique_visitors
            FROM {}.link_events 
            WHERE link_id = '{}' 
                AND country != ''
            GROUP BY country, country_code
            ORDER BY clicks DESC
            LIMIT 20",
            self.database, link_id
        )
    }

    /// Build a query to get device/browser analytics
    pub fn build_device_analytics(&self, link_id: &Uuid) -> String {
        format!(
            "SELECT 
                device_type,
                browser,
                os,
                COUNT(*) as clicks
            FROM {}.link_events 
            WHERE link_id = '{}' 
            GROUP BY device_type, browser, os
            ORDER BY clicks DESC
            LIMIT 20",
            self.database, link_id
        )
    }

    /// Build a query to get referrer analytics
    pub fn build_referrer_analytics(&self, link_id: &Uuid) -> String {
        format!(
            "SELECT 
                referrer,
                COUNT(*) as clicks,
                COUNT(DISTINCT ip_address) as unique_visitors
            FROM {}.link_events 
            WHERE link_id = '{}' 
                AND referrer != ''
            GROUP BY referrer
            ORDER BY clicks DESC
            LIMIT 20",
            self.database, link_id
        )
    }

    /// Build a query for hourly analytics (last 24 hours)
    pub fn build_hourly_analytics(&self, link_id: &Uuid) -> String {
        format!(
            "SELECT 
                toHour(timestamp) as hour,
                COUNT(*) as clicks,
                COUNT(DISTINCT ip_address) as unique_visitors
            FROM {}.link_events 
            WHERE link_id = '{}' 
                AND timestamp >= now() - INTERVAL 24 HOUR
            GROUP BY hour
            ORDER BY hour ASC",
            self.database, link_id
        )
    }

    /// Build a query to check total events count (for health checks)
    pub fn build_health_check_query(&self) -> String {
        format!("SELECT COUNT(*) FROM {}.link_events", self.database)
    }
}

/// Response structures for different query types
/// These represent the expected tuple structures for raw queries

/// Single link stats: (total_clicks, unique_visitors, bot_clicks)
pub type SingleLinkStats = (u64, u64, u64);

/// Bulk link stats: (link_id, total_clicks, unique_visitors, bot_clicks)  
pub type BulkLinkStatsRow = (String, u64, u64, u64); // link_id as String for parsing

/// Time series row: (date, clicks, unique_visitors)
pub type TimeSeriesRow = (String, u64, u64); // date as String

/// Geographic row: (country, country_code, clicks, unique_visitors)
pub type GeoRow = (String, String, u64, u64);

/// Device analytics row: (device_type, browser, os, clicks)
pub type DeviceRow = (String, String, String, u64);

/// Referrer row: (referrer, clicks, unique_visitors)
pub type ReferrerRow = (String, u64, u64);

/// Hourly row: (hour, clicks, unique_visitors)
pub type HourlyRow = (u8, u64, u64);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_single_link_stats_query() {
        let builder = ClickHouseQueryBuilder::new("test_db");
        let link_id = Uuid::new_v4();
        let query = builder.build_single_link_stats(&link_id);

        assert!(query.contains("COUNT(*) as total_clicks"));
        assert!(query.contains("COUNT(DISTINCT ip_address) as unique_visitors"));
        assert!(query.contains("SUM(CASE WHEN is_bot = 1 THEN 1 ELSE 0 END) as bot_clicks"));
        assert!(query.contains(&link_id.to_string()));
    }

    #[test]
    fn test_bulk_link_stats_query() {
        let builder = ClickHouseQueryBuilder::new("test_db");
        let link_ids = vec![Uuid::new_v4(), Uuid::new_v4()];
        let query = builder.build_bulk_link_stats(&link_ids);

        assert!(query.contains("GROUP BY link_id"));
        assert!(query.contains("WHERE link_id IN"));
        assert!(query.contains(&link_ids[0].to_string()));
        assert!(query.contains(&link_ids[1].to_string()));
    }

    #[test]
    fn test_health_check_query() {
        let builder = ClickHouseQueryBuilder::new("analytics");
        let query = builder.build_health_check_query();

        assert_eq!(query, "SELECT COUNT(*) FROM analytics.link_events");
    }
}
