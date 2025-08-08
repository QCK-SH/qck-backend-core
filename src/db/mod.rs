pub mod config;
pub mod postgres;

pub use config::DatabaseConfig;
pub use postgres::{PostgresPool, DatabaseHealth, PoolMetrics};

/// Initialize all database connections
pub async fn init_databases() -> Result<PostgresPool, Box<dyn std::error::Error>> {
    let config = DatabaseConfig::from_env();
    let postgres_pool = PostgresPool::new(config).await?;
    
    Ok(postgres_pool)
}