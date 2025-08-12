pub mod config;
pub mod postgres;
pub mod redis_config;
pub mod redis_pool;

pub use config::DatabaseConfig;
pub use postgres::PostgresPool;
pub use redis_config::RedisConfig;
pub use redis_pool::RedisPool;

/// Initialize all database connections
pub async fn init_databases() -> Result<PostgresPool, Box<dyn std::error::Error>> {
    let config = DatabaseConfig::from_env();
    let postgres_pool = PostgresPool::new(config).await?;

    Ok(postgres_pool)
}
