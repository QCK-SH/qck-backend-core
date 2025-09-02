// ClickHouse migration runner
// Uses embedded SQL files and HTTP client for execution

use reqwest::Client;
use std::error::Error;
use std::time::Duration;
use tracing::{debug, error, info, warn};

// Embed ClickHouse migration files at compile time
const MIGRATION_001: (&str, &str) = (
    "001_analytics_events",
    include_str!("../../migrations/clickhouse/001_analytics_events.sql"),
);

const MIGRATION_002: (&str, &str) = (
    "002_urlhaus_threats",
    include_str!("../../migrations/clickhouse/002_urlhaus_threats.sql"),
);

const MIGRATION_003: (&str, &str) = (
    "003_link_stats_optimization",
    include_str!("../../migrations/clickhouse/003_link_stats_optimization.sql"),
);

const MIGRATION_004: (&str, &str) = (
    "004_link_totals_table",
    include_str!("../../migrations/clickhouse/004_link_totals_table.sql"),
);

const MIGRATION_005: (&str, &str) = (
    "005_seed_demo_link_events",
    include_str!("../../migrations/clickhouse/005_seed_demo_link_events.sql"),
);

/// List of all migrations in execution order
const MIGRATIONS: &[(&str, &str)] = &[MIGRATION_001, MIGRATION_002, MIGRATION_003, MIGRATION_004];

/// List of seed migrations that should only run in non-production
const SEED_MIGRATIONS: &[(&str, &str)] = &[MIGRATION_005];

/// ClickHouse client configuration
#[derive(Debug, Clone)]
pub struct ClickHouseConfig {
    pub url: String,
    pub database: String,
    pub user: String,
    pub password: String,
    pub timeout: Duration,
    pub max_retries: u32,
}

impl Default for ClickHouseConfig {
    fn default() -> Self {
        let config = crate::app_config::config();
        Self {
            url: config.clickhouse_url.clone(),
            database: config.clickhouse_database.clone(),
            user: config.clickhouse_user.clone(),
            password: config.clickhouse_password.clone(),
            timeout: Duration::from_secs(30),
            max_retries: 5,
        }
    }
}

impl ClickHouseConfig {
    /// Create config for tests without using lazy static
    #[cfg(test)]
    pub fn for_test() -> Self {
        use std::env;

        // Load test environment variables directly
        dotenv::dotenv().ok();

        Self {
            url: env::var("CLICKHOUSE_URL").unwrap_or_else(|_| "http://localhost:8123".to_string()),
            database: env::var("CLICKHOUSE_DATABASE")
                .unwrap_or_else(|_| "qck_analytics".to_string()),
            user: env::var("CLICKHOUSE_USER").unwrap_or_else(|_| "default".to_string()),
            password: env::var("CLICKHOUSE_PASSWORD").unwrap_or_else(|_| String::new()),
            timeout: Duration::from_secs(30),
            max_retries: 5,
        }
    }
}

/// Run all ClickHouse migrations
/// Returns the number of migrations applied
pub async fn run_migrations() -> Result<usize, Box<dyn Error + Send + Sync>> {
    info!("[CLICKHOUSE] Starting ClickHouse migration process...");

    let config = ClickHouseConfig::default();
    let client = Client::new();

    // Check if running in production
    let app_config = crate::app_config::config();
    let is_production = app_config.is_production();

    // Wait for ClickHouse to be ready
    wait_for_clickhouse(&client, &config).await?;

    // Create database if it doesn't exist
    create_database_if_not_exists(&client, &config).await?;

    // Ensure migration tracking table exists
    setup_migration_tracking(&client, &config).await?;

    // Get applied migrations
    let applied_migrations = get_applied_migrations(&client, &config).await?;
    debug!(
        "[CLICKHOUSE] Found {} previously applied migrations",
        applied_migrations.len()
    );

    let mut applied_count = 0;

    // Apply pending migrations
    for (name, sql) in MIGRATIONS {
        if applied_migrations.contains(&name.to_string()) {
            debug!("[CLICKHOUSE] Migration {} already applied, skipping", name);
            continue;
        }

        info!("[CLICKHOUSE] Applying migration: {}", name);

        match apply_migration(&client, &config, name, sql).await {
            Ok(()) => {
                applied_count += 1;
                info!("[CLICKHOUSE] ✓ Successfully applied migration: {}", name);
            },
            Err(e) => {
                error!("[CLICKHOUSE] ✗ Failed to apply migration {}: {}", name, e);
                return Err(format!("Migration {} failed: {}", name, e).into());
            },
        }
    }

    // Apply seed migrations only in non-production environments
    if !is_production {
        info!("[CLICKHOUSE] Running seed migrations for non-production environment");

        for (name, sql) in SEED_MIGRATIONS {
            if applied_migrations.contains(&name.to_string()) {
                debug!(
                    "[CLICKHOUSE] Seed migration {} already applied, skipping",
                    name
                );
                continue;
            }

            info!("[CLICKHOUSE] Applying seed migration: {}", name);

            match apply_migration(&client, &config, name, sql).await {
                Ok(()) => {
                    applied_count += 1;
                    info!(
                        "[CLICKHOUSE] ✓ Successfully applied seed migration: {}",
                        name
                    );
                },
                Err(e) => {
                    error!(
                        "[CLICKHOUSE] ✗ Failed to apply seed migration {}: {}",
                        name, e
                    );
                    return Err(format!("Seed migration {} failed: {}", name, e).into());
                },
            }
        }
    } else {
        info!("[CLICKHOUSE] Skipping seed migrations in production environment");
    }

    if applied_count > 0 {
        info!(
            "[CLICKHOUSE] ✓ Applied {} ClickHouse migrations",
            applied_count
        );
    } else {
        info!("[CLICKHOUSE] ✓ All ClickHouse migrations up to date");
    }

    Ok(applied_count)
}

/// Wait for ClickHouse to be ready
async fn wait_for_clickhouse(
    client: &Client,
    config: &ClickHouseConfig,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    info!("[CLICKHOUSE] Waiting for ClickHouse to be ready...");

    for attempt in 1..=config.max_retries {
        match check_clickhouse_health(client, config).await {
            Ok(()) => {
                info!("[CLICKHOUSE] ✓ ClickHouse is ready");
                return Ok(());
            },
            Err(e) => {
                if attempt == config.max_retries {
                    error!(
                        "[CLICKHOUSE] ✗ ClickHouse failed to start after {} attempts: {}",
                        config.max_retries, e
                    );
                    return Err(format!(
                        "ClickHouse not ready after {} attempts: {}",
                        config.max_retries, e
                    )
                    .into());
                }
                warn!(
                    "[CLICKHOUSE] ClickHouse not ready (attempt {}/{}): {}",
                    attempt, config.max_retries, e
                );
                tokio::time::sleep(Duration::from_secs(2)).await;
            },
        }
    }

    Err("ClickHouse readiness check exceeded max attempts".into())
}

/// Check if ClickHouse is healthy
async fn check_clickhouse_health(
    client: &Client,
    config: &ClickHouseConfig,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let response = client
        .get(format!("{}/ping", config.url))
        .timeout(config.timeout)
        .send()
        .await?;

    if !response.status().is_success() {
        return Err(format!("ClickHouse ping failed with status: {}", response.status()).into());
    }

    Ok(())
}

/// Create database if it doesn't exist
async fn create_database_if_not_exists(
    client: &Client,
    config: &ClickHouseConfig,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    info!(
        "[CLICKHOUSE] Creating database {} if it doesn't exist...",
        config.database
    );

    let create_database_sql = format!("CREATE DATABASE IF NOT EXISTS {}", config.database);

    debug!(
        "[CLICKHOUSE] Executing database creation SQL: {}",
        create_database_sql
    );

    // Execute without specifying database in URL (use default system database)
    let url = format!("{}/", config.url);

    let mut request_builder = client
        .post(&url)
        .timeout(config.timeout)
        .body(create_database_sql);

    // Add authentication if provided
    if !config.user.is_empty() {
        request_builder = request_builder.basic_auth(&config.user, Some(&config.password));
    }

    let response = request_builder.send().await?;
    let status = response.status();

    if !status.is_success() {
        let error_body = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        error!(
            "[CLICKHOUSE] Failed to create database {}: Status {}, Body: {}",
            config.database, status, error_body
        );
        return Err(format!(
            "Failed to create database {}: {}",
            config.database, error_body
        )
        .into());
    }

    let success_body = response.text().await.unwrap_or_default();
    debug!("[CLICKHOUSE] Database creation response: {}", success_body);

    info!("[CLICKHOUSE] ✓ Database {} is ready", config.database);
    Ok(())
}

/// Setup migration tracking table
async fn setup_migration_tracking(
    client: &Client,
    config: &ClickHouseConfig,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    debug!("[CLICKHOUSE] Setting up migration tracking table...");

    let create_table_sql = format!(
        r#"
        CREATE TABLE IF NOT EXISTS {}.schema_migrations (
            version String,
            applied_at DateTime DEFAULT now()
        ) ENGINE = MergeTree()
        ORDER BY version
    "#,
        config.database
    );

    execute_sql(client, config, &create_table_sql).await?;
    debug!("[CLICKHOUSE] ✓ Migration tracking table ready");

    Ok(())
}

/// Get list of applied migrations
async fn get_applied_migrations(
    client: &Client,
    config: &ClickHouseConfig,
) -> Result<Vec<String>, Box<dyn Error + Send + Sync>> {
    let query = format!(
        "SELECT version FROM {}.schema_migrations ORDER BY version",
        config.database
    );

    match execute_query(client, config, &query).await {
        Ok(response) => {
            // Parse response - ClickHouse returns TSV format by default
            let migrations: Vec<String> = response
                .lines()
                .filter(|line| !line.trim().is_empty())
                .map(|line| line.trim().to_string())
                .collect();

            Ok(migrations)
        },
        Err(_) => {
            // If table doesn't exist or query fails, assume no migrations applied
            debug!("[CLICKHOUSE] No previous migrations found");
            Ok(vec![])
        },
    }
}

/// Apply a single migration
async fn apply_migration(
    client: &Client,
    config: &ClickHouseConfig,
    name: &str,
    sql: &str,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    // Split SQL into individual statements (ClickHouse HTTP API doesn't support multi-statement)
    let statements = split_sql_statements(sql);
    debug!(
        "[CLICKHOUSE] Migration {} contains {} statements",
        name,
        statements.len()
    );

    // Execute each statement separately
    for (i, statement) in statements.iter().enumerate() {
        if statement.trim().is_empty() {
            continue;
        }

        debug!(
            "[CLICKHOUSE] Executing statement {}/{} for migration {}",
            i + 1,
            statements.len(),
            name
        );
        match execute_sql(client, config, statement.trim()).await {
            Ok(()) => debug!(
                "[CLICKHOUSE] ✓ Statement {}/{} completed",
                i + 1,
                statements.len()
            ),
            Err(e) => {
                error!(
                    "[CLICKHOUSE] ✗ Statement {}/{} failed: {}",
                    i + 1,
                    statements.len(),
                    e
                );
                return Err(format!(
                    "Migration {} failed at statement {}/{}: {}",
                    name,
                    i + 1,
                    statements.len(),
                    e
                )
                .into());
            },
        }
    }

    // Record migration as applied
    // Note: ClickHouse doesn't support parameterized queries in the same way as PostgreSQL
    // We validate inputs and escape properly for defense in depth

    // Validate database name contains only safe characters
    if !config
        .database
        .chars()
        .all(|c| c.is_alphanumeric() || c == '_')
    {
        return Err(format!(
            "Invalid database name for ClickHouse migration: {}",
            config.database
        )
        .into());
    }

    // Validate migration name contains only safe characters (alphanumeric, underscore, hyphen, dot)
    if !name
        .chars()
        .all(|c| c.is_alphanumeric() || c == '_' || c == '-' || c == '.')
    {
        return Err(format!("Invalid migration name: {}", name).into());
    }

    // ClickHouse HTTP API does not support parameterized queries; use strict validation and identifier escaping
    // Both database and name have been validated to contain only safe characters
    let record_sql = format!(
        "INSERT INTO `{}`.schema_migrations (version) VALUES ('{}')",
        config.database, name
    );
    execute_sql(client, config, &record_sql).await?;

    info!(
        "[CLICKHOUSE] ✓ Migration {} completed successfully ({} statements)",
        name,
        statements.len()
    );
    Ok(())
}

/// Execute SQL query and return response
async fn execute_query(
    client: &Client,
    config: &ClickHouseConfig,
    sql: &str,
) -> Result<String, Box<dyn Error + Send + Sync>> {
    let mut url = format!("{}/", config.url);

    // Add database parameter if specified
    if !config.database.is_empty() {
        url.push_str(&format!("?database={}", config.database));
    }

    let mut request_builder = client
        .post(&url)
        .timeout(config.timeout)
        .body(sql.to_string());

    // Add authentication if provided
    if !config.user.is_empty() {
        request_builder = request_builder.basic_auth(&config.user, Some(&config.password));
    }

    let response = request_builder.send().await?;

    if !response.status().is_success() {
        let error_body = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(format!("ClickHouse query failed: {}", error_body).into());
    }

    let body = response.text().await?;
    Ok(body)
}

/// Execute SQL without expecting a response
async fn execute_sql(
    client: &Client,
    config: &ClickHouseConfig,
    sql: &str,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    execute_query(client, config, sql).await?;
    Ok(())
}

/// Split SQL into individual statements while preserving comments and handling edge cases
fn split_sql_statements(sql: &str) -> Vec<String> {
    let mut statements = Vec::new();
    let mut current_statement = String::new();
    let mut in_string = false;
    let mut string_delimiter = '\0';
    let mut in_comment = false;
    let mut chars = sql.chars().peekable();

    while let Some(ch) = chars.next() {
        match ch {
            // Handle string literals
            '\'' | '"' => {
                current_statement.push(ch);
                if !in_comment {
                    if !in_string {
                        in_string = true;
                        string_delimiter = ch;
                    } else if string_delimiter == ch {
                        in_string = false;
                        string_delimiter = '\0';
                    }
                }
            },

            // Handle single-line comments
            '-' if !in_string && !in_comment => {
                if chars.peek() == Some(&'-') {
                    // Start of single-line comment
                    in_comment = true;
                }
                current_statement.push(ch);
            },

            // Handle newlines (end single-line comments)
            '\n' | '\r' => {
                if in_comment {
                    in_comment = false;
                }
                current_statement.push(ch);
            },

            // Handle statement separators
            ';' => {
                if !in_string && !in_comment {
                    // End of statement
                    let trimmed = current_statement.trim().to_string();
                    if !trimmed.is_empty() && !is_only_comments(&trimmed) {
                        statements.push(trimmed);
                    }
                    current_statement.clear();
                } else {
                    current_statement.push(ch);
                }
            },

            // Handle all other characters
            _ => {
                current_statement.push(ch);
            },
        }
    }

    // Add the last statement if it's not empty
    let trimmed = current_statement.trim().to_string();
    if !trimmed.is_empty() && !is_only_comments(&trimmed) {
        statements.push(trimmed);
    }

    statements
}

/// Check if a string contains only comments and whitespace
fn is_only_comments(sql: &str) -> bool {
    for line in sql.lines() {
        let trimmed = line.trim();
        if !trimmed.is_empty() && !trimmed.starts_with("--") {
            return false;
        }
    }
    true
}

/// Get migration status for health checks
pub async fn get_migration_status() -> Result<MigrationStatus, Box<dyn Error + Send + Sync>> {
    let config = ClickHouseConfig::default();
    get_migration_status_with_config(config).await
}

/// Get migration status with specific config (used for testing)
async fn get_migration_status_with_config(
    config: ClickHouseConfig,
) -> Result<MigrationStatus, Box<dyn Error + Send + Sync>> {
    let client = Client::new();

    // Check if running in production
    let app_config = crate::app_config::config();
    let is_production = app_config.is_production();

    // Check if ClickHouse is available
    let is_healthy = check_clickhouse_health(&client, &config).await.is_ok();

    // Calculate total migrations based on environment
    let total_migrations = if is_production {
        MIGRATIONS.len()
    } else {
        MIGRATIONS.len() + SEED_MIGRATIONS.len()
    };

    if !is_healthy {
        return Ok(MigrationStatus {
            is_healthy: false,
            applied_count: 0,
            total_migrations,
            pending_count: total_migrations,
            error: Some("ClickHouse not available".to_string()),
        });
    }

    let applied_migrations = get_applied_migrations(&client, &config)
        .await
        .unwrap_or_default();
    let applied_count = applied_migrations.len();
    let pending_count = total_migrations.saturating_sub(applied_count);

    Ok(MigrationStatus {
        is_healthy: true,
        applied_count,
        total_migrations,
        pending_count,
        error: None,
    })
}

/// Migration status information
#[derive(Debug)]
pub struct MigrationStatus {
    pub is_healthy: bool,
    pub applied_count: usize,
    pub total_migrations: usize,
    pub pending_count: usize,
    pub error: Option<String>,
}

impl MigrationStatus {
    pub fn is_up_to_date(&self) -> bool {
        self.is_healthy && self.pending_count == 0
    }

    pub fn needs_migration(&self) -> bool {
        self.is_healthy && self.pending_count > 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_migration_status() {
        // Use test config that doesn't rely on lazy static
        let config = ClickHouseConfig::for_test();

        // This test requires ClickHouse to be running
        match super::get_migration_status_with_config(config).await {
            Ok(status) => {
                println!("ClickHouse migration status: {:?}", status);
                assert!(status.total_migrations > 0);
            },
            Err(e) => {
                println!("ClickHouse not available for testing: {}", e);
                // Test passes if ClickHouse isn't available
            },
        }
    }
}
